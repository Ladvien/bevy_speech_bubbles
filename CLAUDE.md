# bevy_speech_bubbles — notes for agents

The dialog kit: world-space speech and thought balloons (text rasterized on the CPU into an `Image`, on a billboarded quad), a conversation graph and runtime (lines, choices, barks), and a skinnable screen dialog box built from Bevy UI nodes.

## Force pushes and deletions need the user's express permission

Never do any of the following unless the user has approved that specific action, by name, in the current conversation. A loop brief, a skill, a standing instruction such as "commit, push, PR, merge", or an earlier session's approval is not permission.

- Force-push in any form: `git push --force`, `--force-with-lease`, `--force-if-includes`, a `+` refspec, or pushing rewritten history. Deleting a remote branch or tag (`git push --delete`, `gh pr merge --delete-branch`, an API delete).
- Throwing away commits or uncommitted work: `git reset --hard`, `--merge` or `--keep`; `git checkout -- <path>` or `git restore` over changes you did not make; `git clean`; `git stash drop` or `clear`; `git branch -D`; `git worktree remove --force`; `git rebase` or `git commit --amend` on pushed commits; `git filter-repo` or `filter-branch`.
- Deleting files, directories, build caches, target dirs, models, datasets, database rows, issues, PRs, releases or cloud resources that you did not create in this session (`rm`, `rm -rf`, `git rm`, `cargo clean`, an API delete).

When one of these seems necessary, stop and ask. Name the exact command, what it destroys and whose work it touches. A commit that landed on the wrong branch is fixed with a new commit or a question, never by resetting a shared working tree or force-pushing. Files you created yourself in this session (scratch scripts, temp dirs, your own worktrees) you may delete.

## Source of truth

This repository, [`Ladvien/bevy_speech_bubbles`](https://github.com/Ladvien/bevy_speech_bubbles), is canonical. MIT OR Apache-2.0. It co-evolves with Sanctuary (`/Users/ladvien/sanctuary`, a path dependency): fixes and features land here first, in their own commit with tests here, and are never patched into the game. Run `cargo test` here before pushing and in Sanctuary before relying on the change.

## Layout

- `src/lib.rs` — crate docs and re-exports.
- `src/balloon.rs` — the rasterizer, `BalloonSkin`, `BubbleStyle`, `build_bubble`, `track_bubbles::<C>`, `expire_bubbles`, `dwell_secs`.
- `src/style.rs` — `BubbleKind`, `Emotion`.
- `src/conversation.rs` — the model (`DialogueScript`, `Conversation`, `Node`, `Choice`, `validate_script`), `Speakers`, `Presenter`, the messages, `Active` (private), the balloon presenter, `Bark`, `BarkQueue`, `BarkSettings`.
- `src/dialog_box.rs` — `DialogSkin`, the screen box, typewriter, caret, choices.
- `src/plugin.rs` — `SpeechBubblesPlugin<C>`.
- `tests/leaf.rs` — the boundary ratchet. `tests/walk.rs` — the headless walk through the plugin. `tests/data/` — a test font.

## Build and test

`cargo test`, `cargo build --examples`, `cargo clippy --all-targets`. CI also runs `cargo build --release`, which takes no dev-dependencies and so compiles exactly the features a consumer gets: the full `bevy` dev-dependency would hide a hole in the trimmed feature list. Examples need a font path argument: `cargo run --example dialog_box -- /System/Library/Fonts/Supplemental/Arial.ttf`.

## The non-negotiables

- **Never name a game's camera.** `tests/leaf.rs` scans the sources and rejects `MainCamera`, `SquadMember` and `MenuState` alongside `avian`, `emerge` and `foundation_vs_slop`. This crate once named a camera marker from the game it was extracted from, and the fix (generic over the marker: `track_bubbles::<C>`, `SpeechBubblesPlugin::<C>`) is what stops it silently breaking in a project with a second 3D camera. Naming one again would compile.
- **One plugin, generic over the camera marker, no game knowledge.** Speakers are strings the host resolves through `Speakers`; skins, fonts and scripts are handed in. The crate registers messages and resources and schedules its systems: the runtime in `PreUpdate` after `InputSystems` inside the public `DialogInputSet` (it consumes the presses it uses with `clear_just_pressed`, only when a conversation actually used them), the cosmetic systems in `Update`; the host keeps everything else.
- **The caller supplies the font.** `BubbleAssets` holds a `FontArc` and the crate never reads a file; the dialog box takes `TextFont`s in `DialogSkin`. A library that hardcoded `assets/fonts/…` would be hardcoding the game's layout.
- **No `unwrap()`, no `expect()` outside tests,** including font loading and image allocation. A misconfigured host (no `DialogSkin`, no `BubbleAssets`, an unknown speaker) gets a loud `error!`/`warn!` and, for a conversation, a clean `ConversationEnded`, never a panic or a held lock.
- **One path per feature.** No fallbacks, no legacy shims, no stub placeholders. A pick from the pointer, the number keys, the arrows or a gamepad is one `ChoicePicked` message resolved in one place.

## Design facts worth keeping

- **CPU raster, not a text stack, for balloons.** Bevy's text pipeline has no public "rasterize a string to a standalone `Image`" API, and a world-space balloon has no 2D camera to host `Text2d`. `ab_glyph` is pure Rust. `BalloonSkin { pixel: true }` thresholds the shape SDF and the glyph coverage at 0.5 and gives the image a nearest sampler.
- **Balloon size is the host's: `BalloonScale` + `BubblePx`.** `track_bubbles` sets the quad scale to `px * world_per_px` every frame and anchors at `anchor_y`; `build_bubble` returns the raster size (`px`) and `size` is only the default-scale starting value. Never bake a world size into anything `track_bubbles` does not recompute (choice stacking offsets are `BubbleOffsetPx`, in texels). `BalloonSkin::text_px` scales padding, tail, border and radius (`Metrics` in `balloon.rs`); pixel mode rounds them to whole texels and puts glyphs on whole texels.
- **The dialog box is UI, not raster.** It uses `Text` with a transparent `TextSpan` for the unrevealed tail so the layout is that of the finished line from the first frame. The typewriter reads `DialogSkin.chars_per_second` every tick, so a settings menu changes it live. One box lives for the whole conversation and pops on each new node.
- **Everything runs on `Time<Real>`** so balloons expire and the typewriter types while the host has paused virtual time on `ConversationLock`.
- **A `Bark` is always a world balloon** whatever `Presenter` says; a speaker without an `entity` has no head to float over, so its bark is dropped and its balloon-mode lines are skipped.

## Rules

- **Bevy 0.20 is pinned.** Read the vendored source (`~/.cargo/registry/src/index.crates.io-*/bevy-0.20.0/`, its `examples/` and `_release-content/migration-guides/`), not bevy.org — that documents `main` and has been wrong for a pin more than once. Pointer events are flat in 0.20 (`On<PointerClick>`, not `On<Pointer<Click>>`).
- **A missing `Res<T>` panics its system.** Take `Option<Res<T>>`, or have the plugin `init_resource` it. This bites hardest for `BubbleAssets` and `DialogSkin`, which are caller-supplied.
- **All run conditions are evaluated — there is no short-circuit.** A bare `Res<T>` in a `.run_if(..)` closure panics whenever that resource is absent.
- **`Single<..>` silently skips its system on a non-unique match** — which is exactly why the camera is a type parameter rather than `With<Camera3d>`.
- In tests, `ButtonInput::clear()` drops `just_pressed` only: also `release` a key before pressing it again.
