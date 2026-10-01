//! Explicit source-only BF16 inspection/publication through the actual rustc callback.
//! No edited intermediate, native compiler, simulator, or GPU is resumed here.
use super::{
    Callbacks, Compilation, Compiler, Path, TyCtxt, lower_hex_v1, publish_new_inert_output,
    require_canonical_overflow_checks_v1, transaction_in_active_session_v1,
};
use crate::production_tiled_region_source_v1::{
    Bf16TileSourcePublishRequestV1, PublishedBf16TileSourceV1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};

#[path = "bf16_tile_source_driver_io_v1.rs"]
mod input;
#[cfg(test)]
#[path = "bf16_tile_source_driver_v1_tests.rs"]
mod tests;

const REPORT_CAP: usize = 16 * 1024;
const PATH_CAP: usize = 4096;

struct CallbacksV1<'a> {
    output: &'a Path,
    request: Option<&'a Bf16TileSourcePublishRequestV1<'a>>,
    request_sha256: Option<[u8; 32]>,
    calls: usize,
    result: Option<Result<(), String>>,
}
impl Callbacks for CallbacksV1<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.calls != 0 {
            self.result = Some(Err(
                "BF16 source action requires exactly one live callback; prior outputs may remain"
                    .into(),
            ));
            return Compilation::Stop;
        }
        self.calls = 1;
        self.result = Some(extract(tcx, self.output, self.request, self.request_sha256));
        Compilation::Stop
    }
}

/// Inspect only the actual original direct BF16 source profile. The new output
/// directory contains inert selection facts, not a saved compiler owner.
/// No source candidate is created, compiled, simulated, or executed.
pub fn run_bf16_tile_source_inspection_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    require_canonical_overflow_checks_v1(args)?;
    validate_output(output)?;
    run(
        args,
        CallbacksV1 {
            output,
            request: None,
            request_sha256: None,
            calls: 0,
            result: None,
        },
    )
}

/// Explicit create-new source publication using the existing closed request.
/// The original source remains unchanged; the candidate requires fresh source
/// compilation and receives no normal artifact/proof/launch authority.
/// On any failure, retain and inspect candidate/output paths before retrying.
/// A propagated panic/abort may prevent a final report; it does not prove that
/// no file was published. This API never rolls back a possible publication.
pub fn run_bf16_tile_source_promotion_driver_v1(
    args: &[String],
    output: &Path,
    request_path: &Path,
) -> Result<(), String> {
    require_canonical_overflow_checks_v1(args)?;
    validate_output(output)?;
    let bytes = input::read(request_path)?;
    let request =
        Bf16TileSourcePublishRequestV1::parse(bytes.bytes()).map_err(|error| error.to_string())?;
    let request_sha256 = Sha256::digest(bytes.bytes()).into();
    run(
        args,
        CallbacksV1 {
            output,
            request: Some(&request),
            request_sha256: Some(request_sha256),
            calls: 0,
            result: None,
        },
    )
}
fn validate_output(output: &Path) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = output.as_os_str().as_bytes();
    if bytes.is_empty() || bytes.len() > PATH_CAP || bytes.contains(&0) {
        return Err("BF16 output path must be nonempty and at most 4096 bytes without NUL".into());
    }
    Ok(())
}
fn run(args: &[String], mut callbacks: CallbacksV1<'_>) -> Result<(), String> {
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks)).err();
    if fatal.is_some() {
        return Err("BF16 rustc analysis failed; candidate/output may already exist; inspect retained paths before retrying".into());
    }
    if callbacks.calls != 1 {
        return Err("BF16 source action did not observe one actual rustc callback; inspect retained paths before retrying".into());
    }
    callbacks.result.unwrap_or_else(|| {
        Err("BF16 source callback produced no result; candidate/output may remain".into())
    })
}

fn facts(published: &PublishedBf16TileSourceV1) -> serde_json::Value {
    serde_json::json!({
        "original_sha256": lower_hex_v1(&published.original_sha256),
        "candidate_sha256": lower_hex_v1(&published.candidate_sha256),
        "original_bytes": published.original_bytes,
        "candidate_bytes": published.candidate_bytes,
        // Lossless identities, not JavaScript-number addresses.
        "candidate_device": published.candidate_device.to_string(),
        "candidate_inode": published.candidate_inode.to_string(),
        "return_order": published.return_order,
        "created_new": true,
        "original_overwritten": false,
        "fresh_compilation_required": true,
        "fresh_compilation_observed": false,
        "grants_compiler_or_launch_authority": false,
    })
}
fn base_report(mode: &'static str) -> serde_json::Value {
    serde_json::json!({
        "schema": "fe2o3-bf16-tile-source-action-v1",
        "mode": mode, "status": "failed",
        "actual_rustc_callback": true,
        "target": "gfx942:xnack-", "wave_width": 64,
        "canonical_digest_domain": "compiler_identity_digest_not_sha256_serialized_bytes",
        "source_postflight_ok": false,
        "selection": null, "requested_selection": null,
        "request_sha256": null,
        "publication": null, "publication_effect": "not_attempted",
        "publication_error": null,
        "selection_is_compiler_custody": false,
        "candidate_compiled": false, "simulation_performed": false,
        "normal_ranked_admission_performed": false,
        "native_execution": false, "hardware_observed": false,
        "grants_artifact_or_launch_authority": false,
        "memory_measurement": "unavailable",
        "candidate_requires_fresh_frontend": true,
    })
}
fn extract(
    tcx: TyCtxt<'_>,
    output: &Path,
    request: Option<&Bf16TileSourcePublishRequestV1<'_>>,
    request_sha256: Option<[u8; 32]>,
) -> Result<(), String> {
    let original = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?;
    // A reused output fails BEFORE the source action. This directory is an
    // explicit diagnostic side effect; it is never interpreted as authority.
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(output)
        .map_err(|error| format!("fresh BF16 source output directory: {error}"))?;
    let mut report = base_report(if request.is_some() {
        "promote"
    } else {
        "inspect"
    });
    let failure = if let Some(request) = request {
        report["request_sha256"] = request_sha256
            .map(|v| serde_json::Value::String(lower_hex_v1(&v)))
            .unwrap_or(serde_json::Value::Null);
        report["requested_selection"] = serde_json::json!({
            "semantic_sha256": lower_hex_v1(&request.semantic_sha256),
            "canonical_sha256": lower_hex_v1(&request.canonical_sha256),
            "mir_sha256": lower_hex_v1(&request.mir_sha256),
            "original_sha256": lower_hex_v1(&request.original_sha256),
        });
        // The unchanged production action rechecks the actual source, owner,
        // original P0 ledger/floor and filesystem publication eligibility.
        let outcome = original.publish_bf16_tile_source_candidate_v1(request);
        report["publication_effect"] =
            serde_json::to_value(outcome.progress.effect).map_err(|error| {
                format!("publication progress serialization: {error}; candidate may remain")
            })?;
        report["publication"] = outcome
            .progress
            .published
            .as_ref()
            .map(facts)
            .unwrap_or(serde_json::Value::Null);
        report["source_postflight_ok"] = outcome.source_postflight.is_ok().into();
        let publication_failure = match &outcome.publication {
            Some(Ok(published)) if outcome.progress.published.as_ref() == Some(published) => None,
            Some(Ok(_)) => Some("BF16 publication outcome/progress facts differ".to_owned()),
            Some(Err(error)) => Some(error.to_string()),
            None => Some("BF16 source callback did not complete publication".to_owned()),
        };
        report["publication_error"] = publication_failure
            .clone()
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);
        let postflight_failure = outcome
            .source_postflight
            .err()
            .map(|error| format!("BF16 original source postflight: {error}"));
        let failure = combine_failures(publication_failure, postflight_failure);
        if failure.is_none() {
            report["status"] = "candidate_created".into();
        }
        failure
    } else {
        match original.inspect_bf16_tile_source_selection_v1() {
            Ok(selection) => {
                report["selection"] = serde_json::json!({
                    "semantic_sha256": lower_hex_v1(&selection.semantic_sha256),
                    "canonical_sha256": lower_hex_v1(&selection.canonical_sha256),
                    "mir_sha256": lower_hex_v1(&selection.mir_sha256),
                    "original_sha256": lower_hex_v1(&selection.original_sha256),
                    "original_bytes": selection.original_bytes,
                    "publication_eligibility": "rechecked_by_explicit_source_action",
                });
                report["source_postflight_ok"] = true.into();
                report["status"] = "inspected".into();
                None
            }
            Err(error) => Some(format!("BF16 original source inspection: {error}")),
        }
    };
    finish_report(output, &report, failure)
}
use std::os::unix::fs::DirBuilderExt;

fn combine_failures(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}; {second}")),
        (Some(error), None) | (None, Some(error)) => Some(error),
        (None, None) => None,
    }
}
// Fixed output buffer bounds serialization while it happens; no unbounded
// serialized Vec is allocated and no partial buffer is published on overflow.
fn encode_report<'a>(
    report: &impl Serialize,
    bytes: &'a mut [u8; REPORT_CAP],
) -> Result<&'a [u8], String> {
    let mut out = Cursor::new(bytes.as_mut_slice());
    serde_json::to_writer(&mut out, report)
        .map_err(|error| format!("bounded BF16 source report: {error}"))?;
    out.write_all(b"\n")
        .map_err(|error| format!("bounded BF16 report newline: {error}"))?;
    let length = usize::try_from(out.position()).map_err(|_| "BF16 report length overflow")?;
    Ok(&bytes[..length])
}
fn finish_report(
    output: &Path,
    report: &impl Serialize,
    failure: Option<String>,
) -> Result<(), String> {
    finish_report_with_writer(report, failure, |bytes| {
        publish_new_inert_output(
            &output.join("observation.json"),
            bytes,
            REPORT_CAP,
            "BF16 source observation",
        )
    })
}
fn finish_report_with_writer(
    report: &impl Serialize,
    failure: Option<String>,
    write: impl FnOnce(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let mut bytes = [0u8; REPORT_CAP];
    let written = encode_report(report, &mut bytes).and_then(write);
    if let Err(error) = written {
        return Err(format!(
            "{}; report publication failed: {error}; candidate/output may remain; no rollback",
            failure.as_deref().unwrap_or("BF16 source action completed"),
        ));
    }
    if let Some(error) = failure {
        return Err(format!(
            "{error}; retained observation.json; candidate/output may remain; no rollback"
        ));
    }
    Ok(())
}
