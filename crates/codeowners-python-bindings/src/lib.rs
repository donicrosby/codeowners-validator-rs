//! Python bindings for the CODEOWNERS validator.
//!
//! This crate provides Python bindings using PyO3 for the codeowners-validator-core library.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;

mod github_client;
mod types;

use github_client::PyGithubClient;
use types::{PyCheckConfig, PyIssue, PyLine};

/// Parse a CODEOWNERS file content and return the parsed AST.
///
/// Args:
///     content: The CODEOWNERS file content as a string.
///
/// Returns:
///     A dictionary containing:
///     - `is_ok`: Whether parsing was successful (bool)
///     - `ast`: The parsed AST containing lines (dict)
///     - `errors`: List of parse errors if any (list)
///
/// Example:
///     >>> result = parse_codeowners("*.rs @rustacean\\n/docs/ @docs-team\\n")
///     >>> result["is_ok"]
///     True
///     >>> len(result["ast"]["lines"])
///     2
#[pyfunction]
fn parse_codeowners(py: Python<'_>, content: &str) -> PyResult<Py<PyDict>> {
    let result = codeowners_validator_core::parse::parse_codeowners(content);

    let dict = PyDict::new(py);
    dict.set_item("is_ok", result.is_ok())?;

    // Convert AST
    let ast_dict = PyDict::new(py);
    let lines: Vec<PyLine> = result.ast.lines.iter().map(PyLine::from).collect();
    let py_lines: Vec<Py<PyAny>> = lines
        .iter()
        .map(|l| l.to_py(py))
        .collect::<PyResult<Vec<_>>>()?;
    ast_dict.set_item("lines", py_lines)?;
    dict.set_item("ast", ast_dict)?;

    // Convert errors
    let errors: Vec<String> = result.errors.iter().map(|e| e.to_string()).collect();
    dict.set_item("errors", errors)?;

    Ok(dict.into())
}

/// Validate a CODEOWNERS file content with synchronous checks.
///
/// This function runs all synchronous validation checks (syntax, files, duppatterns, etc.)
/// but does NOT run the owners check which requires GitHub API access.
///
/// Args:
///     content: The CODEOWNERS file content as a string.
///     repo_path: Path to the repository root directory.
///     config: Optional configuration dictionary with keys:
///         - ignored_owners: List of owners to ignore during validation
///         - owners_must_be_teams: Whether owners must be teams (bool)
///         - allow_unowned_patterns: Whether to allow patterns without owners (bool)
///         - skip_patterns: List of patterns to skip for not-owned check
///         - repository: Repository in "owner/repo" format
///     checks: Optional list of checks to run. Valid values:
///         - "syntax": Check for syntax errors
///         - "files": Check that patterns match files
///         - "duppatterns": Check for duplicate patterns
///         - "notowned": Check for files not covered by any rule (experimental)
///         - "avoid-shadowing": Check for shadowed patterns (experimental)
///
/// Returns:
///     A dictionary with check results grouped by check name, where each entry contains:
///     - List of issues, each with: line, column, message, severity
///
/// Example:
///     >>> result = validate_codeowners("*.rs @rustacean\\n", "/path/to/repo")
///     >>> result["syntax"]
///     []  # No syntax errors
#[pyfunction]
#[pyo3(signature = (content, repo_path, config=None, checks=None))]
fn validate_codeowners(
    py: Python<'_>,
    content: &str,
    repo_path: &str,
    config: Option<&Bound<'_, PyDict>>,
    checks: Option<Vec<String>>,
) -> PyResult<Py<PyDict>> {
    // Parse the content first
    let parse_result = codeowners_validator_core::parse::parse_codeowners(content);

    // Build check config
    let check_config = match config {
        Some(cfg) => PyCheckConfig::from_dict(py, cfg)?.into_check_config(),
        None => codeowners_validator_core::validate::checks::CheckConfig::new(),
    };

    // Determine which checks to run
    let checks_to_run = checks.unwrap_or_else(|| {
        vec![
            "syntax".to_string(),
            "files".to_string(),
            "duppatterns".to_string(),
        ]
    });

    // Create result dict
    let result_dict = PyDict::new(py);

    // Initialize empty lists for all possible checks
    for check_name in &["syntax", "files", "duppatterns", "owners", "notowned", "avoid-shadowing"] {
        let empty_list: Vec<HashMap<String, Py<PyAny>>> = vec![];
        result_dict.set_item(*check_name, empty_list)?;
    }

    let repo_path = std::path::Path::new(repo_path);

    // Run checks
    use codeowners_validator_core::validate::checks::{
        AvoidShadowingCheck, Check, CheckContext, DupPatternsCheck, FilesCheck, NotOwnedCheck,
        SyntaxCheck,
    };

    let ctx = CheckContext::new(&parse_result.ast, repo_path, &check_config);

    for check_name in &checks_to_run {
        let validation_result = match check_name.as_str() {
            "syntax" => SyntaxCheck::new().run(&ctx),
            "files" => FilesCheck::new().run(&ctx),
            "duppatterns" => DupPatternsCheck::new().run(&ctx),
            "notowned" => NotOwnedCheck::new().run(&ctx),
            "avoid-shadowing" | "shadowing" => AvoidShadowingCheck::new().run(&ctx),
            _ => continue, // Skip unknown checks
        };

        let issues: Vec<Py<PyAny>> = validation_result
            .errors
            .iter()
            .map(|e| PyIssue::from(e).to_py(py))
            .collect::<PyResult<Vec<_>>>()?;

        let canonical_name = if check_name == "shadowing" {
            "avoid-shadowing"
        } else {
            check_name.as_str()
        };
        result_dict.set_item(canonical_name, issues)?;
    }

    Ok(result_dict.into())
}

/// Validate a CODEOWNERS file content with GitHub owner verification.
///
/// This function runs all validation checks including the async owners check
/// which verifies that owners exist on GitHub.
///
/// Args:
///     content: The CODEOWNERS file content as a string.
///     repo_path: Path to the repository root directory.
///     github_client: A GitHub client object implementing the GithubClientProtocol.
///         Must have async methods: user_exists(username) -> bool,
///         team_exists(org, team) -> Literal["exists", "not_found", "unauthorized"]
///     config: Optional configuration dictionary (see validate_codeowners).
///     checks: Optional list of checks to run (see validate_codeowners).
///         The "owners" check is automatically included when using this function.
///
/// Returns:
///     A dictionary with check results grouped by check name.
///
/// Example:
///     >>> class MyGithubClient:
///     ...     async def user_exists(self, username: str) -> bool:
///     ...         return True  # Implement with actual GitHub API call
///     ...     async def team_exists(self, org: str, team: str) -> str:
///     ...         return "exists"  # Implement with actual GitHub API call
///     >>> import asyncio
///     >>> result = asyncio.run(validate_with_github("*.rs @rustacean\\n", "/path/to/repo", MyGithubClient()))
#[pyfunction]
#[pyo3(signature = (content, repo_path, github_client, config=None, checks=None))]
fn validate_with_github<'py>(
    py: Python<'py>,
    content: &str,
    repo_path: &str,
    github_client: Bound<'py, PyAny>,
    config: Option<&Bound<'py, PyDict>>,
    checks: Option<Vec<String>>,
) -> PyResult<Bound<'py, PyAny>> {
    // Create the async coroutine
    let content = content.to_string();
    let repo_path = repo_path.to_string();
    let github_client = github_client.unbind();
    let config_dict: Option<HashMap<String, Py<PyAny>>> = config.map(|c| {
        c.iter()
            .filter_map(|(k, v)| {
                k.extract::<String>().ok().map(|key| (key, v.unbind()))
            })
            .collect()
    });

    pyo3_async_runtimes::tokio::future_into_py(py, async move {
        validate_with_github_impl(&content, &repo_path, &github_client, config_dict.as_ref(), checks).await
    })
}

async fn validate_with_github_impl(
    content: &str,
    repo_path: &str,
    github_client: &Py<PyAny>,
    config: Option<&HashMap<String, Py<PyAny>>>,
    checks: Option<Vec<String>>,
) -> PyResult<Py<PyDict>> {
    // Parse the content first
    let parse_result = codeowners_validator_core::parse::parse_codeowners(content);

    // Build check config from Python dict
    let check_config = Python::attach(|py| {
        match config {
            Some(cfg) => {
                let mut config = codeowners_validator_core::validate::checks::CheckConfig::new();

                if let Some(obj) = cfg.get("ignored_owners") {
                    if let Ok(list) = obj.bind(py).extract::<Vec<String>>() {
                        config = config.with_ignored_owners(list.into_iter().collect());
                    }
                }
                if let Some(obj) = cfg.get("owners_must_be_teams") {
                    if let Ok(val) = obj.bind(py).extract::<bool>() {
                        config = config.with_owners_must_be_teams(val);
                    }
                }
                if let Some(obj) = cfg.get("allow_unowned_patterns") {
                    if let Ok(val) = obj.bind(py).extract::<bool>() {
                        config = config.with_allow_unowned_patterns(val);
                    }
                }
                if let Some(obj) = cfg.get("skip_patterns") {
                    if let Ok(list) = obj.bind(py).extract::<Vec<String>>() {
                        config = config.with_skip_patterns(list);
                    }
                }
                if let Some(obj) = cfg.get("repository") {
                    if let Ok(val) = obj.bind(py).extract::<String>() {
                        config = config.with_repository(val);
                    }
                }
                config
            }
            None => codeowners_validator_core::validate::checks::CheckConfig::new(),
        }
    });

    // Determine which checks to run
    let mut checks_to_run = checks.unwrap_or_else(|| {
        vec![
            "syntax".to_string(),
            "files".to_string(),
            "duppatterns".to_string(),
            "owners".to_string(),
        ]
    });

    // Always include owners for this function
    if !checks_to_run.contains(&"owners".to_string()) {
        checks_to_run.push("owners".to_string());
    }

    let repo_path = std::path::Path::new(repo_path);

    // Run sync checks first
    use codeowners_validator_core::validate::checks::{
        AvoidShadowingCheck, Check, CheckContext, DupPatternsCheck, FilesCheck, NotOwnedCheck,
        SyntaxCheck,
    };

    let ctx = CheckContext::new(&parse_result.ast, repo_path, &check_config);

    // Collect sync check results
    let mut sync_results: Vec<(String, Vec<codeowners_validator_core::validate::ValidationError>)> = Vec::new();

    for check_name in &checks_to_run {
        if check_name == "owners" {
            continue; // Handle owners check separately
        }

        let validation_result = match check_name.as_str() {
            "syntax" => SyntaxCheck::new().run(&ctx),
            "files" => FilesCheck::new().run(&ctx),
            "duppatterns" => DupPatternsCheck::new().run(&ctx),
            "notowned" => NotOwnedCheck::new().run(&ctx),
            "avoid-shadowing" | "shadowing" => AvoidShadowingCheck::new().run(&ctx),
            _ => continue,
        };

        let canonical_name = if check_name == "shadowing" {
            "avoid-shadowing".to_string()
        } else {
            check_name.clone()
        };
        sync_results.push((canonical_name, validation_result.errors));
    }

    // Run async owners check if requested
    let owners_errors = if checks_to_run.contains(&"owners".to_string()) {
        let py_client = Python::attach(|py| {
            PyGithubClient::new(github_client.clone_ref(py))
        });

        use codeowners_validator_core::validate::checks::{AsyncCheck, AsyncCheckContext, OwnersCheck};

        let async_ctx = AsyncCheckContext::new(&parse_result.ast, repo_path, &check_config, &py_client);
        let validation_result = OwnersCheck::new().run(&async_ctx).await;
        validation_result.errors
    } else {
        Vec::new()
    };

    // Convert results to Python dicts using pythonize
    Python::attach(|py| {
        let result_dict = PyDict::new(py);

        // Initialize empty lists for all possible checks
        for check_name in &["syntax", "files", "duppatterns", "owners", "notowned", "avoid-shadowing"] {
            let empty_list: Vec<HashMap<String, Py<PyAny>>> = vec![];
            result_dict.set_item(*check_name, empty_list)?;
        }

        // Add sync check results
        for (check_name, errors) in sync_results {
            let issues: Vec<Py<PyAny>> = errors
                .iter()
                .map(|e| PyIssue::from(e).to_py(py))
                .collect::<PyResult<Vec<_>>>()?;
            result_dict.set_item(check_name, issues)?;
        }

        // Add owners check results
        let owners_issues: Vec<Py<PyAny>> = owners_errors
            .iter()
            .map(|e| PyIssue::from(e).to_py(py))
            .collect::<PyResult<Vec<_>>>()?;
        result_dict.set_item("owners", owners_issues)?;

        Ok(result_dict.into())
    })
}

/// The Python module for codeowners_validator.
#[pymodule]
fn _codeowners_validator(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_codeowners, m)?)?;
    m.add_function(wrap_pyfunction!(validate_codeowners, m)?)?;
    m.add_function(wrap_pyfunction!(validate_with_github, m)?)?;

    // Add version info
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
