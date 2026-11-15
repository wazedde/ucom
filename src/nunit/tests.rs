#[cfg(test)]
mod test_run_tests {
    use nunit::TestRun;

    use crate::nunit;
    use crate::nunit::TestResult;

    #[test]
    fn test_deserialize_standalone() {
        let tr = TestRun::from_file("./src/nunit/test_data/standalone.xml").unwrap();
        let stats = tr.stats;

        assert_eq!(stats.result, TestResult::Passed);
        assert_eq!(stats.total, 31);
        assert_eq!(stats.passed, 31);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.inconclusive, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.asserts, 0);

        let tc = tr.test_cases;
        assert_eq!(tc.len(), 31);
    }

    #[test]
    fn test_deserialize_editmode() {
        let tr = TestRun::from_file("./src/nunit/test_data/editmode.xml").unwrap();
        let stats = tr.stats;

        assert_eq!(stats.result, TestResult::Passed);
        assert_eq!(stats.total, 72);
        assert_eq!(stats.passed, 72);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.inconclusive, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.asserts, 0);

        let tc = tr.test_cases;
        assert_eq!(tc.len(), 72);
    }

    fn test_case<'a>(tr: &'a TestRun, name: &str) -> &'a crate::nunit::TestCase {
        tr.test_cases.iter().find(|tc| tc.name == name).unwrap()
    }

    /// A skipped test has a `<reason>` element, and so do its suites when no test failed.
    /// From `test-projects/minimal` on Unity 2021.3, with `--categories "!Failing"`.
    #[test]
    fn test_deserialize_editmode_with_skipped_test() {
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
    fn test_deserialize_editmode_with_skipped_and_failed_tests() {
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

    #[test]
    fn test_deserialize_standalone_fail() {
        let tr = TestRun::from_file("./src/nunit/test_data/standalone-fail.xml").unwrap();
        let stats = tr.stats;

        assert_eq!(stats.result, TestResult::Failed);
        assert_eq!(stats.total, 31);
        assert_eq!(stats.passed, 30);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.inconclusive, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.asserts, 0);

        let tc = tr.test_cases;
        assert_eq!(tc.len(), 31);
    }

    #[test]
    fn test_deserialize_playmode_fail() {
        let tr = TestRun::from_file("./src/nunit/test_data/playmode-fail.xml").unwrap();
        let stats = tr.stats;

        assert_eq!(stats.result, TestResult::Failed);
        assert_eq!(stats.total, 31);
        assert_eq!(stats.passed, 30);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.inconclusive, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.asserts, 0);

        let tc = tr.test_cases;
        assert_eq!(tc.len(), 31);

        assert_eq!(
            tc.iter().filter(|x| x.result == TestResult::Passed).count(),
            30
        );
        assert_eq!(
            tc.iter().filter(|x| x.result == TestResult::Failed).count(),
            1
        );
    }

    #[test]
    fn test_deserialize_editmode_fail() {
        let tr = TestRun::from_file("./src/nunit/test_data/editmode-fail.xml").unwrap();
        let stats = tr.stats;
        assert_eq!(stats.result, TestResult::Failed);

        assert_eq!(stats.total, 72);
        assert_eq!(stats.passed, 70);
        assert_eq!(stats.failed, 2);
        assert_eq!(stats.inconclusive, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.asserts, 0);

        let tc = tr.test_cases;
        assert_eq!(tc.len(), 72);
        assert_eq!(
            tc.iter().filter(|x| x.result == TestResult::Passed).count(),
            70
        );
        assert_eq!(
            tc.iter().filter(|x| x.result == TestResult::Failed).count(),
            2
        );
    }
}
#[cfg(test)]
mod elements_tests {
    use crate::nunit::elements;

    #[test]
    fn test_deserialize_empty_properties() {
        _ = include_str!("test_data/empty-properties.xml")
            .parse::<elements::TestRun>()
            .unwrap();
    }
}
