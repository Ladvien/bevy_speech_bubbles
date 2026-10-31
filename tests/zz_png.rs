//! Throwaway: dump balloon rasters to PNG for the visual check. Never committed.
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_speech_bubbles::{BalloonSkin, BubbleAssets, BubbleKind, BubbleStyle, Emotion, FontArc, build_bubble};

#[test]
fn dump() {
    let out = std::env::var("PNG_DIR").unwrap();
    let latin = std::fs::read("/System/Library/Fonts/Supplemental/Arial.ttf").unwrap();
    let han = std::fs::read("/System/Library/Fonts/Supplemental/Arial Unicode.ttf").unwrap();
    let cases: Vec<(&str, Vec<u8>, String)> = vec![
        ("german", latin.clone(), "Donaudampfschifffahrtsgesellschaftskapitaensmuetzenverbandsvorsitzender, sagte er.".into()),
        ("sentence", latin.clone(), "Hold the gate until dawn, then ring the bell twice and do not look at what stands in the road.".into()),
        ("url", latin.clone(), "Read https://example.com/a/very/long/path/that/never/ends/index.html tonight.".into()),
        ("han", han, "今晚的风很大，请把灯点亮，不要看路上站着的东西，守夜人的小屋就在山坡上。".into()),
    ];
    for (label, bytes, text) in cases {
        let assets = BubbleAssets { quad: Handle::default(), font: FontArc::try_from_vec(bytes).unwrap() };
        for (name, skin) in [("crt", BalloonSkin::crt()), ("ink", BalloonSkin::ink_on_paper())] {
            for kind in [BubbleKind::Speech, BubbleKind::Thought] {
                let mut images = Assets::<Image>::default();
                let mut materials = Assets::<StandardMaterial>::default();
                let style = BubbleStyle { kind, emotion: Emotion::Neutral, tail: true, skin };
                let r = build_bubble(&assets, &mut images, &mut materials, &style, &text);
                let m = materials.get(&r.material).unwrap();
                let img = images.get(m.base_color_texture.as_ref().unwrap()).unwrap();
                // Composite over mid grey so the translucent crt skin is visible; pixel skins are
                // magnified 3x with nearest so a texel is a block.
                let (w, h) = (img.width() as usize, img.height() as usize);
                let data = img.data.clone().unwrap();
                let k = if name == "ink" { 3 } else { 1 };
                let mut buf = vec![0u8; w * k * h * k * 4];
                for y in 0..h * k {
                    for x in 0..w * k {
                        let s = ((y / k) * w + x / k) * 4;
                        let a = data[s + 3] as f32 / 255.0;
                        let o = (y * w * k + x) * 4;
                        for c in 0..3 {
                            buf[o + c] = (data[s + c] as f32 * a + 90.0 * (1.0 - a)) as u8;
                        }
                        buf[o + 3] = 255;
                    }
                }
                let big = Image::new(
                    Extent3d { width: (w * k) as u32, height: (h * k) as u32, depth_or_array_layers: 1 },
                    TextureDimension::D2,
                    buf,
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                );
                big.try_into_dynamic().unwrap().save(format!("{out}/{label}_{name}_{kind:?}.png")).unwrap();
                println!("{label} {name} {kind:?}: {w}x{h}");
            }
        }
    }
}
