mod common;

use cargo_actix_openapi;

use anyhow::Result;
use common::save_expected;

use pretty_assertions::assert_eq;
use rstest::rstest;

static OVERWRITE: bool = true;

fn compare(name: &str, got: &String, expected: Option<&String>) {
    let expected = match expected {
        Some(value) => value,
        None => {
            if OVERWRITE == true {
                save_expected(name, &got).expect("Could not save expected");
                got
            } else {
                panic!("Could not get expected result")
            }
        }
    };

    if expected != got && OVERWRITE {
        save_expected(name, &got).expect("Could not save expected");
    }

    assert_eq!(expected, got);
}

#[rstest]
#[case("helloworld")]
#[case("request_body")]
#[case("request_body_nested")]
#[case("default_parameter")]
#[case("error")]
fn test_specs(#[case] case_name: &str) -> Result<()> {
    let data = common::load_openapi_case(case_name).expect("Could not read test case");

    let spec = data.spec;
    let expected = data.expected;

    let got = cargo_actix_openapi::generate_api(&spec)?;

    compare(case_name, &got, expected.as_ref());

    Ok(())
}
