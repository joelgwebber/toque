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
//! subset to the glyphs toque frames use today; widening the subset means
//! replacing the files, not changing this code.

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
                &include_bytes!("../assets/fonts/DejaVuSansMono-subset.ttf")[..],
                &include_bytes!("../assets/fonts/DejaVuSansMono-Bold-subset.ttf")[..],
                &include_bytes!("../assets/fonts/NotoColorEmoji-COLRv1-subset.ttf")[..],
            ] {
                db.load_font_data(font.to_vec());
            }
            db.set_monospace_family("DejaVu Sans Mono");
            Arc::new(db)
        })
        .clone()
}

/// Draw `svg` at its own size with the embedded fonts, or `None` if it does
/// not parse or has an empty size.
fn render_pixmap(svg: &[u8]) -> Option<Pixmap> {
    let opt = Options {
        fontdb: embedded_fonts(),
        ..Default::default()
    };
    let tree = Tree::from_data(svg, &opt).ok()?;
    let size = tree.size().to_int_size();
    let mut pixmap = Pixmap::new(size.width(), size.height())?;
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    Some(pixmap)
}

/// Rasterize SVG source at its own size. Text in a family the embedded fonts
/// lack is dropped (not replaced by tofu); a malformed SVG yields `None`.
pub fn rasterize_svg(svg: &[u8]) -> Option<Raster> {
    let pixmap = render_pixmap(svg)?;
    Some(Raster {
        width: pixmap.width(),
        height: pixmap.height(),
        rgba: pixmap.take_demultiplied(),
    })
}

/// Rasterize SVG source to PNG file bytes. `None` as for [`rasterize_svg`].
pub fn svg_to_png(svg: &[u8]) -> Option<Vec<u8>> {
    render_pixmap(svg)?.encode_png().ok()
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
        assert_eq!(embedded_fonts().faces().count(), 3);
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
