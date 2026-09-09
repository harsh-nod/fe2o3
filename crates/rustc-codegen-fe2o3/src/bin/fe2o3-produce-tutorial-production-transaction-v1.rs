#![feature(rustc_private)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use rustc_codegen_fe2o3::production_pipeline::prepare_tutorial_capability_qualification_transaction_v1;

const FINALIZE_INCOMPLETE: &str = "finalize phase is incomplete: authenticated hardware evidence cannot yet be joined to the prepared compiler transaction";

fn main() -> ExitCode {
    match parse_options(std::env::args_os().skip(1).collect()).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("fe2o3 tutorial production transaction: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Options {
    Prepare {
        cargo_fe2o3: PathBuf,
        request: PathBuf,
        output_directory: PathBuf,
    },
    Finalize,
}

fn run(options: Options) -> Result<(), String> {
    match options {
        Options::Prepare {
            cargo_fe2o3,
            request,
            output_directory,
        } => prepare_tutorial_capability_qualification_transaction_v1(
            &cargo_fe2o3,
            &request,
            &output_directory,
        )
        .map(|_| ())
        .map_err(|error| error.to_string()),
        Options::Finalize => Err(FINALIZE_INCOMPLETE.to_owned()),
    }
}

fn parse_options(arguments: Vec<OsString>) -> Result<Options, String> {
    let mut phase = None;
    let mut cargo_fe2o3 = None;
    let mut hardware_trust_policy = None;
    let mut request = None;
    let mut pre_hardware_directory = None;
    let mut hardware_archive = None;
    let mut output_directory = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let Some(argument) = argument.to_str() else {
            return Err("option names must be valid UTF-8".to_owned());
        };
        let slot = match argument {
            "--phase" => &mut phase,
            "--cargo-fe2o3" => &mut cargo_fe2o3,
            "--hardware-trust-policy" => &mut hardware_trust_policy,
            "--request" => &mut request,
            "--pre-hardware-directory" => &mut pre_hardware_directory,
            "--hardware-archive" => &mut hardware_archive,
            "--output-directory" => &mut output_directory,
            _ => return Err(format!("unknown option {argument:?}; {}", usage())),
        };
        let value = arguments
            .next()
            .ok_or_else(|| format!("{argument} requires a value"))?;
        if slot.replace(value).is_some() {
            return Err(format!("{argument} may be specified only once"));
        }
    }

    let phase = phase
        .ok_or_else(|| "missing --phase".to_owned())?
        .into_string()
        .map_err(|_| "--phase must be valid UTF-8".to_owned())?;
    match phase.as_str() {
        "prepare" => {
            if hardware_trust_policy.is_some()
                || pre_hardware_directory.is_some()
                || hardware_archive.is_some()
            {
                return Err("prepare phase received finalize-only options".to_owned());
            }
            Ok(Options::Prepare {
                cargo_fe2o3: required(cargo_fe2o3, "--cargo-fe2o3")?,
                request: required(request, "--request")?,
                output_directory: required(output_directory, "--output-directory")?,
            })
        }
        "finalize" => {
            required(cargo_fe2o3, "--cargo-fe2o3")?;
            required(hardware_trust_policy, "--hardware-trust-policy")?;
            required(request, "--request")?;
            required(pre_hardware_directory, "--pre-hardware-directory")?;
            required(hardware_archive, "--hardware-archive")?;
            required(output_directory, "--output-directory")?;
            Ok(Options::Finalize)
        }
        _ => Err(format!("unsupported --phase {phase:?}; {}", usage())),
    }
}

fn required(value: Option<OsString>, option: &str) -> Result<PathBuf, String> {
    value
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing {option}"))
}

fn usage() -> &'static str {
    "usage: fe2o3-produce-tutorial-production-transaction-v1 \
       --phase prepare --cargo-fe2o3 <absolute-path> \
       --request <request.json> --output-directory <new-directory>"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn owned_temp_root() -> PathBuf {
        loop {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "fe2o3-finalize-cli-test-{}-{sequence}",
                std::process::id()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => return root,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create owned test directory: {error}"),
            }
        }
    }

    #[test]
    fn parses_the_prepare_orchestrator_contract() {
        let options = parse_options(
            [
                "--phase",
                "prepare",
                "--cargo-fe2o3",
                "/opt/fe2o3/cargo-fe2o3",
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
        assert_eq!(
            options,
            Options::Prepare {
                cargo_fe2o3: PathBuf::from("/opt/fe2o3/cargo-fe2o3"),
                request: PathBuf::from("/request.json"),
                output_directory: PathBuf::from("/export"),
            }
        );
    }

    #[test]
    fn finalize_fails_closed_before_output() {
        let root = owned_temp_root();
        let output = root.join("finalized");
        let options = parse_options(
            [
                OsString::from("--phase"),
                OsString::from("finalize"),
                OsString::from("--cargo-fe2o3"),
                OsString::from("/opt/fe2o3/cargo-fe2o3"),
                OsString::from("--hardware-trust-policy"),
                OsString::from("/policy.json"),
                OsString::from("--request"),
                OsString::from("/request.json"),
                OsString::from("--pre-hardware-directory"),
                OsString::from("/prepared"),
                OsString::from("--hardware-archive"),
                OsString::from("/hardware.zip"),
                OsString::from("--output-directory"),
                output.as_os_str().to_owned(),
            ]
            .into_iter()
            .collect(),
        )
        .unwrap();

        assert_eq!(run(options), Err(FINALIZE_INCOMPLETE.to_owned()));
        assert!(!output.exists());
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn requires_explicit_phase_and_rejects_phase_specific_mismatches() {
        assert!(parse_options(vec![OsString::from("--unknown")]).is_err());
        assert!(
            parse_options(
                [
                    "--cargo-fe2o3",
                    "a",
                    "--request",
                    "b",
                    "--output-directory",
                    "c",
                ]
                .into_iter()
                .map(OsString::from)
                .collect(),
            )
            .is_err()
        );
        assert!(
            parse_options(
                [
                    "--phase",
                    "prepare",
                    "--request",
                    "a",
                    "--request",
                    "b",
                    "--output-directory",
                    "d",
                ]
                .into_iter()
                .map(OsString::from)
                .collect(),
            )
            .is_err()
        );
        assert!(
            parse_options(
                [
                    "--phase",
                    "prepare",
                    "--cargo-fe2o3",
                    "a",
                    "--hardware-trust-policy",
                    "b",
                    "--request",
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
