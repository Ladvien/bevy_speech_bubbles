# bevy_speech_bubbles loop log

One section per iteration, appended; never rewrite another pass's entry. The loop command is
`.omp/commands/build-loop.md`; rules and traps are in `CLAUDE.md`.

## Iteration 0 — 2026-10-30 — loop setup

Set up by the owner's session, not by a pass: added `.omp/commands/build-loop.md` (adapted from the
bevy_stigmergy loop command), this log and `CHANGELOG.md`. No code changed.

- **Published:** `bevy_speech_bubbles` 0.2.1 on crates.io (tag `v0.2.1`, 2026-10-24). `main` has one
  package change since: `b2d67f0` "Bark and BarkSettings are serde types" (src/, tests/bark_serde.rs),
  so the first pass that releases carries it. Its `CHANGELOG.md` entry is to be written by that pass
  from `git log v0.2.1..HEAD`.
- **Gate as of setup:** CI (`.github/workflows/ci.yml`, self-hosted big) runs `cargo build`,
  `cargo test`, `cargo build --examples`. Section 7 adds `cargo build --release`, clippy
  `-D warnings`, rustdoc `-D warnings` and rustfmt on changed files. Probe on `b2d67f0`:
  `cargo clippy --all-targets -- -D warnings` passes; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`
  fails on one redundant explicit link target (`src/conversation.rs:19`, `[dwell_secs](crate::dwell_secs)`),
  which the first pass fixes in its own commit; `cargo fmt --check` reports 71 hunks across src/,
  examples/ and tests/, so the gate checks only files a pass changes.
- **No bench, no capture knob yet:** there is no `examples/bench.rs` and no `BUBBLES_SHOT` /
  `BUBBLES_FRAMES` in the examples. All three examples need a GPU, a window and a font path argument
  (`/System/Library/Fonts/Supplemental/Arial.ttf`).
- **Issues:** four open, all unlabelled, all filed from Sanctuary (consumer requests in substance; none
  reports a bug). Ranked oldest first; #7 builds on #6's press count.

### Ranked backlog

1. **#5 Speaker aliases** — two speaker keys (`person:<id>`, `society:<id>`) sharing one bark queue and
   one head. Acceptance: aliased barks on one frame show one balloon at a time, the second after
   `BarkSettings::gap`; anchors at the target's entity; absent target dropped with no panic; unaliased
   keys keep today's stacking (test).
2. **#6 Presses to close, and skip** — `presses_to_close` (`Some(L + 1)` for L boxed lines while the
   first types, `Some(L)` after; `None` before a choice) and a `SkipConversation` message that ends it
   once and keeps speakers registered.
3. **#7 Cost before, record after** — `cost`/`fits` over a script (presses per conversation, an
   `Err` naming an unknown conversation) and a `DialogueFinished` with `nodes_seen` (distinct nodes)
   fired once on every exit.
4. **#8 Unattended delivery** — `Delivery::Unattended`: no `ConversationLock`, `Time<Virtual>` keeps
   running, keys reach the game, the box closes after its reading time; `Delivery::Modal`
   bit-identical to today; separate "is one up?" and "is it modal?" queries.
5. **`examples/bench.rs`** — the section 2 bench: `build_bubble` median/p99 µs for 12/80/240-char lines
   × speech/thought × `crt()`/`ink_on_paper()` × `text_px` 16/32, raster `px` and bytes; headless frame
   cost at 0/50/200 live balloons and the worst frame of a 20-bark burst. Nothing measures the raster
   cost today, and every bark pays it on the frame it lands.
6. **Asset lifetime under barks** — a test that `Assets<Image>` and `Assets<StandardMaterial>` return
   to their starting counts after 1000 expired barks (each balloon allocates one of each; nothing tests
   that `expire_bubbles`' despawn frees them).
7. **Typed script errors for bevy_emerge #106** — `validate_script` returns the first problem as a
   `String`; the editor (#106) must refuse an invalid graph with that message and point at the node. A
   typed error naming conversation and node, reporting every problem rather than the first, with a test
   per case.
8. **Capture knob** — `BUBBLES_SHOT` / `BUBBLES_FRAMES` in `chatter` and `dialog_box` (Bevy
   `Screenshot` after N frames, then exit), so `docs/chatter.gif` and before/after stills can be
   regenerated on big_mac's Metal display.
9. **Pixel-mode legibility check** — a test that an `ink_on_paper()` raster has zero in-between alpha
   values and glyph origins on whole texels, so `pixel: true` cannot silently go soft.
