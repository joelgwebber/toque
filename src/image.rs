//! A picture in a pane (feature `image`): decoded pixels drawn by `ratatui-image` over the
//! terminal's graphics protocol (kitty, sixel, iTerm2) when it speaks one, else as half-block
//! cells. Headless frames always take the half-block path: ordinary coloured cells, so the
//! picture lands in text, SVG and PNG snapshots like anything else.
//!
//! The picture is zoomed and panned here, not by `ratatui-image`: each draw crops the visible
//! window out of the decoded pixels, scales it to the pane's pixel grid and hands the library a
//! finished image, so every protocol zooms the same way. The grid is the terminal's cell size in
//! pixels, except for half-blocks, whose "pixel" is one column by half a row (so `100%` is one
//! image pixel per half-block, not per 10x20 cell).
//!
//! The app owns everything around it: decoding (enable the `image` crate's format features you
//! need), the header, and which keys call [`ImageView::zoom_step`], [`ImageView::set_zoom`] and
//! [`ImageView::pan`]. A live app queries the terminal once, before reading events
//! ([`Picker::from_query_stdio`]); tests and headless runs use [`Picker::halfblocks`].
//! Extracted from yaks (`src/tui/imageview.rs`).

use std::cell::{Cell, RefCell};

pub use image::DynamicImage;
use image::RgbaImage;
use image::imageops::overlay;
use ratatui::Frame;
use ratatui::layout::Rect;
pub use ratatui_image::picker::Picker;
use ratatui_image::picker::ProtocolType;
use ratatui_image::protocol::Protocol;
use ratatui_image::{FilterType, FontSize, Image, Resize};

/// How the picture is scaled: to the pane, to its width, or by a factor.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Zoom {
    /// Shrink to fit the pane, never enlarge (the default).
    Fit,
    /// Scale to the pane's width (enlarging a narrow image); a taller image
    /// then pans down.
    FitWidth,
    /// A fixed factor: `1.0` is one image pixel per pixel of the grid.
    Scale(f64),
}

/// The factors `+` and `-` step through (1.0 is actual size).
const ZOOM_STEPS: [f64; 19] = [
    0.05, 0.1, 0.125, 0.25, 0.33, 0.5, 0.67, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0,
    24.0, 32.0,
];

/// The factor for `zoom` showing an `img`-pixel picture in a `pane`-pixel
/// window. An unmeasured pane (no frame drawn yet) scales nothing.
fn scale_for(zoom: Zoom, img: (u32, u32), pane: (u32, u32)) -> f64 {
    let (iw, ih) = (f64::from(img.0.max(1)), f64::from(img.1.max(1)));
    let (pw, ph) = (f64::from(pane.0), f64::from(pane.1));
    match zoom {
        Zoom::Scale(s) => s,
        _ if pw == 0.0 || ph == 0.0 => 1.0,
        Zoom::Fit => (pw / iw).min(ph / ih).min(1.0),
        Zoom::FitWidth => pw / iw,
    }
}

/// The picture's size in pixels at `scale`.
fn shown_size(img: (u32, u32), scale: f64) -> (u32, u32) {
    let side = |n: u32| ((f64::from(n) * scale).round() as u32).max(1);
    (side(img.0), side(img.1))
}

/// The top-left of the window, kept inside the picture: a picture no larger
/// than the pane on an axis has nothing to pan there.
fn clamp_offset(off: (u32, u32), shown: (u32, u32), pane: (u32, u32)) -> (u32, u32) {
    (
        off.0.min(shown.0.saturating_sub(pane.0)),
        off.1.min(shown.1.saturating_sub(pane.1)),
    )
}

/// The pixel grid a picker's cells are drawn on: the font size, but one
/// column by half a row for half-blocks (the library draws those two pixels
/// to a cell whatever the font).
fn grid_of(picker: &Picker) -> FontSize {
    match picker.protocol_type() {
        ProtocolType::Halfblocks => FontSize::new(1, 2),
        _ => picker.font_size(),
    }
}

/// What the cached [`Protocol`] was built for: the factor, the window's
/// offset and the pane's cells.
type DrawKey = (u64, (u32, u32), (u16, u16));

/// A decoded picture shown in a pane, zoomed and panned.
pub struct ImageView {
    /// Pixel size of the decoded image.
    dims: (u32, u32),
    image: DynamicImage,
    /// The picker, its half-block grid corrected by [`grid_of`] (see
    /// [`ImageView::open`]).
    picker: Picker,
    grid: FontSize,
    zoom: Cell<Zoom>,
    /// The window's top-left in pixels of the zoomed picture. Clamped when
    /// drawn too, so a pane that grows pulls it back.
    offset: Cell<(u32, u32)>,
    /// The pane's cells at the last draw (0 before the first), which zoom
    /// and pan measure against; drawing takes `&self`, so these are `Cell`s.
    pane: Cell<(u16, u16)>,
    /// The window's finished protocol, rebuilt only when its [`DrawKey`]
    /// changes, since a sixel or kitty encode is not free.
    drawn: RefCell<Option<(DrawKey, Option<Protocol>)>>,
}

impl ImageView {
    /// A view of `image`, drawn with `picker`'s protocol (half-blocks headless, so text and SVG
    /// snapshots show it). Starts at [`Zoom::Fit`].
    #[must_use]
    pub fn new(image: DynamicImage, picker: &Picker) -> Self {
        let mut picker = picker.clone();
        let grid = grid_of(&picker);
        if picker.protocol_type() == ProtocolType::Halfblocks {
            // The library sizes an image to cells by the picker's font; tell it the half-block
            // grid, so the pixels handed to it are not rescaled.
            #[allow(deprecated)]
            let mut p = Picker::from_fontsize(grid);
            p.set_protocol_type(ProtocolType::Halfblocks);
            picker = p;
        }
        Self {
            dims: (image.width(), image.height()),
            image,
            picker,
            grid,
            zoom: Cell::new(Zoom::Fit),
            offset: Cell::new((0, 0)),
            pane: Cell::new((0, 0)),
            drawn: RefCell::new(None),
        }
    }

    /// The picture's size in pixels.
    #[must_use]
    pub fn dims(&self) -> (u32, u32) {
        self.dims
    }

    /// The zoom mode.
    #[must_use]
    pub fn zoom(&self) -> Zoom {
        self.zoom.get()
    }

    /// The window's top-left, in pixels of the zoomed picture.
    #[must_use]
    pub fn offset(&self) -> (u32, u32) {
        self.offset.get()
    }

    /// The pane in pixels of the grid.
    pub fn pane_px(&self) -> (u32, u32) {
        let (c, r) = self.pane.get();
        (
            u32::from(c) * u32::from(self.grid.width),
            u32::from(r) * u32::from(self.grid.height),
        )
    }

    /// The factor the picture is drawn at now.
    pub fn scale(&self) -> f64 {
        scale_for(self.zoom.get(), self.dims, self.pane_px())
    }

    /// The zoom as the status line says it: `fit`, `fit width`, `200%`.
    pub fn zoom_label(&self) -> String {
        match self.zoom.get() {
            Zoom::Fit => "fit".into(),
            Zoom::FitWidth => "fit width".into(),
            Zoom::Scale(s) => format!("{}%", (s * 100.0).round() as u32),
        }
    }

    /// Whether the picture is larger than the pane on some axis, so `h`/`j`/
    /// `k`/`l` have something to pan.
    pub fn pannable(&self) -> bool {
        let shown = shown_size(self.dims, self.scale());
        let pane = self.pane_px();
        shown.0 > pane.0 || shown.1 > pane.1
    }

    /// Switch to `zoom`, keeping the middle of the window on the same part of
    /// the picture (a mode that fits the pane clamps the offset to 0).
    pub fn set_zoom(&self, zoom: Zoom) {
        let pane = self.pane_px();
        let (old, new) = (self.scale(), scale_for(zoom, self.dims, pane));
        let shown = shown_size(self.dims, old);
        let off = self.offset.get();
        let mid = |o: u32, shown: u32, pane: u32| f64::from(o) + f64::from(shown.min(pane)) / 2.0;
        let (cx, cy) = (
            mid(off.0, shown.0, pane.0) / old,
            mid(off.1, shown.1, pane.1) / old,
        );
        self.zoom.set(zoom);
        let shown = shown_size(self.dims, new);
        let at = |c: f64, shown: u32, pane: u32| {
            (c * new - f64::from(shown.min(pane)) / 2.0)
                .max(0.0)
                .round() as u32
        };
        self.offset.set(clamp_offset(
            (at(cx, shown.0, pane.0), at(cy, shown.1, pane.1)),
            shown,
            pane,
        ));
    }

    /// One step up (`+`) or down (`-`) the [`ZOOM_STEPS`] from the factor
    /// drawn now (so from `fit` it steps off whatever fit came to).
    pub fn zoom_step(&self, up: bool) {
        let cur = self.scale();
        let next = if up {
            ZOOM_STEPS.iter().copied().find(|&s| s > cur * 1.001)
        } else {
            ZOOM_STEPS.iter().rev().copied().find(|&s| s < cur / 1.001)
        };
        if let Some(s) = next {
            self.set_zoom(Zoom::Scale(s));
        }
    }

    /// Pan the window a quarter of the pane (at least a cell) toward `dx`/`dy`
    /// (each -1, 0 or 1), as far as the picture goes.
    pub fn pan(&self, dx: i32, dy: i32) {
        let (pane, shown) = (self.pane_px(), shown_size(self.dims, self.scale()));
        let step = |len: u32, cell: u16| i64::from((len / 4).max(u32::from(cell)));
        let off = self.offset.get();
        let moved = |o: u32, d: i32, len: u32, cell: u16| {
            (i64::from(o) + i64::from(d) * step(len, cell)).max(0) as u32
        };
        self.offset.set(clamp_offset(
            (
                moved(off.0, dx, pane.0, self.grid.width),
                moved(off.1, dy, pane.1, self.grid.height),
            ),
            shown,
            pane,
        ));
    }

    /// The picture's window for a pane of `cols`x`rows` cells: the part of the
    /// zoomed picture at the offset, cropped from the source pixels and
    /// scaled once to exactly its size, on a canvas of whole cells. Returns the
    /// cells it covers.
    fn window(&self, cols: u16, rows: u16) -> Option<((u16, u16), Option<Protocol>)> {
        let (gw, gh) = (u32::from(self.grid.width), u32::from(self.grid.height));
        let pane = (u32::from(cols) * gw, u32::from(rows) * gh);
        self.pane.set((cols, rows));
        let scale = self.scale();
        let shown = shown_size(self.dims, scale);
        let off = clamp_offset(self.offset.get(), shown, pane);
        self.offset.set(off);
        let vis = (shown.0.min(pane.0), shown.1.min(pane.1));
        let cells = (vis.0.div_ceil(gw) as u16, vis.1.div_ceil(gh) as u16);
        let key: DrawKey = (scale.to_bits(), off, (cols, rows));
        let mut drawn = self.drawn.borrow_mut();
        if drawn.as_ref().is_none_or(|(k, _)| *k != key) {
            let proto = self.encode(scale, off, vis, cells);
            *drawn = Some((key, proto));
        }
        drawn.as_ref().map(|(_, p)| (cells, p.clone()))
    }

    /// The protocol for the `vis`-pixel window at `off` of the picture at
    /// `scale`, padded to `cells`.
    fn encode(
        &self,
        scale: f64,
        off: (u32, u32),
        vis: (u32, u32),
        cells: (u16, u16),
    ) -> Option<Protocol> {
        let (iw, ih) = self.dims;
        // The source pixels under the window, widened to whole pixels.
        let src = |o: u32, len: u32, max: u32| {
            let a = ((f64::from(o) / scale).floor() as u32).min(max.saturating_sub(1));
            let b = (((f64::from(o + len)) / scale).ceil() as u32).clamp(a + 1, max);
            (a, b - a)
        };
        let (x, w) = src(off.0, vis.0, iw);
        let (y, h) = src(off.1, vis.1, ih);
        let part = self.image.crop_imm(x, y, w, h);
        let part = if (part.width(), part.height()) == vis {
            part
        } else {
            part.resize_exact(vis.0, vis.1, FilterType::Triangle)
        };
        let canvas = (
            u32::from(cells.0) * u32::from(self.grid.width),
            u32::from(cells.1) * u32::from(self.grid.height),
        );
        let img = if (canvas.0, canvas.1) == vis {
            part
        } else {
            let mut bg = RgbaImage::new(canvas.0, canvas.1);
            overlay(&mut bg, &part.to_rgba8(), 0, 0);
            DynamicImage::ImageRgba8(bg)
        };
        self.picker
            .new_protocol(
                img,
                ratatui::layout::Size::new(cells.0, cells.1),
                Resize::Fit(None),
            )
            .ok()
    }

    /// Draw the window the zoom and pan show into `area`, top-left aligned. It measures the pane
    /// for later zoom and pan, so draw before acting on keys that depend on its size.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if let Some(((w, h), Some(proto))) = self.window(area.width, area.height) {
            frame.render_widget(
                Image::new(&proto),
                Rect {
                    width: w,
                    height: h,
                    ..area
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    use super::*;

    /// A `w`x`h` picture, red on the left half and blue on the right.
    fn halves(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_fn(w, h, |x, _| {
            if x < w / 2 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 255, 255])
            }
        }))
    }

    fn draw(view: &ImageView, cols: u16, rows: u16) -> ratatui::buffer::Buffer {
        let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
        terminal
            .draw(|frame| view.render(frame, frame.area()))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn fit_shrinks_but_never_enlarges_and_fit_width_may_enlarge() {
        let pane = (100, 44);
        assert_eq!(scale_for(Zoom::Fit, (200, 100), pane), 0.44);
        assert_eq!(scale_for(Zoom::Fit, (50, 20), pane), 1.0);
        assert_eq!(scale_for(Zoom::FitWidth, (50, 20), pane), 2.0);
        assert_eq!(scale_for(Zoom::FitWidth, (200, 100), pane), 0.5);
        assert_eq!(scale_for(Zoom::Scale(3.0), (200, 100), pane), 3.0);
        // No frame drawn yet: nothing to measure against.
        assert_eq!(scale_for(Zoom::Fit, (200, 100), (0, 0)), 1.0);
    }

    #[test]
    fn the_window_stays_inside_the_picture() {
        assert_eq!(clamp_offset((90, 90), (300, 200), (100, 44)), (90, 90));
        assert_eq!(clamp_offset((500, 500), (300, 200), (100, 44)), (200, 156));
        // A picture that fits an axis has nothing to pan on it.
        assert_eq!(clamp_offset((9, 9), (80, 200), (100, 44)), (0, 9));
        assert_eq!(shown_size((200, 100), 0.5), (100, 50));
        assert_eq!(shown_size((1, 1), 0.05), (1, 1));
    }

    #[test]
    fn a_picture_draws_as_half_block_cells_in_its_own_colours() {
        let view = ImageView::new(halves(20, 20), &Picker::halfblocks());
        let buf = draw(&view, 10, 10);
        // Fit never enlarges, and the pane is 10x20 half-block pixels: the picture shrinks to
        // 10x10 pixels, which is 10 columns by 5 rows.
        let left = &buf[(1, 1)];
        let right = &buf[(8, 1)];
        assert_eq!(left.fg, Color::Rgb(255, 0, 0), "{left:?}");
        assert_eq!(right.fg, Color::Rgb(0, 0, 255), "{right:?}");
        assert_eq!(buf[(1, 7)].fg, Color::Reset, "nothing below the picture");
        assert_eq!(view.zoom_label(), "fit");
    }

    #[test]
    fn zooming_in_makes_the_picture_pannable_and_panning_stays_inside_it() {
        let view = ImageView::new(halves(20, 20), &Picker::halfblocks());
        draw(&view, 10, 10);
        assert!(!view.pannable());
        view.set_zoom(Zoom::Scale(4.0));
        assert!(view.pannable());
        for _ in 0..50 {
            view.pan(1, 1);
        }
        // 80x80 shown in a 10x20 window: the offset stops at the far edge.
        assert_eq!(view.offset(), (70, 60));
        let buf = draw(&view, 10, 10);
        assert_eq!(buf[(5, 5)].fg, Color::Rgb(0, 0, 255), "the right half");
        view.set_zoom(Zoom::Fit);
        assert_eq!(view.offset(), (0, 0));
    }
}
