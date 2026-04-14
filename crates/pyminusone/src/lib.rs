use minusone::engine::{DeobfuscateEngine, DeobfuscationBackend};
use minusone::js::backend::JavaScriptBackend;
use minusone::ps::backend::PowershellBackend;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::fmt::Debug;

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

pub(crate) fn run_deobf<B: DeobfuscationBackend>(
    source: &str,
    rule_set: Option<Vec<String>>,
    skip_rule_set: Option<Vec<String>>,
) -> PyResult<String>
where
    <B as DeobfuscationBackend>::Language: Debug,
{
    let cleaned = DeobfuscateEngine::<B>::remove_extra(source).map_err(PyMinusOneError)?;

    let mut engine = DeobfuscateEngine::<B>::from_source(&cleaned).map_err(PyMinusOneError)?;

    if let Some(rules) = rule_set {
        engine
            .deobfuscate_with_custom_ruleset(rules.iter().map(AsRef::as_ref).collect())
            .map_err(PyMinusOneError)?;
    } else if let Some(skip_rules) = skip_rule_set {
        engine
            .deobfuscate_without_custom_ruleset(skip_rules.iter().map(AsRef::as_ref).collect())
            .map_err(PyMinusOneError)?;
    } else {
        engine.deobfuscate().map_err(PyMinusOneError)?;
    }

    Ok(engine.lint().map_err(PyMinusOneError)?)
}

#[pyfunction]
fn deobfuscate(language: String, source: String) -> PyResult<String> {
    match language.to_lowercase().as_str() {
        "ps" | "ps1" | "powershell" => run_deobf::<PowershellBackend>(&source, None, None),
        "js" | "javascript" => run_deobf::<JavaScriptBackend>(&source, None, None),
        _ => Err(PyErr::new::<PyRuntimeError, _>(format!(
            "Unsupported language: {}",
            language
        ))),
    }
}

#[pyfunction]
fn deobfuscate_with(language: String, source: String, ruleset: Vec<String>) -> PyResult<String> {
    match language.to_lowercase().as_str() {
        "ps" | "ps1" | "powershell" => run_deobf::<PowershellBackend>(&source, Some(ruleset), None),
        "js" | "javascript" => run_deobf::<JavaScriptBackend>(&source, Some(ruleset), None),
        _ => Err(PyErr::new::<PyRuntimeError, _>(format!(
            "Unsupported language: {}",
            language
        ))),
    }
}

#[pyfunction]
fn deobfuscate_without(language: String, source: String, ruleset: Vec<String>) -> PyResult<String> {
    match language.to_lowercase().as_str() {
        "ps" | "ps1" | "powershell" => run_deobf::<PowershellBackend>(&source, None, Some(ruleset)),
        "js" | "javascript" => run_deobf::<JavaScriptBackend>(&source, None, Some(ruleset)),
        _ => Err(PyErr::new::<PyRuntimeError, _>(format!(
            "Unsupported language: {}",
            language
        ))),
    }
}

#[pymodule]
fn pyminusone(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(deobfuscate, m)?)?;
    m.add_function(wrap_pyfunction!(deobfuscate_with, m)?)?;
    m.add_function(wrap_pyfunction!(deobfuscate_without, m)?)?;
    Ok(())
}
