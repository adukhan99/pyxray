//! Python bindings.
//!
//! The report crosses the boundary as JSON rather than as a hand-built dict:
//! the schema is already defined once by serde in `pyxray-core`, and keeping
//! one definition beats keeping two in step.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyxray_render::export::{self, Format};
use pyxray_render::layout::{self, Layout};
use pyxray_render::panels::{Icons, Opts};
use pyxray_render::theme::{self, THEMES};

fn pick_theme(id: &str) -> PyResult<theme::Theme> {
    theme::theme(id).ok_or_else(|| PyValueError::new_err(format!("unknown theme {id:?}")))
}

fn pick_layout(id: &str) -> PyResult<Layout> {
    layout::layout(id).ok_or_else(|| PyValueError::new_err(format!("unknown layout {id:?}")))
}

fn pick_format(id: &str) -> PyResult<Format> {
    export::format(id).ok_or_else(|| PyValueError::new_err(format!("unknown format {id:?}")))
}

fn pick_icons(id: &str) -> PyResult<Icons> {
    match id {
        "glyph" => Ok(Icons::Glyph),
        "tag" => Ok(Icons::Tag),
        "both" => Ok(Icons::Both),
        other => Err(PyValueError::new_err(format!(
            "unknown icon mode {other:?}"
        ))),
    }
}

/// Analyse `source` and return the report as a JSON string.
#[pyfunction]
#[pyo3(signature = (source, name = "<stdin>"))]
fn analyze_json(source: &str, name: &str) -> PyResult<String> {
    let report = pyxray_core::xray(source, name);
    serde_json::to_string(&report).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Render `source` to a string in the requested format.
#[pyfunction]
#[pyo3(signature = (
    source,
    name = "<stdin>",
    theme = "blueprint",
    layout = "card",
    format = "ansi",
    width = 100,
    height = None,
    icons = "both",
    depth = 6,
    gutter = true,
    code = false,
))]
#[allow(clippy::too_many_arguments)]
fn render(
    source: &str,
    name: &str,
    theme: &str,
    layout: &str,
    format: &str,
    width: u16,
    height: Option<u16>,
    icons: &str,
    depth: u16,
    gutter: bool,
    code: bool,
) -> PyResult<String> {
    let report = pyxray_core::xray(source, name);
    let style = pyxray_render::Style {
        theme: pick_theme(theme)?,
        layout: pick_layout(layout)?,
        opts: Opts {
            max_depth: depth,
            gutter,
            icons: pick_icons(icons)?,
            code,
        },
    };
    Ok(pyxray_render::render_to_string(
        &report,
        &style,
        pick_format(format)?,
        width,
        height,
    ))
}

/// Pull the Python out of a shell command: `python3 <<'EOF' … EOF`,
/// `python -c '…'`, or a bare script. Returns `(source, label)`.
#[pyfunction]
fn extract(command: &str) -> (String, String) {
    pyxray_core::extract_python(command)
}

/// The available themes, as `(id, name, blurb)` triples.
#[pyfunction]
fn themes() -> Vec<(String, String, String)> {
    THEMES
        .iter()
        .map(|t| (t.id.to_string(), t.name.to_string(), t.blurb.to_string()))
        .collect()
}

/// The available layouts, as `(id, blurb)` pairs.
#[pyfunction]
fn layouts() -> Vec<(String, String)> {
    Layout::all()
        .into_iter()
        .map(|l| (l.id().to_string(), l.blurb().to_string()))
        .collect()
}

#[pymodule]
fn _pyxray(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(analyze_json, m)?)?;
    m.add_function(wrap_pyfunction!(render, m)?)?;
    m.add_function(wrap_pyfunction!(extract, m)?)?;
    m.add_function(wrap_pyfunction!(themes, m)?)?;
    m.add_function(wrap_pyfunction!(layouts, m)?)?;
    Ok(())
}
