use std::fmt::Display;
use std::path::Path;

use chrono::{DateTime, Utc};

mod elements;
mod tests;

#[derive(Debug, PartialEq)]
pub struct TestRun {
    pub stats: TestStats,
    pub test_cases: Vec<TestCase>,
}

impl TestRun {
    /// Parses the given XML string into a `TestRun`.
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<TestRun> {
        let xml = std::fs::read_to_string(path)?;
        let test_run = xml.parse::<elements::TestRun>()?;
        let test_cases = test_run.collect_test_cases();
        let stats = test_run.stats();
        Ok(TestRun { stats, test_cases })
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TestResult {
    Passed,
    Failed,
    Inconclusive,
    Skipped,

    /// The result could not be parsed.
    Invalid,
}

impl Display for TestResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            TestResult::Passed => "Passed".to_string(),
            TestResult::Failed => "Failed".to_string(),
            TestResult::Inconclusive => "Inconclusive".to_string(),
            TestResult::Skipped => "Skipped".to_string(),
            TestResult::Invalid => "Invalid".to_string(),
        };
        write!(f, "{}", str)
    }
}

impl From<&str> for TestResult {
    fn from(s: &str) -> Self {
        match s {
            "Passed" => TestResult::Passed,
            "Failed" | "Failed(Child)" => TestResult::Failed,
            "Inconclusive" => TestResult::Inconclusive,
            "Skipped" => TestResult::Skipped,
            _ => TestResult::Invalid,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestStats {
    pub id: i32,
    pub test_case_count: i32,
    pub result: TestResult,
    pub total: i32,
    pub passed: i32,
    pub failed: i32,
    pub inconclusive: i32,
    pub skipped: i32,
    pub asserts: i32,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestCase {
    pub id: i32,
    pub name: String,
    pub full_name: String,
    pub run_state: String,
    pub result: TestResult,
    pub duration: f64,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub text: String,
    pub failure_message: String,
    pub failure_stack_trace: String,
    pub failure_text: String,
}
