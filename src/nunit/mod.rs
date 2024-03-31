use std::path::Path;

use chrono::{DateTime, Utc};
use strum::{AsRefStr, Display};

mod elements;
mod tests;

#[derive(Debug, PartialEq)]
pub(crate) struct TestRun {
    pub stats: TestStats,
    pub test_cases: Vec<TestCase>,
}

impl TestRun {
    /// Parses the given XML string into a `TestRun`.
    pub(crate) fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<TestRun> {
        let xml = std::fs::read_to_string(path)?;
        let test_run = xml.parse::<elements::TestRun>()?;
        let test_cases = test_run.collect_test_cases();
        let stats = test_run.stats();
        Ok(TestRun { stats, test_cases })
    }
}

#[derive(Display, AsRefStr, Copy, Clone, Debug, PartialEq)]
pub(crate) enum TestResult {
    Passed,
    Failed,
    Inconclusive,
    Skipped,

    /// The result could not be parsed.
    Invalid,
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
pub(crate) struct TestStats {
    pub(crate) id: i32,
    pub(crate) test_case_count: i32,
    pub(crate) result: TestResult,
    pub(crate) total: i32,
    pub(crate) passed: i32,
    pub(crate) failed: i32,
    pub(crate) inconclusive: i32,
    pub(crate) skipped: i32,
    pub(crate) asserts: i32,
    pub(crate) start_time: DateTime<Utc>,
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) duration: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TestCase {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) full_name: String,
    pub(crate) run_state: String,
    pub(crate) result: TestResult,
    pub(crate) duration: f64,
    pub(crate) start_time: DateTime<Utc>,
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) text: String,
    pub(crate) failure_message: String,
    pub(crate) failure_stack_trace: String,
    pub(crate) failure_text: String,
}
