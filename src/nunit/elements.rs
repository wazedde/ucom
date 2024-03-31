use std::fmt::{Display, Formatter};
use std::str::FromStr;

use chrono::{DateTime, Utc};
use quick_xml::DeError;
use serde::Deserialize;

use crate::nunit;
use crate::nunit::{TestResult, TestStats};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "test-run")]
pub(crate) struct TestRun {
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
    pub(crate) fn test_result(&self) -> TestResult {
        self.result.as_str().into()
    }

    pub(crate) fn stats(&self) -> TestStats {
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

    pub(crate) fn collect_test_cases(&self) -> Vec<nunit::TestCase> {
        let mut test_cases = Vec::new();
        for element in &self.elements {
            match element {
                TestRunElement::TestSuite(ts) => {
                    ts.collect_test_cases(&mut test_cases);
                }
            }
        }
        test_cases
    }
}

impl FromStr for TestRun {
    type Err = DeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        quick_xml::de::from_str::<TestRun>(s)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
enum TestRunElement {
    #[serde(rename = "test-suite")]
    TestSuite(TestSuite),
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

#[derive(Debug, Deserialize, PartialEq)]
enum TestSuiteElement {
    #[serde(rename = "test-suite")]
    TestSuite(Box<TestSuite>),

    #[serde(rename = "properties")]
    Properties(Properties),

    #[serde(rename = "test-case")]
    TestCase(Box<TestCase>),

    #[serde(rename = "output")]
    Output(TextElement),

    #[serde(rename = "failure")]
    Failure(Failure),

    #[serde(rename = "$text")]
    Text(String),
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct Properties {
    #[serde(rename = "property")]
    property: Option<Vec<Property>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct Property {
    #[serde(rename = "@name")]
    name: String,

    #[serde(rename = "@value")]
    value: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct TextElement {
    #[serde(rename = "$value")]
    text: String,
}

impl Display for TextElement {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
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
    fn get_text(&self) -> Option<String> {
        self.elements.iter().find_map(|e| match e {
            TestCaseElement::Text(s) => Some(s.clone()),
            _ => None,
        })
    }
    fn get_failure(&self) -> Option<&Failure> {
        self.elements.iter().find_map(|e| match e {
            TestCaseElement::Failure(f) => Some(f),
            _ => None,
        })
    }
}

impl TestSuite {
    fn collect_test_cases(&self, test_cases: &mut Vec<nunit::TestCase>) {
        for element in &self.elements {
            match element {
                TestSuiteElement::TestCase(tc) => {
                    let tc = tc.as_ref().into();
                    test_cases.push(tc);
                }
                TestSuiteElement::TestSuite(ts) => {
                    ts.collect_test_cases(test_cases);
                }
                _ => continue,
            }
        }
    }
}

impl From<&TestCase> for nunit::TestCase {
    fn from(value: &TestCase) -> Self {
        let failure = value.get_failure();
        nunit::TestCase {
            id: value.id,
            name: value.name.clone(),
            full_name: value.full_name.clone(),
            run_state: value.run_state.clone(),
            result: value.result.as_str().into(),
            duration: value.duration,
            start_time: value.start_time,
            end_time: value.end_time,
            text: value.get_text().unwrap_or_default(),
            failure_message: failure.and_then(Failure::get_message).unwrap_or_default(),
            failure_stack_trace: failure
                .and_then(Failure::get_stack_trace)
                .unwrap_or_default(),
            failure_text: failure.and_then(Failure::get_text).unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
enum TestCaseElement {
    #[serde(rename = "properties")]
    Properties(Properties),

    #[serde(rename = "output")]
    Output(TextElement),

    #[serde(rename = "failure")]
    Failure(Failure),

    #[serde(rename = "$text")]
    Text(String),
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct Failure {
    #[serde(rename = "$value")]
    elements: Vec<FailureElement>,
}

impl Failure {
    fn get_message(&self) -> Option<String> {
        self.elements.iter().find_map(|e| match e {
            FailureElement::Message(m) => Some(m.text.clone()),
            _ => None,
        })
    }
    fn get_stack_trace(&self) -> Option<String> {
        self.elements.iter().find_map(|e| match e {
            FailureElement::StackTrace(st) => Some(st.text.clone()),
            _ => None,
        })
    }

    fn get_text(&self) -> Option<String> {
        self.elements.iter().find_map(|e| match e {
            FailureElement::Text(s) => Some(s.clone()),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
enum FailureElement {
    #[serde(rename = "message")]
    Message(TextElement),

    #[serde(rename = "stack-trace")]
    StackTrace(TextElement),

    #[serde(rename = "$text")]
    Text(String),
}
