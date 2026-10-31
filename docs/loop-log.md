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

## Iteration 3 — 2026-10-31 — a speaker that is a child entity gets its balloon over its world position (#10)

Numbering: iterations 1 and 2 were passes killed before they logged; their work sits on `loop/iter-1`
(pass 1, parked) and `loop/iter-2` (pass 7, parked) and neither reached `main`. This pass is 3.

- **Item and why it ranked first.** Issue triage: open bugs #10 (track_bubbles reads the owner's local
  `Transform`/own `Visibility`), #11 (unknown speaker dropped without `warn!`), #12 (`Node`/`Bark`/
  `BarkSettings` accept unknown fields); no `consumer` label on anything; enhancements #5, #6, #7, #8.
  #10 is the oldest bug and its input is the one a bevy_emerge person under a placed piece hits first, so
  it beat #11 and #12. Skipped, not blocked: #11, #12 (newer bugs), #5-#8 (enhancements rank behind any
  bug). Runner-up: #11.
- **Adopted parked work.** Pass 7's `loop/iter-2` already held a researched, tested fix for #10. Its real
  commits (not the driver's WIP snapshots) were cherry-picked onto `loop/iter-3`: the docs-link fix, the
  two research notes, `examples/bench.rs`, the `two_balloons` cart and `BUBBLES_SHOT`, the fix, the rustfmt
  of its four files; the parked CHANGELOG and README were brought over by path. Its `tests/zz_probe.rs`
  (a throwaway checklist probe) was run from `/tmp` and not committed. Then this pass built, measured,
  gated, fixed one example defect (below) and shipped. `loop/iter-1` (pass 1: `examples/bench.rs`
  draft, `tests/alias.rs`, `Research: barks`) is untouched: its `tests/alias.rs` is the head start for #5.
- **What changed.** `track_bubbles` composes the owner's and its ancestors' `Transform`s in the frame it
  runs (no `PostUpdate` lag, root owner costs one absent-`ChildOf` check), and mirrors Bevy's visibility
  inheritance over the same chain, writing the balloon's `Visibility` only when it changes. Tests in
  `src/balloon.rs`: child owner at x=10, nested rotated/scaled chain equal to the propagated
  `GlobalTransform` to 1e-4 (also on the frame the grandparent moves), `InheritedVisibility` with a hidden
  parent. New: `examples/bench.rs`, `two_balloons` cart + `BUBBLES_SHOT` (offscreen, fixed clock). This
  pass fixed `two_balloons` spawning balloon quads without `NotShadowCaster` (both stills showed a black
  floor-shadow bar per balloon).
- **Home-still.** `hs status`: scribe 192.168.1.110 idle and bmb 4 converting, distill idle (Cuda), qdrant up;
  Pending index below. `catalog_recent` (limit 200): all SIGGRAPH/Advances-in-Real-Time-Rendering and
  medical documents; no dialogue, comics, legibility or text-layout source, so no `Research: corpus`
  commit.
- **Sources, by stem.** Craft: `10.48550_arxiv.2306.17469` (Manga109Dialog), `10.1145_3361218`
  (Sidenmark & Gellersen). Shipping game: `GameAIPro2_Chapter02_Combat_Dialogue_in_FEAR_The_Illusion_of_Communication`,
  `gameaipro3-ch37-simulating-character-knowledge-phenomena-talk`. Technical: `Game Engine Architecture,
  Third Edition` (§5.3.10, §12.3.2), `book_real_time_rendering_4th_akenine` (ch. 19), Bevy 0.20 source
  (`bevy_transform` helper and propagation, `bevy_camera` visibility). Note: `docs/research/balloon-anchoring.md`.
- **Critique and verdict** (written before the verdict in `/tmp/b3/critique.md`, removed at cleanup;
  its findings are in this entry).
  Child-entity anchoring: **GREAT**. Visibility through a parent: **GREAT**. `examples/bench.rs`:
  **GREAT** (every case the loop asks for, one `bubbles:` line each, fixed 16 ms clock). `two_balloons`
  cart and capture knob: **GREAT** (still after: thought balloon over the thinker on the cart, tail
  under it; before: the same balloon floats at the world origin on top of the speech balloon). Known
  limits, all documented in the note: a `ChildOf` cycle loops (as Bevy's `TransformHelper` does), a
  `Transform`-less ancestor ends the composition, `anchor_y` is world units (the host's).
- **Dialog checklist, before -> after** (`/tmp/b3/{before,after}/probe*.txt`, pass 7's probe):
  anchor, issue input: balloon x 0.004 -> 10.000 (owner at 10.0); parent moved to x=20 and one update:
  0.004 -> 19.996; hidden parent: balloon `Inherited` (shows) -> `Hidden`; root owner unchanged
  (3.0028, 3.5942, 0.39999). Unchanged by this pass: `ink_on_paper` 0 in-between alpha texels and 3
  opaque colours; ink padding inside the cap L12 R14 T13 B15 px (speech) at `ink_on_paper`, L31 R35 T33
  B37 at `crt`; every exit (last line in both presenters, choice into end, missing `DialogSkin`, missing
  `BubbleAssets`, unknown speaker, speaker despawned) writes `ConversationEnded` exactly once with the
  lock released; refusals quoted (`next 'zz' not found`, `start node 'a' not found`, `choice has no
  options`); 1000 barks leave Image 0 -> peak 115 -> 0 and StandardMaterial 0 -> peak 114 -> 0; with
  `Time<Virtual>` paused a bark still expires after 3.0 s of real time; presses: greeting 3
  (balloons) / 6 (box), gate 3 / 5, a one-line box 1 press and the host sees 0 of them.
- **Bugs found, not fixed** (rank ahead of enhancements next pass): a single unbreakable 60-character
  word grows the balloon to 1269x136 px (`crt`) and 506x53 (`ink_on_paper`), past the 568 / 234 px of a
  wrapped 240-character line (text stays inside the cap, but the balloon exceeds the skin's width); the
  speech tail tip sits 3 texels left of the balloon centre at `ink_on_paper` (design offset, unmeasured
  against the anchor column).
- **Bench, medians of three runs, before -> after** (`/tmp/b3/bench_compare.txt`; the host was at load
  average 12 on 10 cores, so identical code swings +-40 % run to run and the raster rows, whose code did
  not change, move in both directions: crt speech 80 chars 1.55 -> 2.27 ms, thought 80 chars 7.9 -> 5.5 ms).
  The code this pass changed is `track_bubbles`, exercised by the frame rows: `frame-live` 0 balloons
  249 -> 282 us (inside the 182-306 us spread of the before runs), 50 balloons 307 -> 244 us, 200
  balloons 230.3 -> 231.0 us (best of three 225.5 -> 230.3), 200 child owners 307 -> 252 us (best of three
  303 -> 240 us); 20-bark burst worst frame 91.0 -> 138.7 ms (best of three 78.6 -> 88.0 ms; the burst is
  rasterization, which this pass did not touch). No regression is attributable to the change; the
  per-balloon walk costs under 0.1 us at the bench's resolution. Raw numbers worth acting on: a
  240-character `crt()` line at `text_px` 16 rasterizes in 17-30 ms and at `text_px` 32 in 54-100 ms.
- **Gate** (`/tmp/b3/gate.log`): `cargo build`, `cargo build --release` (133 min on the loaded host),
  `cargo test` (40 unit, 2 serde, 2 leaf, 13 walk, 0 failed), `cargo build --examples`, `cargo clippy
  --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` and `rustfmt
  --edition 2024 --check` on the four changed `.rs` files: all green.
- **Capture note.** The first offscreen `BUBBLES_SHOT` run on a cold Metal shader cache takes minutes (the
  render thread sits in `MTLCompilerScheduler` compiling pipelines; 5 min 18 s measured on the loaded
  host); the next run, another build of the same example, captured in 26 s. Use a long timeout, never
  `timeout 100`.
- **Commits.** Fix `edf9f4f` ("A speaker that is a child entity gets its balloon over its world
  position", Fixes #10) on `loop/iter-3`; release recorded by the follow-up commit below.
- **Pending index.** `10.48550_arxiv.2306.17469` and `10.3390_jimaging4070087` await conversion (scribe
  queue), carried over from `balloon-anchoring.md`.

### Ranked backlog for the next iteration

1. **#11 unknown speaker key dropped without a `warn!`** (bug, filed): `speaker_entity` returns the same
   `None` for a narrator and for an unregistered key; barks and balloon-mode lines must warn on the
   latter only.
2. **#12 unknown fields accepted** on `Node`, `Bark`, `BarkSettings` (bug, filed; bevy_emerge's
   hot-reloaded RON): a misspelt `emotion` or `kind` silently becomes the default.
3. **An unbreakable word widens the balloon past the skin's maximum** (bug found this pass, numbers
   above): break the word at the cap, or refuse it, with a test at 60 `W`.
4. **#5 speaker aliases**, with `loop/iter-1`'s `tests/alias.rs` and `Research: barks` as the first draft
   (read it, bring it by path, finish and gate it); then #6, #7, #8 in order.
5. **Raster cost on the caller's frame**: a 240-character line is 17-30 ms at `text_px` 16, and the
   worst frame of a 20-bark burst is 79-217 ms against a 16.7 ms budget; cost per pixel grows with line
   length (30 ns/px at 12 chars, 111 at 80, 355 at 240), so profile `rasterize` (glyph draw and `blend`)
   first, then cache or move it off the frame. Earlier backlog items 5 (bench) and 8 (capture knob in
   `two_balloons`) are done; `chatter` and `dialog_box` still lack `BUBBLES_SHOT`, and nothing asserts
   the 1000-bark asset return as a test.
