use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use rustc_codegen_fe2o3::production_pipeline::produce_tutorial_capability_qualification_transaction_v1;

fn main() -> ExitCode {
    match parse_options(std::env::args_os().skip(1).collect()) {
        Ok(options) => match produce_tutorial_capability_qualification_transaction_v1(
            &options.cargo_fe2o3,
            &options.hardware_trust_policy,
            &options.request,
            &options.output_directory,
        ) {
            Ok(_) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("fe2o3 tutorial production transaction: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("fe2o3 tutorial production transaction: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    cargo_fe2o3: PathBuf,
    hardware_trust_policy: PathBuf,
    request: PathBuf,
    output_directory: PathBuf,
}

fn parse_options(arguments: Vec<OsString>) -> Result<Options, String> {
    let mut cargo_fe2o3 = None;
    let mut hardware_trust_policy = None;
    let mut request = None;
    let mut output_directory = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let Some(argument) = argument.to_str() else {
            return Err("option names must be valid UTF-8".to_owned());
        };
        let slot = match argument {
            "--cargo-fe2o3" => &mut cargo_fe2o3,
            "--hardware-trust-policy" => &mut hardware_trust_policy,
            "--request" => &mut request,
            "--output-directory" => &mut output_directory,
            _ => return Err(format!("unknown option {argument:?}; {}", usage())),
        };
        let value = arguments
            .next()
            .ok_or_else(|| format!("{argument} requires a value"))?;
        if slot.replace(PathBuf::from(value)).is_some() {
            return Err(format!("{argument} may be specified only once"));
        }
    }
    Ok(Options {
        cargo_fe2o3: cargo_fe2o3.ok_or_else(|| "missing --cargo-fe2o3".to_owned())?,
        hardware_trust_policy: hardware_trust_policy
            .ok_or_else(|| "missing --hardware-trust-policy".to_owned())?,
        request: request.ok_or_else(|| "missing --request".to_owned())?,
        output_directory: output_directory
            .ok_or_else(|| "missing --output-directory".to_owned())?,
    })
}

fn usage() -> &'static str {
    "usage: fe2o3-produce-tutorial-production-transaction-v1 \\\n+       --cargo-fe2o3 <absolute-path> --hardware-trust-policy <policy.json> \\\n+       --request <request.json> --output-directory <new-directory>"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_orchestrator_contract() {
        let options = parse_options(
            [
                "--cargo-fe2o3",
                "/opt/fe2o3/cargo-fe2o3",
                "--hardware-trust-policy",
                "/policy.json",
                "--request",
                "/request.json",
                "--output-directory",
                "/export",
            ]
            .into_iter()
            .map(OsString::from)
            .collect(),
        )
        .unwrap();
        assert_eq!(options.cargo_fe2o3, PathBuf::from("/opt/fe2o3/cargo-fe2o3"));
        assert_eq!(options.hardware_trust_policy, PathBuf::from("/policy.json"));
        assert_eq!(options.request, PathBuf::from("/request.json"));
        assert_eq!(options.output_directory, PathBuf::from("/export"));
    }

    #[test]
    fn rejects_unknown_or_duplicate_options() {
        assert!(parse_options(vec![OsString::from("--unknown")]).is_err());
        assert!(
            parse_options(
                [
                    "--request",
                    "a",
                    "--request",
                    "b",
                    "--hardware-trust-policy",
                    "c",
                    "--output-directory",
                    "d",
                ]
                .into_iter()
                .map(OsString::from)
                .collect(),
            )
            .is_err()
        );
    }
}
