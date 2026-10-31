//! Throwaway dialog-checklist probe for loop iteration 2. Copied into tests/ to run, never committed.
//! `cargo test --test zz_probe -- --nocapture --test-threads=1`

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_speech_bubbles::{
    BalloonSkin, Bark, Bubble, BubbleAssets, BubbleKind, BubblePx, BubbleStyle, ConversationEnded,
    ConversationLock, DialogInputSet, DialogSkin, DialogueScript, Emotion, FontArc, Presenter,
    Speaker, Speakers, SpeechBubblesPlugin, StartConversation, build_bubble, track_bubbles,
    validate_script,
};

#[derive(Component)]
struct Cam;

fn font() -> FontArc {
    FontArc::try_from_vec(std::fs::read("tests/data/FiraMono-subset.ttf").unwrap()).unwrap()
}

fn raster(skin: BalloonSkin, kind: BubbleKind, tail: bool, text: &str) -> (Vec<[u8; 4]>, u32, u32) {
    let assets = BubbleAssets { quad: Handle::default(), font: font() };
    let mut images = Assets::<Image>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let style = BubbleStyle { kind, emotion: Emotion::Neutral, tail, skin };
    let r = build_bubble(&assets, &mut images, &mut materials, &style, text);
    let m = materials.get(&r.material).unwrap();
    let img = images.get(m.base_color_texture.as_ref().unwrap()).unwrap();
    let data = img.data.clone().unwrap();
    (data.as_chunks::<4>().0.to_vec(), r.px.x, r.px.y)
}

fn bbox(px: &[[u8; 4]], w: u32, f: impl Fn(&[u8; 4]) -> bool) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    let mut any = false;
    for (i, p) in px.iter().enumerate() {
        if f(p) {
            let (x, y) = (i as u32 % w, i as u32 / w);
            x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x); y1 = y1.max(y);
            any = true;
        }
    }
    any.then_some((x0, y0, x1, y1))
}

/// Last row whose opaque run is at least 90 % of the widest: the cap's bottom, above the tail.
fn cap_rows(px: &[[u8; 4]], w: u32, h: u32) -> (u32, u32, u32) {
    let widths: Vec<u32> = (0..h).map(|y| (0..w).filter(|&x| px[(y * w + x) as usize][3] > 0).count() as u32).collect();
    let max = *widths.iter().max().unwrap();
    let top = widths.iter().position(|&c| c > 0).unwrap() as u32;
    let bottom = widths.iter().rposition(|&c| c * 10 >= max * 9).unwrap() as u32;
    (top, bottom, max)
}

#[test]
fn p1_pixel_text_and_cap() {
    for (name, skin, ink) in [
        ("ink_on_paper", BalloonSkin::ink_on_paper(), Box::new(|p: &[u8; 4]| p[3] == 255 && p[..3] == [0x2a, 0x1c, 0x12]) as Box<dyn Fn(&[u8; 4]) -> bool>),
        ("crt", BalloonSkin::crt(), Box::new(|p: &[u8; 4]| p[3] > 0 && p[0] > 150)),
    ] {
        for kind in [BubbleKind::Speech, BubbleKind::Thought] {
            let (px, w, h) = raster(skin, kind, true, "Hold the gate until dawn, then run.");
            let between = px.iter().filter(|p| p[3] != 0 && p[3] != 255).count();
            let mut colours: Vec<[u8; 4]> = px.iter().filter(|p| p[3] == 255).copied().collect();
            colours.sort(); colours.dedup();
            let body = bbox(&px, w, |p| p[3] > 0).unwrap();
            let (top, bottom, _) = cap_rows(&px, w, h);
            let text = bbox(&px, w, &*ink).unwrap();
            println!(
                "pixel/cap {name} {kind:?}: {w}x{h} between_alpha={between} opaque_colours={} pad L={} R={} T={} B={} (cap rows {top}..={bottom})",
                colours.len(), text.0 - body.0, body.2 - text.2, text.1 - top, bottom as i64 - text.3 as i64
            );
        }
    }
}

#[test]
fn p2_wrapping() {
    let long: String = "The lamps are out on the west road and the gate is shut until dawn, so we wait here. ".chars().cycle().take(240).collect();
    let word: String = "W".repeat(60);
    for (name, skin) in [("crt", BalloonSkin::crt()), ("ink_on_paper", BalloonSkin::ink_on_paper())] {
        let (_, w240, h240) = raster(skin, BubbleKind::Speech, true, &long);
        let (px, w60, h60) = raster(skin, BubbleKind::Speech, true, &word);
        let ink = if skin.pixel { bbox(&px, w60, |p| p[3] == 255 && p[..3] == [0x2a, 0x1c, 0x12]) } else { bbox(&px, w60, |p| p[3] > 0 && p[0] > 150) };
        let body = bbox(&px, w60, |p| p[3] > 0).unwrap();
        println!("wrap {name}: 240 chars -> {w240}x{h240}; 60-char word -> {w60}x{h60}, ink x {:?} inside body x {}..={}", ink.map(|b| (b.0, b.2)), body.0, body.2);
    }
}

#[test]
fn p3_tail_column() {
    for (name, skin) in [("crt", BalloonSkin::crt()), ("ink_on_paper", BalloonSkin::ink_on_paper())] {
        for kind in [BubbleKind::Speech, BubbleKind::Thought] {
            let (px, w, h) = raster(skin, kind, true, "Over here.");
            let last = (0..h).rev().find(|&y| (0..w).any(|x| px[(y * w + x) as usize][3] > 0)).unwrap();
            let xs: Vec<u32> = (0..w).filter(|&x| px[(last * w + x) as usize][3] > 0).collect();
            let mid = (xs[0] + xs[xs.len() - 1]) as f32 / 2.0;
            println!("tail {name} {kind:?}: {w}x{h}, lowest ink row {last}, tip x {mid:.1} vs centre {:.1}", w as f32 / 2.0);
        }
    }
}

fn track_app() -> App {
    let mut app = App::new();
    app.add_plugins(TransformPlugin).add_systems(Update, track_bubbles::<Cam>);
    app.world_mut().spawn((Cam, GlobalTransform::from(Transform::from_xyz(10.0, 1.0, 1000.0))));
    app
}

fn balloon_on(app: &mut App, owner: Entity) -> Entity {
    app.world_mut()
        .spawn((Bubble { owner, offset: Vec2::ZERO }, BubblePx(UVec2::new(100, 40)), Transform::default(), Visibility::default()))
        .id()
}

#[test]
fn p4_anchor() {
    // Issue #10's input.
    let mut app = track_app();
    let parent = app.world_mut().spawn((Transform::from_xyz(10.0, 0.0, 0.0), Visibility::default())).id();
    let owner = app.world_mut().spawn((Transform::default(), Visibility::default(), ChildOf(parent))).id();
    let b = balloon_on(&mut app, owner);
    app.update();
    app.update();
    let tf = *app.world().get::<Transform>(b).unwrap();
    let gt = app.world().get::<GlobalTransform>(owner).unwrap().translation();
    println!("anchor child: owner world {gt:?}, balloon {:?}, x error {:.3}", tf.translation, (tf.translation.x - gt.x).abs());
    // Same-frame: move the parent, one update.
    app.world_mut().get_mut::<Transform>(parent).unwrap().translation.x = 20.0;
    app.update();
    let tf = *app.world().get::<Transform>(b).unwrap();
    println!("anchor same-frame: parent moved to x=20, balloon x after one update {:.3}", tf.translation.x);
    // Hidden parent.
    *app.world_mut().get_mut::<Visibility>(parent).unwrap() = Visibility::Hidden;
    app.update();
    println!("anchor hidden parent: balloon Visibility {:?}", app.world().get::<Visibility>(b).unwrap());
    // Root owner: unchanged path.
    let mut app = track_app();
    let owner = app.world_mut().spawn((Transform::from_xyz(3.0, 1.0, 0.0), Visibility::default())).id();
    let b = balloon_on(&mut app, owner);
    app.update();
    println!("anchor root: balloon {:?} (owner x 3, y 1 + 2.5 + 0.2)", app.world().get::<Transform>(b).unwrap().translation);
}

const SCRIPT: &str = r#"
(
  conversations: {
    "line": ( start: "a", nodes: { "a": Line(speaker: "s0", text: "Evening.", next: None) } ),
    "choice": ( start: "q", nodes: {
        "q": Choice(speaker: "s0", prompt: "Open?", options: [(text: "No", next: "end"), (text: "Yes", next: "end")]),
        "end": Line(speaker: "s0", text: "Done.", next: None),
    } ),
    "ghost": ( start: "a", nodes: { "a": Line(speaker: "ghost", text: "Boo.", next: None) } ),
  },
)
"#;

fn skin() -> DialogSkin {
    let slicer = bevy::sprite::TextureSlicer { border: BorderRect::all(4.0), ..default() };
    DialogSkin {
        frame: Handle::default(), frame_slicer: slicer.clone(), paper: Handle::default(), name_tag: Handle::default(),
        name_tag_slicer: slicer, caret: [Handle::default(), Handle::default()], body_font: TextFont::default(),
        title_font: TextFont::default(), text_color: Color::BLACK, choice_color: Color::BLACK,
        choice_hover_color: Color::WHITE, padding: 8.0, width: 320.0, portrait_size: 48.0,
        chars_per_second: 0.0, bottom_margin: 16.0,
    }
}

#[derive(Resource, Default)]
struct HostSaw(u32);

fn conv_app(presenter: Presenter, with_skin: bool, with_assets: bool) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>().init_asset::<Mesh>().init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>().init_resource::<ButtonInput<MouseButton>>()
        .add_plugins(SpeechBubblesPlugin::<Cam>::default())
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)))
        .init_resource::<HostSaw>()
        .add_systems(PreUpdate, (|k: Res<ButtonInput<KeyCode>>, mut s: ResMut<HostSaw>| if k.just_pressed(KeyCode::Space) { s.0 += 1 }).after(DialogInputSet));
    let script: DialogueScript = ron::from_str(SCRIPT).unwrap();
    validate_script(&script).unwrap();
    app.insert_resource(script).insert_resource(presenter);
    if with_skin { app.insert_resource(skin()); }
    if with_assets { app.insert_resource(BubbleAssets { quad: Handle::default(), font: font() }); }
    app.world_mut().spawn((Cam, Transform::from_xyz(0.0, 3.0, 8.0), GlobalTransform::default()));
    let e = app.world_mut().spawn((Transform::default(), GlobalTransform::default(), Visibility::default())).id();
    app.world_mut().resource_mut::<Speakers>().0.insert("s0".into(), Speaker { entity: Some(e), name: "S".into(), portrait: None });
    (app, e)
}

fn tap(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    app.update();
    let mut k = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    k.release(key);
    k.clear();
}

fn run_exit(name: &str, presenter: Presenter, with_skin: bool, with_assets: bool, id: &str, act: impl Fn(&mut App, Entity, u32)) {
    let (mut app, e) = conv_app(presenter, with_skin, with_assets);
    app.world_mut().write_message(StartConversation { id: id.into() });
    let mut ended = 0;
    for f in 0..120 {
        act(&mut app, e, f);
        app.update();
        ended += app.world_mut().resource_mut::<Messages<ConversationEnded>>().drain().count();
    }
    let lock = app.world().contains_resource::<ConversationLock>();
    println!("exit {name}: ConversationEnded x{ended}, lock held {lock}");
}

#[test]
fn p5_exits() {
    run_exit("last line (balloons)", Presenter::Balloons, false, true, "line", |_, _, _| {});
    run_exit("last line (box, Space)", Presenter::DialogBox, true, true, "line", |a, _, f| if f % 5 == 1 { tap(a, KeyCode::Space) });
    run_exit("choice into end (box)", Presenter::DialogBox, true, true, "choice", |a, _, f| if f == 2 { tap(a, KeyCode::Digit1) } else if f % 5 == 1 { tap(a, KeyCode::Space) });
    run_exit("missing DialogSkin", Presenter::DialogBox, false, true, "line", |_, _, _| {});
    run_exit("missing BubbleAssets", Presenter::Balloons, false, false, "line", |_, _, _| {});
    run_exit("unknown speaker (balloons)", Presenter::Balloons, false, true, "ghost", |_, _, _| {});
    run_exit("speaker despawned mid-line", Presenter::Balloons, false, true, "line", |a, e, f| if f == 3 { a.world_mut().despawn(e); });
}

#[test]
fn p6_presses() {
    let (mut app, _) = conv_app(Presenter::DialogBox, true, true);
    tap(&mut app, KeyCode::Space);
    let idle = app.world().resource::<HostSaw>().0;
    app.world_mut().write_message(StartConversation { id: "line".into() });
    app.update();
    let mut presses = 0;
    while app.world().contains_resource::<ConversationLock>() && presses < 10 {
        tap(&mut app, KeyCode::Space);
        presses += 1;
    }
    let during = app.world().resource::<HostSaw>().0 - idle;
    println!("presses: idle Space seen by host {idle}/1; one-line box closed in {presses} presses, host saw {during} of them");
}

#[test]
fn p7_refusals() {
    for (name, src) in [
        ("dangling next", r#"(conversations: {"x": (start: "a", nodes: {"a": Line(speaker: "s", text: "t", next: Some("zz"))})})"#),
        ("empty conversation", r#"(conversations: {"x": (start: "a", nodes: {})})"#),
        ("choice no options", r#"(conversations: {"x": (start: "a", nodes: {"a": Choice(speaker: "s", prompt: "?", options: [])})})"#),
    ] {
        let s: DialogueScript = ron::from_str(src).unwrap();
        println!("refuse {name}: {:?}", validate_script(&s));
    }
}

#[test]
fn p8_paused_and_bark() {
    let (mut app, _) = conv_app(Presenter::Balloons, false, true);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.world_mut().write_message(Bark { speaker: "s0".into(), kind: BubbleKind::Speech, emotion: Emotion::Neutral, text: "Hi.".into() });
    app.update();
    let world = app.world_mut();
    let up = world.query::<&Bubble>().iter(world).count();
    for _ in 0..30 { app.update(); }
    let world = app.world_mut();
    let after = world.query::<&Bubble>().iter(world).count();
    println!("paused: bark shown {up}, after 3.0 s real with virtual paused {after}");
    app.world_mut().write_message(Bark { speaker: "nobody".into(), kind: BubbleKind::Speech, emotion: Emotion::Neutral, text: "?".into() });
    app.update();
    let world = app.world_mut();
    println!("bark unknown speaker: bubbles {}", world.query::<&Bubble>().iter(world).count());
}
