//! **World-space speech and thought balloons.**
//!
//! A bubble is text rasterized on the CPU into an `Image`, put on a unit quad as an unlit
//! alpha-blended material, anchored above an owner entity and turned to face a camera. No 2D camera,
//! no UI layer, no screen-space overlay — the balloon lives in the world, occludes and is occluded
//! like anything else, and moves with the thing that said it.
//!
//! # Why the text is rasterized here
//!
//! Bevy 0.19's text stack has no public "rasterize this string into a standalone `Image`" API, and a
//! world-space balloon has no 2D camera to host a `Text2d`. So the glyphs go through `ab_glyph` into an
//! RGBA buffer, once per line rather than per frame, and the buffer becomes a texture.
//!
//! Two balloon shapes are drawn: a rounded rect with a pointed tail for [`BubbleKind::Speech`], and a
//! soft pill with a trailing dot-tail for [`BubbleKind::Thought`]. [`Emotion`] tints the border —
//! balloon colour reliably conveys affect (An et al., *AniBalloons*, arXiv:2408.06294).
//!
//! # Name your camera
//!
//! [`track_bubbles`] is generic over a camera **marker component**, and that is deliberate rather than
//! incidental. Bevy's `Single<..>` silently skips its system when the query does not match exactly one
//! entity — so a system filtered on `With<Camera3d>` works perfectly until the day something spawns a
//! second 3D camera, at which point every bubble quietly stops tracking and nothing errors.
//!
//! Being generic makes it impossible for this crate to express that filter. You must name a marker you
//! control:
//!
//! ```ignore
//! app.add_systems(Update, (
//!     bevy_speech_bubbles::track_bubbles::<MyMainCamera>,
//!     bevy_speech_bubbles::expire_bubbles,
//! ));
//! ```
//!
//! # The font is yours to load
//!
//! [`BubbleAssets`] holds a quad mesh and a `FontArc`, and the crate never reads a file: a library
//! that assumed an asset path would be assuming your project layout. Build the resource yourself, from
//! wherever your fonts live, and insert it.
//!
//! # Cosmetic by construction
//!
//! Both systems belong on `Update`, and [`expire_bubbles`] reads `Time<Real>` rather than virtual time
//! so bubbles still expire while the simulation is paused — a modal conversation that zeroed virtual
//! time would otherwise freeze its own dialogue on screen forever.

use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::time::Real;

pub use style::{BubbleKind, Emotion};

mod style;

// --- rasterization tuning (pixels) ---
/// Glyph pixel scale. High enough that the baked texture stays crisp when the ortho camera zooms in.
const CAP_PX: f32 = 40.0;
/// Padding between text and the balloon edge.
const PAD: f32 = 20.0;
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

/// How a bubble should be drawn.
pub struct BubbleStyle {
    pub kind: BubbleKind,
    pub emotion: Emotion,
    /// Whether to draw a tail toward the owner (lines/barks: yes; choice menu items: no).
    pub tail: bool,
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
    let image = Image::new(
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

/// Balloon fill / border / text colors for a kind+emotion (CRT phosphor palette; emotion tints the
/// border per AniBalloons).
struct Palette {
    fill: [u8; 4],
    border: [u8; 4],
    text: [u8; 4],
}

fn palette(style: &BubbleStyle) -> Palette {
    let (fill, base_border) = match style.kind {
        BubbleKind::Speech => ([8, 20, 12, 240], [140, 255, 158, 255]),
        BubbleKind::Thought => ([8, 16, 22, 232], [150, 210, 255, 240]),
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
        text: [226, 240, 226, 255],
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

    let balloon_w = text_w + 2.0 * PAD;
    let balloon_h = text_h + 2.0 * PAD;
    let tail_h = if style.tail { TAIL_H } else { 0.0 };
    let img_w = (balloon_w + 2.0 * MARGIN).ceil() as u32;
    let img_h = (balloon_h + 2.0 * MARGIN + tail_h).ceil() as u32;

    let mut buf = vec![0u8; (img_w * img_h * 4) as usize];

    // Balloon body.
    let cx = MARGIN + balloon_w / 2.0;
    let cy = MARGIN + balloon_h / 2.0;
    let half = Vec2::new(balloon_w / 2.0, balloon_h / 2.0);
    let thought = matches!(style.kind, BubbleKind::Thought);
    let radius = if thought {
        (balloon_h * 0.5).min(balloon_w * 0.5) // pill
    } else {
        SPEECH_RADIUS.min(half.x).min(half.y)
    };
    for py in 0..img_h {
        for px in 0..img_w {
            let p = Vec2::new(px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
            let d = rounded_rect_sdf(p, half, radius);
            let outer = (0.5 - d).clamp(0.0, 1.0); // 1 inside, 0 outside, AA at edge
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
    let left = MARGIN + PAD;
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
                    blend(&mut buf, img_w, img_h, x, y, pal.text, cov);
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
            let outer = (0.5 - d).clamp(0.0, 1.0);
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
}
