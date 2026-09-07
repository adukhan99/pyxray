//! pyxray-render — turn a [`pyxray_core::Report`] into something you can look
//! at: in a terminal, in a browser, or in a document.
//!
//! The pipeline is always the same three steps. A [`layout::Layout`] plans
//! where the panels go, the panels draw themselves into a ratatui buffer with
//! a [`theme::Theme`], and an exporter serialises that buffer. Adding a look
//! means adding a theme; adding an arrangement means adding a layout.

pub mod canvas;
pub mod export;
pub mod layout;
pub mod panels;
pub mod theme;

pub use export::Format;
pub use layout::Layout;
pub use panels::{Icons, Opts, Panel};
pub use theme::Theme;

use pyxray_core::model::Report;

/// Everything needed to produce one picture.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub theme: Theme,
    pub layout: Layout,
    pub opts: Opts,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            theme: theme::default_theme(),
            layout: Layout::Dashboard,
            opts: Opts::default(),
        }
    }
}

/// Render a report to a string in the given format, sized to `width`. Pass
/// `height` only when the output has to fit a live terminal.
pub fn render_to_string(
    report: &Report,
    style: &Style,
    format: Format,
    width: u16,
    height: Option<u16>,
) -> String {
    let buf = layout::render(
        report,
        &style.theme,
        &style.opts,
        style.layout,
        width,
        height,
    );
    export::emit(&buf, &style.theme, format, &report.meta.name)
}
