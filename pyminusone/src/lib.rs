use minusone::engine::DeobfuscateEngine;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use tree_sitter_highlight::{HighlightConfiguration, Highlighter, HtmlRenderer};
use tree_sitter_powershell;

struct PyMinusOneError(minusone::error::Error);

impl From<PyMinusOneError> for PyErr {
    fn from(err: PyMinusOneError) -> Self {
        match err.0 {
            minusone::error::Error::Utf8Error(_) => {
                PyErr::new::<PyRuntimeError, _>(format!("Invalid UTF8 token"))
            }
            minusone::error::Error::MinusOneError(minusone_error) => {
                PyErr::new::<PyRuntimeError, _>(format!("{}", minusone_error.message))
            }
        }
    }
}

#[pyfunction]
fn deobfuscate_powershell(src: String) -> PyResult<String> {
    let remove_comment = DeobfuscateEngine::remove_extra(&src).map_err(PyMinusOneError)?;
    let mut engine =
        DeobfuscateEngine::from_powershell(&remove_comment).map_err(PyMinusOneError)?;
    engine.deobfuscate().map_err(PyMinusOneError)?;
    Ok(engine.lint().map_err(PyMinusOneError)?)
}

#[pyfunction]
fn deobfuscate_powershell_html(src: String) -> PyResult<String> {
    let highlight_names = [
        "attribute",
        "constant",
        "function.builtin",
        "function",
        "keyword",
        "operator",
        "property",
        "punctuation",
        "punctuation.bracket",
        "punctuation.delimiter",
        "string",
        "string.special",
        "tag",
        "type",
        "type.builtin",
        "variable",
        "variable.builtin",
        "variable.parameter",
        "number",
        "array",
        "assignvalue",
    ];

    let remove_comment = DeobfuscateEngine::remove_extra(&src).map_err(PyMinusOneError)?;

    let mut engine =
        DeobfuscateEngine::from_powershell(&remove_comment).map_err(PyMinusOneError)?;
    engine.deobfuscate().map_err(PyMinusOneError)?;

    let mut highlighter = Highlighter::new();

    let mut psconfig = HighlightConfiguration::new(
        tree_sitter_powershell::LANGUAGE.into(),
        "powershell",
        tree_sitter_powershell::HIGHLIGHTS_QUERY,
        "",
        "",
    )
    .unwrap();

    psconfig.configure(&highlight_names);

    let deobfuscate_ps = engine.lint_format("\t").map_err(PyMinusOneError)?;

    let highlights = highlighter
        .highlight(&psconfig, deobfuscate_ps.as_bytes(), None, |_| None)
        .unwrap();

    let html_attrs: Vec<String> = highlight_names
        .iter()
        .map(|s| format!("class=\"{}\"", s.replace('.', " ")))
        .collect();

    let mut renderer = HtmlRenderer::new();
    renderer
        .render(
            highlights,
            deobfuscate_ps.as_bytes(),
            &|highlight, output| output.extend(html_attrs[highlight.0].as_bytes()),
        )
        .unwrap();

    Ok(unsafe { String::from_utf8_unchecked(renderer.html) })
}

#[pymodule]
fn pyminusone(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(deobfuscate_powershell, m)?)?;
    m.add_function(wrap_pyfunction!(deobfuscate_powershell_html, m)?)?;
    Ok(())
}
