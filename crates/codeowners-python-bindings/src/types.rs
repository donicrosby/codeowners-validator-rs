//! Python wrapper types for the CODEOWNERS validator.

use codeowners_validator_core::parse::{Line, LineKind, Owner, Pattern, Span};
use codeowners_validator_core::validate::{Severity, ValidationError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::pythonize;
use serde::Serialize;
use std::collections::HashSet;

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
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub severity: PySeverity,
}

impl From<&ValidationError> for PyIssue {
    fn from(error: &ValidationError) -> Self {
        let (line, column) = error
            .span()
            .map(|s| (Some(s.line), Some(s.column)))
            .unwrap_or_else(|| (error.line(), None));

        Self {
            line,
            column,
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

/// Python wrapper for CheckConfig.
#[derive(Debug, Clone, Default)]
pub struct PyCheckConfig {
    pub ignored_owners: HashSet<String>,
    pub owners_must_be_teams: bool,
    pub allow_unowned_patterns: bool,
    pub skip_patterns: Vec<String>,
    pub repository: Option<String>,
}

impl PyCheckConfig {
    pub fn from_dict(_py: Python<'_>, dict: &Bound<'_, PyDict>) -> PyResult<Self> {
        let mut config = Self::default();

        if let Ok(Some(val)) = dict.get_item("ignored_owners")
            && let Ok(list) = val.extract::<Vec<String>>()
        {
            config.ignored_owners = list.into_iter().collect();
        }

        if let Ok(Some(val)) = dict.get_item("owners_must_be_teams")
            && let Ok(b) = val.extract::<bool>()
        {
            config.owners_must_be_teams = b;
        }

        if let Ok(Some(val)) = dict.get_item("allow_unowned_patterns")
            && let Ok(b) = val.extract::<bool>()
        {
            config.allow_unowned_patterns = b;
        }

        if let Ok(Some(val)) = dict.get_item("skip_patterns")
            && let Ok(list) = val.extract::<Vec<String>>()
        {
            config.skip_patterns = list;
        }

        if let Ok(Some(val)) = dict.get_item("repository")
            && let Ok(s) = val.extract::<String>()
        {
            config.repository = Some(s);
        }

        Ok(config)
    }

    pub fn into_check_config(self) -> codeowners_validator_core::validate::checks::CheckConfig {
        let mut config = codeowners_validator_core::validate::checks::CheckConfig::new()
            .with_ignored_owners(self.ignored_owners)
            .with_owners_must_be_teams(self.owners_must_be_teams)
            .with_allow_unowned_patterns(self.allow_unowned_patterns)
            .with_skip_patterns(self.skip_patterns);

        if let Some(repo) = self.repository {
            config = config.with_repository(repo);
        }

        config
    }
}
