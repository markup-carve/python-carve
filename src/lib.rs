//! PyO3 native binding exposing the `carve` (carve-rs) engine to Python.
//!
//! The compiled module is imported as `import carve` and provides:
//!   - `carve.to_html(source)`                       core, no extensions
//!   - `carve.to_html(source, extensions=[...])`     named extensions
//!   - `carve.to_html(source, mode='static')`        static render mode
//!   - `carve.to_html(source, renderers={...})`      build-time renderers
//!   - `carve.to_html_with_extensions(source, exts)` explicit variant
//!   - `carve.to_markdown(source)` / `to_plain_text(source)` / `to_ansi(source)`
//!   - `carve.extensions()`                          list of supported names
//!   - `carve.__version__`
//!
//! We never reimplement the parser; every call delegates to carve-rs.

use carve_rs::{
    Autolink, CarveExtension, Citations, CodeCallouts, Details, ExternalLinks, FencedRender,
    HeadingPermalinks, ListTable, MathBlock, Mode, Options, Spoiler, StaticRenderers, TabNormalize,
    Wikilinks,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// HTML-escape a string for the renderer-failure fallback path.
///
/// carve-rs inserts a *present* static renderer's return value verbatim (it is
/// the renderer's job to produce safe HTML). So when our Python wrapper has to
/// fall back to the construct source - because the callable raised or returned
/// a non-string - that source MUST be escaped here, or a source containing HTML
/// (e.g. `<img onerror=...>`) would be emitted raw. The no-renderer path inside
/// carve-rs already escapes its `<pre><code>` source block; this keeps the
/// failing-renderer floor equally safe rather than a raw-passthrough hole.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Map a Python-facing extension name to an owned boxed carve-rs extension.
///
/// Returns an error for unknown names so typos surface immediately in Python
/// rather than silently producing core output.
fn build_extension(name: &str) -> PyResult<Box<dyn CarveExtension>> {
    let ext: Box<dyn CarveExtension> = match name {
        "autolink" => Box::new(Autolink::new()),
        "details" => Box::new(Details::new()),
        "external_links" => Box::new(ExternalLinks::new()),
        // The mermaid preset carries the static-renderer key, so a static
        // render can consult `renderers={'mermaid': ...}`. (Plain
        // `FencedRender::new("mermaid")` would degrade to source even with a
        // renderer supplied, since it has no static-renderer key.)
        "fenced_render" => Box::new(FencedRender::mermaid()),
        // Chart.js preset (JSON mode); its static path consults
        // `renderers={'chart': ...}`, else degrades to the JSON source.
        "fenced_render_chart" => Box::new(FencedRender::chart()),
        "heading_permalinks" => Box::new(HeadingPermalinks::new()),
        "list_table" => Box::new(ListTable::new()),
        "math_block" => Box::new(MathBlock::new()),
        "spoiler" => Box::new(Spoiler::new()),
        "tab_normalize" => Box::new(TabNormalize::new()),
        "wikilinks" => Box::new(Wikilinks::new()),
        "citations" => Box::new(Citations::new()),
        "code-callouts" => Box::new(CodeCallouts::new()),
        other => {
            return Err(PyValueError::new_err(format!(
                "unknown carve extension: {other:?} (supported: {})",
                SUPPORTED.join(", ")
            )));
        }
    };
    Ok(ext)
}

/// The canonical list of extension names accepted by the binding.
const SUPPORTED: &[&str] = &[
    "autolink",
    "details",
    "external_links",
    "fenced_render",
    "fenced_render_chart",
    "heading_permalinks",
    "list_table",
    "math_block",
    "spoiler",
    "tab_normalize",
    "wikilinks",
    "citations",
    "code-callouts",
];

/// Build an owned vec of boxed extensions from the requested names.
fn boxed_extensions(names: &[String]) -> PyResult<Vec<Box<dyn CarveExtension>>> {
    names.iter().map(|n| build_extension(n)).collect()
}

/// Map a Python-facing mode string to a carve-rs [`Mode`].
///
/// Rejects any unknown string with `ValueError`, mirroring the spec's
/// "MUST reject an unknown mode value" (no guessing). Omitting the mode in
/// Python defaults to `"interactive"`, so existing callers are unaffected.
fn parse_mode(mode: &str) -> PyResult<Mode> {
    match mode {
        "interactive" => Ok(Mode::Interactive),
        "static" => Ok(Mode::Static),
        other => Err(PyValueError::new_err(format!(
            "unknown carve render mode: {other:?} (supported: \"interactive\", \"static\")"
        ))),
    }
}

/// Wrap a Python diagram callable `(str) -> str` into a carve-rs closure.
///
/// The closure acquires the GIL, calls the Python callable with the construct
/// source, and returns its string result. If the callable raises or returns a
/// non-string, the closure degrades to the HTML-ESCAPED source, so a bad
/// renderer never produces blank output and can never inject raw HTML. (A
/// present renderer's return value is emitted verbatim by carve-rs, so the
/// fallback must escape rather than pass source through raw.) The callable is
/// stored as a thread-safe `Py<PyAny>`.
fn wrap_diagram(callable: Py<PyAny>) -> Box<dyn Fn(&str) -> String + 'static> {
    Box::new(move |src: &str| {
        Python::attach(|py| {
            match callable.call1(py, (src,)) {
                Ok(result) => match result.extract::<String>(py) {
                    Ok(s) => s,
                    // Non-string return: fall back to escaped source.
                    Err(_) => escape_html(src),
                },
                // Callable raised: fall back to escaped source rather than
                // propagating (the static path has no error channel).
                Err(_) => escape_html(src),
            }
        })
    })
}

/// Wrap a Python math callable `(str, bool) -> str` into a carve-rs closure.
///
/// Same contract as [`wrap_diagram`] (including the HTML-escaped fallback on a
/// raising / non-string-returning callable), but the callable receives the TeX
/// source and a `display` flag (`True` for block / display math, `False` for
/// inline).
fn wrap_math(callable: Py<PyAny>) -> Box<dyn Fn(&str, bool) -> String + 'static> {
    Box::new(move |tex: &str, display: bool| {
        Python::attach(|py| {
            match callable.call1(py, (tex, display)) {
                Ok(result) => match result.extract::<String>(py) {
                    Ok(s) => s,
                    // Non-string return: fall back to escaped source.
                    Err(_) => escape_html(tex),
                },
                // Callable raised: fall back to escaped source.
                Err(_) => escape_html(tex),
            }
        })
    })
}

/// Build a [`StaticRenderers`] from a Python dict of callables.
///
/// Recognized keys: `"mermaid"` / `"chart"` (callables `(str) -> str`) and
/// `"math"` (callable `(str, bool) -> str`). Unknown keys raise `ValueError`.
/// A missing key leaves that renderer absent, so the matching static path
/// degrades to source.
fn build_renderers(renderers: &Bound<'_, PyDict>) -> PyResult<StaticRenderers> {
    let mut out = StaticRenderers::default();
    for (key, value) in renderers.iter() {
        let name: String = key.extract()?;
        let callable: Py<PyAny> = value.unbind();
        match name.as_str() {
            "mermaid" => out.mermaid = Some(wrap_diagram(callable)),
            "chart" => out.chart = Some(wrap_diagram(callable)),
            "math" => out.math = Some(wrap_math(callable)),
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown renderer key: {other:?} (supported: \"mermaid\", \"chart\", \"math\")"
                )));
            }
        }
    }
    Ok(out)
}

/// Run `f` with an `Options` that borrows the given owned extensions, applying
/// the requested render mode and static renderers.
///
/// `Options<'a>` holds `&'a dyn CarveExtension`, so the owned boxes must
/// outlive the borrow. Both live in this single stack frame, satisfying the
/// lifetime without leaking.
fn render<F>(
    source: &str,
    names: &[String],
    mode: Mode,
    renderers: StaticRenderers,
    f: F,
) -> PyResult<String>
where
    F: FnOnce(&str, &Options<'_>) -> String,
{
    let owned = boxed_extensions(names)?;
    let mut options = Options::new().with_mode(mode).with_renderers(renderers);
    for ext in &owned {
        options = options.with_extension(ext.as_ref());
    }
    Ok(f(source, &options))
}

/// Resolve the mode string and renderers dict into the carve-rs types.
///
/// `mode` defaults to `"interactive"`. `renderers` is optional; absent it is an
/// empty `StaticRenderers`. Both are validated here so callers fail fast.
fn resolve_mode_and_renderers(
    mode: &str,
    renderers: Option<&Bound<'_, PyDict>>,
) -> PyResult<(Mode, StaticRenderers)> {
    let parsed_mode = parse_mode(mode)?;
    let static_renderers = match renderers {
        Some(dict) => build_renderers(dict)?,
        None => StaticRenderers::default(),
    };
    Ok((parsed_mode, static_renderers))
}

/// Convert Carve source to HTML.
///
/// With no `extensions`, this is the core renderer. Pass a list of extension
/// names to enable opt-in behavior. `mode` is `"interactive"` (default) or
/// `"static"`; `renderers` is an optional dict of build-time renderer callables
/// (keys `"mermaid"` / `"chart"` -> `(str) -> str`, `"math"` -> `(str, bool) ->
/// str`) consulted only on the static HTML path.
#[pyfunction]
#[pyo3(signature = (source, extensions = None, mode = "interactive", renderers = None))]
fn to_html(
    source: &str,
    extensions: Option<Vec<String>>,
    mode: &str,
    renderers: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let (parsed_mode, static_renderers) = resolve_mode_and_renderers(mode, renderers.as_ref())?;
    // The fast no-options path only applies in interactive mode with no
    // renderers and no extensions; anything else must go through `render`.
    let names = extensions.unwrap_or_default();
    if names.is_empty()
        && parsed_mode == Mode::Interactive
        && static_renderers.mermaid.is_none()
        && static_renderers.chart.is_none()
        && static_renderers.math.is_none()
    {
        return Ok(carve_rs::to_html(source));
    }
    render(
        source,
        &names,
        parsed_mode,
        static_renderers,
        carve_rs::to_html_with_options,
    )
}

/// Convert Carve source to HTML with an explicit (required) extension list.
///
/// Supports the same `mode` / `renderers` keywords as [`to_html`].
#[pyfunction]
#[pyo3(signature = (source, extensions, mode = "interactive", renderers = None))]
fn to_html_with_extensions(
    source: &str,
    extensions: Vec<String>,
    mode: &str,
    renderers: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let (parsed_mode, static_renderers) = resolve_mode_and_renderers(mode, renderers.as_ref())?;
    render(
        source,
        &extensions,
        parsed_mode,
        static_renderers,
        carve_rs::to_html_with_options,
    )
}

/// True when no extensions were requested (None or empty list).
fn is_core(extensions: &Option<Vec<String>>) -> bool {
    extensions.as_ref().is_none_or(|v| v.is_empty())
}

/// Convert Carve source to Markdown.
#[pyfunction]
#[pyo3(signature = (source, extensions = None))]
fn to_markdown(source: &str, extensions: Option<Vec<String>>) -> PyResult<String> {
    if is_core(&extensions) {
        return Ok(carve_rs::to_markdown(source));
    }
    render(
        source,
        &extensions.unwrap(),
        Mode::Interactive,
        StaticRenderers::default(),
        carve_rs::to_markdown_with_options,
    )
}

/// Convert Carve source to plain text.
#[pyfunction]
#[pyo3(signature = (source, extensions = None))]
fn to_plain_text(source: &str, extensions: Option<Vec<String>>) -> PyResult<String> {
    if is_core(&extensions) {
        return Ok(carve_rs::to_plain_text(source));
    }
    render(
        source,
        &extensions.unwrap(),
        Mode::Interactive,
        StaticRenderers::default(),
        carve_rs::to_plain_text_with_options,
    )
}

/// Convert Carve source to ANSI-colored terminal text.
#[pyfunction]
#[pyo3(signature = (source, extensions = None))]
fn to_ansi(source: &str, extensions: Option<Vec<String>>) -> PyResult<String> {
    if is_core(&extensions) {
        return Ok(carve_rs::to_ansi(source));
    }
    render(
        source,
        &extensions.unwrap(),
        Mode::Interactive,
        StaticRenderers::default(),
        carve_rs::to_ansi_with_options,
    )
}

/// Return the list of supported extension names.
#[pyfunction]
fn extensions() -> Vec<String> {
    SUPPORTED.iter().map(|s| s.to_string()).collect()
}

#[pymodule]
fn carve(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(to_html, m)?)?;
    m.add_function(wrap_pyfunction!(to_html_with_extensions, m)?)?;
    m.add_function(wrap_pyfunction!(to_markdown, m)?)?;
    m.add_function(wrap_pyfunction!(to_plain_text, m)?)?;
    m.add_function(wrap_pyfunction!(to_ansi, m)?)?;
    m.add_function(wrap_pyfunction!(extensions, m)?)?;
    Ok(())
}
