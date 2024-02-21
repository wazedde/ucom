use chrono::{DateTime, Utc};

mod data_structures;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TestResult {
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
            "Failed" => TestResult::Failed,
            "Failed(Child)" => TestResult::Failed,
            "Inconclusive" => TestResult::Inconclusive,
            "Skipped" => TestResult::Skipped,
            _ => TestResult::Invalid,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Stats {
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

#[cfg(test)]
mod tests {
    use crate::nunit::data_structures::TestRun;
    use crate::nunit::TestResult;

    #[test]
    fn test_deserialize_standalone() {
        let tr = include_str!("test_data/standalone.xml")
            .parse::<TestRun>()
            .unwrap();

        assert_eq!(tr.test_result(), TestResult::Passed);

        let stats = tr.stats();
        assert_eq!(stats.total, 31);
        assert_eq!(stats.passed, 31);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.inconclusive, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.asserts, 0);
    }

    #[test]
    fn test_deserialize_editmode() {
        let tr = include_str!("test_data/editmode.xml")
            .parse::<TestRun>()
            .unwrap();

        assert_eq!(tr.test_result(), TestResult::Passed);

        let stats = tr.stats();
        assert_eq!(stats.total, 72);
        assert_eq!(stats.passed, 72);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.inconclusive, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.asserts, 0);
    }

    #[test]
    fn test_deserialize_standalone_fail() {
        let tr = include_str!("test_data/standalone-fail.xml")
            .parse::<TestRun>()
            .unwrap();

        assert_eq!(tr.test_result(), TestResult::Failed);
        let stats = tr.stats();
        assert_eq!(stats.total, 31);
        assert_eq!(stats.passed, 30);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.inconclusive, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.asserts, 0);
    }

    #[test]
    fn test_deserialize_playmode_fail() {
        let tr = include_str!("test_data/playmode-fail.xml")
            .parse::<TestRun>()
            .unwrap();

        assert_eq!(tr.test_result(), TestResult::Failed);

        let stats = tr.stats();
        assert_eq!(stats.total, 31);
        assert_eq!(stats.passed, 30);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.inconclusive, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.asserts, 0);
    }

    #[test]
    fn test_deserialize_editmode_fail() {
        let tr = include_str!("test_data/editmode-fail.xml")
            .parse::<TestRun>()
            .unwrap();

        assert_eq!(tr.test_result(), TestResult::Failed);

        let stats = tr.stats();
        assert_eq!(stats.total, 72);
        assert_eq!(stats.passed, 70);
        assert_eq!(stats.failed, 2);
        assert_eq!(stats.inconclusive, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.asserts, 0);
    }
}
