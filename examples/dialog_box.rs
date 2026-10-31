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
//! `BUBBLES_CONVERSATION=wrap` starts the conversation whose words are longer than the box: they wrap
//! inside the paper instead of running out of it.
//!
//! `BUBBLES_SHOT=<file.png>` renders without a window into a 900x620 image, shows every line at once,
//! saves the image after `BUBBLES_FRAMES` frames (default 30) and exits. The clock then steps a fixed
//! 1/60 s per frame, so a frame number is the same picture on every machine, and a locked or headless
//! screen still captures.
//!
//! Run: `cargo run --example dialog_box -- <path-to-font.ttf>`

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::sprite::TextureSlicer;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
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
    "wrap": (
      start: "ledger",
      nodes: {
        "ledger": Choice(speaker: "keeper", prompt: "The ledger names the ship the Donaudampfschifffahrtsgesellschaftskapitaensmuetzenverbandsvorsitzender and nothing else.", options: [
          (text: "Sign beneath Donaudampfschifffahrtsgesellschaftskapitaensmuetzenverbandsvorsitzender.", next: "done"),
          (text: "Burn it.", next: "done"),
        ]),
        "done": Line(speaker: "keeper", text: "So be it.", next: None),
      },
    ),
  },
)
"#;

#[derive(Resource)]
struct FontPath(String);

#[derive(Resource)]
struct Restart(Timer);

/// The conversation to start: `demo`, or `BUBBLES_CONVERSATION`.
#[derive(Resource)]
struct Conversation(String);

/// `BUBBLES_SHOT`: where the still goes and after how many frames.
#[derive(Resource)]
struct Shot {
    path: String,
    frames: u32,
}

/// The image the camera renders into while capturing.
#[derive(Resource)]
struct ShotImage(Handle<Image>);

/// The still's size in pixels: the window's 900x620 at a scale factor of 1.
const SHOT_SIZE: UVec2 = UVec2::new(900, 620);

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run --example dialog_box -- <path-to-font.ttf>");
        eprintln!();
        eprintln!("  macOS:  /System/Library/Fonts/Supplemental/Arial.ttf");
        eprintln!("  Linux:  /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
        std::process::exit(2);
    };

    let shot = std::env::var("BUBBLES_SHOT").ok().map(|path| Shot {
        path,
        frames: std::env::var("BUBBLES_FRAMES")
            .ok()
            .and_then(|f| f.parse().ok())
            .unwrap_or(30),
    });
    let conversation = std::env::var("BUBBLES_CONVERSATION").unwrap_or_else(|_| "demo".to_string());

    let mut app = App::new();
    match shot {
        None => {
            app.add_plugins(DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "bevy_speech_bubbles — the screen dialog box".into(),
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
    app.add_plugins(SpeechBubblesPlugin::<ExampleCamera>::default())
        .insert_resource(FontPath(path))
        .insert_resource(Conversation(conversation))
        .insert_resource(Restart(Timer::from_seconds(1.0, TimerMode::Once)))
        .insert_resource(Presenter::DialogBox)
        .add_systems(Startup, setup)
        .add_systems(Update, (restart, cycle_speed))
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
    conversation: Res<Conversation>,
    shot: Option<Res<Shot>>,
    mut starts: MessageWriter<StartConversation>,
) {
    let camera = commands
        .spawn((Camera2d, ExampleCamera, IsDefaultUiCamera))
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
        // A still shows the finished line; a window types it out.
        chars_per_second: if shot.is_some() {
            0.0
        } else {
            DEFAULT_CHARS_PER_SECOND
        },
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
    starts.write(StartConversation {
        id: conversation.0.clone(),
    });
}

/// Start the conversation again a second after it ends.
fn restart(
    time: Res<Time>,
    conversation: Res<Conversation>,
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
        starts.write(StartConversation {
            id: conversation.0.clone(),
        });
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
