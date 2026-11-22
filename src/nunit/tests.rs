//! Tests for reading NUnit result files, using the files in `test_data/`.

use crate::nunit::{TestCase, TestResult, TestRun};

/// Results from standalone, EditMode and PlayMode runs, passing and failing.
#[test]
fn results_of_each_test_platform_are_read() {
    for (file, result, total, failed) in [
        ("standalone.xml", TestResult::Passed, 31, 0),
        ("standalone-fail.xml", TestResult::Failed, 31, 1),
        ("editmode.xml", TestResult::Passed, 72, 0),
        ("editmode-fail.xml", TestResult::Failed, 72, 2),
        ("playmode-fail.xml", TestResult::Failed, 31, 1),
    ] {
        let tr = TestRun::from_file(format!("./src/nunit/test_data/{file}")).unwrap();
        let stats = &tr.stats;
        assert_eq!(stats.result, result, "{file}");
        assert_eq!(
            (stats.total, stats.passed, stats.failed),
            (total, total - failed, failed),
            "{file}"
        );
        // The test cases are read separately from the totals.
        let failed_cases = tr
            .test_cases
            .iter()
            .filter(|tc| tc.result == TestResult::Failed);
        assert_eq!(tr.test_cases.len(), total as usize, "{file}");
        assert_eq!(failed_cases.count(), failed as usize, "{file}");
    }
}

fn test_case<'a>(tr: &'a TestRun, name: &str) -> &'a TestCase {
    tr.test_cases.iter().find(|tc| tc.name == name).unwrap()
}

/// A skipped test has a `<reason>` element, and so do its suites when no test failed.
/// From `test-projects/minimal` on Unity 2021.3, with `--categories "!Failing"`.
#[test]
fn a_skipped_test_has_its_reason() {
    let tr = TestRun::from_file("./src/nunit/test_data/editmode-skipped.xml").unwrap();
    let stats = &tr.stats;

    assert_eq!(stats.result, TestResult::Skipped);
    assert_eq!(stats.total, 2);
    assert_eq!(stats.passed, 1);
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.skipped, 1);

    let skipped = test_case(&tr, "IsSkipped");
    assert_eq!(skipped.result, TestResult::Skipped);
    assert_eq!(
        skipped.skip_reason,
        "Always skipped, to give ucom a skipped result"
    );
    assert_eq!(test_case(&tr, "Passes").result, TestResult::Passed);
}

/// The same project with all tests, including the failing one: the suites have a
/// `<failure>` instead.
#[test]
fn a_failed_test_has_its_message() {
    let tr = TestRun::from_file("./src/nunit/test_data/editmode-skipped-fail.xml").unwrap();
    let stats = &tr.stats;

    assert_eq!(stats.result, TestResult::Failed);
    assert_eq!(stats.total, 3);
    assert_eq!(stats.passed, 1);
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.skipped, 1);

    let failed = test_case(&tr, "FailsOnPurpose");
    assert_eq!(failed.result, TestResult::Failed);
    assert_eq!(failed.failure_message, "Failing on purpose");
    assert_eq!(test_case(&tr, "IsSkipped").result, TestResult::Skipped);
    assert_eq!(test_case(&tr, "Passes").result, TestResult::Passed);
}
