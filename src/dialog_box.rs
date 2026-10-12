//! The screen dialog box: a Bevy UI surface skinned by the host's [`DialogSkin`].
//!
//! Shown when [`Presenter::DialogBox`](crate::Presenter) is set. One box serves a whole conversation:
//! it is built when the conversation's first node is presented, rebuilt inside for each new node (with
//! a small pop), and despawned when the conversation ends. The text is revealed by a typewriter whose
//! speed is read from [`DialogSkin::chars_per_second`] every tick, so a settings menu can change it
//! live. The unrevealed tail of a line is laid out in a transparent span, so words never jump to the
//! next row as they appear. Reading speed: Callander et al., *The effects of text presentation format
//! on reading comprehension and video game performance* (Computers in Human Behavior, 2014) put the
//! optimum near 6 letters per second and found comprehension falling above normal reading speed — a
//! typewriter is a pacing aid, so a press always completes the line instantly.

use bevy::picking::Pickable;
use bevy::picking::events::{PointerClick, PointerOver};
use bevy::prelude::*;
use bevy::sprite::TextureSlicer;
use bevy::time::Real;
use bevy::ui::widget::NodeImageMode;

use crate::conversation::{
    Active, ChoicePicked, ConversationEnded, ConversationEntity, DialogueScript, Node as ScriptNode, Presenter,
    Speakers, finish,
};

/// Global z-index of the dialog box.
pub const DIALOG_Z: i32 = 20;

/// A comfortable default for [`DialogSkin::chars_per_second`].
pub const DEFAULT_CHARS_PER_SECOND: f32 = 40.0;

/// Seconds a caret frame is shown before it swaps.
const CARET_FRAME_SECS: f32 = 0.4;
/// Scale the box grows from when a conversation opens, and over how long.
const OPEN_FROM: f32 = 0.96;
const OPEN_SECS: f32 = 0.12;
/// Scale a new line pops from, and over how long.
const POP_FROM: f32 = 1.08;
const POP_SECS: f32 = 0.15;

/// How the dialog box looks. The host builds it (from its own theme) and inserts it; mutate
/// `chars_per_second` at run time for a text-speed setting. All sizes are in the host's UI pixels.
#[derive(Resource, Clone)]
pub struct DialogSkin {
    /// The box's frame, drawn 9-sliced over the paper.
    pub frame: Handle<Image>,
    pub frame_slicer: TextureSlicer,
    /// The box's background, tiled.
    pub paper: Handle<Image>,
    /// The speaker's name plate above the top-left corner, 9-sliced.
    pub name_tag: Handle<Image>,
    pub name_tag_slicer: TextureSlicer,
    /// Two frames of the "line is complete" caret at the bottom-right; they alternate.
    pub caret: [Handle<Image>; 2],
    pub body_font: TextFont,
    pub title_font: TextFont,
    pub text_color: Color,
    pub choice_color: Color,
    pub choice_hover_color: Color,
    pub padding: f32,
    pub width: f32,
    pub portrait_size: f32,
    /// Typewriter speed; `<= 0.0` shows a line at once.
    pub chars_per_second: f32,
    pub bottom_margin: f32,
}

/// The option the keyboard (or the pointer hovering it) has focused in a choice node.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChoiceFocus(pub usize);

/// The box's root: one per conversation.
#[derive(Component)]
pub(crate) struct DialogRoot;

/// Reveals a line a few characters at a time.
#[derive(Component, Debug)]
pub(crate) struct Typewriter {
    /// Characters currently shown.
    pub(crate) shown: usize,
    /// Fractional characters carried to the next tick.
    pub(crate) acc: f32,
    /// Characters in the whole line.
    pub(crate) total: usize,
    /// Characters the `Text` currently displays (`usize::MAX` before the first draw).
    drawn: usize,
}

impl Typewriter {
    fn complete(&self) -> bool {
        self.shown >= self.total
    }
}

/// The whole line, and the transparent span holding its unrevealed tail.
#[derive(Component)]
pub(crate) struct LineText {
    full: String,
    rest: Entity,
}

/// The blinking "line is complete" caret.
#[derive(Component)]
pub(crate) struct CaretBlink {
    elapsed: f32,
}

/// One selectable row of a choice node.
#[derive(Component)]
pub(crate) struct ChoiceRow(usize);

/// An eased scale of the box's `UiTransform` toward 1.0.
#[derive(Component)]
pub(crate) struct BoxAnim {
    from: f32,
    seconds: f32,
    elapsed: f32,
}

impl BoxAnim {
    fn open() -> Self {
        Self {
            from: OPEN_FROM,
            seconds: OPEN_SECS,
            elapsed: 0.0,
        }
    }

    fn pop() -> Self {
        Self {
            from: POP_FROM,
            seconds: POP_SECS,
            elapsed: 0.0,
        }
    }
}

/// Complete every running typewriter. `true` if one was still running — the press that asked is spent
/// on that, not on advancing.
pub(crate) fn complete_typing(typewriters: &mut Query<&mut Typewriter>) -> bool {
    let mut any = false;
    for mut tw in typewriters {
        if !tw.complete() {
            tw.shown = tw.total;
            any = true;
        }
    }
    any
}

/// What a node asks the box to show.
struct Shown {
    speaker: String,
    text: String,
    options: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn present_dialog_box(
    mut commands: Commands,
    presenter: Res<Presenter>,
    active: Option<ResMut<Active>>,
    script: Res<DialogueScript>,
    speakers: Res<Speakers>,
    skin: Option<Res<DialogSkin>>,
    root: Query<Entity, With<DialogRoot>>,
    existing: Query<Entity, With<ConversationEntity>>,
    mut focus: ResMut<ChoiceFocus>,
    mut ended: MessageWriter<ConversationEnded>,
) {
    if *presenter != Presenter::DialogBox {
        return;
    }
    let Some(mut active) = active else {
        return;
    };
    if active.presented {
        return;
    }

    let Some(skin) = skin else {
        error!("bevy_speech_bubbles: Presenter::DialogBox needs a DialogSkin");
        let id = active.conv.clone();
        finish(&mut commands, &existing, &mut ended, id);
        return;
    };
    let shown = match script
        .conversation(&active.conv)
        .and_then(|c| c.nodes.get(&active.node))
    {
        Some(ScriptNode::Line { speaker, text, .. }) => Shown {
            speaker: speaker.clone(),
            text: text.clone(),
            options: Vec::new(),
        },
        Some(ScriptNode::Choice {
            speaker,
            prompt,
            options,
            ..
        }) => Shown {
            speaker: speaker.clone(),
            text: prompt.clone(),
            options: options.iter().map(|o| o.text.clone()).collect(),
        },
        None => {
            warn!(
                "bevy_speech_bubbles: node '{}' of '{}' is missing; ending",
                active.node, active.conv
            );
            let id = active.conv.clone();
            finish(&mut commands, &existing, &mut ended, id);
            return;
        }
    };

    let (name, portrait) = match speakers.0.get(&shown.speaker) {
        Some(s) => (s.name.clone(), s.portrait.clone()),
        None => {
            warn!(
                "bevy_speech_bubbles: speaker '{}' is not in Speakers; showing its key",
                shown.speaker
            );
            (shown.speaker.clone(), None)
        }
    };
    focus.0 = 0;

    // One box per conversation: reuse it (with a pop) for the next node, build it for the first.
    let root_e = match root.single() {
        Ok(e) => {
            commands.entity(e).despawn_children();
            commands.entity(e).insert(BoxAnim::pop());
            e
        }
        Err(_) => {
            commands
                .spawn((
                    DialogRoot,
                    ConversationEntity,
                    GlobalZIndex(DIALOG_Z),
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(50),
                        margin: UiRect::left(px(-skin.width * 0.5)),
                        bottom: px(skin.bottom_margin),
                        width: px(skin.width),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    UiTransform::from_scale(Vec2::splat(OPEN_FROM)),
                    BoxAnim::open(),
                ))
                .id()
        }
    };

    let paper = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                ..default()
            },
            ImageNode {
                image: skin.paper.clone(),
                image_mode: NodeImageMode::Tiled {
                    tile_x: true,
                    tile_y: true,
                    stretch_value: 1.0,
                },
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let frame = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                ..default()
            },
            ImageNode {
                image: skin.frame.clone(),
                image_mode: NodeImageMode::Sliced(skin.frame_slicer.clone()),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();

    // The text: the revealed part in the text colour, the rest in a transparent span so the layout is
    // that of the finished line from the first frame.
    let total = shown.text.chars().count();
    let span = commands
        .spawn((
            TextSpan::new(shown.text.clone()),
            skin.body_font.clone(),
            TextColor(Color::NONE),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(""),
            skin.body_font.clone(),
            TextColor(skin.text_color),
            Node {
                width: percent(100),
                ..default()
            },
            Typewriter {
                shown: 0,
                acc: 0.0,
                total,
                drawn: usize::MAX,
            },
            LineText {
                full: shown.text,
                rest: span,
            },
        ))
        .add_child(span)
        .id();

    let column = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            row_gap: px(skin.padding * 0.5),
            ..default()
        })
        .add_child(text)
        .id();
    for (i, option) in shown.options.iter().enumerate() {
        let row = commands
            .spawn((
                ChoiceRow(i),
                Node {
                    padding: UiRect::axes(px(0), px(1)),
                    ..default()
                },
                Text::new(format!("{}. {option}", i + 1)),
                skin.body_font.clone(),
                TextColor(skin.choice_color),
                Pickable::default(),
            ))
            .observe(move |_: On<PointerClick>, mut picked: MessageWriter<ChoicePicked>| {
                picked.write(ChoicePicked { index: i });
            })
            .observe(move |_: On<PointerOver>, mut focus: ResMut<ChoiceFocus>| {
                focus.0 = i;
            })
            .id();
        commands.entity(column).add_child(row);
    }

    let content = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            padding: UiRect::all(px(skin.padding)),
            column_gap: px(skin.padding),
            ..default()
        })
        .id();
    if let Some(portrait) = portrait {
        let p = commands
            .spawn((
                Node {
                    width: px(skin.portrait_size),
                    height: px(skin.portrait_size),
                    flex_shrink: 0.0,
                    ..default()
                },
                ImageNode::new(portrait),
            ))
            .id();
        commands.entity(content).add_child(p);
    }
    commands.entity(content).add_child(column);

    let caret = commands
        .spawn((
            CaretBlink { elapsed: 0.0 },
            Node {
                position_type: PositionType::Absolute,
                right: px(skin.padding),
                bottom: px(skin.padding * 0.5),
                ..default()
            },
            ImageNode::new(skin.caret[0].clone()),
            Visibility::Hidden,
        ))
        .id();

    commands
        .entity(root_e)
        .add_children(&[paper, frame, content, caret]);

    if !name.is_empty() {
        let tag = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(skin.padding),
                    bottom: percent(100),
                    padding: UiRect::axes(px(skin.padding * 0.5), px(2)),
                    ..default()
                },
                ImageNode {
                    image: skin.name_tag.clone(),
                    image_mode: NodeImageMode::Sliced(skin.name_tag_slicer.clone()),
                    ..default()
                },
            ))
            .with_children(|t| {
                t.spawn((
                    Text::new(name),
                    skin.title_font.clone(),
                    TextColor(skin.text_color),
                ));
            })
            .id();
        commands.entity(root_e).add_child(tag);
    }

    active.advance_at = f32::INFINITY; // a boxed line waits for the player
    active.presented = true;
}

/// Reveal text. The speed is read from the skin every tick, so a settings change applies to the line
/// on screen.
pub(crate) fn typewriter_tick(
    time: Res<Time<Real>>,
    skin: Option<Res<DialogSkin>>,
    mut lines: Query<(&mut Typewriter, &LineText, &mut Text)>,
    mut spans: Query<&mut TextSpan>,
) {
    let cps = skin.map_or(DEFAULT_CHARS_PER_SECOND, |s| s.chars_per_second);
    let dt = time.delta_secs();
    for (mut tw, line, mut text) in &mut lines {
        if !tw.complete() {
            if cps <= 0.0 {
                tw.shown = tw.total;
            } else {
                tw.acc += dt * cps;
                let n = tw.acc.floor();
                tw.acc -= n;
                tw.shown = (tw.shown + n as usize).min(tw.total);
            }
        }
        if tw.drawn == tw.shown {
            continue;
        }
        tw.drawn = tw.shown;
        let split = line
            .full
            .char_indices()
            .nth(tw.shown)
            .map_or(line.full.len(), |(i, _)| i);
        text.0 = line.full[..split].to_owned();
        if let Ok(mut span) = spans.get_mut(line.rest) {
            span.0 = line.full[split..].to_owned();
        }
    }
}

/// Swap the caret's two frames and show it only once the line is complete.
pub(crate) fn caret_blink(
    time: Res<Time<Real>>,
    skin: Option<Res<DialogSkin>>,
    typewriters: Query<&Typewriter>,
    mut carets: Query<(&mut CaretBlink, &mut ImageNode, &mut Visibility)>,
) {
    let Some(skin) = skin else { return };
    let typing = typewriters.iter().any(|t| !t.complete());
    for (mut blink, mut image, mut vis) in &mut carets {
        blink.elapsed += time.delta_secs();
        let frame = (blink.elapsed / CARET_FRAME_SECS) as usize % 2;
        if image.image != skin.caret[frame] {
            image.image = skin.caret[frame].clone();
        }
        let want = if typing {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Arrows move the focused option, `Enter`/`Space` pick it (the first press while the prompt is still
/// typing completes it instead). Writes the same [`ChoicePicked`] as the pointer and the digits.
pub(crate) fn choice_focus_input(
    keys: Res<ButtonInput<KeyCode>>,
    presenter: Res<Presenter>,
    active: Option<Res<Active>>,
    script: Res<DialogueScript>,
    mut focus: ResMut<ChoiceFocus>,
    mut typewriters: Query<&mut Typewriter>,
    mut picked: MessageWriter<ChoicePicked>,
) {
    if *presenter != Presenter::DialogBox {
        return;
    }
    let Some(active) = active else { return };
    if !active.presented {
        return;
    }
    let Some(ScriptNode::Choice { options, .. }) = script
        .conversation(&active.conv)
        .and_then(|c| c.nodes.get(&active.node))
    else {
        return;
    };
    let n = options.len();
    if n == 0 {
        return;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        focus.0 = (focus.0 + 1) % n;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        focus.0 = (focus.0 + n - 1) % n;
    }
    let confirm = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::NumpadEnter)
        || keys.just_pressed(KeyCode::Space);
    if confirm && !complete_typing(&mut typewriters) {
        picked.write(ChoicePicked {
            index: focus.0.min(n - 1),
        });
    }
}

/// Colour each option by whether it is the focused one.
pub(crate) fn paint_choices(
    skin: Option<Res<DialogSkin>>,
    focus: Res<ChoiceFocus>,
    mut rows: Query<(&ChoiceRow, &mut TextColor)>,
) {
    let Some(skin) = skin else { return };
    for (row, mut colour) in &mut rows {
        let want = if row.0 == focus.0 {
            skin.choice_hover_color
        } else {
            skin.choice_color
        };
        if colour.0 != want {
            colour.0 = want;
        }
    }
}

/// Ease the box's scale toward 1.0 (ease-out cubic), then drop the animation.
pub(crate) fn animate_box(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut boxes: Query<(Entity, &mut BoxAnim, &mut UiTransform)>,
) {
    for (entity, mut anim, mut transform) in &mut boxes {
        anim.elapsed += time.delta_secs();
        let t = (anim.elapsed / anim.seconds).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        transform.scale = Vec2::splat(anim.from + (1.0 - anim.from) * eased);
        if t >= 1.0 {
            transform.scale = Vec2::ONE;
            commands.entity(entity).remove::<BoxAnim>();
        }
    }
}
