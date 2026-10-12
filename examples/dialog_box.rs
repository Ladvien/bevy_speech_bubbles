//! **The screen dialog box, with a choice.**
//!
//! A four-node script — two lines, a choice, a closing line — shown in the screen dialog box that
//! [`SpeechBubblesPlugin`] drives. The box is skinned by a [`DialogSkin`]; here every texture is a flat
//! colour generated in code (`Image::new_fill`), so the example needs no art. A real game hands in
//! 9-sliced pixel art instead; the box does not care.
//!
//! Keys: `Space`/`Enter`/click completes the typing line, then advances it; `1`/`2`, the arrows and
//! `Enter`, or the mouse pick an option. When the conversation ends it starts again after a moment.
//! `F` cycles the typewriter speed (the skin is read live), which is how a settings menu would do it.
//!
//! **The font is yours.** The path is a required argument.
//!
//! Run: `cargo run --example dialog_box -- <path-to-font.ttf>`

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::TextureSlicer;
use bevy_speech_bubbles::{
    ConversationEnded, DEFAULT_CHARS_PER_SECOND, DialogSkin, DialogueScript, Presenter, Speaker,
    Speakers, SpeechBubblesPlugin, StartConversation, validate_script,
};

/// This example has no world balloons, but the plugin is generic over a camera marker anyway.
#[derive(Component)]
struct ExampleCamera;

const SCRIPT: &str = r#"
(
  conversations: {
    "demo": (
      start: "greet",
      nodes: {
        "greet": Line(speaker: "keeper", text: "The gate has held three nights. It will not hold a fourth without help.", next: Some("ask")),
        "ask": Choice(speaker: "keeper", prompt: "Will you take the watch tonight?", options: [
          (text: "I will take the watch.", next: "yes"),
          (text: "Ask someone else.", next: "no"),
        ]),
        "yes": Line(speaker: "keeper", text: "Good. Bring a lantern, and do not look at what stands in the road.", next: None),
        "no": Line(speaker: "keeper", emotion: Sadness, text: "Then I will ask the boy, and God help him.", next: None),
      },
    ),
  },
)
"#;

#[derive(Resource)]
struct FontPath(String);

#[derive(Resource)]
struct Restart(Timer);

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run --example dialog_box -- <path-to-font.ttf>");
        eprintln!();
        eprintln!("  macOS:  /System/Library/Fonts/Supplemental/Arial.ttf");
        eprintln!("  Linux:  /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
        std::process::exit(2);
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "bevy_speech_bubbles — the screen dialog box".into(),
                resolution: (900u32, 620u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SpeechBubblesPlugin::<ExampleCamera>::default())
        .insert_resource(FontPath(path))
        .insert_resource(Restart(Timer::from_seconds(1.0, TimerMode::Once)))
        .insert_resource(Presenter::DialogBox)
        .add_systems(Startup, setup)
        .add_systems(Update, (restart, cycle_speed))
        .run();
}

/// A flat `w`×`h` RGBA image.
fn flat(images: &mut Assets<Image>, w: u32, h: u32, rgba: [u8; 4]) -> Handle<Image> {
    images.add(Image::new_fill(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

/// A 12×12 frame: a 3 px dark border around a clear centre, so the paper shows through.
fn frame(images: &mut Assets<Image>) -> Handle<Image> {
    let mut data = Vec::with_capacity(12 * 12 * 4);
    for y in 0..12 {
        for x in 0..12 {
            let edge = !(3..9).contains(&x) || !(3..9).contains(&y);
            data.extend_from_slice(if edge { &[58, 42, 30, 255] } else { &[0, 0, 0, 0] });
        }
    }
    images.add(Image::new(
        Extent3d {
            width: 12,
            height: 12,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut speakers: ResMut<Speakers>,
    mut fonts: ResMut<Assets<Font>>,
    path: Res<FontPath>,
    mut starts: MessageWriter<StartConversation>,
) {
    commands.spawn((Camera2d, ExampleCamera));
    commands.insert_resource(ClearColor(Color::srgb(0.10, 0.09, 0.08)));

    let script: DialogueScript = match ron::from_str(SCRIPT) {
        Ok(s) => s,
        Err(e) => {
            error!("the example script does not parse: {e}");
            return;
        }
    };
    if let Err(e) = validate_script(&script) {
        error!("the example script is invalid: {e}");
        return;
    }
    commands.insert_resource(script);

    // The font: read from the path you gave. A path outside the `assets` folder is not the asset
    // server's to load, so the bytes go in as a `Font` asset directly.
    let bytes = match std::fs::read(&path.0) {
        Ok(b) => b,
        Err(e) => {
            error!("could not read `{}`: {e}", path.0);
            return;
        }
    };
    let font = fonts.add(Font::from_bytes(bytes));
    let body = TextFont {
        font: font.clone().into(),
        font_size: FontSize::Px(22.0),
        ..default()
    };
    let title = TextFont {
        font: font.into(),
        font_size: FontSize::Px(18.0),
        ..default()
    };
    let slicer = TextureSlicer {
        border: BorderRect::all(3.0),
        ..default()
    };
    commands.insert_resource(DialogSkin {
        frame: frame(&mut images),
        frame_slicer: slicer.clone(),
        paper: flat(&mut images, 4, 4, [232, 217, 176, 255]),
        name_tag: flat(&mut images, 6, 6, [196, 150, 60, 255]),
        name_tag_slicer: TextureSlicer {
            border: BorderRect::all(1.0),
            ..default()
        },
        caret: [
            flat(&mut images, 8, 8, [42, 28, 18, 255]),
            flat(&mut images, 8, 8, [42, 28, 18, 90]),
        ],
        body_font: body,
        title_font: title,
        text_color: Color::srgb_u8(42, 28, 18),
        choice_color: Color::srgb_u8(92, 74, 56),
        choice_hover_color: Color::srgb_u8(200, 150, 60),
        padding: 16.0,
        width: 640.0,
        portrait_size: 64.0,
        chars_per_second: DEFAULT_CHARS_PER_SECOND,
        bottom_margin: 48.0,
    });

    speakers.0.insert(
        "keeper".into(),
        Speaker {
            entity: None,
            name: "The Keeper".into(),
            portrait: Some(flat(&mut images, 16, 16, [120, 86, 64, 255])),
        },
    );
    starts.write(StartConversation { id: "demo".into() });
}

/// Start the conversation again a second after it ends.
fn restart(
    time: Res<Time>,
    mut timer: ResMut<Restart>,
    mut ends: MessageReader<ConversationEnded>,
    mut starts: MessageWriter<StartConversation>,
    mut waiting: Local<bool>,
) {
    if ends.read().next().is_some() {
        *waiting = true;
        timer.0.reset();
    }
    if *waiting && timer.0.tick(time.delta()).just_finished() {
        *waiting = false;
        starts.write(StartConversation { id: "demo".into() });
    }
}

/// `F`: cycle slow / normal / fast / instant. The typewriter reads the skin every tick.
fn cycle_speed(keys: Res<ButtonInput<KeyCode>>, skin: Option<ResMut<DialogSkin>>) {
    let Some(mut skin) = skin else { return };
    if keys.just_pressed(KeyCode::KeyF) {
        skin.chars_per_second = match skin.chars_per_second {
            s if s <= 0.0 => 20.0,
            s if s < 40.0 => 40.0,
            s if s < 80.0 => 80.0,
            _ => 0.0,
        };
        info!("typewriter: {} chars/s", skin.chars_per_second);
    }
}
