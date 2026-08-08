//! **Two balloons in the world, tracked to a camera you name.**
//!
//! One `Speech` balloon (rounded rect, pointed tail) and one `Thought` balloon (soft pill, dot tail),
//! each anchored above its own cube. They are quads in the scene, not a UI layer — orbit the camera
//! and they turn to face it; walk one behind the other and it occludes.
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

use ab_glyph::FontArc;
use bevy::prelude::*;
use bevy_speech_bubbles::{
    build_bubble, dwell_secs, expire_bubbles, track_bubbles, Bubble, BubbleAssets, BubbleKind,
    BubbleStyle, BubbleTtl, Emotion,
};

/// The example's own camera marker. This is the whole point — the crate never names it.
#[derive(Component)]
struct ExampleCamera;

/// Carries the font from `main` into `setup`, where `Assets<Mesh>` finally exists.
#[derive(Resource)]
struct LoadedFont(FontArc);

const SPOKEN: &str = "Contact, west corridor.";
const THOUGHT: &str = "...that was not a rat.";

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run -p bevy_speech_bubbles --example two_balloons -- <path-to-font.ttf>");
        eprintln!();
        eprintln!("A font path is required rather than defaulted. This crate rasterizes glyphs itself,");
        eprintln!("and guessing where your assets live is exactly the assumption a library must not make.");
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

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "bevy_speech_bubbles — balloons live in the world".into(),
                resolution: (900u32, 620u32).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(LoadedFont(font))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                // Your marker, not `Camera3d`.
                track_bubbles::<ExampleCamera>,
                expire_bubbles,
                orbit_camera,
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    font: Res<LoadedFont>,
    time: Res<Time<Real>>,
) {
    // The unit quad every balloon is drawn on, scaled per bubble to the size the raster came out.
    let assets = BubbleAssets { quad: meshes.add(Rectangle::new(1.0, 1.0)), font: font.0.clone() };

    let cube = meshes.add(Cuboid::new(0.8, 1.6, 0.8));
    let speakers = [
        (Vec3::new(-1.6, 0.8, 0.0), Color::srgb(0.30, 0.55, 0.85), SPOKEN, BubbleKind::Speech, Emotion::Surprise),
        (Vec3::new(1.6, 0.8, 0.0), Color::srgb(0.75, 0.45, 0.30), THOUGHT, BubbleKind::Thought, Emotion::Fear),
    ];

    for (pos, color, text, kind, emotion) in speakers {
        let owner = commands
            .spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(materials.add(StandardMaterial { base_color: color, ..default() })),
                Transform::from_translation(pos),
            ))
            .id();

        // Built once per line, not per frame — the raster is the expensive part.
        let style = BubbleStyle { kind, emotion, tail: true };
        let face = build_bubble(&assets, &mut images, &mut materials, &style, text);

        commands.spawn((
            Mesh3d(assets.quad.clone()),
            MeshMaterial3d(face.material),
            // `track_bubbles` reads `scale.y` to work out how far above the head to sit.
            Transform::from_scale(Vec3::new(face.size.x, face.size.y, 1.0)),
            Bubble { owner, offset: Vec2::ZERO },
            // Ambient balloons time out on their own. Dwell scales with how much there is to read.
            BubbleTtl { expires_at: time.elapsed_secs() + dwell_secs(text) * 4.0 },
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
        DirectionalLight { illuminance: 9_000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(4.0, 9.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 3.0, 7.0).looking_at(Vec3::new(0.0, 1.6, 0.0), Vec3::Y),
        ExampleCamera,
    ));

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
