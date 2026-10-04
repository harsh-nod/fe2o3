//! Bounded authoring queries and explicit text candidates; never executable authority.

use std::io::{self, Read, Write};
use std::process::ExitCode;

use fe2o3_kernel_ir::{MAX_SIMULATION_BUNDLE_BYTES_V6, VerifiedSimulationBundleV6};
use fe2o3_source_isa_observation::multilevel_authoring_v1::{
    AuthoringRegionSelectorV1, AuthoringSnapshotV1,
};
use serde::Serialize;

#[path = "fe2o3_author/preview.rs"]
mod preview;

#[path = "fe2o3_author/candidate.rs"]
mod candidate;

#[cfg(test)]
#[path = "fe2o3_author/call_target_tests.rs"]
mod call_target_tests;

const USAGE: &str = "usage: fe2o3-author inspect\n       fe2o3-author operations --bundle-identity HEX --start N --limit N\n       fe2o3-author select --selector JSON\n       fe2o3-author call-target --selector JSON\n       fe2o3-author materialize --selector JSON --helper NAME\n       fe2o3-author materialize-const-u32 --selector JSON --helper NAME\n       fe2o3-author preview-helper-insertion --selector JSON --helper NAME --source PATH --expected-source-sha256 HEX\n       fe2o3-author create-source-candidate --selector JSON --helper NAME --source PATH --expected-source-sha256 HEX --expected-proposal-sha256 HEX --candidate PATH\nRead exact canonical simulation Bundle V6 bytes from stdin. Output is diagnostic JSON, never a production resume token. Helper-insertion preview reads only the explicit relative Rust source path and never writes, compiles, or runs source. Explicit candidate creation writes only a new named file on supported Linux filesystems, never replaces the original, and never compiles or runs source.";
const MAX_SELECTOR_BYTES: usize = 16 * 1024;

enum Query {
    Inspect,
    Operations {
        identity: String,
        start: u32,
        limit: u32,
    },
    Select(AuthoringRegionSelectorV1),
    CallTarget(AuthoringRegionSelectorV1),
    Materialize(AuthoringRegionSelectorV1, String),
    MaterializeConstU32(AuthoringRegionSelectorV1, String),
    PreviewHelperInsertion {
        selector: AuthoringRegionSelectorV1,
        helper: String,
        source: String,
        expected_sha256: String,
    },
    CreateSourceCandidate(candidate::Options),
}

fn decimal(value: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| "expected a bounded decimal integer")?;
    if parsed.to_string() != value {
        return Err("expected canonical decimal integer".into());
    }
    Ok(parsed)
}

fn selector(value: &str) -> Result<AuthoringRegionSelectorV1, String> {
    if value.len() > MAX_SELECTOR_BYTES {
        return Err("selector exceeds 16 KiB".into());
    }
    serde_json::from_str(value).map_err(|error| format!("invalid selector: {error}"))
}

fn parse(arguments: &[String]) -> Result<Query, String> {
    match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["inspect"] => Ok(Query::Inspect),
        [
            "operations",
            "--bundle-identity",
            identity,
            "--start",
            start,
            "--limit",
            limit,
        ] => Ok(Query::Operations {
            identity: (*identity).into(),
            start: decimal(start)?,
            limit: decimal(limit)?,
        }),
        ["select", "--selector", value] => Ok(Query::Select(selector(value)?)),
        ["call-target", "--selector", value] => Ok(Query::CallTarget(selector(value)?)),
        ["materialize", "--selector", value, "--helper", helper] => {
            Ok(Query::Materialize(selector(value)?, (*helper).into()))
        }
        [
            "materialize-const-u32",
            "--selector",
            value,
            "--helper",
            helper,
        ] => Ok(Query::MaterializeConstU32(
            selector(value)?,
            (*helper).into(),
        )),
        [
            "preview-helper-insertion",
            "--selector",
            value,
            "--helper",
            helper,
            "--source",
            source,
            "--expected-source-sha256",
            expected_sha256,
        ] => {
            preview::validate_arguments(source, expected_sha256)?;
            Ok(Query::PreviewHelperInsertion {
                selector: selector(value)?,
                helper: (*helper).into(),
                source: (*source).into(),
                expected_sha256: (*expected_sha256).into(),
            })
        }
        [
            "create-source-candidate",
            "--selector",
            value,
            "--helper",
            helper,
            "--source",
            source,
            "--expected-source-sha256",
            expected_source_sha256,
            "--expected-proposal-sha256",
            expected_proposal_sha256,
            "--candidate",
            destination,
        ] => {
            let options = candidate::Options {
                selector: selector(value)?,
                helper: (*helper).into(),
                source: (*source).into(),
                expected_source_sha256: (*expected_source_sha256).into(),
                expected_proposal_sha256: (*expected_proposal_sha256).into(),
                candidate: (*destination).into(),
            };
            options.validate()?;
            Ok(Query::CreateSourceCandidate(options))
        }
        _ => Err(USAGE.into()),
    }
}

fn output(value: &impl Serialize) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer(&mut stdout, value).map_err(|error| error.to_string())?;
    stdout.write_all(b"\n").map_err(|error| error.to_string())
}

fn run(query: Query) -> Result<(), String> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take(MAX_SIMULATION_BUNDLE_BYTES_V6 as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read bundle: {error}"))?;
    if bytes.len() > MAX_SIMULATION_BUNDLE_BYTES_V6 {
        return Err("bundle exceeds the canonical V6 byte limit".into());
    }
    let bundle = VerifiedSimulationBundleV6::from_canonical_bytes(bytes)
        .map_err(|error| format!("bundle rejected: {error}"))?;
    let snapshot =
        AuthoringSnapshotV1::from_bundle_v6(bundle).map_err(|error| error.to_string())?;
    match query {
        Query::Inspect => output(&snapshot.summary()),
        Query::Operations {
            identity,
            start,
            limit,
        } => output(
            &snapshot
                .operation_page(&identity, start, limit)
                .map_err(|error| error.to_string())?,
        ),
        Query::Select(selector) => output(
            &snapshot
                .select_region(&selector)
                .map_err(|error| error.to_string())?,
        ),
        Query::CallTarget(selector) => output(
            &snapshot
                .inspect_call_target_v1(&selector)
                .map_err(|error| error.to_string())?,
        ),
        Query::Materialize(selector, helper) => output(
            &snapshot
                .materialize_typed_rust(&selector, &helper)
                .map_err(|error| error.to_string())?,
        ),
        Query::MaterializeConstU32(selector, helper) => output(
            &snapshot
                .materialize_const_u32_helper_v1(&selector, &helper)
                .map_err(|error| error.to_string())?,
        ),
        Query::PreviewHelperInsertion {
            selector,
            helper,
            source,
            expected_sha256,
        } => output(&preview::prepare(
            &snapshot,
            &selector,
            &helper,
            &source,
            &expected_sha256,
        )?),
        Query::CreateSourceCandidate(options) => {
            let receipt = candidate::create(&snapshot, &options)?;
            output(&receipt).map_err(|error| {
                format!(
                    "candidate created and retained at {:?}; receipt output failed; no rollback: {error}",
                    options.candidate
                )
            })
        }
    }
}

fn utf8_arguments(
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<Vec<String>, String> {
    arguments
        .into_iter()
        .map(|argument| {
            argument
                .into_string()
                .map_err(|_| "arguments must be valid UTF-8".to_owned())
        })
        .collect()
}

fn main() -> ExitCode {
    let arguments = match utf8_arguments(std::env::args_os().skip(1)) {
        Ok(arguments) => arguments,
        Err(error) => {
            eprintln!("fe2o3-author: {error}");
            return ExitCode::FAILURE;
        }
    };
    if arguments == ["--help"] {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match parse(&arguments).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("fe2o3-author: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn invalid_native_arguments_are_rejected_without_a_panic() {
        use std::os::unix::ffi::OsStringExt;
        assert!(utf8_arguments([std::ffi::OsString::from_vec(vec![0xff])]).is_err());
    }

    #[test]
    fn arguments_are_closed_and_bounded() {
        for arguments in [
            vec![],
            vec!["inspect", "--resume"],
            vec!["select", "--selector", "{}"],
            vec![
                "operations",
                "--bundle-identity",
                "abc",
                "--start",
                "01",
                "--limit",
                "1",
            ],
        ] {
            assert!(parse(&arguments.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        assert!(parse(&["inspect".into()]).is_ok());
        assert!(selector(&" ".repeat(MAX_SELECTOR_BYTES + 1)).is_err());
    }

    #[test]
    fn const_u32_materialization_has_an_additive_closed_command() {
        let selected = AuthoringRegionSelectorV1 {
            bundle_identity: "11".repeat(32),
            canonical_kir_digest: "22".repeat(32),
            target: "gfx942:xnack-".into(),
            operations: vec![fe2o3_source_isa_observation::multilevel_authoring_v1::AuthoringOperationCoordinateV1 {
                function: 0, block: 0, operation: 1,
            }],
        };
        let encoded = serde_json::to_string(&selected).unwrap();
        let arguments = vec![
            "materialize-const-u32".into(),
            "--selector".into(),
            encoded.clone(),
            "--helper".into(),
            "compute".into(),
        ];
        let Query::MaterializeConstU32(parsed, helper) = parse(&arguments).unwrap() else {
            panic!("exact new command required");
        };
        assert_eq!(parsed, selected);
        assert_eq!(helper, "compute");
        let mut legacy = arguments.clone();
        legacy[0] = "materialize".into();
        assert!(matches!(parse(&legacy), Ok(Query::Materialize(_, _))));
        let mut extra = arguments.clone();
        extra.extend(["--const-value".into(), "256".into()]);
        assert!(parse(&extra).is_err());
        extra = arguments.clone();
        extra.push("--resume".into());
        assert!(parse(&extra).is_err());
        extra = arguments.clone();
        extra.swap(1, 3);
        assert!(parse(&extra).is_err());
        let mut injected = serde_json::to_value(selected).unwrap();
        injected
            .as_object_mut()
            .unwrap()
            .insert("constant_value".into(), 256.into());
        extra = arguments;
        extra[2] = injected.to_string();
        assert!(parse(&extra).is_err());
    }
}
