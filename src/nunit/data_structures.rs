use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "test-run")]
struct TestRun {
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

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum TestRunElement {
    TestSuite(TestSuite),
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "test-suite")]
struct TestSuite {
    #[serde(rename = "@type")]
    r#type: String,

    #[serde(rename = "@id")]
    id: i32,

    #[serde(rename = "@name")]
    name: String,

    #[serde(rename = "@fullname")]
    full_name: String,

    #[serde(rename = "@classname", default = "String::new")]
    class_name: String,

    #[serde(rename = "@testcasecount")]
    test_case_count: i32,

    #[serde(rename = "@runstate")]
    run_state: String,

    #[serde(rename = "@result")]
    result: String,

    #[serde(rename = "@label", default = "String::new")]
    label: String,

    #[serde(rename = "@site", default = "String::new")]
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
    Output(Output),

    #[serde(rename = "$text")]
    Text(String),
}

#[derive(Debug, Deserialize, PartialEq)]
struct Properties {
    #[serde(rename = "property")]
    property: Vec<Property>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "property")]
struct Property {
    #[serde(rename = "@name")]
    name: String,

    #[serde(rename = "@value")]
    value: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "output")]
struct Output {
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

    #[serde(rename = "@label", default = "String::new")]
    label: String,

    #[serde(rename = "@site", default = "String::new")]
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
    Output(Output),

    #[serde(rename = "$text")]
    Text(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_standalone() {
        let data = include_str!("test_data/standalone.xml");
        let test_run = quick_xml::de::from_str::<TestRun>(data).unwrap();
        dbg!(test_run.elements);
    }

    #[test]
    fn test_deserialize_editmode() {
        let data = include_str!("test_data/editmode.xml");
        let test_run = quick_xml::de::from_str::<TestRun>(data).unwrap();
        dbg!(test_run.elements);
    }
}
