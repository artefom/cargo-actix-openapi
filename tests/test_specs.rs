use std::{
    fs::{read_to_string, File},
    io::Write,
};

use cargo_actix_openapi;

use anyhow::Result;

use pretty_assertions::assert_eq;
use rstest::rstest;

static OVERWRITE: bool = true;

pub fn save_expected(filename: &str, data: &str) -> Result<(), std::io::Error> {
    let mut file = File::create(filename)?;
    file.write_all(data.as_bytes())?;

    Ok(())
}

fn compare(got: &String, expected_path: &String) {
    let expected = read_to_string(expected_path).ok();

    let expected = match expected {
        Some(ref value) => value,
        None => {
            if OVERWRITE == true {
                save_expected(expected_path, &got).expect("Could not save expected");
                got
            } else {
                panic!("Could not get expected result")
            }
        }
    };

    if expected != got && OVERWRITE {
        save_expected(expected_path, &got).expect("Could not save expected");
    }

    assert_eq!(expected, got);
}

#[rstest]
#[case("helloworld")]
#[case("request_body")]
#[case("request_body_nested")]
#[case("default_parameter")]
#[case("error")]
#[case("enum")]
#[case("reference")]
#[case("ratelimit")]
fn test_specs(#[case] case_name: &str) -> Result<()> {
    let filename = format!("tests/openapi/{case_name}.yaml");
    let expected_filename = format!("tests/expected/{case_name}.rs");
    let expected_model = format!("tests/expected/{case_name}.yaml");

    let spec = read_to_string(filename)?;

    let (got_model, got) = cargo_actix_openapi::generate_api(&spec)?;

    compare(&got, &expected_filename);
    compare(&got_model, &expected_model);

    Ok(())
}
