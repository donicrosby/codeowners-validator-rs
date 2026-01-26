//! Python GitHub client bridge.
//!
//! This module provides a bridge between Python GitHub clients (like githubkit or pygithub)
//! and the Rust GithubClient trait.

use async_trait::async_trait;
use codeowners_validator_core::validate::github_client::{
    GithubClient, GithubClientError, TeamExistsResult, UserExistsResult,
};
use log::debug;
use pyo3::prelude::*;

/// Helper function to convert PyErr to GithubClientError.
fn py_err_to_github_err(e: PyErr) -> GithubClientError {
    GithubClientError::Other(e.to_string())
}

/// A Python-based GitHub client that delegates to a Python object.
pub struct PyGithubClient {
    /// The Python object implementing the GithubClientProtocol.
    client: Py<PyAny>,
}

impl PyGithubClient {
    /// Creates a new PyGithubClient wrapping a Python object.
    pub fn new(client: Py<PyAny>) -> Self {
        debug!("Creating new PyGithubClient wrapper");
        Self { client }
    }

    /// Helper to call a Python method (sync or async) and get the result.
    /// For async methods, this uses pyo3-async-runtimes to properly await the coroutine.
    async fn call_python_method_async(
        &self,
        method_name: &str,
        args: Vec<String>,
    ) -> Result<Py<PyAny>, GithubClientError> {
        debug!(
            "Calling Python method '{}' with args: {:?}",
            method_name, args
        );
        let client = Python::attach(|py| self.client.clone_ref(py));
        let method_name = method_name.to_string();

        // Call the Python method and check if it returns a coroutine.
        // If it's a coroutine, convert it to a future inside the GIL.
        // We use Option to distinguish between a future (Some) and a sync result (None).
        type BoxedFuture =
            std::pin::Pin<Box<dyn std::future::Future<Output = PyResult<Py<PyAny>>> + Send>>;

        let (maybe_future, maybe_result): (Option<BoxedFuture>, Option<Py<PyAny>>) =
            Python::attach(
                |py| -> Result<(Option<BoxedFuture>, Option<Py<PyAny>>), GithubClientError> {
                    let client = client.bind(py);

                    // Check if the method exists
                    if !client.hasattr(&method_name).map_err(py_err_to_github_err)? {
                        return Err(GithubClientError::Other(format!(
                            "GitHub client does not have {} method",
                            method_name
                        )));
                    }

                    // Build args tuple
                    let py_args = pyo3::types::PyTuple::new(py, args.iter().map(|s| s.as_str()))
                        .map_err(py_err_to_github_err)?;

                    // Call the method
                    let result = client
                        .call_method1(&method_name, py_args)
                        .map_err(py_err_to_github_err)?;

                    // Check if it's a coroutine
                    let is_coroutine = result.hasattr("__await__").map_err(py_err_to_github_err)?;

                    if is_coroutine {
                        // Convert the coroutine to a future inside the GIL
                        debug!("Python method returned a coroutine, awaiting...");
                        let future = pyo3_async_runtimes::tokio::into_future(result)
                            .map_err(py_err_to_github_err)?;
                        Ok((Some(Box::pin(future)), None))
                    } else {
                        // It's a regular value, return it directly
                        debug!("Python method returned sync result");
                        Ok((None, Some(result.unbind())))
                    }
                },
            )?;

        if let Some(future) = maybe_future {
            future
                .await
                .map_err(|e| GithubClientError::Other(e.to_string()))
        } else {
            // When maybe_future is None, maybe_result must be Some (sync result path).
            // This is guaranteed by the logic above, but we handle it defensively.
            maybe_result.ok_or_else(|| {
                GithubClientError::Other(
                    "Internal error: Python method returned neither a coroutine nor a value"
                        .to_string(),
                )
            })
        }
    }
}

#[async_trait]
impl GithubClient for PyGithubClient {
    async fn user_exists(&self, username: &str) -> Result<UserExistsResult, GithubClientError> {
        debug!("Checking if user exists: {}", username);

        let result = self
            .call_python_method_async("user_exists", vec![username.to_string()])
            .await?;

        // Parse the result
        let parsed = Python::attach(|py| {
            let result = result.bind(py);

            if let Ok(exists) = result.extract::<bool>() {
                if exists {
                    Ok(UserExistsResult::Exists)
                } else {
                    Ok(UserExistsResult::NotFound)
                }
            } else if let Ok(status) = result.extract::<String>() {
                match status.as_str() {
                    "exists" => Ok(UserExistsResult::Exists),
                    "not_found" => Ok(UserExistsResult::NotFound),
                    "unauthorized" => Ok(UserExistsResult::Unauthorized),
                    _ => Err(GithubClientError::Other(format!(
                        "Unknown user_exists result: {}",
                        status
                    ))),
                }
            } else {
                Err(GithubClientError::Other(
                    "user_exists returned an unexpected type".to_string(),
                ))
            }
        });

        debug!("User '{}' check result: {:?}", username, parsed);
        parsed
    }

    async fn team_exists(
        &self,
        org: &str,
        team: &str,
    ) -> Result<TeamExistsResult, GithubClientError> {
        debug!("Checking if team exists: {}/{}", org, team);

        let result = self
            .call_python_method_async("team_exists", vec![org.to_string(), team.to_string()])
            .await?;

        // Parse the result - team_exists returns a string status
        let parsed = Python::attach(|py| {
            let result = result.bind(py);

            if let Ok(status) = result.extract::<String>() {
                match status.as_str() {
                    "exists" => Ok(TeamExistsResult::Exists),
                    "not_found" => Ok(TeamExistsResult::NotFound),
                    "unauthorized" => Ok(TeamExistsResult::Unauthorized),
                    _ => Err(GithubClientError::Other(format!(
                        "Unknown team_exists result: {}",
                        status
                    ))),
                }
            } else if let Ok(exists) = result.extract::<bool>() {
                // Fallback for simple bool return
                if exists {
                    Ok(TeamExistsResult::Exists)
                } else {
                    Ok(TeamExistsResult::NotFound)
                }
            } else {
                Err(GithubClientError::Other(
                    "team_exists returned an unexpected type".to_string(),
                ))
            }
        });

        debug!("Team '{}/{}' check result: {:?}", org, team, parsed);
        parsed
    }
}

// SAFETY: PyGithubClient can implement Send and Sync because:
//
// 1. The only field is `client: Py<PyAny>`, which is PyO3's GIL-independent reference.
//    `Py<T>` is explicitly designed to be Send+Sync - it's a reference-counted pointer
//    to a Python object that can be safely transferred between threads.
//
// 2. All access to the Python object happens through `Python::attach()` which properly
//    acquires the GIL before any Python operations. We never hold borrowed references
//    (`Bound<'py, T>`) across await points or thread boundaries.
//
// 3. The async methods (`call_python_method_async`, `user_exists`, `team_exists`) use
//    `Python::attach()` to acquire the GIL within the current thread context, ensuring
//    thread-safe access to the Python interpreter.
//
// 4. When converting Python coroutines to Rust futures via `pyo3_async_runtimes::tokio::into_future`,
//    the resulting future is Send-safe as documented by pyo3-async-runtimes.
unsafe impl Send for PyGithubClient {}
unsafe impl Sync for PyGithubClient {}

#[cfg(test)]
mod tests {
    // Tests would require Python interpreter
}
