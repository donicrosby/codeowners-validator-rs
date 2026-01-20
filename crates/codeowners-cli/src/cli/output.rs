//! Output formatting for the CLI.
//!
//! This module provides human-readable and JSON output formatters for validation results.

use codeowners_validator_core::validate::{Severity, ValidationError, ValidationResult};
use colored::Colorize;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;

/// JSON output format matching the Go version.
#[derive(Debug, Serialize)]
pub struct JsonOutput {
    /// Syntax check results.
    pub syntax: Vec<JsonIssue>,
    /// Duplicate patterns check results.
    pub duppatterns: Vec<JsonIssue>,
    /// Files check results.
    pub files: Vec<JsonIssue>,
    /// Owners check results.
    pub owners: Vec<JsonIssue>,
    /// Not-owned check results (experimental).
    pub notowned: Vec<JsonIssue>,
    /// Avoid-shadowing check results (experimental).
    #[serde(rename = "avoid-shadowing")]
    pub avoid_shadowing: Vec<JsonIssue>,
}

impl Default for JsonOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonOutput {
    /// Creates a new empty JSON output.
    pub fn new() -> Self {
        Self {
            syntax: Vec::new(),
            duppatterns: Vec::new(),
            files: Vec::new(),
            owners: Vec::new(),
            notowned: Vec::new(),
            avoid_shadowing: Vec::new(),
        }
    }

    /// Adds issues from a validation result to the appropriate check category.
    pub fn add_check_results(&mut self, check_name: &str, result: &ValidationResult) {
        let issues: Vec<JsonIssue> = result.errors.iter().map(JsonIssue::from).collect();

        match check_name {
            "syntax" => self.syntax.extend(issues),
            "duppatterns" => self.duppatterns.extend(issues),
            "files" => self.files.extend(issues),
            "owners" => self.owners.extend(issues),
            "notowned" => self.notowned.extend(issues),
            "avoid-shadowing" | "shadowing" => self.avoid_shadowing.extend(issues),
            _ => {} // Unknown check name, ignore
        }
    }

    /// Writes the JSON output to a writer.
    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        writeln!(writer, "{}", json)
    }
}

/// A single issue in JSON format.
#[derive(Debug, Serialize)]
pub struct JsonIssue {
    /// Line number where the issue occurred.
    pub line: usize,
    /// Column number where the issue occurred.
    pub column: usize,
    /// Human-readable message.
    pub message: String,
    /// Severity of the issue.
    pub severity: Severity,
}

impl From<&ValidationError> for JsonIssue {
    fn from(error: &ValidationError) -> Self {
        let span = error.span();
        Self {
            line: span.line,
            column: span.column,
            message: error.to_string(),
            severity: error.severity(),
        }
    }
}

/// Output formatter for human-readable console output.
pub struct HumanOutput<W: Write> {
    writer: W,
    use_colors: bool,
}

impl<W: Write> HumanOutput<W> {
    /// Creates a new human output formatter.
    pub fn new(writer: W, use_colors: bool) -> Self {
        Self { writer, use_colors }
    }

    /// Writes a header for a check.
    pub fn write_check_header(&mut self, check_name: &str) -> std::io::Result<()> {
        let header = format!("==> {}", check_name);
        if self.use_colors {
            writeln!(self.writer, "\n{}", header.cyan().bold())?;
        } else {
            writeln!(self.writer, "\n{}", header)?;
        }
        Ok(())
    }

    /// Writes validation results for a check.
    pub fn write_check_results(
        &mut self,
        check_name: &str,
        result: &ValidationResult,
    ) -> std::io::Result<()> {
        if result.errors.is_empty() {
            return Ok(());
        }

        self.write_check_header(check_name)?;

        for error in &result.errors {
            self.write_issue(error)?;
        }

        Ok(())
    }

    /// Writes a single issue.
    pub fn write_issue(&mut self, error: &ValidationError) -> std::io::Result<()> {
        let severity = error.severity();
        let message = error.to_string();

        let label = match severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN",
        };

        if self.use_colors {
            let colored_label = match severity {
                Severity::Error => format!("[{}]", label).red().bold(),
                Severity::Warning => format!("[{}]", label).yellow().bold(),
            };
            writeln!(self.writer, "  {} {}", colored_label, message)?;
        } else {
            writeln!(self.writer, "  [{}] {}", label, message)?;
        }

        Ok(())
    }

    /// Writes a summary of all validation results.
    pub fn write_summary(
        &mut self,
        total_errors: usize,
        total_warnings: usize,
    ) -> std::io::Result<()> {
        writeln!(self.writer)?;

        if total_errors == 0 && total_warnings == 0 {
            let message = "✓ CODEOWNERS file is valid";
            if self.use_colors {
                writeln!(self.writer, "{}", message.green().bold())?;
            } else {
                writeln!(self.writer, "{}", message)?;
            }
        } else {
            let message = format!(
                "✗ Found {} error(s) and {} warning(s)",
                total_errors, total_warnings
            );
            if self.use_colors {
                writeln!(self.writer, "{}", message.red().bold())?;
            } else {
                writeln!(self.writer, "{}", message)?;
            }
        }

        Ok(())
    }

    /// Writes a startup error.
    pub fn write_error(&mut self, message: &str) -> std::io::Result<()> {
        if self.use_colors {
            writeln!(self.writer, "{} {}", "Error:".red().bold(), message)?;
        } else {
            writeln!(self.writer, "Error: {}", message)?;
        }
        Ok(())
    }
}

/// Collects all validation results organized by check name.
#[derive(Debug, Default)]
pub struct ValidationResults {
    results: HashMap<String, ValidationResult>,
    order: Vec<String>,
}

impl ValidationResults {
    /// Creates a new empty results collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds results for a check.
    pub fn add(&mut self, check_name: impl Into<String>, result: ValidationResult) {
        let name = check_name.into();
        if !self.results.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.results.entry(name).or_default().merge(result);
    }

    /// Returns the total number of errors.
    pub fn total_errors(&self) -> usize {
        self.results.values().map(|r| r.errors_only().count()).sum()
    }

    /// Returns the total number of warnings.
    pub fn total_warnings(&self) -> usize {
        self.results
            .values()
            .map(|r| r.warnings_only().count())
            .sum()
    }

    /// Returns true if there are any errors.
    pub fn has_errors(&self) -> bool {
        self.total_errors() > 0
    }

    /// Returns true if there are any warnings.
    pub fn has_warnings(&self) -> bool {
        self.total_warnings() > 0
    }

    /// Iterates over results in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ValidationResult)> {
        self.order
            .iter()
            .filter_map(|name| self.results.get(name).map(|r| (name.as_str(), r)))
    }

    /// Writes results in human-readable format.
    pub fn write_human<W: Write>(&self, writer: &mut W, use_colors: bool) -> std::io::Result<()> {
        let mut output = HumanOutput::new(writer, use_colors);

        for (name, result) in self.iter() {
            output.write_check_results(name, result)?;
        }

        output.write_summary(self.total_errors(), self.total_warnings())?;

        Ok(())
    }

    /// Writes results in JSON format.
    pub fn write_json<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let mut json_output = JsonOutput::new();

        for (name, result) in self.iter() {
            json_output.add_check_results(name, result);
        }

        json_output.write(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codeowners_validator_core::parse::span::Span;

    fn test_span() -> Span {
        Span::new(0, 1, 1, 5)
    }

    #[test]
    fn test_json_issue_from_error() {
        let error = ValidationError::duplicate_pattern("*.rs", test_span(), 1);
        let issue = JsonIssue::from(&error);

        assert_eq!(issue.line, 1);
        assert_eq!(issue.column, 1);
        assert!(issue.message.contains("duplicate"));
        assert_eq!(issue.severity, Severity::Warning);
    }

    #[test]
    fn test_json_issue_from_file_not_owned() {
        let eof_span = Span::point(50, 3, 1); // EOF at line 3
        let error = ValidationError::file_not_owned("src/main.rs", eof_span);
        let issue = JsonIssue::from(&error);

        assert_eq!(issue.line, 3);
        assert_eq!(issue.column, 1);
        assert!(issue.message.contains("src/main.rs"));
        assert_eq!(issue.severity, Severity::Warning);
    }

    #[test]
    fn test_json_output_add_results() {
        let mut output = JsonOutput::new();
        let mut result = ValidationResult::new();
        result.add_error(ValidationError::duplicate_pattern("*.rs", test_span(), 1));

        output.add_check_results("duppatterns", &result);

        assert_eq!(output.duppatterns.len(), 1);
        assert!(output.syntax.is_empty());
    }

    #[test]
    fn test_json_output_serialize() {
        let mut output = JsonOutput::new();
        let mut result = ValidationResult::new();
        result.add_error(ValidationError::duplicate_pattern("*.rs", test_span(), 1));
        output.add_check_results("syntax", &result);

        let mut buf = Vec::new();
        output.write(&mut buf).unwrap();

        let json: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert!(json["syntax"].is_array());
        assert_eq!(json["syntax"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_human_output_no_colors() {
        let mut buf = Vec::new();
        let mut output = HumanOutput::new(&mut buf, false);

        let error = ValidationError::duplicate_pattern("*.rs", test_span(), 1);
        output.write_issue(&error).unwrap();

        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("[WARN]"));
        assert!(text.contains("duplicate"));
    }

    #[test]
    fn test_validation_results_totals() {
        let mut results = ValidationResults::new();

        let mut r1 = ValidationResult::new();
        r1.add_error(ValidationError::invalid_owner_format(
            "bad",
            "reason",
            test_span(),
        ));
        results.add("syntax", r1);

        let mut r2 = ValidationResult::new();
        r2.add_error(ValidationError::duplicate_pattern("*.rs", test_span(), 1));
        results.add("duppatterns", r2);

        assert_eq!(results.total_errors(), 1);
        assert_eq!(results.total_warnings(), 1);
        assert!(results.has_errors());
        assert!(results.has_warnings());
    }

    #[test]
    fn test_validation_results_order() {
        let mut results = ValidationResults::new();
        results.add("syntax", ValidationResult::new());
        results.add("files", ValidationResult::new());
        results.add("owners", ValidationResult::new());

        let names: Vec<_> = results.iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["syntax", "files", "owners"]);
    }

    #[test]
    fn test_human_output_summary_valid() {
        let mut buf = Vec::new();
        let mut output = HumanOutput::new(&mut buf, false);
        output.write_summary(0, 0).unwrap();

        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("valid"));
    }

    #[test]
    fn test_human_output_summary_with_issues() {
        let mut buf = Vec::new();
        let mut output = HumanOutput::new(&mut buf, false);
        output.write_summary(2, 3).unwrap();

        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("2 error(s)"));
        assert!(text.contains("3 warning(s)"));
    }
}
