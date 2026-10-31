//! The conversation kit: an authored graph of lines and choices, the runtime that walks it, and barks.
//!
//! A [`DialogueScript`] is a set of named [`Conversation`]s; each is a graph of [`Node`]s keyed by id.
//! A node is either a spoken/thought [`Node::Line`] (advances to `next`) or a [`Node::Choice`] that
//! offers options and branches on the pick. Two bubble kinds carry distinct meaning — speech =
//! directed dialogue/story, thought = ambient inner state — following the Comic-Strip-Conversation
//! convention that holds thoughts "equal to spoken words" (Gray; Rajendran & Mitchell, *Bubble
//! Dialogue*, 2000).
//!
//! # Speakers are strings
//!
//! The script names speakers with plain strings and the host resolves them through [`Speakers`]: a
//! speaker with an `entity` gets a world balloon anchored to it, one without is a narrator (no balloon;
//! the screen dialog box still shows its `name`). A key that is not in [`Speakers`] at all is the host's
//! mistake, not a narrator: the crate `warn!`s naming it and skips the line or bark. The crate knows no
//! game type.
//!
//! # Two ways to show a line
//!
//! [`Presenter::Balloons`] puts every line over its speaker's head and auto-advances after
//! [`dwell_secs`]; [`Presenter::DialogBox`] shows the screen box of
//! [`crate::dialog_box`] and waits for the player. [`Bark`]s are one-off ambient lines and are always
//! world balloons, whatever the presenter. A modal conversation holds [`ConversationLock`]; hosts that
//! must yield input or pause the simulation read it.

use std::collections::{HashMap, HashSet, VecDeque};

use bevy::light::NotShadowCaster;
use bevy::picking::Pickable;
use bevy::picking::events::{PointerClick, PointerOut, PointerOver};
use bevy::prelude::*;
use bevy::time::Real;
use serde::{Deserialize, Serialize};

use crate::balloon::{
    Bubble, BubbleAssets, BubbleOffsetPx, BubblePx, BubbleStyle, BubbleTtl, DefaultBalloonSkin,
    build_bubble, dwell_secs,
};
use crate::dialog_box::{Typewriter, complete_typing};
use crate::style::{BubbleKind, Emotion};

// -------------------------------------------------------------------------------------------------
// Model
// -------------------------------------------------------------------------------------------------

fn default_speech() -> BubbleKind {
    BubbleKind::Speech
}

/// One selectable option in a [`Node::Choice`]: its label and the node id it jumps to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub text: String,
    pub next: String,
}

/// A single node in a conversation graph.
///
/// Authored data: a key the node does not have (a misspelt `emotion`, a `kind` on a choice) is a parse
/// error naming the key, never a field silently left at its default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Node {
    /// A spoken/thought line by `speaker`; advances to `next` (or ends the conversation on `None`).
    Line {
        speaker: String,
        #[serde(default = "default_speech")]
        kind: BubbleKind,
        #[serde(default)]
        emotion: Emotion,
        text: String,
        next: Option<String>,
    },
    /// A prompt by `speaker`, followed by one selectable option per [`Choice`].
    Choice {
        speaker: String,
        #[serde(default)]
        emotion: Emotion,
        prompt: String,
        options: Vec<Choice>,
    },
}

/// A named conversation: the id of its `start` node plus the node graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conversation {
    pub start: String,
    pub nodes: HashMap<String, Node>,
}

/// The whole authored dialogue set. A resource: [`StartConversation`] looks conversations up here.
#[derive(Resource, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueScript {
    pub conversations: HashMap<String, Conversation>,
}

impl DialogueScript {
    pub fn conversation(&self, id: &str) -> Option<&Conversation> {
        self.conversations.get(id)
    }

    /// Add every conversation of `other`. A host loads several files into one script; an id defined
    /// twice is a loud error rather than one silently shadowing the other, and `self` is left
    /// unchanged when it fails.
    pub fn merge(&mut self, other: DialogueScript) -> Result<(), String> {
        if let Some(id) = other
            .conversations
            .keys()
            .find(|id| self.conversations.contains_key(*id))
        {
            return Err(format!("dialogue: conversation '{id}' is defined twice"));
        }
        self.conversations.extend(other.conversations);
        Ok(())
    }
}

/// Validate a parsed script: every conversation's `start` exists, every `next`/option target
/// resolves, choices are non-empty, and speakers are named. One path: a malformed script is a loud
/// error at load, never a half-broken conversation at runtime.
pub fn validate_script(s: &DialogueScript) -> Result<(), String> {
    if s.conversations.is_empty() {
        return Err("dialogue: no conversations defined".into());
    }
    for (cid, conv) in &s.conversations {
        if !conv.nodes.contains_key(&conv.start) {
            return Err(format!(
                "dialogue '{cid}': start node '{}' not found",
                conv.start
            ));
        }
        for (nid, node) in &conv.nodes {
            match node {
                Node::Line { speaker, next, .. } => {
                    check_speaker(speaker, cid, nid)?;
                    if let Some(n) = next
                        && !conv.nodes.contains_key(n)
                    {
                        return Err(format!(
                            "dialogue '{cid}' node '{nid}': next '{n}' not found"
                        ));
                    }
                }
                Node::Choice {
                    speaker, options, ..
                } => {
                    check_speaker(speaker, cid, nid)?;
                    if options.is_empty() {
                        return Err(format!(
                            "dialogue '{cid}' node '{nid}': choice has no options"
                        ));
                    }
                    for o in options {
                        if !conv.nodes.contains_key(&o.next) {
                            return Err(format!(
                                "dialogue '{cid}' node '{nid}': option next '{}' not found",
                                o.next
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn check_speaker(s: &str, cid: &str, nid: &str) -> Result<(), String> {
    if s.trim().is_empty() {
        return Err(format!("dialogue '{cid}' node '{nid}': speaker is empty"));
    }
    Ok(())
}

// -------------------------------------------------------------------------------------------------
// Host-facing resources and messages
// -------------------------------------------------------------------------------------------------

/// Who a script's speaker string stands for, as the host knows it.
#[derive(Debug, Clone, Default)]
pub struct Speaker {
    /// The entity a world balloon anchors to; `None` is a narrator (no balloon). Any entity with a
    /// `Transform` and a `Visibility`, a root or a child: the balloon floats over its world position
    /// and hides when it is hidden, through its parent too.
    pub entity: Option<Entity>,
    /// Shown on the dialog box's name tag; empty hides the tag.
    pub name: String,
    /// Shown beside the text in the dialog box when present.
    pub portrait: Option<Handle<Image>>,
}

/// Speaker string → [`Speaker`]. The host keeps this current (register on spawn, remove on despawn).
///
/// A registered speaker without an `entity` is a narrator and is skipped quietly; a key that is not here
/// at all is skipped with a `warn!` naming it (a bark's once per key, a balloon-mode line's once per
/// node), because nothing but a misspelt or not-yet-registered key explains it.
#[derive(Resource, Debug, Clone, Default)]
pub struct Speakers(pub HashMap<String, Speaker>);

/// How a conversation's lines are shown. Set by the host; one rule for the whole game.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Presenter {
    /// World balloons over the speakers; lines auto-advance after their dwell time.
    #[default]
    Balloons,
    /// The screen dialog box; lines wait for the player. Needs a [`crate::DialogSkin`].
    DialogBox,
}

/// Start (or ignore, if one is already running) the named conversation.
#[derive(Message, Debug, Clone)]
pub struct StartConversation {
    pub id: String,
}

/// A choice option was picked (index within the current choice node). Written by the pointer, the
/// number keys and the dialog box's focus; a host may write it too (a gamepad, say).
#[derive(Message, Debug, Clone)]
pub struct ChoicePicked {
    pub index: usize,
}

/// A conversation ran to its end.
#[derive(Message, Debug, Clone)]
pub struct ConversationEnded {
    pub id: String,
}

/// The conversation cursor moved onto `node` of `conversation` — at the start node, after a line
/// advances, and after a choice resolves. Written once per node, before it is presented. Hosts attach
/// effects (a relationship change, a quest flag) to the node a choice leads to.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct DialogueNodeEntered {
    pub conversation: String,
    pub node: String,
}

/// Present while a conversation is active. Hosts yield the click and pause the simulation on it.
#[derive(Resource, Debug)]
pub struct ConversationLock;

/// A one-off ambient balloon over a speaker — barks, reactions. Never blocks play, always a world
/// balloon whatever the [`Presenter`]; the speaker needs an `entity` in [`Speakers`] or it is dropped
/// (quietly for a narrator, with one `warn!` per key for a key that is not registered).
/// Serde, so a game or a tool authors barks as data (RON) with this type and no second schema. A key
/// the bark does not have (`ttl`) is a parse error naming it, not a value dropped without a word.
#[derive(Message, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bark {
    pub speaker: String,
    pub kind: BubbleKind,
    pub emotion: Emotion,
    pub text: String,
}

/// Bark pacing.
///
/// Barks are produced by independent systems, so two acting on the same frame used to pop their
/// balloons simultaneously — a reply landing 16 ms after its cue reads as noise, not as conversation.
/// Human turn-taking is fast (modal gap ≈ 200 ms; Stivers et al., *Universals and cultural variation
/// in turn-taking*, PNAS 2009, DOI 10.1073/pnas.0903616106) but speech carries prosody. A *read*
/// balloon has no such cue, so the gap must cover the eye-movement + comprehension cost of noticing
/// the new balloon; ~1 s is the low end of that.
///
/// Serde, like [`Bark`]: a key this struct does not have (`max_live`) is a parse error, not a setting
/// that was read and ignored.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BarkSettings {
    /// Minimum seconds between two balloons appearing.
    pub gap: f32,
    /// How long a bark may wait its turn before it is no longer worth saying: a line is a reaction to
    /// an event, and narrated late it describes a situation that has already moved on.
    pub stale: f32,
    /// Backlog cap. Beyond it the *oldest* pending barks are dropped: in a burst the freshest
    /// reactions are the relevant ones.
    pub queue_cap: usize,
}

impl Default for BarkSettings {
    fn default() -> Self {
        Self {
            gap: 1.0,
            stale: 3.0,
            queue_cap: 4,
        }
    }
}

struct PendingBark {
    bark: Bark,
    queued_at: f32,
}

/// Paces ambient chatter: barks are enqueued as they arrive and released one at a time, at least
/// [`BarkSettings::gap`] apart. The single gate on bark presentation. FIFO, so a reply follows its cue.
#[derive(Resource, Default)]
pub struct BarkQueue {
    pending: VecDeque<PendingBark>,
    /// Real-time stamp before which no bark may be released.
    next_at: f32,
}

impl BarkQueue {
    /// File a bark, evicting the oldest if the backlog is full.
    pub(crate) fn push(&mut self, bark: Bark, now: f32, settings: &BarkSettings) {
        self.pending.push_back(PendingBark {
            bark,
            queued_at: now,
        });
        while self.pending.len() > settings.queue_cap {
            self.pending.pop_front();
        }
    }

    /// Discard barks that have waited past `stale`, then hand back the next one if the gap since the
    /// last balloon has elapsed. Staleness is applied unconditionally so a dead bark can never hold the
    /// queue head and starve the fresh ones behind it.
    pub(crate) fn take_ready(&mut self, now: f32, settings: &BarkSettings) -> Option<Bark> {
        while self
            .pending
            .front()
            .is_some_and(|p| now - p.queued_at > settings.stale)
        {
            self.pending.pop_front();
        }
        if now < self.next_at {
            return None;
        }
        self.pending.pop_front().map(|p| p.bark)
    }

    /// A balloon actually reached the screen at `now` — open the gap before the next one may.
    pub(crate) fn spoke_at(&mut self, now: f32, settings: &BarkSettings) {
        self.next_at = now + settings.gap;
    }

    /// Barks still waiting their turn.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

// -------------------------------------------------------------------------------------------------
// Runtime
// -------------------------------------------------------------------------------------------------

/// The live conversation cursor.
#[derive(Resource)]
pub(crate) struct Active {
    pub(crate) conv: String,
    pub(crate) node: String,
    /// Set once the current node is on screen (so a node is presented exactly once).
    pub(crate) presented: bool,
    /// Real time at which a [`Presenter::Balloons`] line auto-advances (lines only).
    pub(crate) advance_at: f32,
}

/// Marks everything owned by the current conversation (balloons, the dialog box), so a node
/// transition or the end can clear it.
#[derive(Component)]
pub(crate) struct ConversationEntity;

/// Marks a bubble that lingers on a timer (barks) — never a conversation bubble.
#[derive(Component)]
pub(crate) struct AmbientBubble {
    owner: Entity,
}

/// Vertical gap (raster px at the reference glyph size of 40 px) between stacked choice balloons and
/// above the speaker's head; scaled by the skin's `text_px / 40`. Texels, so the stack follows
/// [`BalloonScale`](crate::BalloonScale) like the balloons do.
const CHOICE_GAP_PX: f32 = 25.0;
const CHOICE_BASE_PX: f32 = 31.0;
/// Tint of a choice balloon until hovered, for a clickable affordance.
const CHOICE_DIM: Color = Color::srgb(0.78, 0.78, 0.78);

/// What a script's speaker string means to the host.
enum Resolved {
    /// Registered with an entity: a balloon anchors to it.
    Head(Entity),
    /// Registered without an entity: a narrator, no head to float over. Quiet by design.
    Narrator,
    /// Not a key of [`Speakers`]: a misspelt or not-yet-registered key, which is the host's mistake.
    Unknown,
}

fn resolve_speaker(speakers: &Speakers, key: &str) -> Resolved {
    match speakers.0.get(key) {
        Some(Speaker {
            entity: Some(entity),
            ..
        }) => Resolved::Head(*entity),
        Some(_) => Resolved::Narrator,
        None => Resolved::Unknown,
    }
}

/// Most distinct unknown bark speakers remembered at once; see [`WarnedSpeakers`].
const MAX_WARNED_SPEAKERS: usize = 64;

/// The unknown speaker keys a bark path has already warned about, so a bark that fires every few
/// seconds says its key is missing once rather than every time. Bounded: a game that mints a key per
/// entity cannot grow it past [`MAX_WARNED_SPEAKERS`]; a full set is cleared, so a key may be warned
/// about again, never lost.
#[derive(Default)]
pub(crate) struct WarnedSpeakers(HashSet<String>);

impl WarnedSpeakers {
    /// `true` the first time `key` is seen since the set was last cleared.
    fn first_sight(&mut self, key: &str) -> bool {
        if self.0.contains(key) {
            return false;
        }
        if self.0.len() >= MAX_WARNED_SPEAKERS {
            self.0.clear();
        }
        self.0.insert(key.to_owned());
        true
    }
}

pub(crate) fn open_conversation(
    mut commands: Commands,
    mut starts: MessageReader<StartConversation>,
    active: Option<Res<Active>>,
    script: Res<DialogueScript>,
    mut entered: MessageWriter<DialogueNodeEntered>,
) {
    // One conversation at a time; take the first request and drain the rest of the queue.
    let id = {
        let Some(start) = starts.read().next() else {
            return;
        };
        start.id.clone()
    };
    starts.clear();
    if active.is_some() {
        return;
    }
    let Some(conv) = script.conversation(&id) else {
        warn!("bevy_speech_bubbles: no conversation '{id}'");
        return;
    };
    commands.insert_resource(Active {
        conv: id.clone(),
        node: conv.start.clone(),
        presented: false,
        advance_at: 0.0,
    });
    entered.write(DialogueNodeEntered {
        conversation: id.clone(),
        node: conv.start.clone(),
    });
    commands.insert_resource(ConversationLock);
}

/// Present the current node as world balloons. The dialog box has its own presenter.
#[allow(clippy::too_many_arguments)]
pub(crate) fn present_current(
    mut commands: Commands,
    time: Res<Time<Real>>,
    presenter: Res<Presenter>,
    active: Option<ResMut<Active>>,
    script: Res<DialogueScript>,
    speakers: Res<Speakers>,
    skin: Res<DefaultBalloonSkin>,
    assets: Option<Res<BubbleAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<ConversationEntity>>,
    mut ended: MessageWriter<ConversationEnded>,
) {
    if *presenter != Presenter::Balloons {
        return;
    }
    let Some(mut active) = active else {
        return;
    };
    if active.presented {
        return;
    }

    // Clear the previous node's balloons.
    for e in &existing {
        commands.entity(e).despawn();
    }

    let Some(assets) = assets else {
        error!("bevy_speech_bubbles: Presenter::Balloons needs a BubbleAssets resource");
        let id = active.conv.clone();
        finish(&mut commands, &existing, &mut ended, id);
        return;
    };
    let Some(node) = script
        .conversation(&active.conv)
        .and_then(|c| c.nodes.get(&active.node))
    else {
        warn!(
            "bevy_speech_bubbles: node '{}' of '{}' is missing; ending",
            active.node, active.conv
        );
        let id = active.conv.clone();
        finish(&mut commands, &existing, &mut ended, id);
        return;
    };

    let now = time.elapsed_secs();
    match node {
        Node::Line {
            speaker,
            kind,
            emotion,
            text,
            ..
        } => {
            match resolve_speaker(&speakers, speaker) {
                Resolved::Head(owner) => {
                    spawn_line_bubble(
                        &mut commands,
                        &assets,
                        &mut images,
                        &mut materials,
                        owner,
                        BubbleStyle {
                            kind: *kind,
                            emotion: *emotion,
                            tail: true,
                            skin: skin.0,
                        },
                        text,
                    );
                    active.advance_at = now + dwell_secs(text);
                }
                // A narrator has no head to float over: in balloon mode the line is skipped.
                Resolved::Narrator => active.advance_at = now,
                // A key nobody registered is the host's mistake: the line is skipped just the same,
                // and the log says which key, which node and which conversation.
                Resolved::Unknown => {
                    warn!(
                        "bevy_speech_bubbles: speaker '{speaker}' of node '{}' in '{}' is not in \
                         Speakers; skipping its line in balloon mode",
                        active.node, active.conv
                    );
                    active.advance_at = now;
                }
            }
        }
        Node::Choice {
            speaker,
            emotion,
            prompt,
            options,
        } => {
            let Resolved::Head(owner) = resolve_speaker(&speakers, speaker) else {
                warn!(
                    "bevy_speech_bubbles: choice '{}' needs a speaker with an entity in balloon mode \
                     ('{speaker}' has none); ending",
                    active.node
                );
                let id = active.conv.clone();
                finish(&mut commands, &existing, &mut ended, id);
                return;
            };
            let prompt_height_px = spawn_line_bubble(
                &mut commands,
                &assets,
                &mut images,
                &mut materials,
                owner,
                BubbleStyle {
                    kind: BubbleKind::Speech,
                    emotion: *emotion,
                    tail: true,
                    skin: skin.0,
                },
                prompt,
            )
            .y;
            // Clickable option balloons stacked above the PROMPT, not at a bare `CHOICE_BASE`: both
            // columns hang off the same entity, so starting lower would draw option 1 across it.
            // Offsets apply along camera up, so clearing the prompt's own height is exactly the
            // screen-space separation needed.
            let glyph_k = skin.0.text_px / 40.0;
            let gap_px = (CHOICE_GAP_PX * glyph_k).round();
            let mut offset_y = (CHOICE_BASE_PX * glyph_k).round() + prompt_height_px;
            for (i, opt) in options.iter().enumerate() {
                let rendered = build_bubble(
                    &assets,
                    &mut images,
                    &mut materials,
                    &BubbleStyle {
                        kind: BubbleKind::Speech,
                        emotion: Emotion::Neutral,
                        tail: false,
                        skin: skin.0,
                    },
                    &format!("{}. {}", i + 1, opt.text),
                );
                if let Some(mut m) = materials.get_mut(&rendered.material) {
                    m.base_color = CHOICE_DIM;
                }
                let mat = rendered.material.clone();
                let mat_over = mat.clone();
                let mat_out = mat.clone();
                let center_offset = offset_y + rendered.px.y as f32 * 0.5;
                commands
                    .spawn((
                        Bubble {
                            owner,
                            offset: Vec2::ZERO,
                        },
                        BubbleOffsetPx(Vec2::new(0.0, center_offset)),
                        BubblePx(rendered.px),
                        ConversationEntity,
                        Pickable::default(),
                        Mesh3d(assets.quad.clone()),
                        MeshMaterial3d(mat),
                        NotShadowCaster,
                        Transform::from_scale(Vec3::new(rendered.size.x, rendered.size.y, 1.0)),
                    ))
                    .observe(
                        move |_ev: On<PointerClick>, mut picked: MessageWriter<ChoicePicked>| {
                            picked.write(ChoicePicked { index: i });
                        },
                    )
                    .observe(
                        move |_ev: On<PointerOver>, mut mats: ResMut<Assets<StandardMaterial>>| {
                            if let Some(mut m) = mats.get_mut(&mat_over) {
                                m.base_color = Color::WHITE;
                            }
                        },
                    )
                    .observe(
                        move |_ev: On<PointerOut>, mut mats: ResMut<Assets<StandardMaterial>>| {
                            if let Some(mut m) = mats.get_mut(&mat_out) {
                                m.base_color = CHOICE_DIM;
                            }
                        },
                    );
                offset_y += rendered.px.y as f32 + gap_px;
            }
            active.advance_at = f32::INFINITY; // choices don't auto-advance
        }
    }
    active.presented = true;
}

/// A line is read: `Space`, `Enter`, the numpad `Enter` or a left click advance it. In dialog-box mode
/// the first press while the typewriter is still running completes the line instead (and a line never
/// auto-advances); in balloon mode a line also advances once its dwell time is up.
#[allow(clippy::too_many_arguments)]
pub(crate) fn advance_line(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    presenter: Res<Presenter>,
    active: Option<ResMut<Active>>,
    script: Res<DialogueScript>,
    existing: Query<Entity, With<ConversationEntity>>,
    mut typewriters: Query<&mut Typewriter>,
    mut ended: MessageWriter<ConversationEnded>,
    mut entered: MessageWriter<DialogueNodeEntered>,
) {
    let Some(mut active) = active else {
        return;
    };
    if !active.presented {
        return;
    }
    // Only Line nodes advance here (choices are resolved by a pick).
    let next = match script
        .conversation(&active.conv)
        .and_then(|c| c.nodes.get(&active.node))
    {
        Some(Node::Line { next, .. }) => next.clone(),
        _ => return,
    };
    // Only a press the dialog actually uses is cleared; a timed-out balloon line uses none.
    let mut pressed = mouse.get_just_pressed().any(|b| *b == MouseButton::Left);
    for key in [KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space] {
        pressed |= keys.just_pressed(key);
    }
    let timed_out = *presenter == Presenter::Balloons && time.elapsed_secs() >= active.advance_at;
    if !pressed && !timed_out {
        return;
    }
    if pressed {
        mouse.clear_just_pressed(MouseButton::Left);
        for key in [KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space] {
            keys.clear_just_pressed(key);
        }
        if complete_typing(&mut typewriters) {
            return;
        }
    }
    match next {
        Some(n) => {
            entered.write(DialogueNodeEntered {
                conversation: active.conv.clone(),
                node: n.clone(),
            });
            active.node = n;
            active.presented = false;
        }
        None => {
            let id = active.conv.clone();
            finish(&mut commands, &existing, &mut ended, id);
        }
    }
}

/// Number-key selection: `1`..`9` (and the numpad row) pick that option.
///
/// **This writes the same [`ChoicePicked`] the pointer writes** — a second input device, not a second
/// resolution path. An out-of-range digit is ignored rather than clamped: pressing `5` on a two-option
/// choice should do nothing, not silently pick the last one.
pub(crate) fn choice_hotkeys(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    active: Option<Res<Active>>,
    script: Res<DialogueScript>,
    mut picked: MessageWriter<ChoicePicked>,
) {
    let Some(active) = active else { return };
    if !active.presented {
        return;
    }
    // Only while a Choice is on screen — otherwise a stray `1` during a Line would queue a pick that
    // `resolve_choice` would then apply to the *next* choice node.
    let Some(Node::Choice { options, .. }) = script
        .conversation(&active.conv)
        .and_then(|c| c.nodes.get(&active.node))
    else {
        return;
    };

    const DIGITS: [(KeyCode, KeyCode); 9] = [
        (KeyCode::Digit1, KeyCode::Numpad1),
        (KeyCode::Digit2, KeyCode::Numpad2),
        (KeyCode::Digit3, KeyCode::Numpad3),
        (KeyCode::Digit4, KeyCode::Numpad4),
        (KeyCode::Digit5, KeyCode::Numpad5),
        (KeyCode::Digit6, KeyCode::Numpad6),
        (KeyCode::Digit7, KeyCode::Numpad7),
        (KeyCode::Digit8, KeyCode::Numpad8),
        (KeyCode::Digit9, KeyCode::Numpad9),
    ];
    for (index, (row, numpad)) in DIGITS.iter().enumerate().take(options.len()) {
        let row_pressed = keys.clear_just_pressed(*row);
        let numpad_pressed = keys.clear_just_pressed(*numpad);
        if row_pressed || numpad_pressed {
            picked.write(ChoicePicked { index });
            return; // one pick per frame, even if two keys land together
        }
    }
}

pub(crate) fn resolve_choice(
    mut picks: MessageReader<ChoicePicked>,
    active: Option<ResMut<Active>>,
    script: Res<DialogueScript>,
    mut entered: MessageWriter<DialogueNodeEntered>,
) {
    let Some(mut active) = active else {
        picks.clear();
        return;
    };
    let Some(pick) = picks.read().next() else {
        return;
    };
    let index = pick.index;
    picks.clear();
    if !active.presented {
        return; // a pick for a node that is not on screen yet answers nothing
    }

    // An index past the last option answers nothing, rather than picking the last one.
    let next = match script
        .conversation(&active.conv)
        .and_then(|c| c.nodes.get(&active.node))
    {
        Some(Node::Choice { options, .. }) => options.get(index).map(|o| o.next.clone()),
        _ => return,
    };
    if let Some(n) = next {
        entered.write(DialogueNodeEntered {
            conversation: active.conv.clone(),
            node: n.clone(),
        });
        active.node = n;
        active.presented = false;
    }
}

/// Tear down the conversation: despawn its balloons / box, drop the lock and cursor, say it ended.
pub(crate) fn finish(
    commands: &mut Commands,
    existing: &Query<Entity, With<ConversationEntity>>,
    ended: &mut MessageWriter<ConversationEnded>,
    id: String,
) {
    for e in existing {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<Active>();
    commands.remove_resource::<ConversationLock>();
    ended.write(ConversationEnded { id });
}

/// Queue every incoming [`Bark`]. Runs before [`release_bark`] so a bark generated this frame is a
/// candidate this frame — the queue adds pacing, never a frame of latency to an idle world.
pub(crate) fn queue_barks(
    time: Res<Time<Real>>,
    settings: Res<BarkSettings>,
    mut barks: MessageReader<Bark>,
    mut queue: ResMut<BarkQueue>,
) {
    let now = time.elapsed_secs();
    for bark in barks.read() {
        queue.push(bark.clone(), now, &settings);
    }
}

/// Release at most one queued bark per [`BarkSettings::gap`], dropping any that waited past
/// [`BarkSettings::stale`].
///
/// Silent while a modal conversation holds the [`ConversationLock`]: this layer runs on `Time<Real>`
/// (so balloons keep expiring through a pause), which means the gap would otherwise elapse
/// mid-conversation and pop chatter over the authored scene. Anything still queued when the lock lifts
/// is aged out by `stale` on the next release.
#[allow(clippy::too_many_arguments)]
pub(crate) fn release_bark(
    mut commands: Commands,
    time: Res<Time<Real>>,
    lock: Option<Res<ConversationLock>>,
    settings: Res<BarkSettings>,
    mut queue: ResMut<BarkQueue>,
    speakers: Res<Speakers>,
    mut warned: Local<WarnedSpeakers>,
    skin: Res<DefaultBalloonSkin>,
    assets: Option<Res<BubbleAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ambient: Query<(Entity, &AmbientBubble)>,
) {
    if lock.is_some() {
        return;
    }
    let now = time.elapsed_secs();
    let Some(bark) = queue.take_ready(now, &settings) else {
        return;
    };
    let Some(assets) = assets else {
        warn_once!("bevy_speech_bubbles: barks need a BubbleAssets resource; dropping them");
        return;
    };
    let owner = match resolve_speaker(&speakers, &bark.speaker) {
        Resolved::Head(owner) => owner,
        // Nothing was shown, so the gap is not started — the next bark is eligible immediately. A
        // narrator's silence is documented; an unregistered key is warned about, once per key.
        Resolved::Narrator => return,
        Resolved::Unknown => {
            if warned.first_sight(&bark.speaker) {
                warn!(
                    "bevy_speech_bubbles: bark speaker '{}' is not in Speakers; dropping the bark ({:?})",
                    bark.speaker, bark.text
                );
            }
            return;
        }
    };
    queue.spoke_at(now, &settings);

    // Replace any existing bark on this owner so they don't stack.
    for (e, a) in &ambient {
        if a.owner == owner {
            commands.entity(e).despawn();
        }
    }
    let rendered = build_bubble(
        &assets,
        &mut images,
        &mut materials,
        &BubbleStyle {
            kind: bark.kind,
            emotion: bark.emotion,
            tail: true,
            skin: skin.0,
        },
        &bark.text,
    );
    commands.spawn((
        Bubble {
            owner,
            offset: Vec2::ZERO,
        },
        AmbientBubble { owner },
        BubblePx(rendered.px),
        BubbleTtl {
            expires_at: now + dwell_secs(&bark.text),
        },
        Mesh3d(assets.quad.clone()),
        MeshMaterial3d(rendered.material),
        NotShadowCaster,
        Transform::from_scale(Vec3::new(rendered.size.x, rendered.size.y, 1.0)),
    ));
}

/// Spawn one balloon over `owner`, returning its **size in raster px**.
///
/// The caller needs the size because a balloon's footprint is only known after rasterisation (the text
/// wraps), and the `Choice` arm stacks its options clear of the prompt's height rather than at a fixed
/// offset that cannot know it. Texels, not world units: `track_bubbles` converts with the live
/// `BalloonScale`.
fn spawn_line_bubble(
    commands: &mut Commands,
    assets: &BubbleAssets,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    owner: Entity,
    style: BubbleStyle,
    text: &str,
) -> Vec2 {
    let rendered = build_bubble(assets, images, materials, &style, text);
    commands.spawn((
        Bubble {
            owner,
            offset: Vec2::ZERO,
        },
        BubblePx(rendered.px),
        ConversationEntity,
        Mesh3d(assets.quad.clone()),
        MeshMaterial3d(rendered.material),
        NotShadowCaster,
        Transform::from_scale(Vec3::new(rendered.size.x, rendered.size.y, 1.0)),
    ));
    rendered.px.as_vec2()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
(
  conversations: {
    "intro": (
      start: "a",
      nodes: {
        "a": Line(speaker: "lead", kind: Speech, text: "Move up.", next: Some("b")),
        "b": Line(speaker: "scout", kind: Thought, emotion: Fear, text: "This place is wrong.", next: Some("c")),
        "c": Choice(speaker: "lead", prompt: "Which way?", options: [
          (text: "Left", next: "d"),
          (text: "Hold", next: "d"),
        ]),
        "d": Line(speaker: "lead", text: "Copy.", next: None),
      },
    ),
  },
)
"#;

    fn sample() -> DialogueScript {
        ron::from_str(SAMPLE).expect("sample parses")
    }

    #[test]
    fn parses_and_validates_sample() {
        let script = sample();
        validate_script(&script).expect("sample validates");
        let conv = script.conversation("intro").expect("intro exists");
        assert_eq!(conv.start, "a");
        assert_eq!(conv.nodes.len(), 4);
    }

    #[test]
    fn kind_defaults_to_speech() {
        let script = sample();
        let conv = script.conversation("intro").expect("intro exists");
        match conv.nodes.get("d") {
            Some(Node::Line { kind, .. }) => assert_eq!(*kind, BubbleKind::Speech),
            other => panic!("d is a Line, got {other:?}"),
        }
    }

    #[test]
    fn a_script_survives_a_ron_round_trip() {
        let script = sample();
        let text = ron::to_string(&script).expect("script serializes");
        let back: DialogueScript = ron::from_str(&text).expect("script parses back");
        assert_eq!(back, script);
    }

    fn invalid(src: &str) -> String {
        let script: DialogueScript = ron::from_str(src).expect("parses");
        validate_script(&script).expect_err("must not validate")
    }

    #[test]
    fn rejects_dangling_next() {
        let e = invalid(
            r#"( conversations: { "x": ( start: "a", nodes: {
            "a": Line(speaker: "s", text: "hi", next: Some("nope")),
        } ) } )"#,
        );
        assert!(e.contains("next 'nope' not found"), "{e}");
    }

    #[test]
    fn rejects_missing_start() {
        let e = invalid(
            r#"( conversations: { "x": ( start: "z", nodes: {
            "a": Line(speaker: "s", text: "hi", next: None),
        } ) } )"#,
        );
        assert!(e.contains("start node 'z' not found"), "{e}");
    }

    #[test]
    fn rejects_empty_speaker() {
        let e = invalid(
            r#"( conversations: { "x": ( start: "a", nodes: {
            "a": Line(speaker: "  ", text: "hi", next: None),
        } ) } )"#,
        );
        assert!(e.contains("speaker is empty"), "{e}");
    }

    #[test]
    fn rejects_a_choice_without_options() {
        let e = invalid(
            r#"( conversations: { "x": ( start: "a", nodes: {
            "a": Choice(speaker: "s", prompt: "?", options: []),
        } ) } )"#,
        );
        assert!(e.contains("choice has no options"), "{e}");
    }

    #[test]
    fn rejects_a_dangling_option() {
        let e = invalid(
            r#"( conversations: { "x": ( start: "a", nodes: {
            "a": Choice(speaker: "s", prompt: "?", options: [(text: "t", next: "gone")]),
        } ) } )"#,
        );
        assert!(e.contains("option next 'gone' not found"), "{e}");
    }

    #[test]
    fn rejects_an_empty_script() {
        let e = validate_script(&DialogueScript::default()).expect_err("empty script");
        assert!(e.contains("no conversations"), "{e}");
    }

    #[test]
    fn unknown_fields_are_refused() {
        let bad = r#"( conversations: { "x": ( start: "a", extra: 1, nodes: {} ) } )"#;
        assert!(ron::from_str::<DialogueScript>(bad).is_err());
    }

    /// Parse a script of one conversation whose only node, `a`, is `node`.
    fn one_node(node: &str) -> Result<DialogueScript, ron::error::SpannedError> {
        ron::from_str(&format!(
            r#"( conversations: {{ "x": ( start: "a", nodes: {{ "a": {node} }} ) }} )"#
        ))
    }

    /// `kind` and `emotion` are optional on a node, so a misspelt key used to parse, fall back to the
    /// default and draw the wrong balloon with no word to the author (#12). Each typo is refused
    /// with the key named, and the same node spelt right loads.
    #[test]
    fn a_misspelt_node_field_is_refused() {
        let cases = [
            // (typo, the key meant, its value, the node around it)
            (
                "emtion",
                "emotion",
                "Fear",
                r#"Line(speaker: "s", KEY: VALUE, text: "hi", next: None)"#,
            ),
            (
                "knid",
                "kind",
                "Thought",
                r#"Line(speaker: "s", KEY: VALUE, text: "hm", next: None)"#,
            ),
            (
                "emotoin",
                "emotion",
                "Anger",
                r#"Choice(speaker: "s", KEY: VALUE, prompt: "?", options: [(text: "t", next: "a")])"#,
            ),
        ];
        for (typo, meant, value, node) in cases {
            let node = node.replace("VALUE", value);
            let err = one_node(&node.replace("KEY", typo))
                .expect_err("a misspelt field must not parse")
                .to_string();
            assert!(
                err.contains(typo),
                "the diagnostic names the misspelt key: {err}"
            );
            one_node(&node.replace("KEY", meant))
                .unwrap_or_else(|e| panic!("`{meant}` spelt right loads: {e}"));
        }
    }

    /// A choice is always a speech balloon: it has no `kind`, so `kind: Thought` on one is an
    /// unknown field, not a silently dropped request for a thought balloon.
    #[test]
    fn a_field_the_variant_lacks_is_refused() {
        let err = one_node(
            r#"Choice(speaker: "s", kind: Thought, prompt: "?", options: [(text: "t", next: "a")])"#,
        )
        .expect_err("a choice has no `kind`")
        .to_string();
        assert!(err.contains("kind"), "{err}");
    }

    #[test]
    fn merge_adds_and_refuses_duplicates() {
        let mut a = sample();
        let mut other = DialogueScript::default();
        other
            .conversations
            .insert("outro".into(), a.conversations["intro"].clone());
        a.merge(other.clone()).expect("a new id merges");
        assert!(a.conversation("outro").is_some());
        let before = a.clone();
        assert!(a.merge(other).is_err(), "a duplicate id must be refused");
        assert_eq!(a, before, "a refused merge changed the script");
    }

    // ----- the state machine, without rendering -----

    /// Build the conversation **state machine** — and only it.
    ///
    /// The presenters are deliberately absent: they spawn quads or UI and need assets a GPU-free unit
    /// test lacks. What they contribute to the graph is one boolean — `presented` — which these tests
    /// set by hand at the point the presenter would have. So the coverage claim is exact: *input →
    /// pick → node walk → teardown*, not rendering, and not pointer picking.
    fn machine() -> App {
        let mut app = App::new();
        app.add_message::<ChoicePicked>()
            .add_message::<ConversationEnded>()
            .add_message::<DialogueNodeEntered>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Time<Real>>()
            .init_resource::<Presenter>()
            .insert_resource(ConversationLock)
            // Same order as the plugin: a digit pressed this frame must resolve this frame.
            .add_systems(
                Update,
                (choice_hotkeys, resolve_choice, advance_line).chain(),
            );
        app
    }

    /// One conversation: a two-option choice, each option landing on a terminal line.
    fn script() -> DialogueScript {
        let mut nodes = HashMap::new();
        nodes.insert(
            "ask".into(),
            Node::Choice {
                speaker: "lead".into(),
                emotion: Emotion::Neutral,
                prompt: "Orders?".into(),
                options: vec![
                    Choice {
                        text: "Hold.".into(),
                        next: "held".into(),
                    },
                    Choice {
                        text: "Advance.".into(),
                        next: "moved".into(),
                    },
                ],
            },
        );
        for (id, text) in [("held", "Holding."), ("moved", "Moving up.")] {
            nodes.insert(
                id.into(),
                Node::Line {
                    speaker: "lead".into(),
                    kind: BubbleKind::Speech,
                    emotion: Emotion::Neutral,
                    text: text.into(),
                    next: None,
                },
            );
        }
        let mut conversations = HashMap::new();
        conversations.insert(
            "t".into(),
            Conversation {
                start: "ask".into(),
                nodes,
            },
        );
        DialogueScript { conversations }
    }

    /// Park the cursor on `node`, already presented — i.e. the state the player is looking at.
    ///
    /// `advance_at` is `INFINITY` throughout, which is not decoration: it is what `present_current`
    /// writes for a choice, and it means **no test here can pass on the auto-advance timeout**. If a
    /// key stops working, these fail rather than quietly falling through to the clock.
    fn park(app: &mut App, node: &str) {
        app.insert_resource(script());
        app.insert_resource(Active {
            conv: "t".into(),
            node: node.into(),
            presented: true,
            advance_at: f32::INFINITY,
        });
    }

    fn node_of(app: &App) -> Option<String> {
        app.world().get_resource::<Active>().map(|a| a.node.clone())
    }

    /// Press `key`, run one frame, release. Bevy's own input system (which clears `just_pressed`) is
    /// not registered here, so the release is manual.
    fn tap(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    /// A choice never auto-advances, so an *unanswerable* choice is an unrecoverable run. This walks
    /// the graph on the keyboard alone and asserts the host is handed back its game: cursor gone, lock
    /// gone, `ConversationEnded` sent.
    #[test]
    fn the_keyboard_can_answer_a_choice_so_a_conversation_can_never_soft_lock() {
        let mut app = machine();
        park(&mut app, "ask");

        tap(&mut app, KeyCode::Digit2);
        assert_eq!(
            node_of(&app).as_deref(),
            Some("moved"),
            "'2' takes the second option"
        );

        // The presenter would put the new line on screen; stand in for it and read it.
        if let Some(mut a) = app.world_mut().get_resource_mut::<Active>() {
            a.presented = true;
        }
        tap(&mut app, KeyCode::Space);

        assert!(
            app.world().get_resource::<Active>().is_none(),
            "the conversation cursor outlived the conversation"
        );
        assert!(
            app.world().get_resource::<ConversationLock>().is_none(),
            "the lock survived — the host would still be yielding input"
        );
        let ended: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<ConversationEnded>>()
            .drain()
            .collect();
        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].id, "t");
    }

    /// The numpad row is a second *device*, not a second path: it writes the same `ChoicePicked`.
    #[test]
    fn the_numpad_picks_the_same_option_as_the_number_row() {
        let mut app = machine();
        park(&mut app, "ask");
        tap(&mut app, KeyCode::Numpad1);
        assert_eq!(node_of(&app).as_deref(), Some("held"));
    }

    /// An out-of-range digit is ignored, never clamped.
    #[test]
    fn a_digit_past_the_last_option_does_nothing() {
        let mut app = machine();
        park(&mut app, "ask");
        tap(&mut app, KeyCode::Digit5);
        assert_eq!(node_of(&app).as_deref(), Some("ask"));
    }

    /// A stray digit pressed during a *line* must not queue a pick that lands on the next choice the
    /// player reaches.
    #[test]
    fn a_digit_pressed_during_a_line_does_not_answer_the_next_choice() {
        let mut app = machine();
        park(&mut app, "held"); // a Line node
        tap(&mut app, KeyCode::Digit2);

        // A digit does not read a line, so the cursor is still there. Park a fresh choice and run a
        // quiet frame: nothing was queued, so the cursor must still be sitting on the choice.
        park(&mut app, "ask");
        app.update();
        assert_eq!(node_of(&app).as_deref(), Some("ask"));
    }

    /// `Enter` reads a line as well as `Space` — with `advance_at` at infinity, only the key can.
    #[test]
    fn enter_advances_a_line() {
        let mut app = machine();
        park(&mut app, "held");
        tap(&mut app, KeyCode::Enter);
        assert!(app.world().get_resource::<Active>().is_none());
    }

    #[test]
    fn a_left_click_advances_a_line() {
        let mut app = machine();
        park(&mut app, "held");
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(app.world().get_resource::<Active>().is_none());
    }

    // ----- barks -----

    fn bark(text: &str) -> Bark {
        Bark {
            speaker: "lead".into(),
            kind: BubbleKind::Speech,
            emotion: Emotion::Neutral,
            text: text.into(),
        }
    }

    /// Two barks on the same frame used to pop both balloons at once. The second must now wait a full
    /// gap.
    #[test]
    fn a_second_bark_waits_a_full_gap() {
        let s = BarkSettings::default();
        let mut q = BarkQueue::default();
        q.push(bark("Contact. Hold your lane."), 0.0, &s);
        q.push(bark("Copy."), 0.0, &s);

        // The first speaks immediately — an idle world is never made to wait.
        assert_eq!(
            q.take_ready(0.0, &s).map(|b| b.text),
            Some("Contact. Hold your lane.".into())
        );
        q.spoke_at(0.0, &s);

        assert!(q.take_ready(0.5, &s).is_none(), "reply jumped the gap");
        assert!(
            q.take_ready(s.gap - 0.01, &s).is_none(),
            "reply jumped the gap"
        );
        assert_eq!(
            q.take_ready(s.gap, &s).map(|b| b.text),
            Some("Copy.".into())
        );
    }

    #[test]
    fn first_bark_is_never_delayed() {
        let s = BarkSettings::default();
        let mut q = BarkQueue::default();
        q.push(bark("Moving."), 0.0, &s);
        assert!(q.take_ready(0.0, &s).is_some());
    }

    #[test]
    fn a_bark_that_waited_too_long_is_dropped_not_narrated_late() {
        let s = BarkSettings::default();
        let mut q = BarkQueue::default();
        q.push(bark("Contact."), 0.0, &s);
        assert!(q.take_ready(s.stale + 0.01, &s).is_none());
        assert!(q.is_empty());
    }

    /// Staleness must be applied before the gap check, or a dead bark parked at the head would block
    /// every fresh bark behind it.
    #[test]
    fn a_stale_head_does_not_starve_a_fresh_bark() {
        let s = BarkSettings::default();
        let mut q = BarkQueue::default();
        q.push(bark("old"), 0.0, &s);
        q.push(bark("fresh"), 2.5, &s);
        assert_eq!(q.take_ready(3.5, &s).map(|b| b.text), Some("fresh".into()));
    }

    #[test]
    fn a_burst_keeps_the_freshest_barks() {
        let s = BarkSettings::default();
        let mut q = BarkQueue::default();
        for i in 0..s.queue_cap + 2 {
            q.push(bark(&i.to_string()), 0.0, &s);
        }
        assert_eq!(q.len(), s.queue_cap);
        // The two oldest were evicted, not the two newest.
        assert_eq!(q.take_ready(0.0, &s).map(|b| b.text), Some("2".into()));
    }

    /// A burst drains one balloon at a time, each a gap apart — the whole point of the queue.
    #[test]
    fn a_burst_drains_one_balloon_per_gap() {
        let s = BarkSettings::default();
        let mut q = BarkQueue::default();
        for i in 0..s.queue_cap {
            q.push(bark(&i.to_string()), 0.0, &s);
        }
        let mut spoken = Vec::new();
        let mut t = 0.0_f32;
        while t <= s.stale {
            if let Some(b) = q.take_ready(t, &s) {
                q.spoke_at(t, &s);
                spoken.push((b.text, t));
            }
            t += 0.1;
        }
        assert!(spoken.len() >= 3, "expected a paced drain, got {spoken:?}");
        assert!(
            spoken.len() <= s.queue_cap,
            "released more than were queued"
        );
        for w in spoken.windows(2) {
            assert!(
                w[1].1 - w[0].1 >= s.gap - 1.0e-3,
                "balloons {:?} and {:?} overlap",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn a_speaker_resolves_to_a_head_a_narrator_or_nothing() {
        let mut speakers = Speakers::default();
        let head = Entity::from_raw_u32(7).expect("a valid index");
        speakers.0.insert(
            "miller".into(),
            Speaker {
                entity: Some(head),
                ..Default::default()
            },
        );
        speakers.0.insert("voice".into(), Speaker::default());
        assert!(matches!(resolve_speaker(&speakers, "miller"), Resolved::Head(e) if e == head));
        assert!(matches!(
            resolve_speaker(&speakers, "voice"),
            Resolved::Narrator
        ));
        assert!(matches!(
            resolve_speaker(&speakers, "miler"),
            Resolved::Unknown
        ));
    }

    #[test]
    fn a_key_is_warned_once_and_the_set_never_outgrows_its_bound() {
        let mut warned = WarnedSpeakers::default();
        assert!(warned.first_sight("a"));
        assert!(!warned.first_sight("a"));
        for i in 0..MAX_WARNED_SPEAKERS * 3 {
            warned.first_sight(&format!("key{i}"));
            assert!(warned.0.len() <= MAX_WARNED_SPEAKERS);
        }
        // Past the bound the oldest keys were forgotten, so one is warned about again.
        assert!(warned.first_sight("a"));
    }
}
