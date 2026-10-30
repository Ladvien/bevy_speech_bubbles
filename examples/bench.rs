//! **What the kit costs its caller, measured.**
//!
//! Prints one line per case, `bubbles: <case> key=value …`, for the loop's before/after comparison:
//!
//! - `raster-speech` / `raster-thought`: [`build_bubble`] (the CPU rasterize plus the `Image` and the
//!   material it adds) for 12, 80 and 240-character lines, the `crt()` and `ink_on_paper()` skins at
//!   `text_px` 16 and 32. Median and p99 over 200 builds after 20 warm-up builds, the raster size and
//!   the heap bytes one build allocates (a counting global allocator).
//! - `frame-live`: `app.update()` of a headless app with [`SpeechBubblesPlugin`] holding 0, 50 and 200
//!   live balloons that track and are checked for expiry every frame; median and p99 over 300 frames.
//!   `frame-live-child`: the same 200, each owner the child of a rotated, scaled parent.
//! - `bark-burst`: the worst single frame after 20 barks land on one frame, over the 300 frames after.
//! - `assets`: `Assets<Image>` and `Assets<StandardMaterial>` counts before the first of 1000 barks and
//!   after the last has expired.
//! - `presses`: the presses a headless walk spends to close each conversation of a fixed script under
//!   [`Presenter::Balloons`] and [`Presenter::DialogBox`].
//!
//! Time is stepped by a fixed 16 ms per update (`TimeUpdateStrategy::ManualDuration`), so the gaps,
//! dwell times and the typewriter see the same clock on every machine; only the timings vary.
//!
//! Run: `cargo run --release --example bench -- tests/data/FiraMono-subset.ttf`

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use ab_glyph::FontArc;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_speech_bubbles::{
    BalloonSkin, Bark, Bubble, BubbleAssets, BubbleKind, BubblePx, BubbleStyle, BubbleTtl,
    ConversationEnded, DEFAULT_CHARS_PER_SECOND, DialogSkin, DialogueNodeEntered, DialogueScript,
    Emotion, Node as ScriptNode, Presenter, Speaker, Speakers, SpeechBubblesPlugin,
    StartConversation, build_bubble, validate_script,
};

/// Counts every byte the process asks the allocator for, so one build's allocation is the counter's
/// difference across the call.
struct Counting;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards every call to `System` unchanged; the counter is a relaxed atomic add.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract, which `System` shares.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: as `alloc`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System` through this allocator with this `layout`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATED.fetch_add(new_size.saturating_sub(layout.size()), Ordering::Relaxed);
        // SAFETY: as `dealloc`; `new_size` is the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// The bench's camera marker.
#[derive(Component)]
struct BenchCamera;

const STEP: Duration = Duration::from_millis(16);
const WARMUP: usize = 20;
const BUILDS: usize = 200;
const FRAMES: usize = 300;
const SOURCE: &str = "The lamps are out on the west road and the gate is shut until dawn, so we wait here. ";

/// A line of exactly `n` characters cut from `SOURCE`, never ending on a space.
fn line(n: usize) -> String {
    let mut s: String = SOURCE.chars().cycle().take(n).collect();
    if s.ends_with(' ') {
        s.pop();
        s.push('.');
    }
    s
}

fn median_p99(samples: &mut [f64]) -> (f64, f64) {
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    (samples[n / 2], samples[(n * 99).div_ceil(100) - 1])
}

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run --release --example bench -- <path-to-font.ttf>");
        eprintln!();
        eprintln!("A font path is required rather than defaulted: the crate never reads a file. For numbers");
        eprintln!("that do not depend on the host's fonts, use tests/data/FiraMono-subset.ttf.");
        std::process::exit(2);
    };
    let font = match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| {
        FontArc::try_from_vec(b).map_err(|e| e.to_string())
    }) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("`{path}` is not a readable font: {e}");
            std::process::exit(1);
        }
    };

    raster(&font);
    frame_live(&font);
    bark_burst(&font);
    asset_lifetime(&font);
    presses(&font);
}

fn raster(font: &FontArc) {
    let assets = BubbleAssets {
        quad: Handle::default(),
        font: font.clone(),
    };
    let mut images = Assets::<Image>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    for (kind, kind_name) in [
        (BubbleKind::Speech, "raster-speech"),
        (BubbleKind::Thought, "raster-thought"),
    ] {
        for (skin, skin_name) in [
            (BalloonSkin::crt(), "crt"),
            (BalloonSkin::ink_on_paper(), "ink_on_paper"),
        ] {
            for text_px in [16.0, 32.0] {
                for chars in [12, 80, 240] {
                    let text = line(chars);
                    let style = BubbleStyle {
                        kind,
                        emotion: Emotion::Neutral,
                        tail: true,
                        skin: BalloonSkin { text_px, ..skin },
                    };
                    let mut times = Vec::with_capacity(BUILDS);
                    let mut bytes = Vec::with_capacity(BUILDS);
                    let mut px = UVec2::ZERO;
                    for i in 0..WARMUP + BUILDS {
                        let before = ALLOCATED.load(Ordering::Relaxed);
                        let t0 = Instant::now();
                        let rendered = build_bubble(&assets, &mut images, &mut materials, &style, &text);
                        let dt = t0.elapsed().as_secs_f64() * 1e6;
                        let allocated = ALLOCATED.load(Ordering::Relaxed) - before;
                        px = rendered.px;
                        if let Some(material) = materials.remove(&rendered.material)
                            && let Some(image) = material.base_color_texture
                        {
                            images.remove(&image);
                        }
                        if i >= WARMUP {
                            times.push(dt);
                            bytes.push(allocated as f64);
                        }
                    }
                    let (median, p99) = median_p99(&mut times);
                    let (bytes_median, _) = median_p99(&mut bytes);
                    println!(
                        "bubbles: {kind_name} chars={chars} skin={skin_name} text_px={text_px} \
                         median_us={median:.1} p99_us={p99:.1} px={}x{} bytes={bytes_median:.0}",
                        px.x, px.y
                    );
                }
            }
        }
    }
}

/// A headless app built the way `tests/walk.rs` builds one, on a fixed 16 ms step, with a camera.
fn app(font: &FontArc) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_plugins(SpeechBubblesPlugin::<BenchCamera>::default())
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
        .insert_resource(BubbleAssets {
            quad: Handle::default(),
            font: font.clone(),
        });
    app.world_mut().spawn((
        BenchCamera,
        Transform::from_xyz(0.0, 6.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
        GlobalTransform::default(),
    ));
    app
}

/// `n` speaker entities registered as `s0`..`s{n-1}`.
fn speakers(app: &mut App, n: usize) -> Vec<Entity> {
    (0..n)
        .map(|i| {
            let e = app
                .world_mut()
                .spawn((
                    Transform::from_xyz(i as f32, 0.0, 0.0),
                    GlobalTransform::default(),
                    Visibility::default(),
                ))
                .id();
            app.world_mut().resource_mut::<Speakers>().0.insert(
                format!("s{i}"),
                Speaker {
                    entity: Some(e),
                    name: format!("Speaker {i}"),
                    portrait: None,
                },
            );
            e
        })
        .collect()
}

fn time_frames(app: &mut App, frames: usize) -> Vec<f64> {
    (0..frames)
        .map(|_| {
            let t0 = Instant::now();
            app.update();
            t0.elapsed().as_secs_f64() * 1e6
        })
        .collect()
}

/// `frame-live`: owners are root entities. `frame-live-child`: each owner is the child of a rotated,
/// scaled parent, so tracking composes the owner's world position through its parent.
fn frame_live(font: &FontArc) {
    for (live, case) in [(0usize, "frame-live"), (50, "frame-live"), (200, "frame-live"), (200, "frame-live-child")] {
        let mut app = app(font);
        let owners = speakers(&mut app, live);
        if case == "frame-live-child" {
            let parent = app
                .world_mut()
                .spawn((
                    Transform::from_xyz(0.0, 0.5, -2.0)
                        .with_rotation(Quat::from_rotation_y(0.3))
                        .with_scale(Vec3::splat(1.5)),
                    GlobalTransform::default(),
                    Visibility::default(),
                ))
                .id();
            for owner in &owners {
                app.world_mut().entity_mut(*owner).insert(ChildOf(parent));
            }
        }
        let text = line(12);
        let world = app.world_mut();
        world.resource_scope(|world, mut images: Mut<Assets<Image>>| {
            world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
                let assets = world.resource::<BubbleAssets>();
                let style = BubbleStyle {
                    kind: BubbleKind::Speech,
                    emotion: Emotion::Neutral,
                    tail: true,
                    skin: BalloonSkin::crt(),
                };
                let built: Vec<_> = owners
                    .iter()
                    .map(|_| build_bubble(assets, &mut images, &mut materials, &style, &text))
                    .collect();
                let quad = assets.quad.clone();
                for (owner, rendered) in owners.iter().zip(built) {
                    world.spawn((
                        Bubble {
                            owner: *owner,
                            offset: Vec2::ZERO,
                        },
                        BubblePx(rendered.px),
                        // Live for the whole measurement: tracked and checked every frame.
                        BubbleTtl { expires_at: 1.0e9 },
                        Mesh3d(quad.clone()),
                        MeshMaterial3d(rendered.material),
                        Transform::default(),
                        Visibility::default(),
                    ));
                }
            });
        });
        time_frames(&mut app, 30);
        let mut times = time_frames(&mut app, FRAMES);
        let world = app.world_mut();
        let still = world.query::<&Bubble>().iter(world).count();
        let (median, p99) = median_p99(&mut times);
        println!("bubbles: {case} balloons={live} median_us={median:.1} p99_us={p99:.1} live_after={still}");
    }
}

fn bark_burst(font: &FontArc) {
    let mut app = app(font);
    speakers(&mut app, 20);
    time_frames(&mut app, 30);
    let text = line(80);
    for i in 0..20 {
        app.world_mut().write_message(Bark {
            speaker: format!("s{i}"),
            kind: BubbleKind::Speech,
            emotion: Emotion::Neutral,
            text: text.clone(),
        });
    }
    let mut seen = std::collections::HashSet::new();
    let mut times = Vec::with_capacity(FRAMES);
    for _ in 0..FRAMES {
        let t0 = Instant::now();
        app.update();
        times.push(t0.elapsed().as_secs_f64() * 1e6);
        let world = app.world_mut();
        seen.extend(world.query_filtered::<Entity, With<Bubble>>().iter(world));
    }
    let shown = seen.len();
    let worst = times.iter().copied().fold(0.0, f64::max);
    let (median, _) = median_p99(&mut times);
    println!("bubbles: bark-burst barks=20 worst_us={worst:.1} median_us={median:.1} shown={shown}");
}

fn asset_lifetime(font: &FontArc) {
    let mut app = app(font);
    speakers(&mut app, 200);
    app.insert_resource(bevy_speech_bubbles::BarkSettings {
        gap: 0.0,
        ..default()
    });
    time_frames(&mut app, 10);
    let count = |app: &App| {
        (
            app.world().resource::<Assets<Image>>().len(),
            app.world().resource::<Assets<StandardMaterial>>().len(),
        )
    };
    let (images_before, materials_before) = count(&app);
    let text = line(12);
    for i in 0..1000 {
        app.world_mut().write_message(Bark {
            speaker: format!("s{}", i % 200),
            kind: if i % 2 == 0 { BubbleKind::Speech } else { BubbleKind::Thought },
            emotion: Emotion::Neutral,
            text: text.clone(),
        });
        app.update();
    }
    let (images_peak, materials_peak) = count(&app);
    // Every bark's dwell is under 5 s; 400 frames is 6.4 s, then a few for the handles to drop.
    time_frames(&mut app, 400);
    let world = app.world_mut();
    let live = world.query::<&Bubble>().iter(world).count();
    let (images_after, materials_after) = count(&app);
    println!(
        "bubbles: assets barks=1000 images_before={images_before} images_peak={images_peak} \
         images_after={images_after} materials_before={materials_before} \
         materials_peak={materials_peak} materials_after={materials_after} live_after={live}"
    );
}

const PRESS_SCRIPT: &str = r#"
(
  conversations: {
    "greeting": (
      start: "a",
      nodes: {
        "a": Line(speaker: "s0", text: "Evening, traveller.", next: Some("b")),
        "b": Line(speaker: "s1", text: "Evening. Is the inn still open?", next: Some("c")),
        "c": Line(speaker: "s0", text: "Until the lamps go out.", next: None),
      },
    ),
    "gate": (
      start: "shut",
      nodes: {
        "shut": Line(speaker: "s0", text: "The gate is shut.", next: Some("ask")),
        "ask": Choice(speaker: "s0", prompt: "Open it?", options: [
          (text: "Leave it", next: "left"),
          (text: "Open it", next: "opened"),
        ]),
        "left": Line(speaker: "s1", text: "Left.", next: None),
        "opened": Line(speaker: "s1", text: "It creaks.", next: None),
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
        chars_per_second: DEFAULT_CHARS_PER_SECOND,
        bottom_margin: 16.0,
    }
}

/// Press `key` for one frame, then release it (`clear` alone keeps it held).
fn tap(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
}

fn presses(font: &FontArc) {
    let script: DialogueScript = match ron::from_str(PRESS_SCRIPT) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bench script: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = validate_script(&script) {
        eprintln!("bench script: {e}");
        std::process::exit(1);
    }
    for (presenter, presenter_name) in [
        (Presenter::Balloons, "balloons"),
        (Presenter::DialogBox, "dialog_box"),
    ] {
        for id in ["greeting", "gate"] {
            let mut app = app(font);
            speakers(&mut app, 2);
            app.insert_resource(script.clone())
                .insert_resource(presenter)
                .insert_resource(dialog_skin());
            app.world_mut().write_message(StartConversation { id: id.into() });
            let mut node = String::new();
            let mut presses = 0;
            let mut ended = 0;
            let mut frames = 0;
            loop {
                if frames == 0 {
                    app.update();
                } else {
                    let on_choice = matches!(
                        script.conversation(id).and_then(|c| c.nodes.get(&node)),
                        Some(ScriptNode::Choice { .. })
                    );
                    tap(&mut app, if on_choice { KeyCode::Digit1 } else { KeyCode::Space });
                    presses += 1;
                }
                frames += 1;
                let world = app.world_mut();
                if let Some(entered) = world
                    .resource_mut::<Messages<DialogueNodeEntered>>()
                    .drain()
                    .last()
                {
                    node = entered.node;
                }
                ended += world.resource_mut::<Messages<ConversationEnded>>().drain().count();
                if ended > 0 || frames > 100 {
                    break;
                }
            }
            // A few more frames: an exit that writes `ConversationEnded` twice would show here.
            for _ in 0..10 {
                app.update();
                ended += app
                    .world_mut()
                    .resource_mut::<Messages<ConversationEnded>>()
                    .drain()
                    .count();
            }
            println!("bubbles: presses conversation={id} presenter={presenter_name} presses={presses} ended={ended}");
        }
    }
}
