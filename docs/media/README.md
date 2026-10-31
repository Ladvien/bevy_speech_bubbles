# Media: every image, with the command that made it

| File | What it shows | Command | Captured | Shows |
|---|---|---|---|---|
| `docs/chatter.gif` | the README's hero: five figures in a ring, balloons appearing and fading while the camera orbits; 7 s, 77 frames | not recorded when it was made; `cargo run --example chatter -- <font.ttf>` shows the same scene live | 2026-08-09 (`ca20c5f`) | 0.1.0 |
| `docs/media/chatter.png` | the README's still: frame 38 of the hero, a spoken line and a thought | `ffmpeg -i docs/chatter.gif -vf "select=eq(n\,38)" -frames:v 1 docs/media/chatter.png` | 2026-10-31 | 0.1.0 (from the GIF) |

`docs/chatter.gif` stays at that path: the READMEs of published releases name it, and a published README never changes. `two_balloons` and `dialog_box` take `BUBBLES_SHOT=<file.png>` for an offscreen still.
