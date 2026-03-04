use minusone::engine::DeobfuscateEngine;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use tree_sitter_highlight::{HighlightConfiguration, Highlighter, HtmlRenderer};
use tree_sitter_powershell;
use minusone::ps::backend::PowershellBackend;

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

fn deobfuscate_powershell_ex(
    src: String,
    ruleset: Vec<String>,
    with: bool,
    format_lint: bool,
) -> PyResult<String> {
    let ruleset: Vec<String> = ruleset.iter().map(|s| s.to_lowercase()).collect();
    let remove_comment = DeobfuscateEngine::<PowershellBackend>::remove_extra(&src).map_err(PyMinusOneError)?;
    let mut engine =
        DeobfuscateEngine::from_powershell(&remove_comment).map_err(PyMinusOneError)?;

    match (ruleset.len(), with) {
        (0, _) => engine.deobfuscate(),
        (_, true) => {
            engine.deobfuscate_with_custom_ruleset(ruleset.iter().map(AsRef::as_ref).collect())
        }
        (_, false) => {
            engine.deobfuscate_without_custom_ruleset(ruleset.iter().map(AsRef::as_ref).collect())
        }
    }
    .map_err(PyMinusOneError)?;

    Ok(match format_lint {
        true => engine.lint_format("\t"),
        false => engine.lint(),
    }
    .map_err(PyMinusOneError)?)
}

fn deobfuscate_powershell_html_ex(deobfuscate_ps: String) -> PyResult<String> {
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

#[pyfunction]
fn deobfuscate_powershell(src: String) -> PyResult<String> {
    deobfuscate_powershell_ex(src, vec![], false, false)
}

#[pyfunction]
fn deobfuscate_powershell_with(src: String, ruleset: Vec<String>) -> PyResult<String> {
    deobfuscate_powershell_ex(src, ruleset, true, false)
}

#[pyfunction]
fn deobfuscate_powershell_without(src: String, ruleset: Vec<String>) -> PyResult<String> {
    deobfuscate_powershell_ex(src, ruleset, false, false)
}

#[pyfunction]
fn deobfuscate_powershell_html(src: String) -> PyResult<String> {
    deobfuscate_powershell_html_ex(deobfuscate_powershell_ex(src, vec![], false, true)?)
}

#[pyfunction]
fn deobfuscate_powershell_html_with(src: String, ruleset: Vec<String>) -> PyResult<String> {
    deobfuscate_powershell_html_ex(deobfuscate_powershell_ex(src, ruleset, true, true)?)
}

#[pyfunction]
fn deobfuscate_powershell_html_without(src: String, ruleset: Vec<String>) -> PyResult<String> {
    deobfuscate_powershell_html_ex(deobfuscate_powershell_ex(src, ruleset, false, true)?)
}

#[pymodule]
fn pyminusone(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(deobfuscate_powershell, m)?)?;
    m.add_function(wrap_pyfunction!(deobfuscate_powershell_with, m)?)?;
    m.add_function(wrap_pyfunction!(deobfuscate_powershell_without, m)?)?;
    m.add_function(wrap_pyfunction!(deobfuscate_powershell_html, m)?)?;
    m.add_function(wrap_pyfunction!(deobfuscate_powershell_html_with, m)?)?;
    m.add_function(wrap_pyfunction!(deobfuscate_powershell_html_without, m)?)?;
    Ok(())
}
