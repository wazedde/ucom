use std::str::FromStr;

use chrono::{DateTime, Utc};
use quick_xml::DeError;
use serde::Deserialize;

use crate::nunit;
use crate::nunit::{TestResult, TestStats};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "test-run")]
pub struct TestRun {
    #[serde(rename = "@id")]
    id: i32,

    #[serde(rename = "@testcasecount")]
    test_case_count: i32,

    #[serde(rename = "@result")]
    result: String,

    #[serde(rename = "@total")]
    total: i32,

    #[serde(rename = "@passed")]
    passed: i32,

    #[serde(rename = "@failed")]
    failed: i32,

    #[serde(rename = "@inconclusive")]
    inconclusive: i32,

    #[serde(rename = "@skipped")]
    skipped: i32,

    #[serde(rename = "@asserts")]
    asserts: i32,

    #[serde(rename = "@engine-version")]
    engine_version: String,

    #[serde(rename = "@clr-version")]
    clr_version: String,

    #[serde(rename = "@start-time")]
    start_time: DateTime<Utc>,

    #[serde(rename = "@end-time")]
    end_time: DateTime<Utc>,

    #[serde(rename = "@duration")]
    duration: f64,

    #[serde(rename = "$value")]
    elements: Vec<TestRunElement>,
}

impl TestRun {
    /// The overall result. Unlike tests and suites, which have a separate `label` attribute,
    /// the run writes the label after a colon, as in `Skipped:Ignored`; it is ignored here.
    pub fn test_result(&self) -> TestResult {
        let result = self
            .result
            .split_once(':')
            .map_or(self.result.as_str(), |(r, _)| r);
        result.into()
    }

    pub fn stats(&self) -> TestStats {
        TestStats {
            id: self.id,
            test_case_count: self.test_case_count,
            result: self.test_result(),
            total: self.total,
            passed: self.passed,
            failed: self.failed,
            inconclusive: self.inconclusive,
            skipped: self.skipped,
            asserts: self.asserts,
            start_time: self.start_time,
            end_time: self.end_time,
            duration: self.duration,
        }
    }

    pub fn collect_test_cases(&self) -> Vec<nunit::TestCase> {
        let mut cases = Vec::new();
        for element in &self.elements {
            if let TestRunElement::TestSuite(ts) = element {
                ts.add_test_cases_to(&mut cases);
            }
        }
        cases
    }
}

impl FromStr for TestRun {
    type Err = DeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        quick_xml::de::from_str::<Self>(s)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
enum TestRunElement {
    #[serde(rename = "test-suite")]
    TestSuite(Box<TestSuite>),

    /// Any other element, such as `command-line` or `filter` from other NUnit runners.
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize, PartialEq)]
struct TestSuite {
    #[serde(rename = "@type")]
    r#type: String,

    #[serde(rename = "@id")]
    id: i32,

    #[serde(rename = "@name")]
    name: String,

    #[serde(rename = "@fullname")]
    full_name: String,

    #[serde(rename = "@classname", default)]
    class_name: String,

    #[serde(rename = "@testcasecount")]
    test_case_count: i32,

    #[serde(rename = "@runstate")]
    run_state: String,

    #[serde(rename = "@result")]
    result: String,

    #[serde(rename = "@label", default)]
    label: String,

    #[serde(rename = "@site", default)]
    site: String,

    #[serde(rename = "@start-time")]
    start_time: DateTime<Utc>,

    #[serde(rename = "@end-time")]
    end_time: DateTime<Utc>,

    #[serde(rename = "@duration")]
    duration: f64,

    #[serde(rename = "@total")]
    total: i32,

    #[serde(rename = "@passed")]
    passed: i32,

    #[serde(rename = "@failed")]
    failed: i32,

    #[serde(rename = "@inconclusive")]
    inconclusive: i32,

    #[serde(rename = "@skipped")]
    skipped: i32,

    #[serde(rename = "@asserts")]
    asserts: i32,

    #[serde(rename = "$value")]
    elements: Vec<TestSuiteElement>,
}

/// The children of a `test-suite` that ucom reads. Anything else is skipped: properties,
/// output, failure and reason elements, text, and elements from newer NUnit versions.
#[derive(Debug, Deserialize, PartialEq)]
enum TestSuiteElement {
    #[serde(rename = "test-suite")]
    TestSuite(Box<TestSuite>),

    #[serde(rename = "test-case")]
    TestCase(Box<TestCase>),

    #[serde(other)]
    Other,
}

/// Why a test was skipped or ignored, for example the message of `[Ignore]`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
struct Reason {
    #[serde(rename = "message")]
    message: Option<TextElement>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct TextElement {
    /// Empty for an empty element such as `<message />`.
    #[serde(rename = "$value", default)]
    text: String,
}

/// The text of an optional element, or an empty string.
fn text_of(element: Option<&TextElement>) -> String {
    element.map_or_default(|e| e.text.clone())
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename = "test-case")]
struct TestCase {
    #[serde(rename = "@id")]
    id: i32,

    #[serde(rename = "@name")]
    name: String,

    #[serde(rename = "@fullname")]
    full_name: String,

    #[serde(rename = "@methodname")]
    method_name: String,

    #[serde(rename = "@classname")]
    class_name: String,

    #[serde(rename = "@runstate")]
    run_state: String,

    #[serde(rename = "@seed")]
    seed: i32,

    #[serde(rename = "@result")]
    result: String,

    #[serde(rename = "@label", default)]
    label: String,

    #[serde(rename = "@site", default)]
    site: String,

    #[serde(rename = "@start-time")]
    start_time: DateTime<Utc>,

    #[serde(rename = "@end-time")]
    end_time: DateTime<Utc>,

    #[serde(rename = "@duration")]
    duration: f64,

    #[serde(rename = "$value")]
    elements: Vec<TestCaseElement>,
}

impl TestCase {
    fn text(&self) -> Option<String> {
        self.elements.iter().find_map(|e| match e {
            TestCaseElement::Text(s) => Some(s.clone()),
            _ => None,
        })
    }
    fn failure(&self) -> Option<&Failure> {
        self.elements.iter().find_map(|e| match e {
            TestCaseElement::Failure(f) => Some(f),
            _ => None,
        })
    }
    fn reason(&self) -> Option<&Reason> {
        self.elements.iter().find_map(|e| match e {
            TestCaseElement::Reason(r) => Some(r),
            _ => None,
        })
    }
}

impl TestSuite {
    fn add_test_cases_to(&self, test_cases: &mut Vec<nunit::TestCase>) {
        for element in &self.elements {
            match element {
                TestSuiteElement::TestCase(tc) => {
                    let tc = tc.as_ref().into();
                    test_cases.push(tc);
                }
                TestSuiteElement::TestSuite(ts) => {
                    ts.add_test_cases_to(test_cases);
                }
                _ => { /* Ignore other elements */ }
            }
        }
    }
}

impl From<&TestCase> for nunit::TestCase {
    fn from(value: &TestCase) -> Self {
        let failure = value.failure();
        Self {
            id: value.id,
            name: value.name.clone(),
            full_name: value.full_name.clone(),
            run_state: value.run_state.clone(),
            result: value.result.as_str().into(),
            duration: value.duration,
            start_time: value.start_time,
            end_time: value.end_time,
            text: value.text().unwrap_or_default(),
            failure_message: text_of(failure.and_then(|f| f.message.as_ref())),
            failure_stack_trace: text_of(failure.and_then(|f| f.stack_trace.as_ref())),
            failure_text: failure.map_or_default(|f| f.text.clone()),
            skip_reason: text_of(value.reason().and_then(|r| r.message.as_ref())),
        }
    }
}

/// The children of a `test-case` that ucom reads. Anything else is skipped: properties,
/// output, and elements from newer NUnit versions such as `assertions` and `attachments`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
enum TestCaseElement {
    #[serde(rename = "failure")]
    Failure(Failure),

    #[serde(rename = "reason")]
    Reason(Reason),

    #[serde(rename = "$text")]
    Text(String),

    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct Failure {
    #[serde(rename = "message")]
    message: Option<TextElement>,

    #[serde(rename = "stack-trace")]
    stack_trace: Option<TextElement>,

    #[serde(rename = "$text", default)]
    text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_message_is_accepted() {
        for xml in [
            "<reason><message /></reason>",
            "<reason><message></message></reason>",
            "<reason><message><![CDATA[]]></message></reason>",
        ] {
            let reason: Reason = quick_xml::de::from_str(xml).unwrap();
            assert_eq!(text_of(reason.message.as_ref()), "", "{xml}");
        }
    }

    /// Elements ucom does not read, including ones from newer NUnit versions, are skipped
    /// at every level.
    #[test]
    fn unknown_elements_are_skipped() {
        let xml = r#"
            <test-run id="1" testcasecount="1" result="Skipped:Ignored" total="1" passed="0"
                failed="0" inconclusive="0" skipped="1" asserts="0" engine-version="3.5.0.0"
                clr-version="4.0" start-time="2026-11-15 19:00:00Z"
                end-time="2026-11-15 19:00:01Z" duration="1">
              <command-line>unity -runTests</command-line>
              <test-suite type="TestFixture" id="2" name="Fixture" fullname="Fixture"
                  testcasecount="1" runstate="Runnable" result="Skipped"
                  start-time="2026-11-15 19:00:00Z" end-time="2026-11-15 19:00:01Z"
                  duration="1" total="1" passed="0" failed="0" inconclusive="0" skipped="1"
                  asserts="0">
                <properties><property name="_PID" value="1" /></properties>
                <reason><message>Child ignored</message></reason>
                <attachments><attachment><filePath>a.png</filePath></attachment></attachments>
                <test-case id="3" name="Case" fullname="Fixture.Case" methodname="Case"
                    classname="Fixture" runstate="Ignored" seed="1" result="Skipped"
                    start-time="2026-11-15 19:00:00Z" end-time="2026-11-15 19:00:01Z"
                    duration="0">
                  <properties />
                  <reason><message>Ignored on purpose</message></reason>
                  <assertions><assertion result="Warning"><message>Careful</message></assertion></assertions>
                  <output>Some output</output>
                </test-case>
              </test-suite>
            </test-run>"#;

        let run: TestRun = xml.parse().unwrap();
        assert_eq!(run.test_result(), TestResult::Skipped);
        let cases = run.collect_test_cases();
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].result, TestResult::Skipped);
        assert_eq!(cases[0].skip_reason, "Ignored on purpose");
    }
}
