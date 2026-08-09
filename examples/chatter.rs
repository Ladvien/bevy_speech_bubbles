//! **A room that keeps talking, while the camera keeps moving.**
//!
//! Five speakers in a ring take turns. Each line is rasterized once into an `Image`, put on a quad,
//! and left to expire on its own — and the camera orbits the whole time, so you can watch every
//! balloon hold its face toward you without any of them being UI.
//!
//! That is the thing to look at. These are **quads in the scene**, not a screen-space layer: they sit
//! at a world position above their owner, they are occluded by geometry, and they turn because
//! `track_bubbles` turns them. A UI overlay would give you none of that, and a `Text2d` would need a
//! 2D camera this scene does not have.
//!
//! The two shapes are the semantic channel — a tailed balloon for what is said aloud, a soft pill with
//! a dot-tail for what is only thought — and the border tint is the affect. Both are the crate's
//! vocabulary rather than a colour the caller invents, which is the point An et al. make in
//! *AniBalloons* (arXiv:2408.06294): balloon colour reliably carries emotion, so it deserves to be a
//! first-class axis.
//!
//! Dwell time is not a constant. `dwell_secs` scales with how much there is to read, so the long lines
//! linger and the short ones snap away — watch the ring and you will see them fall out of sync.
//!
//! **The font is yours.** This crate rasterizes glyphs itself and will not guess where your assets
//! live, so the path is a required argument.
//!
//! Run: `cargo run -p bevy_speech_bubbles --example chatter -- <path-to-font.ttf>`

use ab_glyph::FontArc;
use bevy::prelude::*;
use bevy_speech_bubbles::{
    Bubble, BubbleAssets, BubbleKind, BubbleStyle, BubbleTtl, Emotion, build_bubble, dwell_secs,
    expire_bubbles, track_bubbles,
};

/// The example's own camera marker. This is the whole point of `track_bubbles` being generic: the
/// crate never names a camera, because a project with two 3D cameras would break if it did.
#[derive(Component)]
struct ExampleCamera;

#[derive(Resource)]
struct LoadedFont(FontArc);

/// The speakers, in ring order, so turns go round rather than jumping about.
#[derive(Resource)]
struct Ring {
    speakers: Vec<Entity>,
    next: usize,
    line: usize,
    at: f32,
}

/// One line each: the text, the channel it is said on, and the affect that tints it.
const SCRIPT: &[(&str, BubbleKind, Emotion)] = &[
    ("Did you hear that?", BubbleKind::Speech, Emotion::Fear),
    ("nothing down here for weeks", BubbleKind::Thought, Emotion::Calm),
    ("It moved. It definitely moved.", BubbleKind::Speech, Emotion::Surprise),
    ("I told them the door was open", BubbleKind::Thought, Emotion::Anger),
    ("We are not going back for it.", BubbleKind::Speech, Emotion::Anger),
    ("so that is what the smell was", BubbleKind::Thought, Emotion::Sadness),
    ("Found it! Over here!", BubbleKind::Speech, Emotion::Joy),
    ("perfectly calm, perfectly fine", BubbleKind::Thought, Emotion::Calm),
    ("Stay where I can see you.", BubbleKind::Speech, Emotion::Neutral),
    ("i want to go home", BubbleKind::Thought, Emotion::Sadness),
];

/// Seconds between turns. Shorter than the shortest dwell, so the ring overlaps and several balloons
/// are alive at once — which is when the billboarding is most obvious.
const TURN_EVERY: f32 = 1.15;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run -p bevy_speech_bubbles --example chatter -- <path-to-font.ttf>");
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
                // 0.19 takes physical pixels as `u32` — there is no `(f32, f32)` conversion.
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
                take_a_turn,
                // Your marker, not `Camera3d` — the crate is generic over it on purpose.
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
    font: Res<LoadedFont>,
) {
    commands.spawn((
        ExampleCamera,
        Camera3d::default(),
        Transform::from_xyz(0.0, 4.4, 10.5).looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y),
    ));
    commands.spawn((
        // 0.19 spells this `shadow_maps_enabled`; it was `shadows_enabled` in earlier releases.
        DirectionalLight { illuminance: 6_500.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(4.0, 9.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(24.0, 24.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.13, 0.13, 0.16),
            perceptual_roughness: 0.95,
            ..default()
        })),
    ));

    // The unit quad every balloon is drawn on, scaled per bubble to the size its raster came out.
    commands.insert_resource(BubbleAssets {
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
        font: font.0.clone(),
    });

    // 2.0 tall, and the OWNER entity sits at floor level with the mesh as a child.
    //
    // That is not decoration: `BUBBLE_ANCHOR_Y` is 2.5 **above the owner's own translation**, so the
    // crate is assuming an origin at the feet, the way a character controller usually has it. Put the
    // owner at the box's centre instead and every balloon floats an extra half-height up, detached,
    // with its tail pointing at nothing. Worth knowing before you wonder why yours are in orbit.
    let body = meshes.add(Cuboid::new(0.7, 2.0, 0.7));
    let hues = [
        Color::srgb(0.30, 0.55, 0.85),
        Color::srgb(0.75, 0.45, 0.30),
        Color::srgb(0.40, 0.70, 0.45),
        Color::srgb(0.68, 0.38, 0.68),
        Color::srgb(0.80, 0.72, 0.35),
    ];
    let mut speakers = Vec::new();
    for (i, hue) in hues.into_iter().enumerate() {
        let a = i as f32 / hues.len() as f32 * std::f32::consts::TAU;
        let owner = commands
            .spawn((
                Transform::from_xyz(a.cos() * 3.0, 0.0, a.sin() * 3.0),
                Visibility::default(),
            ))
            .id();
        commands.entity(owner).with_children(|p| {
            p.spawn((
                Mesh3d(body.clone()),
                MeshMaterial3d(materials.add(StandardMaterial { base_color: hue, ..default() })),
                Transform::from_xyz(0.0, 1.0, 0.0),
            ));
        });
        speakers.push(owner);
    }
    commands.insert_resource(Ring { speakers, next: 0, line: 0, at: 0.0 });
}

/// Hand the next speaker the next line. One rasterization per line — never per frame, because the
/// raster is the expensive part and a balloon's text does not change once it is said.
fn take_a_turn(
    mut commands: Commands,
    time: Res<Time<Real>>,
    assets: Res<BubbleAssets>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ring: ResMut<Ring>,
) {
    let now = time.elapsed_secs();
    if now < ring.at {
        return;
    }
    ring.at = now + TURN_EVERY;

    let Some(&owner) = ring.speakers.get(ring.next) else { return };
    let (text, kind, emotion) = SCRIPT[ring.line % SCRIPT.len()];
    ring.next = (ring.next + 1) % ring.speakers.len();
    ring.line += 1;

    let style = BubbleStyle { kind, emotion, tail: true };
    let face = build_bubble(&assets, &mut images, &mut materials, &style, text);

    commands.spawn((
        Mesh3d(assets.quad.clone()),
        MeshMaterial3d(face.material),
        // `track_bubbles` reads `scale.y` to work out how far above the head to sit.
        Transform::from_scale(Vec3::new(face.size.x, face.size.y, 1.0)),
        Bubble { owner, offset: Vec2::ZERO },
        // Dwell scales with how much there is to read, so long lines linger and short ones snap away.
        BubbleTtl { expires_at: now + dwell_secs(text) },
    ));
}

fn orbit_camera(time: Res<Time>, mut cam: Query<&mut Transform, With<ExampleCamera>>) {
    let a = time.elapsed_secs() * 0.42;
    for mut tf in &mut cam {
        tf.translation = Vec3::new(a.cos() * 10.5, 4.4, a.sin() * 10.5);
        *tf = tf.looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y);
    }
}
