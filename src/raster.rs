//! Rasterize an SVG frame to pixels or a PNG, behind the `png` feature.
//!
//! [`crate::render_to_svg`] is the visual channel, but an SVG still needs a
//! renderer to become something a vision model, a terminal image protocol or a
//! PR comment can show. A browser works, but draws with whatever fonts the
//! machine has, so the same frame differs from host to host (and with no
//! monospace font at all, a pure-Rust rasterizer drops the text silently).
//!
//! This module closes that gap: it rasterizes with `resvg` and a font database
//! built from *only* the fonts embedded in the crate (`assets/fonts/`), never
//! the system's. A frame is therefore the same on every machine. The fonts are
//! subsets (DejaVu Sans Mono in four faces, so bold and italic draw, and a
//! common set of Noto Color Emoji) made by `scripts/subset_fonts.py`, which
//! lists the ranges; widening them means rerunning it, not changing this code.
//!
//! A character no embedded face has draws as DejaVu's `.notdef` box (the
//! subsets keep its outline), never as blank, so a hole in a frame is visible.

use std::sync::{Arc, OnceLock};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree, fontdb};

use crate::{HeadlessApp, render_to_svg};

/// A rasterized image: `rgba` is `width * height * 4` bytes of straight
/// (non-premultiplied) RGBA8, row-major from the top left.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The embedded font database, built once per process. The monospace family is
/// pointed at DejaVu Sans Mono, so the frames' `monospace` fallback resolves to
/// it; the emoji font is found by glyph fallback.
fn embedded_fonts() -> Arc<fontdb::Database> {
    static FONTS: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = fontdb::Database::new();
            for font in [
                // The text faces come first: glyph fallback takes the first
                // face that has the character, so text beats emoji.
                &include_bytes!("../assets/fonts/DejaVuSansMono-subset.ttf")[..],
                &include_bytes!("../assets/fonts/DejaVuSansMono-Bold-subset.ttf")[..],
                &include_bytes!("../assets/fonts/DejaVuSansMono-Oblique-subset.ttf")[..],
                &include_bytes!("../assets/fonts/DejaVuSansMono-BoldOblique-subset.ttf")[..],
                &include_bytes!("../assets/fonts/NotoColorEmoji-COLRv1-subset.ttf")[..],
            ] {
                db.load_font_data(font.to_vec());
            }
            db.set_monospace_family("DejaVu Sans Mono");
            Arc::new(db)
        })
        .clone()
}

/// Draw `svg` at `scale` times its own size with the embedded fonts, or `None`
/// if it does not parse, has an empty size, or `scale` is not a positive finite
/// number. Vectors and glyphs are drawn at the final size (not enlarged
/// afterwards), so a larger scale is sharper, not blurrier.
fn render_pixmap(svg: &[u8], scale: f32) -> Option<Pixmap> {
    if !(scale.is_finite() && scale > 0.0) {
        return None;
    }
    let opt = Options {
        fontdb: embedded_fonts(),
        ..Default::default()
    };
    let tree = Tree::from_data(svg, &opt).ok()?;
    let size = tree.size();
    let px = |len: f32| (len * scale).ceil() as u32;
    let mut pixmap = Pixmap::new(px(size.width()), px(size.height()))?;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(pixmap)
}

/// Rasterize SVG source at its own size; [`rasterize_svg_scaled`] at scale 1.
pub fn rasterize_svg(svg: &[u8]) -> Option<Raster> {
    rasterize_svg_scaled(svg, 1.0)
}

/// Rasterize SVG source at `scale` times its own size (2.0 doubles both
/// dimensions), rounding a fractional pixel size up. A character no embedded
/// face has draws as a box (see the module docs); text in a family the
/// embedded fonts lack falls back to them. `None` for a malformed SVG, an empty
/// size or a `scale` that is not a positive finite number.
pub fn rasterize_svg_scaled(svg: &[u8], scale: f32) -> Option<Raster> {
    let pixmap = render_pixmap(svg, scale)?;
    Some(Raster {
        width: pixmap.width(),
        height: pixmap.height(),
        rgba: pixmap.take_demultiplied(),
    })
}

/// Rasterize SVG source to PNG file bytes; [`svg_to_png_scaled`] at scale 1.
pub fn svg_to_png(svg: &[u8]) -> Option<Vec<u8>> {
    svg_to_png_scaled(svg, 1.0)
}

/// Rasterize SVG source to PNG file bytes at `scale` times its own size. `None`
/// as for [`rasterize_svg_scaled`].
pub fn svg_to_png_scaled(svg: &[u8], scale: f32) -> Option<Vec<u8>> {
    render_pixmap(svg, scale)?.encode_png().ok()
}

/// Drive a [`HeadlessApp`] at the given size and render the frame to PNG
/// bytes: [`render_to_svg`] then [`svg_to_png`].
pub fn render_to_png<A: HeadlessApp>(app: &A, w: u16, h: u16) -> Option<Vec<u8>> {
    svg_to_png(render_to_svg(app, w, h).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="40"
        viewBox="0 0 200 40" font-family="monospace" font-size="14">
        <rect width="200" height="40" fill="#1d1f21"/>
        <g fill="#c5c8c6"><text xml:space="preserve" y="24">
        <tspan x="10">abc ─│─</tspan><tspan x="120">🐑</tspan><tspan x="150">⭐</tspan></text></g></svg>"##;
    const FRAME: &[u8] = FRAME_SVG.as_bytes();

    /// Only the embedded fonts are loaded (never the system's), so a frame with
    /// text, a box-drawing character and a color emoji draws the same on a
    /// machine with no fonts at all. A non-gray pixel can only be an emoji: the
    /// text and the background are gray. (Noto's 🐑 is itself white and gray, so
    /// ⭐ is what proves color.)
    #[test]
    fn text_and_color_emoji_render_from_the_embedded_fonts_alone() {
        let r = rasterize_svg(FRAME).unwrap();
        assert_eq!((r.width, r.height), (200, 40));
        assert_eq!(r.rgba.len(), 200 * 40 * 4);
        let bg = [0x1d, 0x1f, 0x21, 0xff];
        assert!(r.rgba.chunks(4).any(|p| p != bg), "blank frame");
        let gray = |p: &[u8]| p[0].abs_diff(p[1]) < 24 && p[1].abs_diff(p[2]) < 24;
        assert!(!r.rgba.chunks(4).all(gray), "no color: emoji font missing");
        // Only the embedded faces exist: no system font could have drawn this.
        assert_eq!(embedded_fonts().faces().count(), 5);
    }

    /// An SVG with one row of text: `body` is `<tspan>`s on a gray-on-dark
    /// 200x40 canvas, `style` extra attributes for the `<text>`.
    fn text_svg(style: &str, body: &str) -> Vec<u8> {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="40"
            viewBox="0 0 200 40" font-family="monospace" font-size="14">
            <rect width="200" height="40" fill="#1d1f21"/>
            <g fill="#c5c8c6"><text xml:space="preserve" y="24" {style}>{body}</text></g></svg>"##
        )
        .into_bytes()
    }

    const BG: [u8; 4] = [0x1d, 0x1f, 0x21, 0xff];

    /// Does any pixel in columns `x0..x1` differ from the background?
    fn has_ink(r: &Raster, x0: u32, x1: u32) -> bool {
        (0..r.height).any(|y| {
            (x0..x1).any(|x| {
                let i = ((y * r.width + x) * 4) as usize;
                r.rgba[i..i + 4] != BG
            })
        })
    }

    /// Box Drawing corners (blank in the first subset) draw pixels, each in
    /// its own 20px slot.
    #[test]
    fn box_drawing_corners_draw() {
        let body: String = "┌┐└┘"
            .chars()
            .enumerate()
            .map(|(i, c)| format!(r#"<tspan x="{}">{c}</tspan>"#, 10 + i * 20))
            .collect();
        let r = rasterize_svg(&text_svg("", &body)).unwrap();
        for i in 0..4 {
            let x = 10 + i * 20;
            assert!(has_ink(&r, x, x + 9), "corner {i} is blank");
        }
    }

    /// `font-style:italic` (what toque writes for `Modifier::ITALIC`) selects
    /// the embedded oblique face: the same text draws differently, and the db
    /// has a face that is not upright to pick.
    #[test]
    fn italic_text_differs_from_upright() {
        use resvg::usvg::fontdb::Style;
        assert!(embedded_fonts().faces().any(|f| f.style != Style::Normal));
        let upright = rasterize_svg(&text_svg("", r#"<tspan x="10">Italic fox</tspan>"#)).unwrap();
        let italic = rasterize_svg(&text_svg(
            r#"font-style="italic""#,
            r#"<tspan x="10">Italic fox</tspan>"#,
        ))
        .unwrap();
        assert_ne!(upright.rgba, italic.rgba, "italic drew upright");
        assert!(has_ink(&italic, 10, 100));
    }

    /// A text face that lacks a character makes the rasterizer fall back to
    /// another face for the rest of the run, so a bold or italic run holding a
    /// box-drawing character or a Braille pattern would silently draw upright
    /// and light. Every text face has them (`scripts/subset_fonts.py`), so the
    /// text after the character is the same with and without it, in each style.
    #[test]
    fn a_wide_charset_keeps_bold_and_italic_after_symbols() {
        for style in [
            r#"font-weight="bold""#,
            r#"font-style="italic""#,
            r#"font-weight="bold" font-style="italic""#,
        ] {
            let with = |lead: &str| {
                let body = format!(r#"<tspan x="10">{lead} Style fox</tspan>"#);
                rasterize_svg(&text_svg(style, &body)).unwrap()
            };
            // Columns from the third cell on: the same glyphs, in the same face.
            let tail = |r: &Raster| {
                let from = (30 * 4) as usize;
                (0..r.height as usize)
                    .flat_map(|y| {
                        let row = y * r.width as usize * 4;
                        r.rgba[row + from..row + r.width as usize * 4].to_vec()
                    })
                    .collect::<Vec<u8>>()
            };
            let plain = tail(&with("x"));
            for lead in ["│", "┌", "⣿", "●", "→"] {
                assert_eq!(
                    plain,
                    tail(&with(lead)),
                    "{lead} in {style} changed the face"
                );
            }
        }
    }

    /// A character in no embedded face (a Powerline glyph, private use, not in
    /// DejaVu) draws the `.notdef` box instead of nothing.
    #[test]
    fn a_missing_glyph_draws_a_visible_box() {
        let r = rasterize_svg(&text_svg("", "<tspan x=\"10\">\u{e0b0}</tspan>")).unwrap();
        assert!(has_ink(&r, 10, 20), "missing glyph is blank");
        let blank = rasterize_svg(&text_svg("", "<tspan x=\"10\"> </tspan>")).unwrap();
        assert_ne!(r.rgba, blank.rgba);
    }

    #[test]
    fn scaled_output_scales_the_dimensions() {
        let r = rasterize_svg_scaled(FRAME, 2.0).unwrap();
        assert_eq!((r.width, r.height), (400, 80));
        assert_eq!(r.rgba.len(), 400 * 80 * 4);
        let png = svg_to_png_scaled(FRAME, 2.0).unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(rasterize_svg(FRAME), rasterize_svg_scaled(FRAME, 1.0));
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(rasterize_svg_scaled(FRAME, bad).is_none());
        }
    }

    #[test]
    fn svg_to_png_writes_a_png() {
        let png = svg_to_png(FRAME).unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn a_malformed_svg_is_none() {
        assert!(rasterize_svg(b"not svg").is_none());
        assert!(svg_to_png(b"<svg").is_none());
    }
}
