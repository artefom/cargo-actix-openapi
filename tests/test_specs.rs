mod common;

use cargo_actix_openapi;

use anyhow::Result;
use common::save_expected;

use pretty_assertions::assert_eq;
use rstest::rstest;

static OVERWRITE: bool = true;

fn compare(name: &str, got: &String, expected: Option<&String>) {
    let Some(expected) = expected else {
        assert_eq!(expected, Some(got));
        return
    };

    if expected != got && OVERWRITE {
        save_expected(name, &got).expect("Could not save expected");
    }

    assert_eq!(expected, got);
}

#[rstest]
#[case("helloworld")]
fn test_specs(#[case] case_name: &str) -> Result<()> {
    let data = common::load_openapi_case(case_name).expect("Could not read test case");

    let spec = data.spec;
    let expected = data.expected;

    let got = cargo_actix_openapi::generate_api(&spec)?;

    compare(case_name, &got, expected.as_ref());

    Ok(())
}
