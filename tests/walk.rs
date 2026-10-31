//! **The whole kit, headless.** Walks a three-node script through [`SpeechBubblesPlugin`] with the
//! screen dialog box as the presenter, by writing key presses into `ButtonInput<KeyCode>` — the same
//! path a player's keyboard takes — and checks the box that appears and the message that ends it.
//!
//! No window, no GPU: the dialog box is plain UI components, so it exists in the world without a
//! renderer. What this does not prove is how it looks; `examples/dialog_box.rs` does.

use bevy::prelude::*;
use bevy_speech_bubbles::{
    Bark, BarkSettings, BubbleAssets, ConversationEnded, ConversationLock, DialogInputSet,
    DialogSkin, DialogueNodeEntered, DialogueScript, Presenter, Speaker, Speakers,
    SpeechBubblesPlugin, StartConversation, validate_script,
};

#[derive(Component)]
struct TestCamera;

const SCRIPT: &str = r#"
(
  conversations: {
    "t": (
      start: "hello",
      nodes: {
        "hello": Line(speaker: "narrator", text: "The gate is shut.", next: Some("ask")),
        "ask": Choice(speaker: "narrator", prompt: "Open it?", options: [
          (text: "Leave it", next: "left"),
          (text: "Open it", next: "opened"),
        ]),
        "left": Line(speaker: "narrator", text: "Left.", next: None),
        "opened": Line(speaker: "narrator", text: "It creaks.", next: None),
      },
    ),
  },
)
"#;

/// Three nodes, named for the test: a start line, a choice whose second option leads to `y`, and two ends.
const NODES: &str = r#"
(
  conversations: {
    "t": (
      start: "start",
      nodes: {
        "start": Line(speaker: "narrator", text: "Hello.", next: Some("ask")),
        "ask": Choice(speaker: "narrator", prompt: "Open it?", options: [
          (text: "Leave it", next: "x"),
          (text: "Open it", next: "y"),
        ]),
        "x": Line(speaker: "narrator", text: "Left.", next: None),
        "y": Line(speaker: "narrator", text: "It creaks.", next: None),
      },
    ),
  },
)
"#;

fn skin() -> DialogSkin {
    let slicer = bevy::sprite::TextureSlicer {
        border: BorderRect::all(4.0),
        ..default()
    };
    DialogSkin {
        frame: Handle::default(),
        frame_slicer: slicer.clone(),
        paper: Handle::default(),
        name_tag: Handle::default(),
        name_tag_slicer: slicer,
        caret: [Handle::default(), Handle::default()],
        body_font: TextFont::default(),
        title_font: TextFont::default(),
        text_color: Color::BLACK,
        choice_color: Color::BLACK,
        choice_hover_color: Color::WHITE,
        padding: 8.0,
        width: 320.0,
        portrait_size: 48.0,
        chars_per_second: 0.0,
        bottom_margin: 16.0,
    }
}

fn app() -> App {
    app_with(SCRIPT)
}

fn app_with(src: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_plugins(SpeechBubblesPlugin::<TestCamera>::default());

    let script: DialogueScript = ron::from_str(src).expect("script parses");
    validate_script(&script).expect("script validates");
    app.insert_resource(script)
        .insert_resource(Presenter::DialogBox)
        .insert_resource(skin());
    app.world_mut().resource_mut::<Speakers>().0.insert(
        "narrator".into(),
        Speaker {
            entity: None,
            name: "Narrator".into(),
            portrait: None,
        },
    );
    app
}

fn tap(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    // Release too: `clear` only drops `just_pressed`, and a key still held cannot be pressed again.
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
}

fn start(app: &mut App) {
    app.world_mut().write_message(StartConversation { id: "t".into() });
    app.update();
}

fn ended(app: &mut App) -> Vec<String> {
    app.world_mut()
        .resource_mut::<Messages<ConversationEnded>>()
        .drain()
        .map(|e| e.id)
        .collect()
}

/// Every `Text` and `TextSpan` string in the world, in no particular order.
fn texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut out: Vec<String> = world.query::<&Text>().iter(world).map(|t| t.0.clone()).collect();
    out.extend(world.query::<&TextSpan>().iter(world).map(|t| t.0.clone()));
    out
}

/// Only the revealed text: the `Text` roots, not the transparent spans that hold the unrevealed tail.
fn revealed(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world.query::<&Text>().iter(world).map(|t| t.0.clone()).collect()
}

fn shows(app: &mut App, needle: &str) -> bool {
    texts(app).iter().any(|t| t.contains(needle))
}

/// Space, `2`, Space: line → choice → line → end. The conversation ends with `ConversationEnded`, the
/// lock is released, and the box is gone.
#[test]
fn a_conversation_walks_on_the_keyboard_and_ends() {
    let mut app = app();
    start(&mut app);
    assert!(app.world().get_resource::<ConversationLock>().is_some());
    assert!(shows(&mut app, "The gate is shut."), "the first line is not on screen");
    assert!(shows(&mut app, "Narrator"), "the name tag is missing");

    tap(&mut app, KeyCode::Space);
    assert!(shows(&mut app, "Open it?"), "the choice prompt is not on screen");
    assert!(shows(&mut app, "1. Leave it") && shows(&mut app, "2. Open it"), "the options are not on screen");

    tap(&mut app, KeyCode::Digit2);
    assert!(shows(&mut app, "It creaks."), "option 2 did not lead to 'opened'");
    assert!(ended(&mut app).is_empty(), "the conversation ended early");

    tap(&mut app, KeyCode::Space);
    assert_eq!(ended(&mut app), vec!["t".to_string()]);
    assert!(app.world().get_resource::<ConversationLock>().is_none());
    assert!(texts(&mut app).is_empty(), "the dialog box outlived its conversation");
}

/// Every `DialogueNodeEntered` written since the last call, as `(conversation, node)`.
fn entered(app: &mut App) -> Vec<(String, String)> {
    app.world_mut()
        .resource_mut::<Messages<DialogueNodeEntered>>()
        .drain()
        .map(|e| (e.conversation, e.node))
        .collect()
}

/// The cursor reports each node it lands on, in order, so a host can hang an effect on the node a choice leads to.
#[test]
fn the_cursor_announces_every_node_it_enters() {
    let mut app = app_with(NODES);
    start(&mut app);
    let mut seen = entered(&mut app);
    tap(&mut app, KeyCode::Space);
    seen.extend(entered(&mut app));
    tap(&mut app, KeyCode::Digit2);
    seen.extend(entered(&mut app));
    tap(&mut app, KeyCode::Space);
    seen.extend(entered(&mut app));
    let t = |node: &str| ("t".to_string(), node.to_string());
    assert_eq!(seen, vec![t("start"), t("ask"), t("y")]);
}

/// A pick past the last option leads nowhere, so it announces no node and the conversation stays open.
#[test]
fn a_pick_past_the_last_option_announces_nothing() {
    let mut app = app_with(NODES);
    start(&mut app);
    tap(&mut app, KeyCode::Space);
    assert_eq!(entered(&mut app).len(), 2, "start and ask should have been announced");
    tap(&mut app, KeyCode::Digit3);
    assert!(entered(&mut app).is_empty(), "a pick past the last option announced a node");
    assert!(ended(&mut app).is_empty(), "a pick past the last option ended the conversation");
}

/// One box is reused across nodes (it pops, it is not respawned), so a conversation never flickers.
#[test]
fn one_box_serves_the_whole_conversation() {
    let mut app = app();
    start(&mut app);
    let world = app.world_mut();
    let first: Vec<Entity> = world
        .query_filtered::<Entity, With<GlobalZIndex>>()
        .iter(world)
        .collect();
    assert_eq!(first.len(), 1, "exactly one dialog root");
    tap(&mut app, KeyCode::Space);
    let world = app.world_mut();
    let second: Vec<Entity> = world
        .query_filtered::<Entity, With<GlobalZIndex>>()
        .iter(world)
        .collect();
    assert_eq!(first, second, "the root was respawned for the next node");
}

/// The box is the screen's: `Enter` and the arrow keys drive a choice too.
#[test]
fn arrows_and_enter_pick_a_choice() {
    let mut app = app();
    start(&mut app);
    tap(&mut app, KeyCode::Space);
    tap(&mut app, KeyCode::ArrowDown);
    tap(&mut app, KeyCode::Enter);
    assert!(shows(&mut app, "It creaks."), "arrow-down then Enter should pick option 2");
}

/// With a typewriter running, the first press completes the line and only the second advances.
#[test]
fn a_press_completes_a_typing_line_before_it_advances() {
    let mut app = app();
    app.world_mut().resource_mut::<DialogSkin>().chars_per_second = 1.0;
    start(&mut app);
    // `Time<Real>` barely moves in a test, so at one character a second the line is still empty.
    assert!(
        !revealed(&mut app).iter().any(|t| t == "The gate is shut."),
        "the line was revealed at once despite 1 char/s"
    );

    tap(&mut app, KeyCode::Space);
    assert!(
        revealed(&mut app).iter().any(|t| t == "The gate is shut."),
        "the first press must complete the line"
    );
    assert!(!shows(&mut app, "Open it?"), "the first press advanced instead of completing");

    tap(&mut app, KeyCode::Space);
    assert!(shows(&mut app, "Open it?"), "the second press must advance");
}

/// The typewriter reads its speed from the skin every tick: lowering it to zero mid-line finishes it.
#[test]
fn the_typewriter_speed_is_read_live() {
    let mut app = app();
    app.world_mut().resource_mut::<DialogSkin>().chars_per_second = 0.5;
    start(&mut app);
    assert!(!revealed(&mut app).iter().any(|t| t == "The gate is shut."));
    app.world_mut().resource_mut::<DialogSkin>().chars_per_second = 0.0;
    app.update();
    assert!(
        revealed(&mut app).iter().any(|t| t == "The gate is shut."),
        "the new speed was not applied to the line on screen"
    );
}

/// A box with no skin refuses loudly and ends the conversation instead of hanging the lock.
#[test]
fn a_missing_skin_ends_the_conversation() {
    let mut app = app();
    app.world_mut().remove_resource::<DialogSkin>();
    start(&mut app);
    assert_eq!(ended(&mut app), vec!["t".to_string()]);
    assert!(app.world().get_resource::<ConversationLock>().is_none());
}

/// A bark is a world balloon whatever the presenter: with the box as presenter it still spawns a quad
/// over its speaker's entity, and a narrator's bark (no entity) is dropped.
#[test]
fn a_bark_is_always_a_world_balloon() {
    use bevy_speech_bubbles::{Bubble, BubbleKind, Emotion};

    let mut app = app();
    let font = ab_glyph::FontArc::try_from_slice(include_bytes!("data/FiraMono-subset.ttf"))
        .expect("the test font parses");
    app.insert_resource(BubbleAssets {
        quad: Handle::default(),
        font,
    });
    let villager = app.world_mut().spawn((Transform::default(), Visibility::default())).id();
    app.world_mut().resource_mut::<Speakers>().0.insert(
        "villager".into(),
        Speaker {
            entity: Some(villager),
            name: "Villager".into(),
            portrait: None,
        },
    );
    assert_eq!(*app.world().resource::<Presenter>(), Presenter::DialogBox);
    assert_eq!(app.world().resource::<BarkSettings>().gap, 1.0);

    for speaker in ["narrator", "villager"] {
        app.world_mut().write_message(Bark {
            speaker: speaker.into(),
            kind: BubbleKind::Speech,
            emotion: Emotion::Neutral,
            text: "Thank you.".into(),
        });
    }
    // One bark is released per update: the narrator's is dropped (no head, no gap started), then the
    // villager's goes up.
    app.update();
    app.update();
    let world = app.world_mut();
    let balloons: Vec<Entity> = world.query_filtered::<Entity, With<Bubble>>().iter(world).collect();
    assert_eq!(balloons.len(), 1, "only the villager's bark has a head to float over");
}

/// Records the WARN and ERROR events this crate logs, so a test can ask what a host would read.
mod warned {
    use std::fmt::Debug;
    use std::sync::{Arc, Mutex};

    use bevy::ecs::schedule::SingleThreadedExecutor;
    use bevy::log::tracing::{
        self, Event, Level, Subscriber,
        field::{Field, Visit},
    };
    use bevy::log::tracing_subscriber::{Layer, layer::Context, layer::SubscriberExt, registry::Registry};
    use bevy::prelude::*;

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<String>>>);

    struct Message(String);

    impl Visit for Message {
        fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
            if field.name() == "message" {
                self.0 = format!("{value:?}");
            }
        }
    }

    impl<S: Subscriber> Layer<S> for Recorder {
        fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
            // `Level` orders by verbosity: ERROR is the least, so `<= WARN` is WARN and ERROR.
            let meta = event.metadata();
            if *meta.level() <= Level::WARN && meta.target().starts_with("bevy_speech_bubbles") {
                let mut message = Message(String::new());
                event.record(&mut message);
                if let Ok(mut log) = self.0.lock() {
                    log.push(message.0);
                }
            }
        }
    }

    /// Run `f` on this thread with a recorder installed, and return every WARN/ERROR the crate logged.
    /// The runtime's schedules run single-threaded so their systems log on the test thread.
    pub fn record(app: &mut App, f: impl FnOnce(&mut App)) -> Vec<String> {
        let single = |schedule: &mut Schedule| {
            schedule.set_executor(SingleThreadedExecutor::new());
        };
        app.edit_schedule(PreUpdate, single);
        app.edit_schedule(Update, single);
        let recorder = Recorder::default();
        let subscriber = Registry::default().with(recorder.clone());
        tracing::subscriber::with_default(subscriber, || f(app));
        recorder.0.lock().map(|log| log.clone()).unwrap_or_default()
    }
}

/// A bark world: the walk app plus a font, a registered villager with a head, and the narrator `app()`
/// registers without one.
fn bark_app() -> App {
    let mut app = app();
    let font = ab_glyph::FontArc::try_from_slice(include_bytes!("data/FiraMono-subset.ttf"))
        .expect("the test font parses");
    app.insert_resource(BubbleAssets {
        quad: Handle::default(),
        font,
    });
    let villager = app.world_mut().spawn((Transform::default(), Visibility::default())).id();
    app.world_mut().resource_mut::<Speakers>().0.insert(
        "villager".into(),
        Speaker {
            entity: Some(villager),
            name: "Villager".into(),
            portrait: None,
        },
    );
    app
}

fn bark(speaker: &str) -> Bark {
    use bevy_speech_bubbles::{BubbleKind, Emotion};
    Bark {
        speaker: speaker.into(),
        kind: BubbleKind::Speech,
        emotion: Emotion::Neutral,
        text: "Stuck!".into(),
    }
}

fn balloons(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query::<&bevy_speech_bubbles::Bubble>().iter(world).count()
}

/// A bark for a key that is not in `Speakers` is a host mistake (a misspelt key in a bark file): it is
/// dropped, no balloon appears, and the log says which key and which bark.
#[test]
fn a_bark_for_an_unknown_speaker_warns_naming_the_key() {
    let mut app = bark_app();
    app.world_mut().write_message(bark("nobody"));
    let log = warned::record(&mut app, |app| {
        app.update();
        app.update();
    });
    assert_eq!(log.len(), 1, "one warning for the one dropped bark: {log:?}");
    assert!(log[0].contains("nobody"), "the warning names the key: {log:?}");
    assert!(log[0].contains("Stuck!"), "the warning names the bark: {log:?}");
    assert_eq!(balloons(&mut app), 0, "nothing is shown for an unknown speaker");
}

/// A registered speaker with no entity is a narrator: it has no head to float over, and the drop is
/// documented and quiet.
#[test]
fn a_bark_for_a_registered_narrator_stays_quiet() {
    let mut app = bark_app();
    app.world_mut().write_message(bark("narrator"));
    let log = warned::record(&mut app, |app| {
        app.update();
        app.update();
    });
    assert!(log.is_empty(), "a narrator's bark is dropped without a warning: {log:?}");
    assert_eq!(balloons(&mut app), 0);
}

/// The warning is once per key, not once per call site: the same misspelt key does not repeat itself,
/// and a second misspelt key still speaks up.
#[test]
fn an_unknown_bark_speaker_warns_once_per_key() {
    let mut app = bark_app();
    for key in ["nobody", "nobody", "somebody", "nobody"] {
        app.world_mut().write_message(bark(key));
    }
    let log = warned::record(&mut app, |app| {
        for _ in 0..6 {
            app.update();
        }
    });
    assert_eq!(log.len(), 2, "one warning per distinct key: {log:?}");
    assert!(log.iter().any(|l| l.contains("'nobody'")), "{log:?}");
    assert!(log.iter().any(|l| l.contains("'somebody'")), "{log:?}");
}

/// A key that was warned about and is registered later shows its bark: the warning set only decides what
/// to say, never what to drop.
#[test]
fn a_bark_speaker_registered_after_its_warning_gets_its_balloon() {
    let mut app = bark_app();
    app.world_mut().write_message(bark("late"));
    let log = warned::record(&mut app, |app| {
        app.update();
        app.update();
    });
    assert_eq!(log.len(), 1, "{log:?}");
    let head = app.world_mut().spawn((Transform::default(), Visibility::default())).id();
    app.world_mut().resource_mut::<Speakers>().0.insert(
        "late".into(),
        Speaker {
            entity: Some(head),
            name: String::new(),
            portrait: None,
        },
    );
    app.world_mut().write_message(bark("late"));
    let log = warned::record(&mut app, |app| {
        app.update();
        app.update();
    });
    assert!(log.is_empty(), "{log:?}");
    assert_eq!(balloons(&mut app), 1);
}

const GHOST: &str = r#"
(
  conversations: {
    "t": (
      start: "a",
      nodes: {
        "a": Line(speaker: "ghost", text: "Boo.", next: None),
      },
    ),
  },
)
"#;

const NARRATED: &str = r#"
(
  conversations: {
    "t": (
      start: "a",
      nodes: {
        "a": Line(speaker: "narrator", text: "Dusk.", next: None),
      },
    ),
  },
)
"#;

/// Walk a one-line conversation in balloon mode until it ends, and return what the log said.
fn balloon_walk(src: &str) -> (Vec<String>, Vec<String>) {
    let mut app = app_with(src);
    app.insert_resource(Presenter::Balloons);
    let font = ab_glyph::FontArc::try_from_slice(include_bytes!("data/FiraMono-subset.ttf"))
        .expect("the test font parses");
    app.insert_resource(BubbleAssets {
        quad: Handle::default(),
        font,
    });
    let mut ends = Vec::new();
    let log = warned::record(&mut app, |app| {
        start(app);
        for _ in 0..10 {
            ends.extend(ended(app));
            app.update();
        }
        ends.extend(ended(app));
    });
    assert!(
        app.world().get_resource::<ConversationLock>().is_none(),
        "the lock is released"
    );
    (log, ends)
}

/// A balloon-mode line by a key that is not in `Speakers` is skipped with a warning naming the key and
/// the node; the conversation still ends once.
#[test]
fn a_balloon_line_by_an_unknown_speaker_warns_and_ends_once() {
    let (log, ends) = balloon_walk(GHOST);
    assert_eq!(log.len(), 1, "{log:?}");
    assert!(log[0].contains("'ghost'"), "the warning names the key: {log:?}");
    assert!(log[0].contains("'a'"), "the warning names the node: {log:?}");
    assert_eq!(ends, vec!["t".to_string()]);
}

/// The registered narrator's balloon-mode line is skipped as it always was, with no warning.
#[test]
fn a_balloon_line_by_a_registered_narrator_stays_quiet() {
    let (log, ends) = balloon_walk(NARRATED);
    assert!(log.is_empty(), "{log:?}");
    assert_eq!(ends, vec!["t".to_string()]);
}

/// What a host system ordered after the dialog saw of Space.
#[derive(Resource, Default)]
struct HostSaw {
    space: Vec<bool>,
}

fn host_system(keys: Res<ButtonInput<KeyCode>>, mut saw: ResMut<HostSaw>) {
    saw.space.push(keys.just_pressed(KeyCode::Space));
}

/// A press the dialog uses is gone for a `PreUpdate` system ordered `.after(DialogInputSet)`, and with
/// no conversation running nothing is cleared.
#[test]
fn a_consumed_press_is_hidden_from_later_preupdate_systems() {
    let mut app = app();
    app.init_resource::<HostSaw>()
        .add_systems(PreUpdate, host_system.after(DialogInputSet));

    // No conversation: Space must stay just_pressed for the host.
    tap(&mut app, KeyCode::Space);
    assert_eq!(app.world().resource::<HostSaw>().space, vec![true], "the dialog ate a key with no conversation");

    // A conversation on a line: Space advances it, so the host must not see it.
    start(&mut app);
    app.world_mut().resource_mut::<HostSaw>().space.clear();
    tap(&mut app, KeyCode::Space);
    assert!(shows(&mut app, "Open it?"), "Space did not advance the line");
    assert_eq!(app.world().resource::<HostSaw>().space, vec![false], "the host saw the Space the dialog used");

}

/// The first Space while a line is still typing completes it and is consumed too.
#[test]
fn a_press_that_completes_typing_is_consumed() {
    let mut app = app();
    app.init_resource::<HostSaw>()
        .add_systems(PreUpdate, host_system.after(DialogInputSet));
    app.world_mut().resource_mut::<DialogSkin>().chars_per_second = 1.0;
    start(&mut app);
    app.world_mut().resource_mut::<HostSaw>().space.clear();
    tap(&mut app, KeyCode::Space);
    assert_eq!(app.world().resource::<HostSaw>().space, vec![false]);
}

/// Digits are consumed only when they pick an option that exists.
#[test]
fn digits_are_consumed_only_when_they_pick() {
    let mut app = app();
    start(&mut app);
    tap(&mut app, KeyCode::Space); // line -> choice with two options
    let press = |app: &mut App, key: KeyCode| {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    };
    press(&mut app, KeyCode::Digit7);
    app.update();
    assert!(
        app.world().resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::Digit7),
        "an out-of-range digit was consumed"
    );
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::Digit7);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
    press(&mut app, KeyCode::Digit1);
    app.update();
    assert!(
        !app.world().resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::Digit1),
        "a digit that picked was not consumed"
    );
}

/// `ImageNode` draws in the padding box by default, which left a padded name tag's art around its text
/// only (a wax seal landed on the first letter). Every image the box spawns uses the border box, and
/// the tag has extra room on the left.
#[test]
fn the_name_tag_art_covers_its_border_box() {
    use bevy::ui::VisualBox;

    let mut app = app();
    start(&mut app);
    let world = app.world_mut();
    let images: Vec<(VisualBox, Val)> = world
        .query::<(&ImageNode, &Node)>()
        .iter(world)
        .map(|(i, n)| (i.visual_box, n.padding.left))
        .collect();
    assert!(!images.is_empty(), "the box spawned no images");
    assert!(
        images.iter().all(|(b, _)| *b == VisualBox::BorderBox),
        "an image of the box is not drawn in the border box: {images:?}"
    );
    // The skin's padding is 8, so the tag's horizontal padding is 6 and it is the only padded image.
    assert_eq!(
        images.iter().filter(|(_, left)| *left == Val::Px(6.0)).count(),
        1,
        "the name tag (padding 0.75 x skin) was not found: {images:?}"
    );
}
