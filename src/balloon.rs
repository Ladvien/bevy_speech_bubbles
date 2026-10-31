//! World-space balloons: the CPU rasterizer, the quad tracking and expiry.
//!
//! A bubble is text rasterized into an `Image`, put on a unit quad as an unlit alpha-blended material,
//! anchored above an owner entity and turned to face a camera. See the crate docs for why.

use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont, point};
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::time::Real;
use unicode_segmentation::UnicodeSegmentation;

use crate::style::{BubbleKind, Emotion};

// --- rasterization tuning (pixels), at the reference glyph size `CAP_PX` ---
// Every length below is scaled by `skin.text_px / CAP_PX` (and, in pixel mode, rounded to whole texels,
// at least one): see `Metrics`. At the `crt()` skin's `text_px` of 40 they are used as written.
/// The reference glyph size the constants below are written for, and the `crt()` skin's `text_px`.
const CAP_PX: f32 = 40.0;
/// Padding between text and the balloon edge.
const PAD: f32 = 28.0;
/// Transparent margin around the balloon for the border's anti-aliased outer edge.
const MARGIN: f32 = 6.0;
/// Wrap width for the text block.
const MAX_TEXT_W: f32 = 520.0;
/// Minimum text-block width so a one-word bubble isn't a sliver.
const MIN_TEXT_W: f32 = 70.0;
/// Vertical space reserved below the balloon for the tail.
const TAIL_H: f32 = 28.0;
/// Balloon border thickness.
const BORDER: f32 = 3.5;
/// Corner radius for speech balloons.
const SPEECH_RADIUS: f32 = 18.0;

// --- world sizing ---
/// Default world units per rasterized pixel (a ~570 px balloon ≈ 2.7 units). See [`BalloonScale`].
const WORLD_PER_PX: f32 = 1.0 / 210.0;
/// Default height above the owner's origin where the bubble's tail-tip sits (health bar is at Y=2.0).
/// See [`BalloonScale`].
pub const BUBBLE_ANCHOR_Y: f32 = 2.5;
/// Nudge toward the camera so knee-walls / geometry don't clip the bubble (see `impact_fx.rs`).
const CAMERA_NUDGE: f32 = 0.4;

/// Reading-speed dwell model (Brysbaert 2019, DOI 10.1016/j.jml.2019.104047): ~238 wpm silent mean;
/// we use 200 wpm for split game attention, floored so short lines don't flash.
const READING_WPM: f32 = 200.0;
const DWELL_FLOOR: f32 = 1.8;

/// Seconds a bubble should linger for its text length.
pub fn dwell_secs(text: &str) -> f32 {
    let words = text.split_whitespace().count().max(1) as f32;
    (words / (READING_WPM / 60.0)).max(DWELL_FLOOR)
}

/// A bubble entity's link to the unit it floats over, plus an in-plane offset (camera right/up) used
/// to stack a choice menu without overlap. Zero offset for a normal single bubble.
#[derive(Component)]
pub struct Bubble {
    pub owner: Entity,
    pub offset: Vec2,
}

/// The balloon's raster size in texels. [`track_bubbles`] sets the quad's `Transform.scale` to
/// `px * BalloonScale::world_per_px` every frame, so a host that changes the scale as its camera
/// zooms resizes every live balloon. A bubble without this component keeps the scale it was spawned
/// with.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BubblePx(pub UVec2);

/// An in-plane offset (camera right/up) in texels, for stacking a choice menu. When present it
/// replaces [`Bubble::offset`] and is converted to world units by [`BalloonScale`] every frame.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct BubbleOffsetPx(pub Vec2);

/// How big balloons are in the host's world and how high they float. The host owns the units: a world
/// in feet and one in metres want different values, and a zooming camera changes `world_per_px`.
/// [`track_bubbles`] reads it every frame; [`SpeechBubblesPlugin`] inserts the default.
///
/// [`SpeechBubblesPlugin`]: crate::SpeechBubblesPlugin
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct BalloonScale {
    /// World units per raster texel.
    pub world_per_px: f32,
    /// Height above the owner's world position (its `GlobalTransform` translation, composed from its
    /// own and its ancestors' `Transform`s in the frame it is tracked) where the balloon's bottom edge
    /// sits.
    pub anchor_y: f32,
}

impl Default for BalloonScale {
    fn default() -> Self {
        Self {
            world_per_px: WORLD_PER_PX,
            anchor_y: BUBBLE_ANCHOR_Y,
        }
    }
}

/// Auto-expiry for ambient (non-modal) bubbles.
#[derive(Component)]
pub struct BubbleTtl {
    pub expires_at: f32,
}

/// Shared render assets: the unit quad mesh and the loaded font.
#[derive(Resource)]
pub struct BubbleAssets {
    pub quad: Handle<Mesh>,
    pub font: FontArc,
}

impl BubbleAssets {
    /// Build the assets from raw font bytes (a `.ttf` or `.otf`), so a host need not depend on
    /// `ab_glyph`. The error string names why the bytes are not a usable font.
    pub fn from_font_bytes(quad: Handle<Mesh>, bytes: Vec<u8>) -> Result<Self, String> {
        let font = FontArc::try_from_vec(bytes).map_err(|e| format!("not a usable font: {e}"))?;
        Ok(Self { quad, font })
    }
}

/// The colours a balloon is drawn with, and whether its edges are anti-aliased.
///
/// The hosts own the look: `crt()` is the phosphor palette this crate always had, `ink_on_paper()` is
/// the pixel-art parchment of Sanctuary. [`Emotion`] still tints `border` (and `thought_border`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BalloonSkin {
    /// Fill of a [`BubbleKind::Speech`] balloon, RGBA.
    pub fill: [u8; 4],
    /// Border of a [`BubbleKind::Speech`] balloon, RGBA, before the emotion tint.
    pub border: [u8; 4],
    /// Glyph colour, RGBA.
    pub text: [u8; 4],
    /// `true`: no anti-aliasing anywhere (the shape's SDF and the glyph coverage are thresholded at
    /// 0.5, so every texel is fully opaque or fully clear) and the image gets a nearest sampler.
    pub pixel: bool,
    /// Fill of a [`BubbleKind::Thought`] balloon, RGBA.
    pub thought_fill: [u8; 4],
    /// Border of a [`BubbleKind::Thought`] balloon, RGBA, before the emotion tint.
    pub thought_border: [u8; 4],
    /// Glyph size in raster pixels (the font's em). Padding, tail, border and corner radius scale with
    /// it, so a pixel skin with the font's native size is one texel per font pixel. In pixel mode the
    /// scaled lengths are rounded to whole texels, at least one.
    pub text_px: f32,
}

impl BalloonSkin {
    /// The CRT phosphor palette (green speech, blue thought), anti-aliased.
    pub const fn crt() -> Self {
        Self {
            fill: [8, 20, 12, 240],
            border: [140, 255, 158, 255],
            text: [226, 240, 226, 255],
            pixel: false,
            thought_fill: [8, 16, 22, 232],
            thought_border: [150, 210, 255, 240],
            text_px: CAP_PX,
        }
    }

    /// Aged parchment, dark ink, hard pixel edges: the pixel-art look. Speech and thought share the
    /// palette; the shape alone tells them apart. `text_px` is 16, the native size of a 16 px pixel
    /// font such as m6x11.
    pub const fn ink_on_paper() -> Self {
        Self {
            fill: [0xe8, 0xd9, 0xb0, 0xff],
            border: [0x3a, 0x2a, 0x1e, 0xff],
            text: [0x2a, 0x1c, 0x12, 0xff],
            pixel: true,
            thought_fill: [0xe8, 0xd9, 0xb0, 0xff],
            thought_border: [0x3a, 0x2a, 0x1e, 0xff],
            text_px: 16.0,
        }
    }
}

impl Default for BalloonSkin {
    fn default() -> Self {
        Self::crt()
    }
}

/// The skin the conversation runtime draws its balloons and barks with. [`SpeechBubblesPlugin`]
/// inserts the `crt()` default; a host `insert_resource`s its own.
///
/// [`SpeechBubblesPlugin`]: crate::SpeechBubblesPlugin
#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub struct DefaultBalloonSkin(pub BalloonSkin);

/// How a bubble should be drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BubbleStyle {
    pub kind: BubbleKind,
    pub emotion: Emotion,
    /// Whether to draw a tail toward the owner (lines/barks: yes; choice menu items: no).
    pub tail: bool,
    pub skin: BalloonSkin,
}

/// A freshly built bubble face: the material to put on the quad, its raster size and its world size
/// at the default [`BalloonScale`].
///
/// Spawn the quad with [`BubblePx`]`(px)`: [`track_bubbles`] then sizes it from the live
/// [`BalloonScale`] every frame, and `size` is only the starting `Transform.scale`.
pub struct RenderedBubble {
    pub material: Handle<StandardMaterial>,
    /// Image size in texels.
    pub px: UVec2,
    /// `px` at the default `world_per_px`; a host with its own [`BalloonScale`] ignores it.
    pub size: Vec2,
}

/// Build a bubble's material + raster size from its text and style. Called once per line/choice (on
/// change), not per frame.
pub fn build_bubble(
    assets: &BubbleAssets,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    style: &BubbleStyle,
    text: &str,
) -> RenderedBubble {
    let (rgba, w, h) = rasterize(&assets.font, style, text);
    let mut image = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    if style.skin.pixel {
        image.sampler = ImageSampler::nearest();
    }
    let image = images.add(image);
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(image),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });
    RenderedBubble {
        material,
        px: UVec2::new(w, h),
        size: Vec2::new(w as f32 * WORLD_PER_PX, h as f32 * WORLD_PER_PX),
    }
}

/// What [`track_bubbles`] reads and writes on a balloon.
type TrackedBubble = (
    Entity,
    &'static Bubble,
    Option<&'static BubblePx>,
    Option<&'static BubbleOffsetPx>,
    &'static mut Transform,
    &'static mut Visibility,
);

/// What [`track_bubbles`] reads on an owner and on each of its ancestors.
type Anchoring = (
    Option<&'static Transform>,
    Option<&'static Visibility>,
    Option<&'static ChildOf>,
);

/// Where `owner` is in the world and whether it is visible, from this frame's `Transform`s and
/// `Visibility`s of the owner and its ancestors: the translation of the `GlobalTransform` and the
/// value of the `InheritedVisibility` Bevy will propagate for it in `PostUpdate`. `None` when the
/// owner has no `Transform` or no `Visibility`.
///
/// The walk follows `ChildOf` to the root. An ancestor without a `Transform` ends the composition (Bevy
/// propagates no world position through it either); one without a `Visibility` counts as visible, as
/// in Bevy's visibility propagation.
fn owner_in_world(
    owner: Entity,
    chain: &Query<Anchoring, Without<Bubble>>,
) -> Option<(Vec3, bool)> {
    let (Some(tf), Some(vis), mut parent) = chain.get(owner).ok()? else {
        return None;
    };
    let mut point = tf.translation;
    let mut composing = true;
    // `Inherited` defers to the parent; the first `Visible` or `Hidden` up the chain decides.
    let mut visible = decides(Some(vis));
    while let Some(child_of) = parent {
        if !composing && visible.is_some() {
            break;
        }
        let Ok((tf, vis, next)) = chain.get(child_of.parent()) else {
            break;
        };
        match tf {
            Some(tf) if composing => point = tf.transform_point(point),
            _ => composing = false,
        }
        if visible.is_none() {
            visible = decides(vis);
        }
        parent = next;
    }
    // A root that inherits is visible.
    Some((point, visible.unwrap_or(true)))
}

/// What one link of the chain says about visibility: `None` defers to its parent.
fn decides(vis: Option<&Visibility>) -> Option<bool> {
    match vis {
        Some(Visibility::Inherited) => None,
        Some(Visibility::Visible) | None => Some(true),
        Some(Visibility::Hidden) => Some(false),
    }
}

/// Track every bubble to its owner: size the quad, anchor above the head, face the camera, mirror
/// visibility, despawn orphans. Cosmetic → `Update` (never pinned state). Copies the `health.rs`
/// billboard.
///
/// The size comes from [`BubblePx`] and the live [`BalloonScale`] (the default when the resource is
/// absent), so changing the scale between frames resizes every balloon that has a [`BubblePx`].
///
/// The owner may sit anywhere in a hierarchy (a rider `ChildOf` a cart, a head under a body): the
/// anchor is its world position, composed from its own and its ancestors' `Transform`s in this frame,
/// so it is not a frame behind `GlobalTransform`'s propagation in `PostUpdate`. The balloon is a root
/// entity and shows exactly when the owner would: hidden when the owner or an ancestor it inherits
/// from is `Visibility::Hidden`.
pub fn track_bubbles<C: Component>(
    mut commands: Commands,
    camera: Single<&GlobalTransform, With<C>>,
    scale: Option<Res<BalloonScale>>,
    chain: Query<Anchoring, Without<Bubble>>,
    mut bubbles: Query<TrackedBubble>,
) {
    let scale = scale.map_or_else(BalloonScale::default, |s| *s);
    let cam_rot = camera.rotation();
    let cam_pos = camera.translation();
    let up = cam_rot * Vec3::Y;
    let right = cam_rot * Vec3::X;
    for (entity, bubble, face, offset_px, mut tf, mut vis) in &mut bubbles {
        let Some((owner_pos, owner_visible)) = owner_in_world(bubble.owner, &chain) else {
            commands.entity(entity).despawn();
            continue;
        };
        if let Some(face) = face {
            let size = face.0.as_vec2() * scale.world_per_px;
            tf.scale = Vec3::new(size.x, size.y, 1.0);
        }
        let offset = offset_px.map_or(bubble.offset, |o| o.0 * scale.world_per_px);
        let anchor = owner_pos + Vec3::Y * scale.anchor_y;
        let half_h = tf.scale.y * 0.5;
        let toward = (cam_pos - anchor).normalize_or_zero();
        tf.translation =
            anchor + up * (half_h + offset.y) + right * offset.x + toward * CAMERA_NUDGE;
        tf.rotation = cam_rot;
        // A root balloon that inherits is visible; written only on a change, so a steady balloon does
        // not re-run visibility propagation every frame.
        vis.set_if_neq(if owner_visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// Despawn ambient bubbles whose dwell time has elapsed. Uses unscaled real time so bubbles still
/// expire while the sim is paused (a modal conversation zeroes virtual `Time`).
pub fn expire_bubbles(
    mut commands: Commands,
    time: Res<Time<Real>>,
    ttls: Query<(Entity, &BubbleTtl)>,
) {
    let now = time.elapsed_secs();
    for (entity, ttl) in &ttls {
        if now >= ttl.expires_at {
            commands.entity(entity).despawn();
        }
    }
}

// -------------------------------------------------------------------------------------------------
// CPU rasterizer
// -------------------------------------------------------------------------------------------------

/// Balloon fill / border / text colors for a kind+emotion, resolved from the skin (emotion tints the
/// border per AniBalloons).
struct Palette {
    fill: [u8; 4],
    border: [u8; 4],
    text: [u8; 4],
    /// Hard edges: coverage is 0 or 1, never between.
    pixel: bool,
    /// Border thickness in raster px.
    border_w: f32,
}

/// Every length of the raster, scaled to the skin's glyph size.
///
/// At `text_px == CAP_PX` the constants are used as written. In pixel mode each length is rounded to
/// whole texels (at least one), so the border is a texel and the corners land on the grid.
struct Metrics {
    /// Glyph em in raster px.
    cap: f32,
    /// `text_px / CAP_PX`.
    k: f32,
    pad: f32,
    margin: f32,
    max_text_w: f32,
    min_text_w: f32,
    tail_h: f32,
    border: f32,
    radius: f32,
}

impl Metrics {
    fn new(skin: &BalloonSkin) -> Self {
        let cap = skin.text_px.max(1.0);
        let k = cap / CAP_PX;
        let sc = |v: f32| {
            let s = v * k;
            if skin.pixel { s.round().max(1.0) } else { s }
        };
        Self {
            cap,
            k,
            pad: sc(PAD),
            margin: sc(MARGIN),
            max_text_w: sc(MAX_TEXT_W),
            min_text_w: sc(MIN_TEXT_W),
            tail_h: sc(TAIL_H),
            border: sc(BORDER),
            radius: sc(SPEECH_RADIUS),
        }
    }
}

impl Palette {
    /// Coverage of a pixel whose centre is `d` px outside the shape's edge (negative inside).
    fn edge(&self, d: f32) -> f32 {
        if self.pixel {
            if d <= 0.0 { 1.0 } else { 0.0 }
        } else {
            (0.5 - d).clamp(0.0, 1.0)
        }
    }

    /// Glyph coverage as drawn: thresholded at 0.5 in pixel mode.
    fn glyph(&self, cov: f32) -> f32 {
        if self.pixel {
            if cov >= 0.5 { 1.0 } else { 0.0 }
        } else {
            cov
        }
    }
}

fn palette(style: &BubbleStyle, m: &Metrics) -> Palette {
    let skin = &style.skin;
    let (fill, base_border) = match style.kind {
        BubbleKind::Speech => (skin.fill, skin.border),
        BubbleKind::Thought => (skin.thought_fill, skin.thought_border),
    };
    let border = match style.emotion {
        Emotion::Neutral => base_border,
        Emotion::Joy => [235, 205, 90, 255],
        Emotion::Anger => [242, 74, 58, 255],
        Emotion::Sadness => [96, 150, 240, 255],
        Emotion::Surprise => [250, 240, 150, 255],
        Emotion::Fear => [178, 118, 226, 255],
        Emotion::Calm => [120, 226, 170, 255],
    };
    Palette {
        fill,
        border,
        text: skin.text,
        pixel: skin.pixel,
        border_w: m.border,
    }
}

/// Alpha-composite `col` (scaled by `cov`) over the RGBA pixel at (x,y). Out-of-bounds is ignored.
fn blend(buf: &mut [u8], w: u32, h: u32, x: i32, y: i32, col: [u8; 4], cov: f32) {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 || cov <= 0.0 {
        return;
    }
    let idx = ((y as u32 * w + x as u32) * 4) as usize;
    let a = (col[3] as f32 / 255.0) * cov.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    for k in 0..3 {
        let src = col[k] as f32;
        let dst = buf[idx + k] as f32;
        buf[idx + k] = (src * a + dst * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
    }
    let da = buf[idx + 3] as f32 / 255.0;
    let out_a = a + da * (1.0 - a);
    buf[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// Signed distance to a rounded rectangle centered at origin (half-extents `half`, corner `r`).
fn rounded_rect_sdf(p: Vec2, half: Vec2, r: f32) -> f32 {
    let q = p.abs() - (half - Vec2::splat(r));
    q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - r
}

/// Advance-width of `s` in the scaled font, kerned against `prev` (the glyph before it, if any).
/// `prev` is left at `s`'s last glyph, so the next piece continues the same run.
fn advance<F: Font>(sf: &impl ScaleFont<F>, s: &str, prev: &mut Option<GlyphId>) -> f32 {
    let mut w = 0.0;
    for c in s.chars() {
        let id = sf.glyph_id(c);
        if let Some(p) = *prev {
            w += sf.kern(p, id);
        }
        w += sf.h_advance(id);
        *prev = Some(id);
    }
    w
}

/// Advance-width of a string in the scaled font (with kerning).
fn measure<F: Font>(sf: &impl ScaleFont<F>, s: &str) -> f32 {
    advance(sf, s, &mut None)
}

/// Word-wrap to `max_w` px, respecting explicit newlines.
///
/// Greedy: a word goes on the current line if it fits, else on a fresh one. A word wider than `max_w`
/// even on a fresh line is broken (see [`break_word`]), so no line is wider than `max_w` unless one
/// grapheme cluster alone is.
fn wrap_lines<F: Font>(sf: &impl ScaleFont<F>, text: &str, max_w: f32) -> Vec<String> {
    let space = measure(sf, " ");
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut cur = String::new();
        let mut cur_w = 0.0;
        for word in para.split_whitespace() {
            let ww = measure(sf, word);
            if !cur.is_empty() && cur_w + space + ww > max_w {
                lines.push(std::mem::take(&mut cur));
                cur_w = 0.0;
            }
            if ww > max_w {
                // `cur` is empty here: a word this wide never fits after anything.
                break_word(sf, word, max_w, &mut lines, &mut cur, &mut cur_w);
                continue;
            }
            if !cur.is_empty() {
                cur.push(' ');
                cur_w += space;
            }
            cur.push_str(word);
            cur_w += ww;
        }
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// UAX #14's emergency break: `word` has no line break opportunity and is wider than `max_w`, so it is
/// broken after the last grapheme cluster that fits. Clusters stay whole (UAX #14 recommends it): an
/// accent, a flag or a ZWJ sequence never ends a line half-written. Full lines go to `lines`; the last
/// fragment stays in `cur` (with its width in `cur_w`) so the next word continues after it. A cluster
/// wider than `max_w` on its own takes a line to itself, which cannot be helped.
fn break_word<F: Font>(
    sf: &impl ScaleFont<F>,
    word: &str,
    max_w: f32,
    lines: &mut Vec<String>,
    cur: &mut String,
    cur_w: &mut f32,
) {
    let mut prev = None;
    for g in word.graphemes(true) {
        let mut after = prev;
        let mut gw = advance(sf, g, &mut after);
        if !cur.is_empty() && *cur_w + gw > max_w {
            lines.push(std::mem::take(cur));
            *cur_w = 0.0;
            // A new line has no glyph before it to kern against.
            after = None;
            gw = advance(sf, g, &mut after);
        }
        cur.push_str(g);
        *cur_w += gw;
        prev = after;
    }
}

/// Horizontal padding wide enough that the text block's corners clear the balloon's corner arc.
///
/// Split out of `rasterize` so it can be tested without a font: the property that matters is pure
/// geometry, and requiring a `.ttf` to check it would mean not checking it. See
/// `text_corners_stay_inside_the_balloon`.
fn horizontal_pad(m: &Metrics, text_w: f32, text_h: f32, thought: bool) -> f32 {
    let balloon_h = text_h + 2.0 * m.pad;
    // The worst point on the text block: its outer corner, relative to the balloon centre.
    let corner = Vec2::new(text_w * 0.5, text_h * 0.5);
    // Clear of the stroke, not merely inside the silhouette — a glyph touching the border reads as a
    // rendering fault even though it is technically within the shape.
    let clear = m.border + 4.0;

    // **Solved against the same SDF the renderer uses, rather than derived.** A closed form for this
    // is easy to get subtly wrong: the first attempt here engineered HORIZONTAL clearance, which on a
    // steeply curved cap is not the same quantity the SDF measures (perpendicular distance), and it
    // came up 0.18px short at four lines while looking correct. Bisecting the real function cannot
    // drift from the shape being drawn, and costs 24 iterations of arithmetic once per line of text.
    let fits = |pad: f32| {
        let half = Vec2::new((text_w + 2.0 * pad) * 0.5, balloon_h * 0.5);
        let r = if thought {
            half.y.min(half.x)
        } else {
            m.radius.min(half.x).min(half.y)
        };
        rounded_rect_sdf(corner, half, r) <= -clear
    };

    // A speech balloon's radius is fixed and small, so it almost always already fits and pays nothing.
    if fits(m.pad) {
        return m.pad;
    }
    let mut lo = m.pad;
    let mut hi = m.pad + text_h + clear;
    // Widen until the upper bound genuinely fits, so the bisection below has a bracket. Bounded
    // rather than `while`, because a shape that never fits must not hang the caller.
    for _ in 0..8 {
        if fits(hi) {
            break;
        }
        hi += text_h.max(m.pad);
    }
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if fits(mid) { hi = mid } else { lo = mid }
    }
    hi
}

/// Rasterize a bubble to RGBA8 bytes + dimensions.
fn rasterize(font: &FontArc, style: &BubbleStyle, text: &str) -> (Vec<u8>, u32, u32) {
    let m = Metrics::new(&style.skin);
    let pal = palette(style, &m);
    let pixel = style.skin.pixel;
    let sf = font.as_scaled(PxScale::from(m.cap));
    let mut line_h = sf.height() + sf.line_gap();
    let mut ascent = sf.ascent();
    if pixel {
        line_h = line_h.round().max(1.0);
        ascent = ascent.round();
    }

    let lines = wrap_lines(&sf, text, m.max_text_w);
    let mut text_w = lines
        .iter()
        .map(|l| measure(&sf, l))
        .fold(0.0_f32, f32::max)
        .max(m.min_text_w);
    if pixel {
        text_w = text_w.ceil();
    }
    let text_h = line_h * lines.len() as f32;

    let balloon_h = text_h + 2.0 * m.pad;
    let thought = matches!(style.kind, BubbleKind::Thought);

    // **Horizontal padding has to clear the corner arc, not just the flat edge.**
    //
    // A thought balloon is a pill: its radius is half the balloon height, so the left and right caps
    // are semicircles that curve inward by `r - sqrt(r^2 - dy^2)` at distance `dy` above and below the
    // centre line. The text block's top and bottom corners sit at `dy = text_h/2`, which is exactly
    // where that inset is largest — so a flat `pad` is only enough while the balloon is short.
    //
    // Measured, before this existed: one line cleared it with 11px to spare, two lines overflowed the
    // cap by ~5px and three by ~23px, which is why the clipping looked intermittent rather than broken.
    // Speech balloons need it too, just less: their radius is a fixed `radius`.
    let mut pad_x = horizontal_pad(&m, text_w, text_h, thought);
    if pixel {
        pad_x = pad_x.ceil();
    }
    let balloon_w = text_w + 2.0 * pad_x;
    let tail_h = if style.tail { m.tail_h } else { 0.0 };
    let img_w = (balloon_w + 2.0 * m.margin).ceil() as u32;
    let img_h = (balloon_h + 2.0 * m.margin + tail_h).ceil() as u32;

    let mut buf = vec![0u8; (img_w * img_h * 4) as usize];

    // Balloon body.
    let cx = m.margin + balloon_w / 2.0;
    let cy = m.margin + balloon_h / 2.0;
    let half = Vec2::new(balloon_w / 2.0, balloon_h / 2.0);
    let radius = if thought {
        (balloon_h * 0.5).min(balloon_w * 0.5) // pill
    } else {
        m.radius.min(half.x).min(half.y)
    };
    for py in 0..img_h {
        for px in 0..img_w {
            let p = Vec2::new(px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
            let d = rounded_rect_sdf(p, half, radius);
            let outer = pal.edge(d); // 1 inside, 0 outside, AA at edge unless the skin is pixel
            if outer <= 0.0 {
                continue;
            }
            if d >= -m.border {
                blend(
                    &mut buf, img_w, img_h, px as i32, py as i32, pal.border, outer,
                );
            } else {
                blend(&mut buf, img_w, img_h, px as i32, py as i32, pal.fill, 1.0);
            }
        }
    }

    // Tail toward the owner (below the balloon). Its geometry is written at the reference size and
    // scaled with the glyphs.
    if style.tail {
        let k = m.k;
        let base_y = m.margin + balloon_h;
        if thought {
            // Trailing shrinking dots — the classic thought tail.
            draw_disc(&mut buf, img_w, img_h, cx, base_y + 7.0 * k, 8.0 * k, &pal);
            draw_disc(
                &mut buf,
                img_w,
                img_h,
                cx - 4.0 * k,
                base_y + 18.0 * k,
                5.0 * k,
                &pal,
            );
            draw_disc(
                &mut buf,
                img_w,
                img_h,
                cx - 8.0 * k,
                base_y + 26.0 * k,
                3.0 * k,
                &pal,
            );
        } else {
            // Pointed speech tail.
            let a = Vec2::new(cx - 14.0 * k, base_y - 2.0 * k);
            let b = Vec2::new(cx + 14.0 * k, base_y - 2.0 * k);
            let tip = Vec2::new(cx - 6.0 * k, base_y + m.tail_h);
            draw_triangle(&mut buf, img_w, img_h, a, b, tip, &pal);
        }
    }

    // Text glyphs, top-down. A pixel skin puts every glyph on a whole texel.
    let snap = |v: f32| if pixel { v.round() } else { v };
    let left = m.margin + pad_x;
    let mut baseline = m.margin + m.pad + ascent;
    for line in &lines {
        let mut caret = left;
        let mut prev = None;
        for c in line.chars() {
            let id = sf.glyph_id(c);
            if let Some(p) = prev {
                caret += sf.kern(p, id);
            }
            let mut glyph = sf.scaled_glyph(c);
            glyph.position = point(snap(caret), snap(baseline));
            if let Some(outline) = sf.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                outline.draw(|gx, gy, cov| {
                    let x = bounds.min.x as i32 + gx as i32;
                    let y = bounds.min.y as i32 + gy as i32;
                    blend(&mut buf, img_w, img_h, x, y, pal.text, pal.glyph(cov));
                });
            }
            caret += sf.h_advance(id);
            prev = Some(id);
        }
        baseline += line_h;
    }

    (buf, img_w, img_h)
}

/// Filled disc with a border ring (for thought-tail dots).
fn draw_disc(buf: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, r: f32, pal: &Palette) {
    let x0 = (cx - r - 1.0).floor() as i32;
    let x1 = (cx + r + 1.0).ceil() as i32;
    let y0 = (cy - r - 1.0).floor() as i32;
    let y1 = (cy + r + 1.0).ceil() as i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let d = Vec2::new(x as f32 + 0.5 - cx, y as f32 + 0.5 - cy).length() - r;
            let outer = pal.edge(d);
            if outer <= 0.0 {
                continue;
            }
            if d >= -pal.border_w {
                blend(buf, w, h, x, y, pal.border, outer);
            } else {
                blend(buf, w, h, x, y, pal.fill, 1.0);
            }
        }
    }
}

/// Filled triangle with bordered edges (for the speech tail). `a`,`b`,`c` in pixel space.
fn draw_triangle(buf: &mut [u8], w: u32, h: u32, a: Vec2, b: Vec2, c: Vec2, pal: &Palette) {
    let min_x = a.x.min(b.x).min(c.x).floor() as i32 - 1;
    let max_x = a.x.max(b.x).max(c.x).ceil() as i32 + 1;
    let min_y = a.y.min(b.y).min(c.y).floor() as i32 - 1;
    let max_y = a.y.max(b.y).max(c.y).ceil() as i32 + 1;
    let area = edge(a, b, c);
    if area.abs() < 1.0e-3 {
        return;
    }
    let sign = area.signum();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge(b, c, p) * sign;
            let w1 = edge(c, a, p) * sign;
            let w2 = edge(a, b, p) * sign;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            // Distance to nearest edge (in px) → border where close.
            let d0 = w0 / (b - c).length();
            let d1 = w1 / (c - a).length();
            let d2 = w2 / (a - b).length();
            let edge_dist = d0.min(d1).min(d2);
            let col = if edge_dist <= pal.border_w {
                pal.border
            } else {
                pal.fill
            };
            blend(buf, w, h, x, y, col, 1.0);
        }
    }
}

/// Signed area of triangle (a,b,p) × 2 — the edge function.
fn edge(a: Vec2, b: Vec2, p: Vec2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::image::ImageSamplerDescriptor;

    /// **The clipping bug, pinned as geometry.**
    ///
    /// A thought balloon is a pill, so its cap curves inward by `r - sqrt(r^2 - dy^2)` at the text
    /// block's top and bottom corners. With a flat pad that was fine for one line and wrong for
    /// three: measured overflow was ~5px at two lines and ~23px at three, which is why the clipping
    /// read as intermittent rather than as a bug.
    ///
    /// This asserts the property directly — the worst point of the text rect must sit inside the
    /// fill, clear of the border — for both shapes at one through four lines.
    #[test]
    fn text_corners_stay_inside_the_balloon() {
        // The reference (crt) metrics and a pixel skin at its native size: the property is the same.
        for skin in [BalloonSkin::crt(), BalloonSkin::ink_on_paper()] {
            let m = Metrics::new(&skin);
            for lines in 1..=4 {
                for thought in [false, true] {
                    let text_w = 300.0 * m.k;
                    let text_h = 56.0 * m.k * lines as f32;
                    let pad = horizontal_pad(&m, text_w, text_h, thought);
                    let balloon_w = text_w + 2.0 * pad;
                    let balloon_h = text_h + 2.0 * m.pad;
                    let half = Vec2::new(balloon_w * 0.5, balloon_h * 0.5);
                    let radius = if thought {
                        (balloon_h * 0.5).min(balloon_w * 0.5)
                    } else {
                        m.radius.min(half.x).min(half.y)
                    };
                    // The outer corner of the text block, relative to the balloon centre.
                    let corner = Vec2::new(text_w * 0.5, text_h * 0.5);
                    let d = rounded_rect_sdf(corner, half, radius);
                    assert!(
                        d < -m.border,
                        "pixel={} lines={lines} thought={thought}: text corner sits at sdf {d:.2}, which \
                         is not clear of the {}px border — glyphs there draw over the outline or outside it",
                        skin.pixel,
                        m.border
                    );
                }
            }
        }
    }

    /// A pill only needs the extra width once it is tall enough to curve into the text, so a
    /// single-line balloon must not get silently fatter for nothing.
    #[test]
    fn one_line_pays_nothing_for_the_fix() {
        let m = Metrics::new(&BalloonSkin::crt());
        assert_eq!(m.pad, PAD, "crt metrics are the reference constants");
        assert_eq!(horizontal_pad(&m, 300.0, 56.0, true), PAD);
        assert_eq!(horizontal_pad(&m, 300.0, 56.0, false), PAD);
        assert!(
            horizontal_pad(&m, 300.0, 168.0, true) > PAD,
            "three lines must widen the pill"
        );
    }

    #[test]
    fn dwell_has_floor() {
        assert!((dwell_secs("hi") - DWELL_FLOOR).abs() < 1.0e-6);
    }

    #[test]
    fn dwell_scales_with_length() {
        // 40 words at 200 wpm = 12 s, well above the floor.
        let long = "word ".repeat(40);
        let secs = dwell_secs(&long);
        assert!(secs > DWELL_FLOOR);
        assert!((secs - 12.0).abs() < 0.5);
    }

    #[test]
    fn empty_text_still_floors() {
        assert!((dwell_secs("") - DWELL_FLOOR).abs() < 1.0e-6);
    }
    // ----- pixel skin -----

    /// `from_font_bytes` builds from a real font and refuses garbage.
    #[test]
    fn from_font_bytes_accepts_a_font_and_refuses_garbage() {
        let bytes = include_bytes!("../tests/data/FiraMono-subset.ttf").to_vec();
        assert!(BubbleAssets::from_font_bytes(Handle::default(), bytes).is_ok());
        let err =
            BubbleAssets::from_font_bytes(Handle::default(), b"definitely not a font".to_vec())
                .err()
                .expect("garbage must be refused");
        assert!(err.contains("font"), "{err}");
    }

    /// Fira Mono subset (OFL), so the rasterizer can be exercised without a system font.
    fn test_font() -> FontArc {
        FontArc::try_from_slice(include_bytes!("../tests/data/FiraMono-subset.ttf"))
            .expect("the test font parses")
    }

    fn styled(kind: BubbleKind, tail: bool, skin: BalloonSkin) -> BubbleStyle {
        BubbleStyle {
            kind,
            emotion: Emotion::Neutral,
            tail,
            skin,
        }
    }

    const LINE: &str = "Hold the gate until dawn, then ring the bell twice.";

    /// **No half-measures in pixel mode.** With a fully opaque skin every texel is either solid or
    /// clear, for both shapes, with and without a tail, and the glyphs are exactly the text colour.
    #[test]
    fn a_pixel_skin_has_no_partially_transparent_texel() {
        let font = test_font();
        let skin = BalloonSkin::ink_on_paper();
        for kind in [BubbleKind::Speech, BubbleKind::Thought] {
            for tail in [true, false] {
                let (rgba, w, h) = rasterize(&font, &styled(kind, tail, skin), LINE);
                assert_eq!(rgba.len(), (w * h * 4) as usize);
                let mut solid = 0;
                let mut text = 0;
                for px in rgba.as_chunks::<4>().0 {
                    assert!(
                        px[3] == 0 || px[3] == 255,
                        "{kind:?} tail={tail}: texel {px:?} is partially transparent"
                    );
                    if px[3] == 255 {
                        solid += 1;
                        if px[..3] == skin.text[..3] {
                            text += 1;
                        }
                    }
                }
                assert!(solid > 0, "{kind:?}: nothing was drawn");
                assert!(text > 0, "{kind:?}: no glyph texel carries the text colour");
            }
        }
    }

    /// The same check on the anti-aliased skin finds blended texels, so the assertion above is capable
    /// of failing.
    #[test]
    fn the_anti_aliased_skin_does_blend() {
        let font = test_font();
        let (rgba, ..) = rasterize(
            &font,
            &styled(BubbleKind::Speech, true, BalloonSkin::crt()),
            LINE,
        );
        assert!(
            rgba.as_chunks::<4>()
                .0
                .iter()
                .any(|px| px[3] != 0 && px[3] != 255),
            "crt skin produced no partially transparent texel; the pixel test proves nothing"
        );
    }

    /// Pixel balloons are sampled with `nearest` (so a texel stays a block when the quad is magnified);
    /// anti-aliased ones keep the default sampler.
    #[test]
    fn a_pixel_skin_gets_a_nearest_sampler() {
        let assets = BubbleAssets {
            quad: Handle::default(),
            font: test_font(),
        };
        let mut images = Assets::<Image>::default();
        let mut materials = Assets::<StandardMaterial>::default();

        for (skin, nearest) in [
            (BalloonSkin::ink_on_paper(), true),
            (BalloonSkin::crt(), false),
        ] {
            let rendered = build_bubble(
                &assets,
                &mut images,
                &mut materials,
                &styled(BubbleKind::Speech, true, skin),
                "Sampler.",
            );
            let material = materials
                .get(&rendered.material)
                .expect("the material exists");
            let handle = material
                .base_color_texture
                .as_ref()
                .expect("a face texture");
            let image = images.get(handle).expect("the image exists");
            assert_eq!(
                matches!(&image.sampler, ImageSampler::Descriptor(d) if *d == ImageSamplerDescriptor::nearest()),
                nearest,
                "pixel={}: wrong sampler {:?}",
                skin.pixel,
                image.sampler
            );
        }
    }

    /// The tint of an emotion still reaches the border in pixel mode.
    #[test]
    fn emotion_tints_the_pixel_border() {
        let font = test_font();
        let mut style = styled(BubbleKind::Speech, false, BalloonSkin::ink_on_paper());
        style.emotion = Emotion::Anger;
        let (rgba, ..) = rasterize(&font, &style, "No.");
        assert!(
            rgba.as_chunks::<4>().0.contains(&[242, 74, 58, 255]),
            "no texel carries the anger tint"
        );
    }

    // ----- pixel scale -----

    /// **A 16 px pixel balloon is pixel-art sized**: one texel per font pixel, a one-texel border, a few
    /// texels of padding. 20 characters of a 16 px, 0.6 em-advance face (192 texels of text) come out a
    /// little over 200 wide and under 50 tall, and every texel is solid or clear.
    #[test]
    fn a_pixel_balloon_is_native_size() {
        let font = test_font();
        let skin = BalloonSkin::ink_on_paper();
        assert_eq!(skin.text_px, 16.0);
        let (rgba, w, h) = rasterize(
            &font,
            &styled(BubbleKind::Speech, false, skin),
            "Hold the gate at dwn",
        );
        assert!(
            (150..=240).contains(&w),
            "width {w} texels is not about 20 chars at 16 px"
        );
        assert!(
            (26..=50).contains(&h),
            "height {h} texels is not one pixel-font line"
        );
        assert!(
            rgba.as_chunks::<4>()
                .0
                .iter()
                .all(|px| px[3] == 0 || px[3] == 255)
        );

        // The hi-res skin is more than twice that.
        let (_, hw, hh) = rasterize(
            &font,
            &styled(BubbleKind::Speech, false, BalloonSkin::crt()),
            "Hold the gate at dwn",
        );
        assert!(hw > 2 * w && hh > 2 * h, "crt {hw}x{hh} vs pixel {w}x{h}");

        // The border is one texel thick on the top edge of the middle column.
        let m = Metrics::new(&skin);
        assert_eq!(m.border, 1.0);
        let x = (w / 2) as usize;
        let col: Vec<[u8; 4]> = (0..h as usize)
            .map(|y| {
                let i = (y * w as usize + x) * 4;
                [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
            })
            .collect();
        let first = col.iter().position(|p| p[3] == 255).expect("a solid texel");
        assert_eq!(
            col[first][..3],
            skin.border[..3],
            "the outermost texel is the border"
        );
        assert_eq!(
            col[first + 1][..3],
            skin.fill[..3],
            "and only one texel thick"
        );
    }

    /// The tail and the thought dots scale with the glyphs too: a pixel balloon with a tail is the
    /// no-tail one plus a few texels, not 28.
    #[test]
    fn a_pixel_tail_is_small() {
        let font = test_font();
        let skin = BalloonSkin::ink_on_paper();
        for kind in [BubbleKind::Speech, BubbleKind::Thought] {
            let (_, w0, h0) = rasterize(&font, &styled(kind, false, skin), "Hi there");
            let (rgba, w1, h1) = rasterize(&font, &styled(kind, true, skin), "Hi there");
            assert_eq!(w0, w1);
            assert!(
                (1..=14).contains(&(h1 - h0)),
                "{kind:?}: the tail adds {} texels",
                h1 - h0
            );
            assert!(
                rgba.as_chunks::<4>()
                    .0
                    .iter()
                    .all(|px| px[3] == 0 || px[3] == 255)
            );
        }
    }

    // ----- line breaking -----

    /// `text` wrapped at the skin's cap: the lines, each one's measured width, and the cap.
    fn wrapped(skin: &BalloonSkin, text: &str) -> (Vec<String>, Vec<f32>, f32) {
        let font = test_font();
        let m = Metrics::new(skin);
        let sf = font.as_scaled(PxScale::from(m.cap));
        let lines = wrap_lines(&sf, text, m.max_text_w);
        let widths = lines.iter().map(|l| measure(&sf, l)).collect();
        (lines, widths, m.max_text_w)
    }

    /// The advance of one `W` at the skin's glyph size: the test font is monospaced.
    fn w_advance(skin: &BalloonSkin) -> f32 {
        let font = test_font();
        let m = Metrics::new(skin);
        measure(&font.as_scaled(PxScale::from(m.cap)), "W")
    }

    /// **A word wider than the line is broken, never spilled.** Before this, a 60-letter word went on a
    /// line of its own, and that line set the balloon's width: 1269 px against a 568 px cap at `crt()`.
    /// A line of Chinese, which has no spaces, was one such word however short.
    #[test]
    fn a_word_wider_than_the_line_is_broken_at_the_cap() {
        let long = "W".repeat(60);
        let cases = [
            long.clone(),
            format!("go {long} now"),
            format!("{long} {long}"),
            "漢".repeat(40),
        ];
        for skin in [BalloonSkin::crt(), BalloonSkin::ink_on_paper()] {
            let adv = w_advance(&skin);
            for text in &cases {
                let (lines, widths, cap) = wrapped(&skin, text);
                assert!(
                    lines.len() > 1,
                    "pixel={}: {text:?} was not broken",
                    skin.pixel
                );
                for (line, w) in lines.iter().zip(&widths) {
                    assert!(
                        *w <= cap + 1.0e-3,
                        "pixel={}: line {line:?} is {w:.1} px, past the {cap} px cap",
                        skin.pixel
                    );
                }
                assert_eq!(
                    lines.concat().replace(' ', ""),
                    text.replace(' ', ""),
                    "breaking a word must not lose or add a character"
                );
            }
            // Broken at the last letter that fits, not early: no fragment has room for one more `W`.
            let (lines, widths, cap) = wrapped(&skin, &long);
            for (line, w) in lines.iter().zip(&widths).take(lines.len() - 1) {
                assert!(
                    cap - w < adv,
                    "pixel={}: {line:?} ends {} px short of the cap, with room for another letter",
                    skin.pixel,
                    cap - w
                );
            }
        }
    }

    /// Text with no over-wide word wraps as the greedy wrap always did: a word that fits on a line of
    /// its own is never broken.
    #[test]
    fn a_sentence_with_no_overlong_word_wraps_greedily() {
        let (lines, ..) = wrapped(&BalloonSkin::crt(), LINE);
        assert_eq!(
            lines,
            ["Hold the gate until dawn,", "then ring the bell twice."]
        );
    }

    /// The fragment a broken word ends on is an ordinary line end: the next word continues after it.
    #[test]
    fn the_word_after_a_broken_word_continues_on_its_last_fragment() {
        let (lines, ..) = wrapped(&BalloonSkin::crt(), &format!("{} id", "W".repeat(60)));
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(
            lines[..2].iter().all(|l| l.chars().all(|c| c == 'W')),
            "{lines:?}"
        );
        assert!(
            lines[2].ends_with(" id") && lines[2].starts_with('W'),
            "`id` should follow the last fragment on its line: {lines:?}"
        );
    }

    /// UAX #14's emergency mode keeps grapheme clusters together: an accent, a flag and a ZWJ family
    /// are each one user-perceived character and never end a line half-written.
    #[test]
    fn a_grapheme_cluster_is_never_split_across_lines() {
        let clusters = [
            "e\u{301}",
            "\u{1F1EB}\u{1F1F7}",
            "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
        ];
        for cluster in clusters {
            let text = cluster.repeat(40);
            for skin in [BalloonSkin::crt(), BalloonSkin::ink_on_paper()] {
                let (lines, ..) = wrapped(&skin, &text);
                assert!(lines.len() > 1, "{cluster:?} x40 was not broken");
                for line in &lines {
                    assert_eq!(
                        line.matches(cluster).count() * cluster.len(),
                        line.len(),
                        "pixel={}: line {line:?} is not whole {cluster:?} clusters",
                        skin.pixel
                    );
                }
            }
        }
    }

    /// Bounding box `(min_x, min_y, max_x, max_y)` of the texels drawn in exactly the glyph colour.
    fn ink_box(rgba: &[u8], w: u32, text: [u8; 4]) -> (u32, u32, u32, u32) {
        let mut bb = (u32::MAX, u32::MAX, 0, 0);
        for (i, px) in rgba.as_chunks::<4>().0.iter().enumerate() {
            if *px == text {
                let (x, y) = (i as u32 % w, i as u32 / w);
                bb = (bb.0.min(x), bb.1.min(y), bb.2.max(x), bb.3.max(y));
            }
        }
        bb
    }

    /// **The balloon of a broken word is an ordinary balloon.** A 60-letter word breaks into pieces that
    /// each fill a line, so its raster is the very raster of the author writing those pieces as
    /// separate words. Its glyphs stay within the cap and inside the border, and a pixel skin is still
    /// ink, border or background and nothing in between.
    #[test]
    fn a_broken_word_makes_an_ordinary_balloon() {
        let font = test_font();
        let long = "W".repeat(60);
        for skin in [BalloonSkin::crt(), BalloonSkin::ink_on_paper()] {
            let m = Metrics::new(&skin);
            let by_hand = wrapped(&skin, &long).0.join(" ");
            for kind in [BubbleKind::Speech, BubbleKind::Thought] {
                let style = styled(kind, true, skin);
                let (rgba, w, h) = rasterize(&font, &style, &long);
                let (by_hand_rgba, bw, bh) = rasterize(&font, &style, &by_hand);
                assert_eq!(
                    (w, h),
                    (bw, bh),
                    "pixel={} {kind:?}: the broken word is not the balloon of its pieces",
                    skin.pixel
                );
                assert!(
                    rgba == by_hand_rgba,
                    "pixel={} {kind:?}: same size, different texels",
                    skin.pixel
                );

                let (x0, y0, x1, y1) = ink_box(&rgba, w, skin.text);
                assert!(
                    (x1 - x0 + 1) as f32 <= m.max_text_w + 2.0,
                    "pixel={} {kind:?}: the ink is {} px wide, past the {} px cap",
                    skin.pixel,
                    x1 - x0 + 1,
                    m.max_text_w
                );
                let edge = m.margin + m.border;
                assert!(
                    x0 as f32 >= edge
                        && y0 as f32 >= edge
                        && (w - 1 - x1) as f32 >= edge
                        && (h - 1 - y1) as f32 >= edge,
                    "pixel={} {kind:?}: ink {x0},{y0}..{x1},{y1} reaches the border of a {w}x{h} raster",
                    skin.pixel
                );
                if skin.pixel {
                    assert!(
                        rgba.as_chunks::<4>()
                            .0
                            .iter()
                            .all(|px| px[3] == 0 || px[3] == 255),
                        "{kind:?}: a texel is partially transparent"
                    );
                }
            }
        }
    }

    // ----- BalloonScale -----

    #[derive(Component)]
    struct Cam;

    /// `track_bubbles` reads the scale every frame: change it and a live balloon is resized and
    /// re-anchored, with no respawn.
    #[test]
    fn track_bubbles_follows_a_change_of_balloon_scale() {
        let mut app = App::new();
        app.add_systems(Update, track_bubbles::<Cam>)
            .insert_resource(BalloonScale {
                world_per_px: 0.01,
                anchor_y: 2.0,
            });
        // Far down +Z, so the nudge toward the camera is almost horizontal.
        app.world_mut().spawn((
            Cam,
            GlobalTransform::from(Transform::from_xyz(5.0, 1.0, 1000.0)),
        ));
        let owner = app
            .world_mut()
            .spawn((Transform::from_xyz(5.0, 1.0, 0.0), Visibility::default()))
            .id();
        let balloon = app
            .world_mut()
            .spawn((
                Bubble {
                    owner,
                    offset: Vec2::ZERO,
                },
                BubblePx(UVec2::new(100, 40)),
                BubbleOffsetPx(Vec2::new(0.0, 10.0)),
                Transform::default(),
                Visibility::default(),
            ))
            .id();

        app.update();
        let tf = *app.world().get::<Transform>(balloon).expect("balloon");
        assert!(
            (tf.scale.x - 1.0).abs() < 1e-5 && (tf.scale.y - 0.4).abs() < 1e-5,
            "{:?}",
            tf.scale
        );
        // anchor 1 + 2, then half height 0.2 and the 10 px offset (0.1) up, the camera looks down -Z.
        assert!(
            (tf.translation.y - (3.0 + 0.2 + 0.1)).abs() < 0.01,
            "{:?}",
            tf.translation
        );

        *app.world_mut().resource_mut::<BalloonScale>() = BalloonScale {
            world_per_px: 0.02,
            anchor_y: 3.0,
        };
        app.update();
        let tf = *app.world().get::<Transform>(balloon).expect("balloon");
        assert!(
            (tf.scale.x - 2.0).abs() < 1e-5 && (tf.scale.y - 0.8).abs() < 1e-5,
            "{:?}",
            tf.scale
        );
        assert!(
            (tf.translation.y - (4.0 + 0.4 + 0.2)).abs() < 0.01,
            "{:?}",
            tf.translation
        );
    }

    // ----- owners in a hierarchy (#10) -----

    /// A balloon on `owner`, raster 100x40 texels, at the default scale.
    fn balloon_on(app: &mut App, owner: Entity) -> Entity {
        app.world_mut()
            .spawn((
                Bubble {
                    owner,
                    offset: Vec2::ZERO,
                },
                BubblePx(UVec2::new(100, 40)),
                Transform::default(),
                Visibility::default(),
            ))
            .id()
    }

    /// **A speaker that is a child entity gets its balloon over its world position**, not at its
    /// parent-relative offset from the world origin (#10's input).
    #[test]
    fn a_child_owner_gets_its_balloon_over_its_world_position() {
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .add_systems(Update, track_bubbles::<Cam>);
        app.world_mut().spawn((
            Cam,
            GlobalTransform::from(Transform::from_xyz(10.0, 1.0, 1000.0)),
        ));
        let parent = app
            .world_mut()
            .spawn((Transform::from_xyz(10.0, 0.0, 0.0), Visibility::default()))
            .id();
        let owner = app
            .world_mut()
            .spawn((Transform::default(), Visibility::default(), ChildOf(parent)))
            .id();
        let balloon = balloon_on(&mut app, owner);
        app.update();
        let tf = app.world().get::<Transform>(balloon).expect("balloon");
        assert!(
            (tf.translation.x - 10.0).abs() < 0.05,
            "{:?}",
            tf.translation
        );
    }

    /// **The anchor is the owner's world position in the frame it is tracked**: through a translated,
    /// rotated and scaled grandparent and parent it equals the `GlobalTransform` Bevy propagates for
    /// the owner at the end of the same update, also on the frame the grandparent moves (no frame of
    /// lag behind propagation).
    #[test]
    fn a_nested_owner_anchors_at_its_propagated_world_position_in_the_same_frame() {
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .add_systems(Update, track_bubbles::<Cam>);
        let cam = Vec3::new(-3.0, 4.0, 900.0);
        app.world_mut()
            .spawn((Cam, GlobalTransform::from(Transform::from_translation(cam))));
        let grandparent = app
            .world_mut()
            .spawn((
                Transform::from_xyz(4.0, 1.0, -2.0)
                    .with_rotation(Quat::from_rotation_y(0.7))
                    .with_scale(Vec3::splat(2.0)),
                Visibility::default(),
            ))
            .id();
        let parent = app
            .world_mut()
            .spawn((
                Transform::from_xyz(1.5, 0.25, 0.0).with_rotation(Quat::from_rotation_z(0.4)),
                Visibility::default(),
                ChildOf(grandparent),
            ))
            .id();
        let owner = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 1.0, 0.5),
                Visibility::default(),
                ChildOf(parent),
            ))
            .id();
        let balloon = balloon_on(&mut app, owner);
        let scale = BalloonScale::default();

        let check = |app: &App, when: &str| {
            let world_pos = app
                .world()
                .get::<GlobalTransform>(owner)
                .expect("owner")
                .translation();
            let anchor = world_pos + Vec3::Y * scale.anchor_y;
            let half_h = 40.0 * scale.world_per_px * 0.5;
            let expected = anchor + Vec3::Y * half_h + (cam - anchor).normalize() * CAMERA_NUDGE;
            let got = app
                .world()
                .get::<Transform>(balloon)
                .expect("balloon")
                .translation;
            assert!(
                got.distance(expected) < 1e-4,
                "{when}: balloon at {got:?}, owner's world position {world_pos:?} wants {expected:?}"
            );
        };

        app.update();
        check(&app, "first frame");
        app.world_mut()
            .get_mut::<Transform>(grandparent)
            .expect("grandparent")
            .translation += Vec3::new(-6.0, 0.5, 3.0);
        app.update();
        check(&app, "the frame the grandparent moved");
    }

    /// **A balloon shows exactly when its owner would**, by Bevy's inheritance rule: hidden with a
    /// hidden parent, back with it, and shown for an owner set `Visible` under a hidden parent.
    #[test]
    fn a_balloon_hides_with_its_owners_parent() {
        use bevy::camera::visibility::VisibilityPlugin;

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            VisibilityPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .add_systems(Update, track_bubbles::<Cam>);
        app.world_mut().spawn((Cam, GlobalTransform::default()));
        let parent = app
            .world_mut()
            .spawn((Transform::from_xyz(10.0, 0.0, 0.0), Visibility::default()))
            .id();
        let owner = app
            .world_mut()
            .spawn((Transform::default(), Visibility::default(), ChildOf(parent)))
            .id();
        let balloon = balloon_on(&mut app, owner);
        let shown = |app: &App| {
            app.world()
                .get::<InheritedVisibility>(balloon)
                .expect("balloon")
                .get()
        };

        app.update();
        assert!(shown(&app), "a visible owner's balloon is visible");

        *app.world_mut()
            .get_mut::<Visibility>(parent)
            .expect("parent") = Visibility::Hidden;
        app.update();
        assert!(
            !app.world()
                .get::<InheritedVisibility>(owner)
                .expect("owner")
                .get()
        );
        assert!(
            !shown(&app),
            "the owner is hidden through its parent, so is its balloon"
        );

        *app.world_mut().get_mut::<Visibility>(owner).expect("owner") = Visibility::Visible;
        app.update();
        assert!(
            shown(&app),
            "an owner set Visible shows under a hidden parent, and so does its balloon"
        );

        *app.world_mut().get_mut::<Visibility>(owner).expect("owner") = Visibility::Inherited;
        *app.world_mut()
            .get_mut::<Visibility>(parent)
            .expect("parent") = Visibility::Inherited;
        app.update();
        assert!(shown(&app), "the parent shown again, the balloon is back");
    }

    /// A bubble without a `BubblePx` (a host that spawns its own quads) keeps the scale it was given.
    #[test]
    fn a_bubble_without_a_raster_size_keeps_its_scale() {
        let mut app = App::new();
        app.add_systems(Update, track_bubbles::<Cam>);
        app.world_mut().spawn((Cam, GlobalTransform::default()));
        let owner = app
            .world_mut()
            .spawn((Transform::default(), Visibility::default()))
            .id();
        let balloon = app
            .world_mut()
            .spawn((
                Bubble {
                    owner,
                    offset: Vec2::ZERO,
                },
                Transform::from_scale(Vec3::new(3.0, 1.0, 1.0)),
                Visibility::default(),
            ))
            .id();
        app.update();
        let tf = app.world().get::<Transform>(balloon).expect("balloon");
        assert_eq!(tf.scale, Vec3::new(3.0, 1.0, 1.0));
    }
}
