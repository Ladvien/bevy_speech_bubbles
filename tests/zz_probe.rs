//! Throwaway probe (iteration 1): do two keys on one entity ever stack bark balloons on main?
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_speech_bubbles::{
    Bark, BubbleAssets, BubbleKind, Bubble, DialogueScript, Emotion, Presenter, Speaker, Speakers,
    SpeechBubblesPlugin, StartConversation,
};

#[derive(Component)]
struct Cam;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_plugins(SpeechBubblesPlugin::<Cam>::default())
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(16)));
    let font = ab_glyph::FontArc::try_from_slice(include_bytes!("data/FiraMono-subset.ttf")).unwrap();
    app.insert_resource(BubbleAssets { quad: Handle::default(), font });
    app.world_mut().spawn((Cam, Transform::default(), GlobalTransform::default()));
    app
}

#[test]
fn two_keys_one_entity() {
    let mut app = app();
    let e = app.world_mut().spawn((Transform::default(), GlobalTransform::default())).id();
    for k in ["person:7", "society:7"] {
        app.world_mut().resource_mut::<Speakers>().0.insert(
            k.into(),
            Speaker { entity: Some(e), name: k.into(), portrait: None },
        );
    }
    for k in ["society:7", "person:7"] {
        app.world_mut().write_message(Bark {
            speaker: k.into(),
            kind: BubbleKind::Speech,
            emotion: Emotion::Neutral,
            text: format!("line from {k}"),
        });
    }
    let mut max_live = 0;
    let mut appear = Vec::new();
    let mut prev = 0;
    for f in 0..400 {
        app.update();
        let w = app.world_mut();
        let n = w.query::<&Bubble>().iter(w).count();
        let spawned = w.query_filtered::<Entity, Added<Bubble>>().iter(w).count();
        if spawned > 0 { appear.push(f); }
        max_live = max_live.max(n);
        prev = n;
    }
    let _ = prev;
    println!("PROBE two_keys_one_entity: max_live={max_live} appear_frames={appear:?}");
}

#[test]
fn bark_then_conversation_same_head() {
    let mut app = app();
    let e = app.world_mut().spawn((Transform::default(), GlobalTransform::default())).id();
    app.world_mut().resource_mut::<Speakers>().0.insert(
        "v".into(),
        Speaker { entity: Some(e), name: "V".into(), portrait: None },
    );
    let script: DialogueScript = ron::from_str(r#"(conversations: {"t": (start: "a", nodes: {"a": Line(speaker: "v", text: "Hello there.", next: None)})})"#).unwrap();
    app.insert_resource(script).insert_resource(Presenter::Balloons);
    app.world_mut().write_message(Bark { speaker: "v".into(), kind: BubbleKind::Speech, emotion: Emotion::Neutral, text: "A bark.".into() });
    app.update();
    app.world_mut().write_message(StartConversation { id: "t".into() });
    app.update();
    app.update();
    let w = app.world_mut();
    let owners: Vec<Entity> = w.query::<&Bubble>().iter(w).map(|b| b.owner).collect();
    println!("PROBE bark_then_conversation: balloons over the same head = {}", owners.iter().filter(|o| **o == e).count());
}
