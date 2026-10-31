//! **Two balloons in the world, tracked to a camera you name.**
//!
//! One `Speech` balloon (rounded rect, pointed tail) and one `Thought` balloon (soft pill, dot tail),
//! each anchored above its own cube. They are quads in the scene, not a UI layer — orbit the camera
//! and they turn to face it; walk one behind the other and it occludes.
//!
//! The thinker rides a cart: its cube is a **child entity** (`ChildOf` the cart), so its own
//! `Transform` is relative to the cart. `track_bubbles` anchors to the owner's world position, so the
//! balloon floats over the cube wherever the cart carries it, and hides with the cart.
//!
//! The camera marker is a **type parameter**, and this example is the reason why. `track_bubbles`
//! takes `Single<&GlobalTransform, With<C>>`, and Bevy's `Single` silently *skips its system* when the
//! query matches anything other than exactly one entity. A crate that hard-coded `With<Camera3d>`
//! would work perfectly until something spawned a second 3D camera, at which point every balloon
//! would quietly stop tracking and nothing would error. Naming your own marker makes that impossible
//! for the crate to get wrong on your behalf.
//!
//! The font is yours to load, too — a library must not assume where your assets live.
//!
//! Run: `cargo run -p bevy_speech_bubbles --example two_balloons -- <path-to-font.ttf>`
//!
//! `BUBBLES_SHOT=<file.png>` renders without a window into a 1800x1240 image, saves it after
//! `BUBBLES_FRAMES` frames (default 90) and exits. The clock then steps a fixed 1/60 s per frame, so a
//! frame number is the same picture on every machine, and a locked or headless screen still captures.

use std::time::Duration;

use ab_glyph::FontArc;
use bevy::app::ScheduleRunnerPlugin;
use bevy::camera::RenderTarget;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use bevy_speech_bubbles::{
    BalloonSkin, Bubble, BubbleAssets, BubbleKind, BubblePx, BubbleStyle, BubbleTtl, Emotion,
    build_bubble, dwell_secs, expire_bubbles, track_bubbles,
};

/// The example's own camera marker. This is the whole point — the crate never names it.
#[derive(Component)]
struct ExampleCamera;

/// Carries the font from `main` into `setup`, where `Assets<Mesh>` finally exists.
#[derive(Resource)]
struct LoadedFont(FontArc);

/// The cart the thinker rides. It slides to and fro, carrying its child cube and that cube's balloon.
#[derive(Component)]
struct Cart;

/// `BUBBLES_SHOT`: where the still goes and after how many frames.
#[derive(Resource)]
struct Shot {
    path: String,
    frames: u32,
}

/// The image the camera renders into while capturing.
#[derive(Resource)]
struct ShotImage(Handle<Image>);

/// The still's size in pixels: the window's 900x620 at a 2x scale factor.
const SHOT_SIZE: UVec2 = UVec2::new(1800, 1240);

const SPOKEN: &str = "Contact, west corridor.";
const THOUGHT: &str = "...that was not a rat.";

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!(
            "usage: cargo run -p bevy_speech_bubbles --example two_balloons -- <path-to-font.ttf>"
        );
        eprintln!();
        eprintln!(
            "A font path is required rather than defaulted. This crate rasterizes glyphs itself,"
        );
        eprintln!(
            "and guessing where your assets live is exactly the assumption a library must not make."
        );
        eprintln!();
        eprintln!("  macOS:  /System/Library/Fonts/Supplemental/Arial.ttf");
        eprintln!("  Linux:  /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
        std::process::exit(2);
    };

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("could not read `{path}`: {e}");
            std::process::exit(1);
        }
    };
    let font = match FontArc::try_from_vec(bytes) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("`{path}` is not a font this crate can parse: {e}");
            std::process::exit(1);
        }
    };

    let shot = std::env::var("BUBBLES_SHOT").ok().map(|path| Shot {
        path,
        frames: std::env::var("BUBBLES_FRAMES")
            .ok()
            .and_then(|f| f.parse().ok())
            .unwrap_or(90),
    });

    let mut app = App::new();
    match shot {
        None => {
            app.add_plugins(DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "bevy_speech_bubbles — balloons live in the world".into(),
                    resolution: (900u32, 620u32).into(),
                    ..default()
                }),
                ..default()
            }));
        }
        Some(shot) => {
            // No window: the camera renders into an image, so a locked or headless screen captures.
            app.add_plugins((
                DefaultPlugins
                    .set(WindowPlugin {
                        primary_window: None,
                        exit_condition: ExitCondition::DontExit,
                        ..default()
                    })
                    .disable::<WinitPlugin>(),
                ScheduleRunnerPlugin::run_loop(Duration::ZERO),
            ))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / 60.0,
            )))
            .insert_resource(shot)
            .add_systems(Last, capture);
        }
    }
    app.insert_resource(LoadedFont(font))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                // Your marker, not `Camera3d`.
                track_bubbles::<ExampleCamera>,
                expire_bubbles,
                orbit_camera,
                slide_cart,
            ),
        )
        .run();
}

/// Save the camera's image on frame `Shot::frames`, then exit.
fn capture(
    mut commands: Commands,
    shot: Res<Shot>,
    image: Option<Res<ShotImage>>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    let Some(image) = image else {
        return;
    };
    if *frame == shot.frames {
        commands
            .spawn(Screenshot::image(image.0.clone()))
            .observe(save_to_disk(shot.path.clone()))
            .observe(
                |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    exit.write(AppExit::Success);
                },
            );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    font: Res<LoadedFont>,
    time: Res<Time<Real>>,
    shot: Option<Res<Shot>>,
) {
    // The unit quad every balloon is drawn on, scaled per bubble to the size the raster came out.
    let assets = BubbleAssets {
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
        font: font.0.clone(),
    };

    let cube = meshes.add(Cuboid::new(0.8, 1.6, 0.8));

    // The talker stands on the floor: a root entity.
    let talker = commands
        .spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.30, 0.55, 0.85),
                ..default()
            })),
            Transform::from_xyz(-1.6, 0.8, 0.0),
        ))
        .id();

    // The thinker rides a cart: its `Transform` is relative to the cart, not to the world.
    let cart = commands
        .spawn((
            Cart,
            Mesh3d(meshes.add(Cuboid::new(1.4, 0.2, 1.4))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.35, 0.30, 0.25),
                ..default()
            })),
            Transform::from_xyz(1.6, 0.1, 0.0),
        ))
        .id();
    let thinker = commands
        .spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.75, 0.45, 0.30),
                ..default()
            })),
            Transform::from_xyz(0.0, 0.9, 0.0),
            ChildOf(cart),
        ))
        .id();

    let speakers = [
        (talker, SPOKEN, BubbleKind::Speech, Emotion::Surprise),
        (thinker, THOUGHT, BubbleKind::Thought, Emotion::Fear),
    ];
    for (owner, text, kind, emotion) in speakers {
        // Built once per line, not per frame — the raster is the expensive part.
        let style = BubbleStyle {
            kind,
            emotion,
            tail: true,
            skin: BalloonSkin::crt(),
        };
        let face = build_bubble(&assets, &mut images, &mut materials, &style, text);

        commands.spawn((
            Mesh3d(assets.quad.clone()),
            MeshMaterial3d(face.material),
            // `track_bubbles` reads `scale.y` to work out how far above the head to sit.
            Transform::from_scale(Vec3::new(face.size.x, face.size.y, 1.0)),
            // A balloon is a flat quad in the air: it must not throw a shadow on the floor.
            NotShadowCaster,
            Bubble {
                owner,
                offset: Vec2::ZERO,
            },
            // The raster size: `track_bubbles` sizes the quad from it and the live `BalloonScale`.
            BubblePx(face.px),
            // Ambient balloons time out on their own. Dwell scales with how much there is to read.
            BubbleTtl {
                expires_at: time.elapsed_secs() + dwell_secs(text) * 4.0,
            },
        ));
    }

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(16.0, 16.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.14, 0.14, 0.16),
            ..default()
        })),
        Transform::IDENTITY,
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 9.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let camera = commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 3.0, 7.0).looking_at(Vec3::new(0.0, 1.6, 0.0), Vec3::Y),
            ExampleCamera,
        ))
        .id();
    if shot.is_some() {
        let mut image = Image::new_target_texture(
            SHOT_SIZE.x,
            SHOT_SIZE.y,
            TextureFormat::Rgba8UnormSrgb,
            None,
        );
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let image = images.add(image);
        commands
            .entity(camera)
            .insert(RenderTarget::Image(image.clone().into()));
        commands.insert_resource(ShotImage(image));
    }

    commands.insert_resource(assets);
    info!("balloons expire on their own — watch them go, and note they never faced away from you");
}

/// Orbit slowly, so it is obvious the balloons are billboarded rather than pasted on the screen.
fn orbit_camera(time: Res<Time>, mut cam: Query<&mut Transform, With<ExampleCamera>>) {
    let t = time.elapsed_secs() * 0.35;
    for mut tf in &mut cam {
        tf.translation = Vec3::new(t.sin() * 7.0, 3.0, t.cos() * 7.0);
        tf.look_at(Vec3::new(0.0, 1.6, 0.0), Vec3::Y);
    }
}

/// Slide the cart to and fro along X: its child cube, and so the thought balloon, ride along.
fn slide_cart(time: Res<Time>, mut cart: Query<&mut Transform, With<Cart>>) {
    for mut tf in &mut cart {
        tf.translation.x = 1.6 + (time.elapsed_secs() * 0.8).sin() * 0.6;
    }
}
