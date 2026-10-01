//! Storage-only qualification companion for four unchanged genuine author APIs.
//! No timer, child process, filesystem publication, Rust compilation or GPU use.
//! Keep the original latency probe unchanged. A complete record measures only
//! the named pre-output retained-owner checkpoint, never peak heap or RSS.

use fe2o3_kernel_ir::{
    LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as StorageError,
    LogicalStorageLimitsV1 as Limits, MAX_SIMULATION_BUNDLE_BYTES_V6, VerifiedSimulationBundleV6,
};
use fe2o3_source_isa_observation::multilevel_authoring_v1::{
    AuthoringOperationPageV1, AuthoringRegionSelectorV1, AuthoringRegionV1,
    AuthoringRustCandidateV1, AuthoringSnapshotSummaryV1, AuthoringSnapshotV1,
    MAX_AUTHORING_SCAN_ITEMS_V1,
};
use serde::Serialize;
use std::io::{self, Read, Write};
use std::mem::size_of;
use std::process::ExitCode;

// Retain the shipping CLI's exact query payload types/layout even though this
// companion admits only four actions. These adapters are never invoked here.
#[path = "../src/bin/fe2o3_author/candidate.rs"]
mod candidate;
#[path = "../src/bin/fe2o3_author/preview.rs"]
mod preview;

const MAX_SELECTOR_BYTES: usize = 16 * 1024;
// An observation work limit only, not a constructor/semantic scan limit.
// Reuse the existing numeric ceiling; a refused walk emits no partial total.
const MEASUREMENT_MAX_ITEMS: usize = MAX_AUTHORING_SCAN_ITEMS_V1;
const RETAINED_TARGET_BYTES: usize = 64 * 1024 * 1024;
const PREFIX: &[u8] = b"FE2O3_AUTHOR_RETAINED_STORAGE_V1 ";
const USAGE: &str = "storage-only qualification requires one of: inspect; operations --bundle-identity HEX --start N --limit N; select --selector JSON; materialize --selector JSON --helper NAME";
const SCOPE: &str = "pre_output_snapshot_argv_query_result_actual_owners";
const MODEL: &str = "actual_capacity_box_payload_logical_btree_payload_v1";

#[allow(dead_code)]
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

impl Query {
    fn name(&self) -> &'static str {
        match self {
            Self::Inspect => "inspect",
            Self::Operations { .. } => "operations",
            Self::Select(_) => "select",
            Self::Materialize(..) => "materialize",
            Self::CallTarget(_) => "call-target",
            Self::MaterializeConstU32(..) => "materialize-const-u32",
            Self::PreviewHelperInsertion { .. } => "preview-helper-insertion",
            Self::CreateSourceCandidate(_) => "create-source-candidate",
        }
    }
    fn charge(&self, c: &mut Counter) -> Result<(), StorageError> {
        c.charge(size_of::<Self>(), 1)?;
        match self {
            Self::Inspect => Ok(()),
            Self::Operations {
                identity,
                start: _,
                limit: _,
            } => c.string(identity),
            Self::Select(selector) | Self::CallTarget(selector) => {
                selector.charge_retained_heap_v1(c)
            }
            Self::Materialize(selector, helper) | Self::MaterializeConstU32(selector, helper) => {
                selector.charge_retained_heap_v1(c)?;
                c.string(helper)
            }
            Self::PreviewHelperInsertion {
                selector,
                helper,
                source,
                expected_sha256,
            } => {
                selector.charge_retained_heap_v1(c)?;
                c.string(helper)?;
                c.string(source)?;
                c.string(expected_sha256)
            }
            Self::CreateSourceCandidate(candidate::Options {
                selector,
                helper,
                source,
                expected_source_sha256,
                expected_proposal_sha256,
                candidate,
            }) => {
                selector.charge_retained_heap_v1(c)?;
                for text in [
                    helper,
                    source,
                    expected_source_sha256,
                    expected_proposal_sha256,
                    candidate,
                ] {
                    c.string(text)?;
                }
                Ok(())
            }
        }
    }
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
    // Exactly the four shipping CLI forms, with no measurement-only action args.
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
        ["materialize", "--selector", value, "--helper", helper] => {
            Ok(Query::Materialize(selector(value)?, (*helper).into()))
        }
        _ => Err(USAGE.into()),
    }
}

fn charge_arguments(arguments: &Vec<String>, c: &mut Counter) -> Result<(), StorageError> {
    c.charge(size_of::<Vec<String>>(), 1)?;
    c.vector(arguments)?;
    for argument in arguments {
        c.string(argument)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
struct Components {
    snapshot: usize,
    argv: usize,
    query: usize,
    result: usize,
    total: usize,
    visited_items: usize,
}
fn part(
    c: &mut Counter,
    walk: impl FnOnce(&mut Counter) -> Result<(), StorageError>,
) -> Result<usize, StorageError> {
    let before = c.bytes();
    walk(c)?;
    c.bytes()
        .checked_sub(before)
        .ok_or(StorageError::Arithmetic)
}
fn measure<T>(
    snapshot: &AuthoringSnapshotV1,
    arguments: &Vec<String>,
    query: &Query,
    result: &T,
    result_heap: fn(&T, &mut Counter) -> Result<(), StorageError>,
) -> Result<Components, StorageError> {
    let limits = Limits {
        max_bytes: None,
        max_items: MEASUREMENT_MAX_ITEMS,
    };
    let snapshot = snapshot.retained_logical_storage_v1(limits)?;
    let mut c = Counter::new(limits);
    // Carry forward the already performed walk's byte and work totals. Do not
    // walk/redecode/copy the snapshot again or duplicate its embedded headers.
    c.charge(snapshot.total_bytes, snapshot.visited_items)?;
    let argv = part(&mut c, |c| charge_arguments(arguments, c))?;
    let query = part(&mut c, |c| query.charge(c))?;
    let result = part(&mut c, |c| {
        c.charge(size_of::<T>(), 1)?;
        result_heap(result, c)
    })?;
    Ok(Components {
        snapshot: snapshot.total_bytes,
        argv,
        query,
        result,
        total: c.bytes(),
        visited_items: c.items(),
    })
}

fn output(value: &impl Serialize) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer(&mut stdout, value).map_err(|error| error.to_string())?;
    stdout.write_all(b"\n").map_err(|error| error.to_string())
}
#[derive(Serialize)]
struct Record {
    schema: &'static str,
    action: &'static str,
    phase: &'static str,
    model: &'static str,
    pointer_width_bits: u32,
    complete: bool,
    storage_error: Option<&'static str>,
    components: Option<Components>,
    target_bytes: usize,
    within_target: Option<bool>,
    observation_item_limit: usize,
    normal_stdout_succeeded: bool,
    latency_measured: bool,
    peak_heap_measured: bool,
    rss_measured: bool,
    grants_authority: bool,
}
fn storage_error(error: StorageError) -> &'static str {
    match error {
        StorageError::Arithmetic => "arithmetic",
        StorageError::ByteLimit => "byte_limit",
        StorageError::ItemLimit => "item_limit",
        StorageError::UnsupportedV11Owner => "unsupported_v11_owner",
    }
}
fn finish<T: Serialize>(
    snapshot: &AuthoringSnapshotV1,
    arguments: &Vec<String>,
    query: &Query,
    result: T,
    result_heap: fn(&T, &mut Counter) -> Result<(), StorageError>,
) -> Result<(), String> {
    // All four modeled roots coexist here. The query is borrowed by dispatch,
    // and argv remains in main. Original bundle bytes have moved into snapshot.
    let measured = measure(snapshot, arguments, query, &result, result_heap);
    // Shipping serialization and exact final newline remain unchanged.
    let normal_output = output(&result);
    let (components, error) = match measured {
        Ok(value) => (Some(value), None),
        Err(error) => (None, Some(storage_error(error))),
    };
    // Telemetry is created only after the retained-owner checkpoint; it is not
    // application payload and not silently included in/used as a peak claim.
    let record = Record {
        schema: "fe2o3-author-retained-storage-v1",
        action: query.name(),
        phase: SCOPE,
        model: MODEL,
        pointer_width_bits: usize::BITS,
        complete: components.is_some(),
        storage_error: error,
        components,
        target_bytes: RETAINED_TARGET_BYTES,
        within_target: components.map(|c| c.total <= RETAINED_TARGET_BYTES),
        observation_item_limit: MEASUREMENT_MAX_ITEMS,
        normal_stdout_succeeded: normal_output.is_ok(),
        latency_measured: false,
        peak_heap_measured: false,
        rss_measured: false,
        grants_authority: false,
    };
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    stderr
        .write_all(PREFIX)
        .map_err(|error| error.to_string())?;
    serde_json::to_writer(&mut stderr, &record).map_err(|error| error.to_string())?;
    stderr.write_all(b"\n").map_err(|error| error.to_string())?;
    normal_output?;
    if let Some(error) = error {
        return Err(format!("storage observation refused: {error}"));
    }
    Ok(())
}

fn run(query: &Query, arguments: &Vec<String>) -> Result<(), String> {
    if !matches!(
        query,
        Query::Inspect | Query::Operations { .. } | Query::Select(_) | Query::Materialize(..)
    ) {
        return Err(USAGE.into());
    }
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
        Query::Inspect => finish(
            &snapshot,
            arguments,
            query,
            snapshot.summary(),
            AuthoringSnapshotSummaryV1::charge_retained_heap_v1,
        ),
        Query::Operations {
            identity,
            start,
            limit,
        } => finish(
            &snapshot,
            arguments,
            query,
            snapshot
                .operation_page(identity, *start, *limit)
                .map_err(|error| error.to_string())?,
            AuthoringOperationPageV1::charge_retained_heap_v1,
        ),
        Query::Select(selector) => finish(
            &snapshot,
            arguments,
            query,
            snapshot
                .select_region(selector)
                .map_err(|error| error.to_string())?,
            AuthoringRegionV1::charge_retained_heap_v1,
        ),
        Query::Materialize(selector, helper) => finish(
            &snapshot,
            arguments,
            query,
            snapshot
                .materialize_typed_rust(selector, helper)
                .map_err(|error| error.to_string())?,
            AuthoringRustCandidateV1::charge_retained_heap_v1,
        ),
        Query::CallTarget(_)
        | Query::MaterializeConstU32(..)
        | Query::PreviewHelperInsertion { .. }
        | Query::CreateSourceCandidate(_) => Err(USAGE.into()),
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
    let outcome = parse(&arguments).and_then(|query| run(&query, &arguments));
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // Refusal before finish emits no checkpoint. The parent must require
            // one complete record AND exit success; absent is never a zero total.
            eprintln!("fe2o3-author: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_source_isa_observation::multilevel_authoring_v1::AuthoringOperationCoordinateV1;

    fn selected() -> AuthoringRegionSelectorV1 {
        AuthoringRegionSelectorV1 {
            bundle_identity: "11".repeat(32),
            canonical_kir_digest: "22".repeat(32),
            target: "gfx942:xnack-".into(),
            operations: vec![AuthoringOperationCoordinateV1 {
                function: 1,
                block: 0,
                operation: 0,
            }],
        }
    }
    fn counter() -> Counter {
        Counter::new(Limits {
            max_bytes: None,
            max_items: 100,
        })
    }

    #[test]
    fn only_four_primary_shipping_forms_are_admitted() {
        let encoded = serde_json::to_string(&selected()).unwrap();
        for values in [
            vec!["inspect"],
            vec![
                "operations",
                "--bundle-identity",
                "11",
                "--start",
                "0",
                "--limit",
                "1",
            ],
            vec!["select", "--selector", &encoded],
            vec![
                "materialize",
                "--selector",
                &encoded,
                "--helper",
                "candidate",
            ],
        ] {
            assert!(parse(&values.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_ok());
        }
        for values in [
            vec![],
            vec!["--help"],
            vec!["call-target"],
            vec!["materialize-const-u32"],
            vec!["preview-helper-insertion"],
            vec!["create-source-candidate"],
            vec!["inspect", "--resume"],
            vec!["select", "--selector", "{}"],
            vec![
                "operations",
                "--bundle-identity",
                "11",
                "--start",
                "01",
                "--limit",
                "1",
            ],
        ] {
            assert!(parse(&values.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        assert!(selector(&" ".repeat(MAX_SELECTOR_BYTES + 1)).is_err());
    }
    #[test]
    fn argv_and_query_count_independent_owned_copies_and_spare_capacity() {
        let mut argument = String::with_capacity(127);
        argument.push_str("materialize");
        let mut arguments = Vec::with_capacity(7);
        arguments.push(argument);
        let mut c = counter();
        charge_arguments(&arguments, &mut c).unwrap();
        assert_eq!(
            c.bytes(),
            size_of::<Vec<String>>()
                + arguments.capacity() * size_of::<String>()
                + arguments[0].capacity()
        );
        let selector = selected();
        let helper = String::with_capacity(83);
        let expected = size_of::<Query>()
            + selector.bundle_identity.capacity()
            + selector.canonical_kir_digest.capacity()
            + selector.target.capacity()
            + selector.operations.capacity() * size_of::<AuthoringOperationCoordinateV1>()
            + helper.capacity();
        let query = Query::Materialize(selector, helper);
        let mut c = counter();
        query.charge(&mut c).unwrap();
        assert_eq!(c.bytes(), expected);
        let mut tiny = Counter::new(Limits {
            max_bytes: Some(expected - 1),
            max_items: 100,
        });
        assert_eq!(query.charge(&mut tiny), Err(StorageError::ByteLimit));
    }
    #[test]
    fn component_addition_is_checked_and_refusal_is_not_a_total() {
        let mut c = Counter::new(Limits {
            max_bytes: None,
            max_items: usize::MAX,
        });
        c.charge(usize::MAX, 0).unwrap();
        assert_eq!(
            part(&mut c, |c| c.charge(1, 0)),
            Err(StorageError::Arithmetic)
        );
        assert_eq!(storage_error(StorageError::ItemLimit), "item_limit");
    }
    #[cfg(unix)]
    #[test]
    fn native_non_utf8_arguments_refuse_without_panicking() {
        use std::os::unix::ffi::OsStringExt;
        assert!(utf8_arguments([std::ffi::OsString::from_vec(vec![0xff])]).is_err());
    }
}
