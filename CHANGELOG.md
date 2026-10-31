# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows semantic versioning
(pre-1.0: a breaking change bumps the minor version).

## [Unreleased]

### Changed

- The README follows the bevy-coven org's documentation guide: the org banner, badges, a Status and a Why line, a quick start that is compiled as a doctest, a still image, and "What you can do" bullets linking `docs/guide/`, where every section it used to carry now lives unchanged. Image and file links are absolute URLs, so docs.rs shows them.

### Added

- `examples/dialog_box.rs`: `BUBBLES_SHOT=<file.png>` (with `BUBBLES_FRAMES`, default 30) renders a still
  offscreen on a fixed clock with every line shown at once, and `BUBBLES_CONVERSATION=wrap` starts a
  conversation whose words are longer than the box.

### Changed

- A new runtime dependency, `unicode-segmentation` 1 (pure Rust, no dependencies of its own), for the
  grapheme clusters a long word is broken between.

### Fixed

- A word wider than a balloon line no longer widens the balloon, and the screen dialog box no longer
  lets it run out of the paper. `wrap_lines` was a greedy wrap on whitespace, so a 60-letter word (a
  long name, a URL, a German compound) went on a line of its own and that line set the balloon's width:
  1269x136 px at `BalloonSkin::crt()` and 506x53 at `ink_on_paper()`, against 568 and 234 px for a
  wrapped 240-character line. A line of Chinese or Japanese, which has no spaces, was one such word
  however short. A word that does not fit a line of its own is now broken after the last grapheme
  cluster that fits (UAX #14's emergency mode; an accent, a flag or an emoji sequence is never split) and
  the next word continues after its last fragment. A word that fits on a line is never broken, so
  ordinary text wraps exactly as before. The dialog box's body text and choice rows ask Bevy for
  `LineBreak::WordOrCharacter`, which is the same rule.

- A misspelt or unknown key in authored data is a parse error, not a value silently left at its
  default. `Node`, `Bark` and `BarkSettings` now refuse unknown fields (`#[serde(deny_unknown_fields)]`),
  as `Choice`, `Conversation` and `DialogueScript` already did. `Line(.., emtion: Fear, ..)` used to
  load as a Neutral balloon and `knid: Thought` as a speech balloon, `ttl` on a bark and `max_live` in
  bark pacing were dropped without a word, and `validate_script` could not see any of it. The error
  names the key, the type and the spellings it could have been, with the line and column (RON). Scripts
  and barks written with the correct keys, and everything this crate serialises, load as before; a host
  whose files carry an unknown key that used to be ignored now gets an error at load time (#12).

- A `Bark`, or a `Presenter::Balloons` line, whose speaker is not a key of `Speakers` is no longer
  dropped in silence. The crate told a registered narrator (`entity: None`, quiet by design) from a key
  nobody registered with one `None`; it now tells them apart and `warn!`s for the second, naming the
  key and the bark's text, or the line's node and conversation. A bark's warning is once per key (a
  per-key set, bounded at 64 keys), not once per call site, so a second misspelt key still speaks up;
  what is shown is unchanged (#11).

## [0.2.2] - 2026-10-31

### Added

- `Bark` and `BarkSettings` are `serde` types, so a game or a tool authors barks and their pacing as
  data (RON) with no second schema (#9).
- `examples/bench.rs`: what the kit costs its caller, one line per case (raster time and bytes per
  balloon, frame cost at 0/50/200 live balloons and on child owners, the worst frame of a bark burst,
  asset counts after 1000 barks, presses per conversation).
- `examples/two_balloons.rs`: the thinker rides a cart as a child entity, its balloons cast no shadow on
  the floor (`NotShadowCaster`, as the crate's own balloons), and `BUBBLES_SHOT=<file.png>` (with
  `BUBBLES_FRAMES`) renders a still offscreen on a fixed clock and exits.

### Fixed

- A speaker that is a child entity gets its balloon over its world position. `track_bubbles` read the
  owner's local `Transform`, so a balloon on a child floated at its parent-relative offset from the
  world origin; it now composes the owner's and its ancestors' `Transform`s in the frame it tracks, so
  it matches the propagated `GlobalTransform` with no frame of lag. The balloon also hides when its
  owner is hidden through a parent, by Bevy's visibility inheritance, and its `Visibility` is written
  only when it changes, so a steady balloon no longer re-runs visibility propagation every frame (#10).
