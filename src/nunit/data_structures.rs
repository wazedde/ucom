use std::str::FromStr;

use chrono::{DateTime, Utc};
use quick_xml::DeError;
use serde::Deserialize;

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
    pub fn test_result(&self) -> TestResult {
        self.result.as_str().into()
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

#[derive(Debug, Deserialize, PartialEq)]
struct Properties {
    #[serde(rename = "property")]
    property: Option<Vec<Property>>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Property {
    #[serde(rename = "@name")]
    name: String,

    #[serde(rename = "@value")]
    value: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct TextElement {
    #[serde(rename = "$value")]
    text: String,
}

#[derive(Debug, Deserialize, PartialEq)]
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
    elements: Vec<TestCaseElements>,
}

#[derive(Debug, Deserialize, PartialEq)]
enum TestCaseElements {
    #[serde(rename = "properties")]
    Properties(Properties),

    #[serde(rename = "output")]
    Output(TextElement),

    #[serde(rename = "failure")]
    Failure(Failure),

    #[serde(rename = "$text")]
    Text(String),
}

#[derive(Debug, Deserialize, PartialEq)]
struct Failure {
    #[serde(rename = "$value")]
    elements: Vec<FailureElement>,
}

#[derive(Debug, Deserialize, PartialEq)]
enum FailureElement {
    #[serde(rename = "message")]
    Message(TextElement),

    #[serde(rename = "stack-trace")]
    StackTrace(TextElement),

    #[serde(rename = "$text")]
    Text(String),
}
