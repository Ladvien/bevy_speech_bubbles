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
  position", Fixes #10) on `loop/iter-3`, merged as `8046adc`; issue #10 closed by it and commented with
  the evidence. Gate re-run green on the merged tree (only `examples/two_balloons.rs`, +3 lines,
  differed from the gated tree).
- **Release.** `cargo semver-checks check-release`: 202 checks pass, "no semver update required", so
  PATCH: `release: v0.2.2` (`cf8a4a0`), `cargo publish --dry-run` then `cargo publish`, tag `v0.2.2`
  pushed. https://crates.io/crates/bevy_speech_bubbles/0.2.2 (`cargo info` resolves it). Last tag
  `v0.2.1` was 148 h old. It carries `Bark`/`BarkSettings` serde (#9), the bench, the `two_balloons`
  cart and the #10 fix.
- **Cleanup.** Removed `~/b3_base` (a baseline worktree whose build was interrupted at 521 MB; the
  before/after bench and still binaries were built in the clone's own target dir by swapping
  `src/balloon.rs` and `src/conversation.rs` to `2ba57c8` and back) and `/tmp/b3` (380 MB: bench output,
  stills, probe, gate logs, binaries). `df -h` read 29 GiB free before and after (it was 44 GiB at the
  start: the clone's `target/` grew with the no-dev-dependency, release and `semver-checks` builds and
  is kept). **Leftover from an earlier pass, not removed:** `~/b2_base` (1.6 MB, detached at `fadce0c`,
  which is on `loop/iter-2` only; NOT clean: modified `examples/two_balloons.rs` and `src/balloon.rs`,
  untracked `tests/zz_probe.rs`; untouched for over two hours; no `b2_base-target`). A person decides.
- **Host load note for the next pass.** The host ran at load average 12 on 10 cores: a release build
  took 133 minutes, `semver-checks` 39. Start `cargo build --release` early, in the background, with no
  timeout.
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

## Iteration 4 — 2026-10-31 — an unregistered speaker's bark or balloon line warns instead of vanishing (#11)

- **Item and why it ranked first.** Issue triage: open bugs #11 (unknown speaker key dropped without a
  `warn!`) and #12 (`Node`, `Bark`, `BarkSettings` accept unknown fields); enhancements #5, #6, #7, #8; no
  `consumer` label on anything. Both bugs were filed in the same minute; #11 has the lower number and its
  input (a misspelt key in a bark RON file) is the first thing a bevy_emerge author does wrong, so it beat
  #12. Runner-up: #12 (a one-attribute fix, next pass). Skipped, not blocked: #12 (a second bug, one
  improvement per pass), #5-#8 (enhancements rank behind any bug). Parked work: `git branch --list
  'wiggum/parked-*'` is empty; `loop/iter-1` and `loop/iter-2` are older parked passes unrelated to #11, so
  nothing to adopt.
- **What changed.** `speaker_entity` returned one `None` for a registered narrator (`entity: None`, quiet
  by design) and for a key that is not in `Speakers`, so `release_bark` and `present_current` dropped both
  silently. `resolve_speaker` now returns `Resolved::{Head, Narrator, Unknown}`. A narrator is dropped or
  skipped as before; an `Unknown` key logs one `warn!`: for a bark, `bark speaker 'miler' is not in
  Speakers; dropping the bark ("Stuck!")`, once per key through a `Local<WarnedSpeakers>` (bounded at 64
  keys, cleared when full; `warn_once!` is per call site and would hide a second misspelt key); for a
  balloon-mode line, `speaker 'ghost' of node 'a' in 't' is not in Speakers; skipping its line in balloon
  mode`, once per node visit. What is shown is unchanged. Tests (`tests/walk.rs`, a `warned` module that
  records WARN/ERROR through a tracing `Layer` with the schedules on `SingleThreadedExecutor`): unknown bark
  warns once naming key and text, narrator bark quiet, one warning per distinct key, a key registered after
  its warning gets its balloon, a balloon line by `ghost` warns once and ends once with the lock released,
  a narrator's balloon line quiet; unit tests for `resolve_speaker` and the 64-key bound. Before the fix 4
  of the 6 walk tests fail (the two narrator tests pass, as quiet is today's behaviour).
- **Home-still.** `hs status`: scribe 192.168.1.110 (3 converting) and bmb (5), distill idle (Cuda), qdrant
  up, 804 queued. `catalog_recent` (limit 200): SIGGRAPH/Advances slides, psychology and medical papers; no
  dialogue, comics, legibility or text-layout source, so no `Research: corpus` commit. Distill searches for
  the note's three kinds worked.
- **Sources, by stem.** Model: `gameaipro3-ch37-simulating-character-knowledge-phenomena-talk` ("displayed
  as speech bubbles emitting from the characters who speak them"). Shipping game:
  `GameAIPro2_Chapter02_Combat_Dialogue_in_FEAR_The_Illusion_of_Communication` ("If the AI didn't say it,
  it didn't happen"; "he says ... nothing, because he's dead"). Technical:
  `book_metamorphic_testing_review_chen2018` (a diagnostic names "the misconfigured parameter's name/value
  or its location"), `kleppmann_designing_data_intensive_applications` ch. 1 (configuration errors by
  operators are the leading cause of outages), Bevy 0.20.1 `bevy_utils/src/once.rs` ("once per call site").
  Note: `docs/research/speaker-resolution.md`; `corpus.md` gained a "Diagnostics and configuration errors"
  table and the gap (no Yarn Spinner / Ink documentation in the corpus).
- **Critique and verdict** (`/tmp/b4/critique.md`, removed at cleanup; findings below). Unknown-speaker
  warning for barks and balloon-mode lines: **GREAT**: every validation target met by a headless test that
  failed before, the two real message lines observed from a plugin run. Flaws left, all minor: the
  balloon-mode `Choice` arm still says `('ghost' has none)` for an unregistered key (it warned before, with
  the old conflated wording; re-pinning wording in a test is not allowed); a bark's warning prints its whole
  text; once-per-key means a speaker registered, removed and barked for again does not re-warn; the
  64-key set can re-warn an old key. No visual surface changed.
- **Dialog checklist** (`/tmp/b4/findings.md`, probe = pass 7's `zz_probe.rs` copied from `~/b2_base`, run,
  removed): unchanged from iteration 3 except the two lines this pass is about. `ink_on_paper` 0 in-between
  alpha texels, 3 opaque colours; ink padding L/R/T/B 12/14/13/15 (speech) at `ink_on_paper`, 31/35/33/37
  at `crt`; every exit (last line in both presenters, choice into end, missing `DialogSkin`, missing
  `BubbleAssets`, unknown speaker, speaker despawned) writes `ConversationEnded` exactly once with the lock
  released; refusals quoted (`next 'zz' not found`, `start node 'a' not found`, `choice has no options`);
  1000 barks leave Image 0 -> 115 -> 0 and StandardMaterial 0 -> 114 -> 0; paused `Time<Virtual>` still
  expires a bark after 3.0 s real; presses greeting 3/6, gate 3/5. A bark for an unknown speaker: 0
  balloons, 0 warnings -> 0 balloons, 1 warning.
- **Bugs found, not fixed** (still open; rank ahead of enhancements): a 60-character unbreakable word
  makes the balloon 1269x136 px at `crt` and 506x53 at `ink_on_paper`, wider than the 568 / 234 px a
  wrapped 240-character line reaches (text stays inside the cap; carried from iteration 3); the speech tail
  tip sits 6 texels left of the balloon centre at `crt` (thought: 8.5) and 3 at `ink_on_paper` (thought: 4).
- **Bench, five runs each side, before -> after** (`/tmp/b4/{before,after}/bench_N.txt`; the host ran at
  load average 8-15 on 10 cores, and one `after` run stalled with a 97 ms median frame at zero balloons, so
  the first three each side plus two alternating reruns are all kept, none dropped). Medians of five, then
  best of five. `frame-live` 0 balloons 197.0 -> 211.7 us (best 193.1 -> 197.0), 50: 223.3 -> 203.4 (199.0
  -> 196.3), 200: 210.8 -> 209.8 (200.0 -> 207.7), 200 child owners 219.0 -> 220.5 (216.0 -> 212.8);
  20-bark burst median 211.6 -> 204.1 us, worst frame 127.8 -> 209.1 ms (best 119.5 -> 16.2 ms: the same
  binary swings 16-372 ms run to run). The raster rows (code this pass did not touch) moved -0.1 % to
  -47 % at the median and -65 % to +6 % at best of five: host noise. Raster size and bytes per balloon are
  identical (e.g. 240 chars crt text_px 16 speech 236x199, 446580 bytes), the 1000-bark asset counts
  return to 0, presses unchanged. The one hot path this pass changed, `release_bark`, returns before
  `resolve_speaker` when no bark is ready and otherwise pays the one HashMap lookup it paid before, so no
  regression is attributable to the change; the bench cannot resolve a change this small on this host.
- **Gate** (`/tmp/b4/gate.log`): `cargo build`, `cargo build --release`, `cargo test` (42 unit, 2 serde,
  2 leaf, 19 walk, 0 failed), `cargo build --examples`, `cargo clippy --all-targets -- -D warnings`,
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`: all green; `rustfmt --edition 2024 --check
  src/conversation.rs` clean, and `tests/walk.rs` clean in the region this pass added (its other hunks were
  there before and stay).
- **Commits** on `loop/iter-4`: `b3e0dd3` Research: speaker resolution; `e6966ee` the fix (Fixes #11);
  `412d91e` README, CHANGELOG and CLAUDE.md; `bac02fe` rustfmt of the new tests; this log. Issue #11 closed
  by the merge with the evidence commented.
- **Release.** None: the last tag `v0.2.2` is under an hour old (the 6-hour rule), so the `CHANGELOG`
  `[Unreleased]` entry rides in the next release.
- **Cleanup.** No baseline worktree this pass (the baseline bench binary came from the start commit's own
  release build, kept in `/tmp/b4/before/`). Removed `/tmp/b4` (bench outputs, binaries, probe, gate log).
  Throwaway `tests/zz_probe.rs` and `tests/zz_msgs.rs` were removed from the tree. `df -h` free 29 GiB at
  the start, 27 GiB after the release builds (the clone's `target/` grew and is kept). **Leftover from an
  earlier pass, not removed:** `~/b2_base` (1.6 MB, detached at `fadce0c`, on `loop/iter-2` only; NOT clean:
  modified `examples/two_balloons.rs` and `src/balloon.rs`, untracked `tests/zz_probe.rs`; its files are
  older than two hours; no `b2_base-target`). A person decides.
- **Host note.** A full `cargo build --release --examples` took 15.5 min on the loaded host (7.4 min the
  second time); the debug gate takes about 5 minutes once cached.
- **Pending index.** `10.48550_arxiv.2306.17469` and `10.3390_jimaging4070087` still await conversion
  (carried from `balloon-anchoring.md`); the new note's sources are all indexed.

### Ranked backlog for the next iteration

1. **#12 unknown fields accepted** on `Node`, `Bark`, `BarkSettings` (bug, filed; bevy_emerge's hot-reloaded
   RON): `#[serde(deny_unknown_fields)]` on the three, with the issue's four `is_err()` tests.
2. **An unbreakable word widens the balloon past the skin's maximum** (bug found in iteration 3, numbers
   above): break the word at the cap, or refuse it, with a test at 60 `W`.
3. **#5 speaker aliases**, with `loop/iter-1`'s `tests/alias.rs` and `Research: barks` as the first draft
   (read it, bring it by path, finish and gate it); then #6, #7, #8 in order.
4. **Raster cost on the caller's frame**: a 240-character `crt()` line is 7-30 ms at `text_px` 16 and the
   worst frame of a 20-bark burst is far over 16.7 ms; profile `rasterize` (glyph draw and `blend`), then
   cache or move it off the frame.
5. **Small ones**: the balloon-mode `Choice` arm's "has none" wording for an unregistered key (use
   `Resolved`), the speech tail tip's 3-6 texel offset from the anchor column, `BUBBLES_SHOT` in `chatter`
   and `dialog_box`, and a permanent test that 1000 expired barks return the asset counts.
