# bevy_speech_bubbles — notes for agents

World-space speech and thought balloons: text rasterized on the CPU into an `Image`, put on a
billboarded quad, anchored above an owner entity and turned to face a camera you name.

## Source of truth

The source of truth for this crate is [`Ladvien/foundation_vs_slop`](https://github.com/Ladvien/foundation_vs_slop)
at `crates/bevy_speech_bubbles/`. If you are reading this in a standalone `Ladvien/bevy_speech_bubbles`
checkout, that is a read-only `git subtree split` mirror — **changes made here cannot be pulled back**.
Make them upstream.

## Build and test

A leaf — `bevy` with defaults off, `ab_glyph`, and optional `serde` — so it builds and tests on its own:
`cargo test -p bevy_speech_bubbles`.

## The non-negotiable: never name a game's camera

`tests/leaf.rs` forbids more than crate names. Its scan rejects `MainCamera`, `SquadMember` and
`MenuState` in the sources, alongside `avian`, `emerge` and `foundation_vs_slop`.

Those three identifiers are there because **this crate already made that mistake**: it used to name a
camera marker from the game it was extracted from. The fix — making the tracking system generic over the
marker (`track_bubbles::<C>`) — is what stops it silently breaking in any project with a second 3D
camera. Naming one again would undo that, **and it would compile**. That is why the test exists.

The same principle applies to the schedule and the font. This crate **exports system functions and
registers no plugin**, so the caller keeps its schedule; and the font path stays with the caller,
because "where do the assets live" is the one thing a library must not assume.

**CPU raster, not a text stack.** Bevy 0.19's text pipeline (parley/swash) has no public "rasterize a
string to a standalone `Image`" API, and a world-space balloon has no 2D camera to host `Text2d` — so
text is baked into an RGBA image sampled on a quad. `ab_glyph` is pure Rust, no system deps. Green,
*"Improved Alpha-Tested Magnification for Vector Textures"* (SIGGRAPH 2007) is the SDF alternative;
raster-on-change is the simpler one path, and it was chosen on purpose.

## Rules

- **Bevy 0.19 is pinned.** Read the vendored source
  (`~/.cargo/registry/src/index.crates.io-*/bevy-0.19.0/`, and its `examples/`), not bevy.org — that
  documents `main` and has been wrong for this pin more than once.
- **A missing `Res<T>` panics its system in 0.19**; it does not skip. Take `Option<Res<T>>`, or have the
  caller `init_resource` it. This bites hardest here, because `BubbleAssets` is caller-supplied.
- **All run conditions are evaluated — there is no short-circuit.** A bare `Res<T>` in a `.run_if(..)`
  closure panics whenever that resource is absent, even behind an earlier condition that returned false.
- **`Single<..>` silently skips its system on a non-unique match** — which is exactly why the camera is a
  type parameter rather than `With<Camera3d>`.
- **No `unwrap()`**, including on font loading and image allocation.
- **One path per feature.** No fallbacks, no legacy shims, no stub placeholders.

## In the monorepo

The game re-exports this through `dialogue::bubble` (`src/dialogue/bubble.rs:15`) and `dialogue::model`
(`src/dialogue/model.rs:21`), which is where the font path and the game's camera marker are supplied.
Root `CLAUDE.md` and `TESTING.md` carry the project-wide rules; neither is part of this mirror.
