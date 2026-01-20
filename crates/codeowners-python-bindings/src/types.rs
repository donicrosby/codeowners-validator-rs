//! Python wrapper types for the CODEOWNERS validator.

use codeowners_validator_core::parse::{Line, LineKind, Owner, Pattern, Span};
use codeowners_validator_core::validate::{Severity, ValidationError};
use pyo3::prelude::*;
use pythonize::pythonize;
use serde::Serialize;

/// Python wrapper for Span.
#[derive(Debug, Clone, Serialize)]
pub struct PySpan {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
    pub length: usize,
}

impl From<&Span> for PySpan {
    fn from(span: &Span) -> Self {
        Self {
            offset: span.offset,
            line: span.line,
            column: span.column,
            length: span.length,
        }
    }
}

/// Python wrapper for Owner.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PyOwner {
    User {
        name: String,
        text: String,
        span: PySpan,
    },
    Team {
        org: String,
        team: String,
        text: String,
        span: PySpan,
    },
    Email {
        email: String,
        text: String,
        span: PySpan,
    },
}

impl From<&Owner> for PyOwner {
    fn from(owner: &Owner) -> Self {
        match owner {
            Owner::User { name, span } => PyOwner::User {
                name: name.clone(),
                text: format!("@{}", name),
                span: PySpan::from(span),
            },
            Owner::Team { org, team, span } => PyOwner::Team {
                org: org.clone(),
                team: team.clone(),
                text: format!("@{}/{}", org, team),
                span: PySpan::from(span),
            },
            Owner::Email { email, span } => PyOwner::Email {
                email: email.clone(),
                text: email.clone(),
                span: PySpan::from(span),
            },
        }
    }
}

/// Python wrapper for Pattern.
#[derive(Debug, Clone, Serialize)]
pub struct PyPattern {
    pub text: String,
    pub span: PySpan,
}

impl From<&Pattern> for PyPattern {
    fn from(pattern: &Pattern) -> Self {
        Self {
            text: pattern.text.clone(),
            span: PySpan::from(&pattern.span),
        }
    }
}

/// Python wrapper for LineKind.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PyLineKind {
    Blank,
    Comment {
        content: String,
    },
    Rule {
        pattern: PyPattern,
        owners: Vec<PyOwner>,
    },
    Invalid {
        raw: String,
        error: String,
    },
}

impl From<&LineKind> for PyLineKind {
    fn from(kind: &LineKind) -> Self {
        match kind {
            LineKind::Blank => PyLineKind::Blank,
            LineKind::Comment { content } => PyLineKind::Comment {
                content: content.clone(),
            },
            LineKind::Rule { pattern, owners } => PyLineKind::Rule {
                pattern: PyPattern::from(pattern),
                owners: owners.iter().map(PyOwner::from).collect(),
            },
            LineKind::Invalid { raw, error } => PyLineKind::Invalid {
                raw: raw.clone(),
                error: error.clone(),
            },
        }
    }
}

/// Python wrapper for Line.
#[derive(Debug, Clone, Serialize)]
pub struct PyLine {
    pub kind: PyLineKind,
    pub span: PySpan,
}

impl From<&Line> for PyLine {
    fn from(line: &Line) -> Self {
        Self {
            kind: PyLineKind::from(&line.kind),
            span: PySpan::from(&line.span),
        }
    }
}

impl PyLine {
    /// Convert to a Python object using pythonize.
    pub fn to_py(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        pythonize(py, self)
            .map(|bound| bound.unbind())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }
}

/// Python wrapper for Severity.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PySeverity {
    Warning,
    Error,
}

impl From<Severity> for PySeverity {
    fn from(severity: Severity) -> Self {
        match severity {
            Severity::Warning => PySeverity::Warning,
            Severity::Error => PySeverity::Error,
        }
    }
}

/// Python wrapper for ValidationError (as a single issue).
#[derive(Debug, Clone, Serialize)]
pub struct PyIssue {
    pub path: String,
    pub span: Option<PySpan>,
    pub message: String,
    pub severity: PySeverity,
}

impl PyIssue {
    /// Creates a new PyIssue from a ValidationError and a path.
    pub fn new(error: &ValidationError, path: String) -> Self {
        Self {
            path,
            span: error.span().map(PySpan::from),
            message: error.to_string(),
            severity: PySeverity::from(error.severity()),
        }
    }
}

impl PyIssue {
    /// Convert to a Python object using pythonize.
    pub fn to_py(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        pythonize(py, self)
            .map(|bound| bound.unbind())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }
}
