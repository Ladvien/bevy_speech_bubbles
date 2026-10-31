# bevy_speech_bubbles

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

Speech balloons, barks, conversation graphs and a skinnable dialog box for Bevy games whose characters talk.

[![crates.io](https://img.shields.io/crates/v/bevy_speech_bubbles.svg)](https://crates.io/crates/bevy_speech_bubbles) [![docs.rs](https://img.shields.io/docsrs/bevy_speech_bubbles)](https://docs.rs/bevy_speech_bubbles) [![CI](https://github.com/bevy-coven/bevy_speech_bubbles/actions/workflows/ci.yml/badge.svg)](https://github.com/bevy-coven/bevy_speech_bubbles/actions/workflows/ci.yml) [![Bevy 0.20](https://img.shields.io/badge/bevy-0.20-purple.svg)](https://bevyengine.org) [![licence](https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-blue.svg)](#who-and-licence)

![Five coloured figures standing in a ring while the camera orbits; speech balloons with pointed tails and thought balloons with dot-tails appear above them and fade, each holding its face toward the camera](https://raw.githubusercontent.com/bevy-coven/bevy_speech_bubbles/main/docs/chatter.gif)

*`examples/chatter.rs`: balloons are quads in the scene, not a UI layer, so they float over their speaker, can be hidden behind a wall, and turn to face the camera.*

**Status:** usable · Bevy 0.20 · MSRV 1.97.1

**Why:** Bevy's text stack cannot rasterize a string into a standalone image and a world-space balloon has no 2D camera to host one, so this crate does the rasterizing, and it puts world balloons, a conversation runtime and a screen dialog box behind one plugin that reads its scripts as data.

## Quick start

```sh
cargo add bevy_speech_bubbles
```

```rust,no_run
use bevy::prelude::*;
use bevy_speech_bubbles::*;

#[derive(Component)]
struct MainCamera; // the camera balloons turn to face

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, SpeechBubblesPlugin::<MainCamera>::default()))
        .add_systems(Startup, |mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>| {
            commands.spawn((Camera3d::default(), MainCamera, Transform::from_xyz(0.0, 2.0, 6.0)));
            let font = std::fs::read("assets/fonts/YourFont.ttf").expect("a font of your own");
            let quad = meshes.add(Rectangle::new(1.0, 1.0));
            commands.insert_resource(BubbleAssets::from_font_bytes(quad, font).unwrap());
        })
        .run();
}
```

Then register a speaker in `Speakers` and write a `Bark` or a `StartConversation` message: [conversations](https://github.com/bevy-coven/bevy_speech_bubbles/blob/main/docs/guide/conversations.md).

## Try it

```sh
cargo run --example chatter -- /System/Library/Fonts/Supplemental/Arial.ttf
```

The GIF above; any `.ttf` works. `dialog_box` shows the screen box with a choice, and `two_balloons` is the smallest thing that works.

![The chatter example: coloured figures on a grey floor, one with a tailed speech balloon reading "I told them the door was open" and one with a thought balloon](https://raw.githubusercontent.com/bevy-coven/bevy_speech_bubbles/main/docs/media/chatter.png)

*A frame of `chatter`: a spoken line and a thought, each over its speaker.*

## What you can do

- **Float a balloon over any entity**: spoken lines get a tail and thoughts a dot-tail, tinted by emotion, following the speaker through a hierarchy and hiding with it: [balloons](https://github.com/bevy-coven/bevy_speech_bubbles/blob/main/docs/guide/balloons.md).
- **Write conversations as data**: a `DialogueScript` of lines and choices loaded from RON or any serde format, refused with a useful error on a misspelt key, and checked by `validate_script`: [conversations](https://github.com/bevy-coven/bevy_speech_bubbles/blob/main/docs/guide/conversations.md).
- **Pick how lines are shown**: `Presenter::Balloons` over the speakers' heads, or `Presenter::DialogBox` with a typewriter and choices by click, arrows or number keys: [conversations](https://github.com/bevy-coven/bevy_speech_bubbles/blob/main/docs/guide/conversations.md).
- **Skin it**: `BalloonSkin` (a CRT phosphor look and pixel-perfect parchment ship) and a 9-sliced `DialogSkin` with portrait and name tag: [what is in it](https://github.com/bevy-coven/bevy_speech_bubbles/blob/main/docs/guide/whats-in-it.md).
- **Pace one-off barks**: `Bark`s with `BarkSettings` limits, for crowds that mutter without a script.
- **Size balloons for your world**: `BalloonScale` sets world units per texel, so feet, metres and a zooming camera all fit: [balloons](https://github.com/bevy-coven/bevy_speech_bubbles/blob/main/docs/guide/balloons.md#scale-and-pixel-balloons).
- **Keep talking while paused**: everything runs on real time, and the dialog's input set consumes the keys it used: [time and input](https://github.com/bevy-coven/bevy_speech_bubbles/blob/main/docs/guide/timing.md).

## Part of the Coven

`bevy_speech_bubbles` is one of the [Bevy Coven](https://github.com/bevy-coven) crates: [`bevy_society`](https://crates.io/crates/bevy_society)'s villagers speak through it. Grimoire, the Coven's editor, stores barks and conversations with this crate's own types: [use it in Grimoire](https://bevy-coven.github.io/crates/bevy_speech_bubbles.html#use-it-in-grimoire).

## Bevy compatibility

| `bevy_speech_bubbles` | Bevy |
|---|---|
| 0.2 | 0.20 |

## Learn more

- [docs.rs/bevy_speech_bubbles](https://docs.rs/bevy_speech_bubbles): the API.
- [The book page](https://bevy-coven.github.io/crates/bevy_speech_bubbles.html).
- [`docs/guide/`](https://github.com/bevy-coven/bevy_speech_bubbles/blob/main/docs/guide/README.md): everything the old README carried, in five pages.
- [`examples/`](https://github.com/bevy-coven/bevy_speech_bubbles/tree/main/examples) and [`CHANGELOG.md`](https://github.com/bevy-coven/bevy_speech_bubbles/blob/main/CHANGELOG.md).

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). This repository is canonical and co-evolves with the game it was built for, Sanctuary; issues and pull requests belong at [github.com/bevy-coven/bevy_speech_bubbles/issues](https://github.com/bevy-coven/bevy_speech_bubbles/issues), and contributions follow the org's [CONTRIBUTING.md](https://github.com/bevy-coven/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option.
