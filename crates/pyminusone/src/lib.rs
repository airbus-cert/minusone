use minusone::engine::{DeobfuscateEngine, DeobfuscationBackend};
use minusone::js::backend::JavaScriptBackend;
use minusone::ps::backend::PowershellBackend;
use minusone::trace::Stepper;
use pyo3::exceptions::{PyRuntimeError, PyStopIteration, PyValueError};
use pyo3::prelude::*;
use std::fmt::Debug;

struct PyMinusOneError(minusone::error::Error);

impl From<PyMinusOneError> for PyErr {
    fn from(err: PyMinusOneError) -> Self {
        match err.0 {
            minusone::error::Error::Utf8Error(_) => {
                PyErr::new::<PyRuntimeError, _>("Invalid UTF8 token".to_string())
            }
            minusone::error::Error::MinusOneError(minusone_error) => {
                PyErr::new::<PyRuntimeError, _>(minusone_error.message.to_string())
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
    let cleaned = DeobfuscateEngine::<B>::remove_extra(source, false).map_err(PyMinusOneError)?;

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

    Ok(engine.lint(false).map_err(PyMinusOneError)?)
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

#[pyclass(name = "Step")]
struct PyStep {
    #[pyo3(get)]
    phase: String,
    #[pyo3(get)]
    rule: String,
    #[pyo3(get)]
    kind: String,
    #[pyo3(get)]
    start: usize,
    #[pyo3(get)]
    end: usize,
    #[pyo3(get)]
    source: String,
    #[pyo3(get)]
    old: String,
    #[pyo3(get)]
    new: String,
}

impl From<minusone::trace::Step> for PyStep {
    fn from(step: minusone::trace::Step) -> Self {
        PyStep {
            phase: step.phase.to_string(),
            rule: step.rule,
            kind: step.kind.to_string(),
            start: step.start,
            end: step.end,
            source: step.source,
            old: step.old,
            new: step.new,
        }
    }
}

#[pymethods]
impl PyStep {
    fn __repr__(&self) -> String {
        format!(
            "Step(phase={:?}, rule={:?}, kind={:?}, start={}, end={})",
            self.phase, self.rule, self.kind, self.start, self.end
        )
    }
}

#[pyclass(name = "Stepper", unsendable)]
struct PyStepper {
    inner: Stepper,
}

#[pymethods]
impl PyStepper {
    fn next(&mut self) -> Option<PyStep> {
        Iterator::next(&mut self.inner).map(PyStep::from)
    }

    fn __iter__(slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> PyResult<PyStep> {
        self.next().ok_or_else(|| PyStopIteration::new_err(()))
    }
}

fn build_stepper(language: &str, source: &str, record_all: bool) -> PyResult<Stepper> {
    let stepper = match language.to_lowercase().as_str() {
        "ps" | "ps1" | "powershell" => {
            PowershellBackend::stepper(source, false, record_all).map_err(PyMinusOneError)?
        }
        "js" | "javascript" => {
            JavaScriptBackend::stepper(source, false, record_all).map_err(PyMinusOneError)?
        }
        _ => {
            return Err(PyErr::new::<PyValueError, _>(format!(
                "Unsupported language: {}",
                language
            )));
        }
    };
    Ok(stepper)
}

#[pyfunction]
#[pyo3(signature = (language, source, record_all=false))]
fn new_stepper(language: String, source: String, record_all: bool) -> PyResult<PyStepper> {
    Ok(PyStepper {
        inner: build_stepper(&language, &source, record_all)?,
    })
}

#[pymodule]
fn pyminusone(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(deobfuscate, m)?)?;
    m.add_function(wrap_pyfunction!(deobfuscate_with, m)?)?;
    m.add_function(wrap_pyfunction!(deobfuscate_without, m)?)?;
    m.add_function(wrap_pyfunction!(new_stepper, m)?)?;
    m.add_class::<PyStep>()?;
    m.add_class::<PyStepper>()?;
    Ok(())
}
