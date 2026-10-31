# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows semantic versioning
(pre-1.0: a breaking change bumps the minor version).

## [Unreleased]

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
