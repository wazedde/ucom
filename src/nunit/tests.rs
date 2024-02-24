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
