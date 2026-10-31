# Balloons: scale, pixels, the camera and the font

Moved from the README on 2026-10-31, unchanged, when the README was brought to the [org documentation guide](https://github.com/bevy-coven/.github/blob/main/DOCUMENTATION.md).

## Scale and pixel balloons

Balloon size is the host's call. `BalloonScale { world_per_px, anchor_y }` (inserted with the default by the plugin; `track_bubbles` reads it every frame) sets world units per raster texel and the float height, so a world in feet and one in metres both fit, and changing it as the camera zooms resizes every live balloon. Spawn your own balloon quads with `BubblePx(rendered.px)` so `track_bubbles` can size them. `BalloonSkin::text_px` is the glyph size in raster px, and padding, tail, border and corner radius scale with it: `BalloonSkin::ink_on_paper()` has `text_px: 16` with `pixel: true`, which rasterizes at the native size of a 16 px pixel font (one texel per font pixel, a one-texel border, glyphs on whole texels, nearest sampler), roughly 150 to 200 texels wide for a 20-character line.

## Name your camera

`track_bubbles` and the plugin are generic over a camera **marker component**, deliberately. Bevy's `Single<..>` silently skips its system when the query does not match exactly one entity, so a tracking system filtered on `With<Camera3d>` works until something spawns a second 3D camera (a render-to-texture pass, a portal, a minimap), and then every balloon quietly stops tracking and nothing reports an error. Being generic makes it impossible for this crate to write that filter for you.

## The font is yours to load

`BubbleAssets` holds a quad mesh and a `FontArc`, and the dialog box takes `TextFont`s in its skin; this crate never reads a file. `BubbleAssets::from_font_bytes(quad, bytes)` builds the resource from raw font bytes, and `FontArc` is re-exported, so you need no `ab_glyph` dependency. A library that hardcoded `assets/fonts/…` would be hardcoding your project layout.

## Why the balloon text is rasterized here

Bevy's text stack has no public "rasterize this string into a standalone `Image`" API, and a world-space balloon has no 2D camera to host a `Text2d`. So glyphs go through `ab_glyph` into an RGBA buffer, once per line, and the buffer becomes a texture. The screen dialog box needs none of that and uses ordinary `Text`.
