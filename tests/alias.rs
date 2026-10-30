//! **Speaker aliases, headless.** Two speaker keys that are one person — a game's `person:7` and a
//! simulation's `society:7` — must read as one person: one balloon over one head at a time, the second
//! line a gap after the first, and the target's name on the dialog box. Issue #5.
//!
//! Time is stepped 16 ms per update, so the bark gap (1 s) is a frame count, not a wall-clock race.

use std::collections::HashSet;
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_speech_bubbles::{
    Bark, BarkSettings, Bubble, BubbleAssets, BubbleKind, DialogSkin, DialogueScript, Emotion,
    Presenter, Speaker, Speakers, SpeechBubblesPlugin, StartConversation,
};

#[derive(Component)]
struct TestCamera;

const STEP: Duration = Duration::from_millis(16);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_plugins(SpeechBubblesPlugin::<TestCamera>::default())
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
    let font = ab_glyph::FontArc::try_from_slice(include_bytes!("data/FiraMono-subset.ttf"))
        .expect("the test font parses");
    app.insert_resource(BubbleAssets {
        quad: Handle::default(),
        font,
    });
    app.world_mut()
        .spawn((TestCamera, Transform::default(), GlobalTransform::default()));
    app
}

/// An entity a balloon can float over: `track_bubbles` despawns a balloon whose owner lacks
/// `Transform` or `Visibility`.
fn head(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((Transform::default(), Visibility::default()))
        .id()
}

fn register(app: &mut App, key: &str, entity: Entity, name: &str) {
    app.world_mut().resource_mut::<Speakers>().0.insert(
        key.into(),
        Speaker {
            entity: Some(entity),
            name: name.into(),
            portrait: None,
        },
    );
}

fn bark(app: &mut App, speaker: &str, text: &str) {
    app.world_mut().write_message(Bark {
        speaker: speaker.into(),
        kind: BubbleKind::Speech,
        emotion: Emotion::Neutral,
        text: text.into(),
    });
}

/// What the player saw over `frames` updates: for every balloon, its owner and the frame it appeared
/// on; and the most balloons alive over one entity on any frame.
struct Seen {
    appeared: Vec<(Entity, usize)>,
    most_over_one_head: usize,
}

fn watch(app: &mut App, frames: usize) -> Seen {
    let mut known = HashSet::new();
    let mut appeared = Vec::new();
    let mut most_over_one_head = 0;
    for frame in 0..frames {
        app.update();
        let world = app.world_mut();
        let live: Vec<(Entity, Entity)> = world
            .query::<(Entity, &Bubble)>()
            .iter(world)
            .map(|(e, b)| (e, b.owner))
            .collect();
        for (balloon, owner) in &live {
            if known.insert(*balloon) {
                appeared.push((*owner, frame));
            }
            let over = live.iter().filter(|(_, o)| o == owner).count();
            most_over_one_head = most_over_one_head.max(over);
        }
    }
    Seen {
        appeared,
        most_over_one_head,
    }
}

/// Frames that span `secs` of the stepped clock, rounded up.
fn frames_for(secs: f32) -> usize {
    (secs / STEP.as_secs_f32()).ceil() as usize
}

/// Issue #5, acceptance 1 and 2: barks for an alias and its target on one frame are one person's —
/// both float over the target's entity, one at a time, the second no sooner than `gap` after the first.
#[test]
fn an_alias_and_its_target_bark_as_one_person() {
    let mut app = app();
    let villager = head(&mut app);
    register(&mut app, "person:7", villager, "Mara");
    app.world_mut()
        .resource_mut::<Speakers>()
        .alias("society:7", "person:7");

    bark(&mut app, "society:7", "The well is dry again.");
    bark(&mut app, "person:7", "Mind the step.");
    let seen = watch(&mut app, frames_for(3.0));

    assert_eq!(seen.appeared.len(), 2, "both barks should be shown: {:?}", seen.appeared);
    assert!(
        seen.appeared.iter().all(|(owner, _)| *owner == villager),
        "an aliased bark floated over another head: {:?}",
        seen.appeared
    );
    assert_eq!(seen.most_over_one_head, 1, "two balloons were up over one head at once");
    let gap = app.world().resource::<BarkSettings>().gap;
    let between = (seen.appeared[1].1 - seen.appeared[0].1) as f32 * STEP.as_secs_f32();
    assert!(between >= gap, "the second bark came {between} s after the first, under the {gap} s gap");
}

/// A key that is aliased and also registered on its own (a simulation registers `society:<id>` for
/// itself) speaks as its target: the alias is the host's statement that the two are one person.
#[test]
fn an_alias_wins_over_the_key_s_own_registration() {
    let mut app = app();
    let figure = head(&mut app);
    let sim_body = head(&mut app);
    register(&mut app, "person:7", figure, "Mara");
    register(&mut app, "society:7", sim_body, "someone");
    app.world_mut()
        .resource_mut::<Speakers>()
        .alias("society:7", "person:7");

    bark(&mut app, "society:7", "The well is dry again.");
    let seen = watch(&mut app, 2);
    assert_eq!(seen.appeared.iter().map(|(o, _)| *o).collect::<Vec<_>>(), vec![figure]);
}

/// Issue #5, acceptance 3: an alias whose target is not registered (never, or no longer) drops the
/// bark like any unknown speaker's, and starts no gap, so the next bark is not held back.
#[test]
fn an_alias_to_an_absent_speaker_drops_the_bark_and_holds_nothing_up() {
    let mut app = app();
    let villager = head(&mut app);
    let other = head(&mut app);
    register(&mut app, "person:7", villager, "Mara");
    register(&mut app, "person:8", other, "Tom");
    app.world_mut()
        .resource_mut::<Speakers>()
        .alias("society:7", "person:7");
    // The villager despawns and the host removes its key.
    app.world_mut().resource_mut::<Speakers>().0.remove("person:7");

    bark(&mut app, "society:7", "The well is dry again.");
    bark(&mut app, "person:8", "Evening.");
    // One bark is released per update: the orphaned alias's is dropped, then the other goes up.
    let seen = watch(&mut app, 2);
    assert_eq!(seen.appeared, vec![(other, 1)], "the dropped bark held up or replaced the next one");
}

/// Issue #5, acceptance 4: without an alias, two keys on one entity are still one head — main has a
/// single bark queue and a new bark replaces the one over its entity, so they never stack.
#[test]
fn two_keys_on_one_entity_show_one_balloon_at_a_time() {
    let mut app = app();
    let villager = head(&mut app);
    register(&mut app, "person:7", villager, "Mara");
    register(&mut app, "society:7", villager, "Mara");

    bark(&mut app, "society:7", "The well is dry again.");
    bark(&mut app, "person:7", "Mind the step.");
    let seen = watch(&mut app, frames_for(3.0));
    assert_eq!(seen.appeared.len(), 2);
    assert_eq!(seen.most_over_one_head, 1, "two keys on one entity stacked their balloons");
}

/// A bark resolves its alias when it is queued: re-pointing the alias while the bark waits for the
/// gap does not move it to another head.
#[test]
fn re_pointing_an_alias_does_not_move_a_waiting_bark() {
    let mut app = app();
    let villager = head(&mut app);
    let stranger = head(&mut app);
    register(&mut app, "person:7", villager, "Mara");
    register(&mut app, "person:9", stranger, "Ilse");
    app.world_mut()
        .resource_mut::<Speakers>()
        .alias("society:7", "person:7");

    bark(&mut app, "person:9", "Who goes there?");
    app.update(); // shown; the gap is now running
    bark(&mut app, "society:7", "Only me.");
    app.update(); // queued behind the gap
    app.world_mut()
        .resource_mut::<Speakers>()
        .alias("society:7", "person:9");

    let seen = watch(&mut app, frames_for(1.5));
    let owners: Vec<Entity> = seen.appeared.iter().map(|(o, _)| *o).collect();
    // The stranger's bark is still up when the watch starts; the waiting one follows the gap. Had it
    // followed the alias it would have replaced the stranger's balloon instead.
    assert_eq!(owners, vec![stranger, villager], "the waiting bark followed the re-pointed alias");
}

const ONE_LINE: &str = r#"
(
  conversations: {
    "well": (
      start: "a",
      nodes: {
        "a": Line(speaker: "society:7", text: "The well is dry again.", next: None),
      },
    ),
  },
)
"#;

fn dialog_skin() -> DialogSkin {
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

/// A conversation line by an alias speaks as its target too: the dialog box shows the target's name,
/// and in balloon mode the line floats over the target's head.
#[test]
fn a_conversation_line_by_an_alias_speaks_as_its_target() {
    for presenter in [Presenter::DialogBox, Presenter::Balloons] {
        let mut app = app();
        let figure = head(&mut app);
        let sim_body = head(&mut app);
        register(&mut app, "person:7", figure, "Mara");
        register(&mut app, "society:7", sim_body, "someone");
        app.world_mut()
            .resource_mut::<Speakers>()
            .alias("society:7", "person:7");
        let script: DialogueScript = ron::from_str(ONE_LINE).expect("script parses");
        app.insert_resource(script)
            .insert_resource(presenter)
            .insert_resource(dialog_skin());
        app.world_mut()
            .write_message(StartConversation { id: "well".into() });
        app.update();

        let world = app.world_mut();
        match presenter {
            Presenter::DialogBox => {
                let texts: Vec<String> =
                    world.query::<&Text>().iter(world).map(|t| t.0.clone()).collect();
                assert!(texts.iter().any(|t| t == "Mara"), "no 'Mara' name tag: {texts:?}");
                assert!(!texts.iter().any(|t| t == "someone"), "the alias's own name showed");
            }
            Presenter::Balloons => {
                let owners: Vec<Entity> =
                    world.query::<&Bubble>().iter(world).map(|b| b.owner).collect();
                assert_eq!(owners, vec![figure], "the line floated over the wrong head");
            }
        }
    }
}
