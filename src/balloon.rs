//! World-space balloons: the CPU rasterizer, the quad tracking and expiry.
//!
//! A bubble is text rasterized into an `Image`, put on a unit quad as an unlit alpha-blended material,
//! anchored above an owner entity and turned to face a camera. See the crate docs for why.

use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::time::Real;

use crate::style::{BubbleKind, Emotion};

// --- rasterization tuning (pixels) ---
/// Glyph pixel scale. High enough that the baked texture stays crisp when the ortho camera zooms in.
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
/// World units per rasterized pixel. Sets the on-screen bubble scale (a ~570 px balloon ≈ 2.7 units).
const WORLD_PER_PX: f32 = 1.0 / 210.0;
/// Height above the owner's origin where the bubble's tail-tip sits (health bar is at Y=2.0).
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
        }
    }

    /// Aged parchment, dark ink, hard pixel edges: the pixel-art look. Speech and thought share the
    /// palette; the shape alone tells them apart.
    pub const fn ink_on_paper() -> Self {
        Self {
            fill: [0xe8, 0xd9, 0xb0, 0xff],
            border: [0x3a, 0x2a, 0x1e, 0xff],
            text: [0x2a, 0x1c, 0x12, 0xff],
            pixel: true,
            thought_fill: [0xe8, 0xd9, 0xb0, 0xff],
            thought_border: [0x3a, 0x2a, 0x1e, 0xff],
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

/// A freshly built bubble face: the material to put on the quad and the quad's world size.
pub struct RenderedBubble {
    pub material: Handle<StandardMaterial>,
    pub size: Vec2,
}

/// Build a bubble's material + world size from its text and style. Called once per line/choice (on
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
        size: Vec2::new(w as f32 * WORLD_PER_PX, h as f32 * WORLD_PER_PX),
    }
}

/// Track every bubble to its owner: anchor above the head, face the camera, mirror visibility,
/// despawn orphans. Cosmetic → `Update` (never pinned state). Copies the `health.rs` billboard.
pub fn track_bubbles<C: Component>(
    mut commands: Commands,
    camera: Single<&GlobalTransform, With<C>>,
    owners: Query<(&Transform, &Visibility), Without<Bubble>>,
    mut bubbles: Query<(Entity, &Bubble, &mut Transform, &mut Visibility)>,
) {
    let cam_rot = camera.rotation();
    let cam_pos = camera.translation();
    let up = cam_rot * Vec3::Y;
    let right = cam_rot * Vec3::X;
    for (entity, bubble, mut tf, mut vis) in &mut bubbles {
        let Ok((owner_tf, owner_vis)) = owners.get(bubble.owner) else {
            commands.entity(entity).despawn();
            continue;
        };
        let anchor = owner_tf.translation + Vec3::Y * BUBBLE_ANCHOR_Y;
        let half_h = tf.scale.y * 0.5;
        let toward = (cam_pos - anchor).normalize_or_zero();
        tf.translation = anchor
            + up * (half_h + bubble.offset.y)
            + right * bubble.offset.x
            + toward * CAMERA_NUDGE;
        tf.rotation = cam_rot;
        *vis = *owner_vis;
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

fn palette(style: &BubbleStyle) -> Palette {
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

/// Advance-width of a string in the scaled font (with kerning).
fn measure<F: Font>(sf: &impl ScaleFont<F>, s: &str) -> f32 {
    let mut w = 0.0;
    let mut prev = None;
    for c in s.chars() {
        let id = sf.glyph_id(c);
        if let Some(p) = prev {
            w += sf.kern(p, id);
        }
        w += sf.h_advance(id);
        prev = Some(id);
    }
    w
}

/// Greedy word-wrap to `max_w` px, respecting explicit newlines.
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

/// Horizontal padding wide enough that the text block's corners clear the balloon's corner arc.
///
/// Split out of `rasterize` so it can be tested without a font: the property that matters is pure
/// geometry, and requiring a `.ttf` to check it would mean not checking it. See
/// `text_corners_stay_inside_the_balloon`.
fn horizontal_pad(text_w: f32, text_h: f32, thought: bool) -> f32 {
    let balloon_h = text_h + 2.0 * PAD;
    // The worst point on the text block: its outer corner, relative to the balloon centre.
    let corner = Vec2::new(text_w * 0.5, text_h * 0.5);
    // Clear of the stroke, not merely inside the silhouette — a glyph touching the border reads as a
    // rendering fault even though it is technically within the shape.
    let clear = BORDER + 4.0;

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
            SPEECH_RADIUS.min(half.x).min(half.y)
        };
        rounded_rect_sdf(corner, half, r) <= -clear
    };

    // A speech balloon's radius is fixed and small, so it almost always already fits and pays nothing.
    if fits(PAD) {
        return PAD;
    }
    let mut lo = PAD;
    let mut hi = PAD + text_h + clear;
    // Widen until the upper bound genuinely fits, so the bisection below has a bracket. Bounded
    // rather than `while`, because a shape that never fits must not hang the caller.
    for _ in 0..8 {
        if fits(hi) {
            break;
        }
        hi += text_h.max(PAD);
    }
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if fits(mid) { hi = mid } else { lo = mid }
    }
    hi
}

/// Rasterize a bubble to RGBA8 bytes + dimensions.
fn rasterize(font: &FontArc, style: &BubbleStyle, text: &str) -> (Vec<u8>, u32, u32) {
    let pal = palette(style);
    let sf = font.as_scaled(PxScale::from(CAP_PX));
    let line_h = sf.height() + sf.line_gap();

    let lines = wrap_lines(&sf, text, MAX_TEXT_W);
    let text_w = lines
        .iter()
        .map(|l| measure(&sf, l))
        .fold(0.0_f32, f32::max)
        .max(MIN_TEXT_W);
    let text_h = line_h * lines.len() as f32;

    let balloon_h = text_h + 2.0 * PAD;
    let thought = matches!(style.kind, BubbleKind::Thought);

    // **Horizontal padding has to clear the corner arc, not just the flat edge.**
    //
    // A thought balloon is a pill: its radius is half the balloon height, so the left and right caps
    // are semicircles that curve inward by `r - sqrt(r^2 - dy^2)` at distance `dy` above and below the
    // centre line. The text block's top and bottom corners sit at `dy = text_h/2`, which is exactly
    // where that inset is largest — so a flat `PAD` is only enough while the balloon is short.
    //
    // Measured, before this existed: one line cleared it with 11px to spare, two lines overflowed the
    // cap by ~5px and three by ~23px, which is why the clipping looked intermittent rather than broken.
    // Speech balloons need it too, just less: their radius is a fixed `SPEECH_RADIUS`.
    let pad_x = horizontal_pad(text_w, text_h, thought);
    let balloon_w = text_w + 2.0 * pad_x;
    let tail_h = if style.tail { TAIL_H } else { 0.0 };
    let img_w = (balloon_w + 2.0 * MARGIN).ceil() as u32;
    let img_h = (balloon_h + 2.0 * MARGIN + tail_h).ceil() as u32;

    let mut buf = vec![0u8; (img_w * img_h * 4) as usize];

    // Balloon body.
    let cx = MARGIN + balloon_w / 2.0;
    let cy = MARGIN + balloon_h / 2.0;
    let half = Vec2::new(balloon_w / 2.0, balloon_h / 2.0);
    let radius = if thought {
        (balloon_h * 0.5).min(balloon_w * 0.5) // pill
    } else {
        SPEECH_RADIUS.min(half.x).min(half.y)
    };
    for py in 0..img_h {
        for px in 0..img_w {
            let p = Vec2::new(px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
            let d = rounded_rect_sdf(p, half, radius);
            let outer = pal.edge(d); // 1 inside, 0 outside, AA at edge unless the skin is pixel
            if outer <= 0.0 {
                continue;
            }
            if d >= -BORDER {
                blend(&mut buf, img_w, img_h, px as i32, py as i32, pal.border, outer);
            } else {
                blend(&mut buf, img_w, img_h, px as i32, py as i32, pal.fill, 1.0);
            }
        }
    }

    // Tail toward the owner (below the balloon).
    if style.tail {
        let base_y = MARGIN + balloon_h;
        if thought {
            // Trailing shrinking dots — the classic thought tail.
            draw_disc(&mut buf, img_w, img_h, cx, base_y + 7.0, 8.0, &pal);
            draw_disc(&mut buf, img_w, img_h, cx - 4.0, base_y + 18.0, 5.0, &pal);
            draw_disc(&mut buf, img_w, img_h, cx - 8.0, base_y + 26.0, 3.0, &pal);
        } else {
            // Pointed speech tail.
            let a = Vec2::new(cx - 14.0, base_y - 2.0);
            let b = Vec2::new(cx + 14.0, base_y - 2.0);
            let tip = Vec2::new(cx - 6.0, base_y + TAIL_H);
            draw_triangle(&mut buf, img_w, img_h, a, b, tip, &pal);
        }
    }

    // Text glyphs, top-down.
    let left = MARGIN + pad_x;
    let mut baseline = MARGIN + PAD + sf.ascent();
    for line in &lines {
        let mut caret = left;
        let mut prev = None;
        for c in line.chars() {
            let id = sf.glyph_id(c);
            if let Some(p) = prev {
                caret += sf.kern(p, id);
            }
            let mut glyph = sf.scaled_glyph(c);
            glyph.position = point(caret, baseline);
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
            if d >= -BORDER {
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
            let col = if edge_dist <= BORDER { pal.border } else { pal.fill };
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
    /// block's top and bottom corners. With a flat `PAD` that was fine for one line and wrong for
    /// three: measured overflow was ~5px at two lines and ~23px at three, which is why the clipping
    /// read as intermittent rather than as a bug.
    ///
    /// This asserts the property directly — the worst point of the text rect must sit inside the
    /// fill, clear of the border — for both shapes at one through four lines.
    #[test]
    fn text_corners_stay_inside_the_balloon() {
        for lines in 1..=4 {
            for thought in [false, true] {
                let text_w = 300.0;
                let text_h = 56.0 * lines as f32;
                let pad = horizontal_pad(text_w, text_h, thought);
                let balloon_w = text_w + 2.0 * pad;
                let balloon_h = text_h + 2.0 * PAD;
                let half = Vec2::new(balloon_w * 0.5, balloon_h * 0.5);
                let radius = if thought {
                    (balloon_h * 0.5).min(balloon_w * 0.5)
                } else {
                    SPEECH_RADIUS.min(half.x).min(half.y)
                };
                // The outer corner of the text block, relative to the balloon centre.
                let corner = Vec2::new(text_w * 0.5, text_h * 0.5);
                let d = rounded_rect_sdf(corner, half, radius);
                assert!(
                    d < -BORDER,
                    "lines={lines} thought={thought}: text corner sits at sdf {d:.2}, which is not \
                     clear of the {BORDER}px border — glyphs there draw over the outline or outside it"
                );
            }
        }
    }

    /// A pill only needs the extra width once it is tall enough to curve into the text, so a
    /// single-line balloon must not get silently fatter for nothing.
    #[test]
    fn one_line_pays_nothing_for_the_fix() {
        assert_eq!(horizontal_pad(300.0, 56.0, true), PAD);
        assert_eq!(horizontal_pad(300.0, 56.0, false), PAD);
        assert!(horizontal_pad(300.0, 168.0, true) > PAD, "three lines must widen the pill");
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
        let err = BubbleAssets::from_font_bytes(Handle::default(), b"definitely not a font".to_vec())
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
        let (rgba, ..) = rasterize(&font, &styled(BubbleKind::Speech, true, BalloonSkin::crt()), LINE);
        assert!(
            rgba.as_chunks::<4>().0.iter().any(|px| px[3] != 0 && px[3] != 255),
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

        for (skin, nearest) in [(BalloonSkin::ink_on_paper(), true), (BalloonSkin::crt(), false)] {
            let rendered = build_bubble(
                &assets,
                &mut images,
                &mut materials,
                &styled(BubbleKind::Speech, true, skin),
                "Sampler.",
            );
            let material = materials.get(&rendered.material).expect("the material exists");
            let handle = material.base_color_texture.as_ref().expect("a face texture");
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
}
