//! Python bindings for the CODEOWNERS validator.
//!
//! This crate provides Python bindings using PyO3 for the codeowners-validator-core library.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;

mod github_client;
mod types;

use github_client::PyGithubClient;
use types::{PyIssue, PyLine};

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

/// Validate a CODEOWNERS file in a repository.
///
/// This function runs validation checks on a CODEOWNERS file. It automatically
/// finds the CODEOWNERS file in the repository by searching in standard locations:
/// `.github/CODEOWNERS`, `CODEOWNERS`, and `docs/CODEOWNERS`.
///
/// When a `github_client` is provided, it also verifies that owners exist on GitHub.
///
/// Args:
///     repo_path: Path to the repository root directory. The CODEOWNERS file will
///         be automatically located within this directory.
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
///         - "owners": Verify owners exist on GitHub (requires github_client)
///         - "notowned": Check for files not covered by any rule (experimental)
///         - "avoid-shadowing": Check for shadowed patterns (experimental)
///     github_client: Optional GitHub client object implementing the GithubClientProtocol.
///         Required for the "owners" check. Must have methods:
///         user_exists(username) -> bool,
///         team_exists(org, team) -> Literal["exists", "not_found", "unauthorized"]
///
/// Returns:
///     A dictionary with check results grouped by check name, where each entry contains:
///     - List of issues, each with: span, message, severity
///
/// Raises:
///     FileNotFoundError: If no CODEOWNERS file is found in the repository.
///     IOError: If the CODEOWNERS file cannot be read.
///
/// Example:
///     >>> import asyncio
///     >>> # Without GitHub client (sync checks only)
///     >>> result = asyncio.run(validate_codeowners("/path/to/repo"))
///     >>> result["syntax"]
///     []  # No syntax errors
///     >>>
///     >>> # With GitHub client (includes owner verification)
///     >>> class MyGithubClient:
///     ...     async def user_exists(self, username: str) -> bool:
///     ...         return True  # Implement with actual GitHub API call
///     ...     async def team_exists(self, org: str, team: str) -> str:
///     ...         return "exists"  # Implement with actual GitHub API call
///     >>> result = asyncio.run(validate_codeowners(
///     ...     "/path/to/repo",
///     ...     github_client=MyGithubClient()
///     ... ))
#[pyfunction]
#[pyo3(signature = (repo_path, config=None, checks=None, github_client=None))]
fn validate_codeowners<'py>(
    py: Python<'py>,
    repo_path: &str,
    config: Option<&Bound<'py, PyDict>>,
    checks: Option<Vec<String>>,
    github_client: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    // Create the async coroutine
    let repo_path = repo_path.to_string();
    let github_client = github_client.map(|c| c.unbind());
    let config_dict: Option<HashMap<String, Py<PyAny>>> = config.map(|c| {
        c.iter()
            .filter_map(|(k, v)| k.extract::<String>().ok().map(|key| (key, v.unbind())))
            .collect()
    });

    pyo3_async_runtimes::tokio::future_into_py(py, async move {
        validate_codeowners_impl(&repo_path, config_dict.as_ref(), checks, github_client).await
    })
}

async fn validate_codeowners_impl(
    repo_path: &str,
    config: Option<&HashMap<String, Py<PyAny>>>,
    checks: Option<Vec<String>>,
    github_client: Option<Py<PyAny>>,
) -> PyResult<Py<PyDict>> {
    use codeowners_validator_core::validate::checks::{CheckRunner, OwnersCheck};

    let repo_path_buf = std::path::Path::new(repo_path);

    // Find the CODEOWNERS file
    let codeowners_path =
        codeowners_validator_core::find_codeowners_file(repo_path_buf).ok_or_else(|| {
            pyo3::exceptions::PyFileNotFoundError::new_err(format!(
            "CODEOWNERS file not found in repository '{}'. Searched in: .github/CODEOWNERS, CODEOWNERS, docs/CODEOWNERS",
            repo_path
        ))
        })?;

    // Read the CODEOWNERS file
    let content = std::fs::read_to_string(&codeowners_path).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!(
            "Failed to read CODEOWNERS file '{}': {}",
            codeowners_path.display(),
            e
        ))
    })?;

    // Parse the content
    let parse_result = codeowners_validator_core::parse::parse_codeowners(&content);

    // Build check config from Python dict
    let check_config = Python::attach(|py| match config {
        Some(cfg) => {
            let mut config = codeowners_validator_core::validate::checks::CheckConfig::new();

            if let Some(obj) = cfg.get("ignored_owners")
                && let Ok(list) = obj.bind(py).extract::<Vec<String>>()
            {
                config = config.with_ignored_owners(list.into_iter().collect());
            }
            if let Some(obj) = cfg.get("owners_must_be_teams")
                && let Ok(val) = obj.bind(py).extract::<bool>()
            {
                config = config.with_owners_must_be_teams(val);
            }
            if let Some(obj) = cfg.get("allow_unowned_patterns")
                && let Ok(val) = obj.bind(py).extract::<bool>()
            {
                config = config.with_allow_unowned_patterns(val);
            }
            if let Some(obj) = cfg.get("skip_patterns")
                && let Ok(list) = obj.bind(py).extract::<Vec<String>>()
            {
                config = config.with_skip_patterns(list);
            }
            if let Some(obj) = cfg.get("repository")
                && let Ok(val) = obj.bind(py).extract::<String>()
            {
                config = config.with_repository(val);
            }
            config
        }
        None => codeowners_validator_core::validate::checks::CheckConfig::new(),
    });

    // Determine which checks to run
    let checks_to_run = checks.unwrap_or_else(|| {
        let mut default = vec![
            "syntax".to_string(),
            "files".to_string(),
            "duppatterns".to_string(),
        ];
        // Only include owners check by default if github_client is provided
        if github_client.is_some() {
            default.push("owners".to_string());
        }
        default
    });

    // Build CheckRunner with requested checks
    use codeowners_validator_core::validate::checks::{
        AvoidShadowingCheck, DupPatternsCheck, FilesCheck, NotOwnedCheck, SyntaxCheck,
    };

    let mut runner = CheckRunner::new();
    let mut run_owners = false;

    for check_name in &checks_to_run {
        match check_name.as_str() {
            "syntax" => runner.add_check(SyntaxCheck::new()),
            "files" => runner.add_check(FilesCheck::new()),
            "duppatterns" => runner.add_check(DupPatternsCheck::new()),
            "notowned" => runner.add_check(NotOwnedCheck::new()),
            "avoid-shadowing" | "shadowing" => runner.add_check(AvoidShadowingCheck::new()),
            "owners" => {
                if github_client.is_some() {
                    runner.add_async_check(OwnersCheck::new());
                    run_owners = true;
                }
                // Skip owners check if no github_client provided
            }
            _ => continue, // Skip unknown checks
        };
    }

    // Run all checks using CheckRunner
    let validation_result = if run_owners {
        let py_client =
            Python::attach(|py| PyGithubClient::new(github_client.as_ref().unwrap().clone_ref(py)));
        runner
            .run_all(
                &parse_result.ast,
                repo_path_buf,
                &check_config,
                Some(
                    &py_client
                        as &dyn codeowners_validator_core::validate::github_client::GithubClient,
                ),
            )
            .await
    } else {
        runner
            .run_all(&parse_result.ast, repo_path_buf, &check_config, None)
            .await
    };

    // Convert results to Python dict
    // Group errors by check name based on the error type
    Python::attach(|py| {
        let result_dict = PyDict::new(py);

        // Initialize empty lists for all possible checks
        for check_name in &[
            "syntax",
            "files",
            "duppatterns",
            "owners",
            "notowned",
            "avoid-shadowing",
        ] {
            let empty_list: Vec<HashMap<String, Py<PyAny>>> = vec![];
            result_dict.set_item(*check_name, empty_list)?;
        }

        // Group errors by their source check
        use codeowners_validator_core::validate::ValidationError;

        let mut syntax_errors = Vec::new();
        let mut files_errors = Vec::new();
        let mut duppatterns_errors = Vec::new();
        let mut owners_errors = Vec::new();
        let mut notowned_errors = Vec::new();
        let mut shadowing_errors = Vec::new();

        for error in &validation_result.errors {
            match error {
                ValidationError::InvalidPatternSyntax { .. }
                | ValidationError::InvalidOwnerFormat { .. }
                | ValidationError::UnsupportedPatternSyntax { .. } => {
                    syntax_errors.push(error);
                }
                ValidationError::PatternNotMatching { .. } => {
                    files_errors.push(error);
                }
                ValidationError::DuplicatePattern { .. } => {
                    duppatterns_errors.push(error);
                }
                ValidationError::OwnerNotFound { .. }
                | ValidationError::InsufficientAuthorization { .. }
                | ValidationError::OwnerMustBeTeam { .. } => {
                    owners_errors.push(error);
                }
                ValidationError::FileNotOwned { .. } => {
                    notowned_errors.push(error);
                }
                ValidationError::PatternShadowed { .. } => {
                    shadowing_errors.push(error);
                }
            }
        }

        // Convert each group to Python
        let convert_errors =
            |errors: Vec<&ValidationError>, py: Python<'_>| -> PyResult<Vec<Py<PyAny>>> {
                errors.iter().map(|e| PyIssue::from(*e).to_py(py)).collect()
            };

        result_dict.set_item("syntax", convert_errors(syntax_errors, py)?)?;
        result_dict.set_item("files", convert_errors(files_errors, py)?)?;
        result_dict.set_item("duppatterns", convert_errors(duppatterns_errors, py)?)?;
        result_dict.set_item("owners", convert_errors(owners_errors, py)?)?;
        result_dict.set_item("notowned", convert_errors(notowned_errors, py)?)?;
        result_dict.set_item("avoid-shadowing", convert_errors(shadowing_errors, py)?)?;

        Ok(result_dict.into())
    })
}

/// The Python module for codeowners_validator.
#[pymodule]
fn _codeowners_validator(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Initialize Rust -> Python logging bridge
    // This must be called before any log statements are executed
    pyo3_log::init();

    m.add_function(wrap_pyfunction!(parse_codeowners, m)?)?;
    m.add_function(wrap_pyfunction!(validate_codeowners, m)?)?;

    // Add version info
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
