# The examples

Moved from the README on 2026-10-31, unchanged, when the README was brought to the [org documentation guide](https://github.com/bevy-coven/.github/blob/main/DOCUMENTATION.md).

```sh
cargo run --example two_balloons -- /System/Library/Fonts/Supplemental/Arial.ttf
cargo run --example chatter      -- /System/Library/Fonts/Supplemental/Arial.ttf
cargo run --example dialog_box   -- /System/Library/Fonts/Supplemental/Arial.ttf
cargo run --release --example bench -- tests/data/FiraMono-subset.ttf
```

`two_balloons` is the smallest thing that works; its thinker rides a cart as a child entity, and `BUBBLES_SHOT=<file.png>` (after `BUBBLES_FRAMES`, default 90) renders a still offscreen and exits. `chatter` is the gif above. `dialog_box` runs a four-node script with a choice in the screen box, skinned with flat colours generated in code (`F` cycles the typewriter speed); `BUBBLES_CONVERSATION=wrap` starts a conversation whose words are longer than the box, and `BUBBLES_SHOT=<file.png>` (after `BUBBLES_FRAMES`, default 30) renders a still of it offscreen with every line shown at once. All three need a GPU and take the font path as a required argument. `bench` is headless, takes a font path too (the test font keeps its numbers independent of the host's fonts), and prints what the kit costs its caller, one line per case.
