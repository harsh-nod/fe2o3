//! Inert, bounded diagnostics for an explicitly selected engineering handoff.

use std::fmt::{self, Write as _};
use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};

const MAX_KIR_BYTES: usize = 8 * 1024 * 1024;
const MAX_LLVM_BYTES: usize = 4 * 1024 * 1024;
const MAX_WRITES: usize = 262_144;
const MAX_METADATA_BYTES: usize = 4096;

pub(super) fn require_v2(is_v2: bool) -> Result<(), String> {
    if is_v2 {
        Ok(())
    } else {
        Err("engineering capture does not accept inert V3 handoffs".to_owned())
    }
}

pub(super) fn require_generic_v2_route(capture: bool, route: &str) -> Result<(), String> {
    if capture {
        Err(format!(
            "engineering capture requires the generic V2 lowering path; {route} capture is unsupported"
        ))
    } else {
        Ok(())
    }
}

struct BoundedText {
    text: String,
    max_bytes: usize,
    writes_left: usize,
    failed: bool,
}

impl fmt::Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.failed || self.writes_left == 0 || value.len() > self.max_bytes - self.text.len() {
            self.failed = true;
            return Err(fmt::Error);
        }
        self.writes_left -= 1;
        self.text.push_str(value);
        Ok(())
    }
}

fn bounded_debug(
    value: &impl fmt::Debug,
    max_bytes: usize,
    max_writes: usize,
) -> Result<String, String> {
    let mut text = String::new();
    text.try_reserve_exact(max_bytes)
        .map_err(|_| "capture allocation unavailable")?;
    let mut output = BoundedText {
        text,
        max_bytes,
        writes_left: max_writes,
        failed: false,
    };
    writeln!(output, "{value:#?}").map_err(|_| "KIR capture exceeds bounded rendering limits")?;
    if output.failed {
        return Err("KIR capture exceeded bounded rendering limits".to_owned());
    }
    Ok(output.text)
}

#[derive(Serialize)]
struct Identity {
    sha256: String,
    byte_len: usize,
}

fn identity(bytes: &[u8]) -> Identity {
    Identity {
        sha256: super::lower_hex_v1(&Sha256::digest(bytes)),
        byte_len: bytes.len(),
    }
}

#[derive(Serialize)]
struct Candidate<'a> {
    schema: &'static str,
    encoding: &'static str,
    target: &'a str,
    kir: Identity,
    llvm: Identity,
    authority: bool,
}

pub(super) fn prepare_kir(
    lowered: &crate::production_pipeline::TargetLoweredProductionCompilation,
) -> Result<String, String> {
    bounded_debug(lowered.module(), MAX_KIR_BYTES, MAX_WRITES)
}

pub(super) fn capture(kir: &str, llvm: &[u8], target: &str, handoff: &Path) -> Result<(), String> {
    capture_bytes(kir, llvm, target, handoff, |path, bytes, bound| {
        super::publish_new_inert_output(path, bytes, bound, "engineering diagnostic candidate")
    })
}

fn capture_bytes(
    kir: &str,
    llvm: &[u8],
    target: &str,
    handoff: &Path,
    mut publish: impl FnMut(&Path, &[u8], usize) -> Result<(), String>,
) -> Result<(), String> {
    if llvm.is_empty() || llvm.len() > MAX_LLVM_BYTES {
        return Err("LLVM capture exceeds its nonempty byte bound".to_owned());
    }
    if kir.is_empty() || kir.len() > MAX_KIR_BYTES {
        return Err("KIR capture exceeds its nonempty byte bound".to_owned());
    }
    let parent = handoff.parent().ok_or("capture handoff has no parent")?;
    if !handoff.is_absolute()
        || handoff.file_name() != Some(std::ffi::OsStr::new("compiler-handoff-v2"))
        || std::fs::canonicalize(parent).map_err(|_| "capture parent unavailable")? != parent
    {
        return Err("capture requires the canonical engineering scratch handoff".to_owned());
    }
    let candidate = serde_json::to_vec(&Candidate {
        schema: "EngineeringDiagnosticCandidateV1",
        encoding: "commit-bound-target-kir-debug-not-canonical-replay",
        target,
        kir: identity(kir.as_bytes()),
        llvm: identity(llvm),
        authority: false,
    })
    .map_err(|_| "capture metadata encoding failed")?;
    if candidate.len() > MAX_METADATA_BYTES {
        return Err("capture metadata exceeds its byte bound".to_owned());
    }
    publish(
        &parent.join("diagnostic-target-kir.txt"),
        kir.as_bytes(),
        MAX_KIR_BYTES,
    )?;
    publish(
        &parent.join("diagnostic-pre-worker.ll"),
        llvm,
        MAX_LLVM_BYTES,
    )?;
    // This only completes the candidate. The CLI must still verify the actual handoff.
    publish(
        &parent.join("diagnostic-candidate.json"),
        &candidate,
        MAX_METADATA_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engineering_capture_rendering_is_all_or_omitted() {
        assert_eq!(bounded_debug(&7, 2, 4).unwrap(), "7\n");
        assert!(bounded_debug(&7, 1, 4).is_err());
        assert!(bounded_debug(&7, 20, 0).is_err());
    }

    #[test]
    fn engineering_capture_formatter_failure_is_not_partial_success() {
        struct Fails;
        impl fmt::Debug for Fails {
            fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                out.write_str("partial")?;
                Err(fmt::Error)
            }
        }
        assert!(bounded_debug(&Fails, 100, 100).is_err());
        struct SwallowsFailure;
        impl fmt::Debug for SwallowsFailure {
            fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                let _ = out.write_str("too long");
                Ok(())
            }
        }
        assert!(bounded_debug(&SwallowsFailure, 1, 100).is_err());
    }

    #[test]
    fn engineering_capture_write_failure_never_publishes_candidate_marker() {
        let handoff = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join("compiler-handoff-v2");
        let mut names = Vec::new();
        let result = capture_bytes("7\n", b"llvm", "gfx950:xnack-", &handoff, |path, _, _| {
            names.push(path.file_name().unwrap().to_owned());
            if names.len() == 2 {
                Err("injected".to_owned())
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        assert_eq!(names.len(), 2);
        assert!(!names.iter().any(|name| name == "diagnostic-candidate.json"));
    }

    #[test]
    fn engineering_capture_candidate_marker_is_last_and_hashes_exact_bytes() {
        let handoff = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join("compiler-handoff-v2");
        let mut files = Vec::new();
        capture_bytes(
            "7\n",
            b"llvm",
            "gfx950:xnack-",
            &handoff,
            |path, bytes, _| {
                files.push((path.file_name().unwrap().to_owned(), bytes.to_vec()));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[2].0, "diagnostic-candidate.json");
        let metadata: serde_json::Value = serde_json::from_slice(&files[2].1).unwrap();
        assert_eq!(metadata["authority"], false);
        assert_eq!(metadata["kir"]["sha256"], identity(&files[0].1).sha256);
        assert_eq!(metadata["llvm"]["sha256"], identity(&files[1].1).sha256);
        assert_eq!(files[1].1, b"llvm");
    }

    #[test]
    fn engineering_capture_llvm_bound_precedes_path_or_writes() {
        assert!(
            capture_bytes("kir", &[], "gfx950:xnack-", Path::new("bad"), |_, _, _| {
                panic!("unexpected write")
            })
            .is_err()
        );
    }

    #[test]
    fn engineering_capture_default_and_v3_gate_remain_explicit() {
        assert!(!super::super::ProductionExtractionCallbacksV1::default().engineering_capture);
        assert!(require_v2(true).is_ok());
        assert!(require_v2(false).is_err());
    }

    #[test]
    fn engineering_capture_reads_final_handoff_payload_after_descriptor_binding() {
        let source = include_str!("amdgpu_outputs_v1.rs");
        let prepare = source
            .find("engineering_capture_v1::prepare_kir(&lowered)")
            .unwrap();
        let consume = source[prepare..]
            .find(".into_inert_worker_handoff_for_extraction()")
            .unwrap()
            + prepare;
        let payload = source[consume..].find("handoff.module_bytes()").unwrap() + consume;
        let publish = source[payload..]
            .find("std::fs::write(output, handoff.canonical_bytes())")
            .unwrap()
            + payload;
        assert!(prepare < consume && consume < payload && payload < publish);
    }
    #[test]
    fn engineering_capture_specialized_routes_fail_only_when_requested() {
        for route in [
            "ordered-composition",
            "MIR39/KIR22 physical-lds-exchange",
            "MIR38/KIR21 physical-global-copy",
            "MIR37/KIR20 physical-entry",
            "MIR36/KIR19 complete-body",
        ] {
            assert!(require_generic_v2_route(false, route).is_ok());
            let error = require_generic_v2_route(true, route).unwrap_err();
            assert!(error.contains("engineering capture requires the generic V2 lowering path"));
            assert!(error.contains(route));
            assert!(error.ends_with("capture is unsupported"));
        }
    }

    #[test]
    fn engineering_capture_refusals_precede_specialized_handoff_and_preserve_budget() {
        let source = include_str!("amdgpu_outputs_v1.rs");
        let (_, generic) = source
            .split_once("pub(super) fn extract_amdgpu_compiler_handoff_in_active_session_v1(")
            .unwrap();
        let (generic, _) = generic
            .split_once("fn extract_amdgpu_semantic_compiler_handoff_in_active_session_v3(")
            .unwrap();
        let compact: String = generic.split_whitespace().collect();
        for (condition, route, dispatch) in [
            (
                "ordered_composition_normal_v1::selected()?",
                "ordered-composition",
                "ordered_composition_normal_v1",
            ),
            (
                "transaction.has_authenticated_physical_lds_exchange_v22()",
                "MIR39/KIR22 physical-lds-exchange",
                "physical_lds_exchange_v22",
            ),
            (
                "transaction.has_authenticated_physical_global_copy_v21()",
                "MIR38/KIR21 physical-global-copy",
                "physical_global_copy_v21",
            ),
            (
                "transaction.has_authenticated_physical_entry_v20()",
                "MIR37/KIR20 physical-entry",
                "physical_entry_v20",
            ),
            (
                "transaction.has_authenticated_complete_body_v19()",
                "MIR36/KIR19 complete-body",
                "complete_body_v19",
            ),
        ] {
            let expected = format!(
                "if {condition} {{\n        engineering_capture_v1::require_generic_v2_route(capture, \"{route}\")?;\n        return {dispatch}::extract_handoff("
            );
            let expected: String = expected.split_whitespace().collect();
            assert!(compact.contains(&expected), "{route}");
        }
        assert_eq!(
            generic
                .matches("engineering_capture_v1::require_generic_v2_route(")
                .count(),
            5
        );
        let v3 = generic
            .find("engineering_capture_v1::require_v2(false)?")
            .unwrap();
        let collection = generic
            .find("transaction_with_census_in_active_session_v1(")
            .unwrap();
        let budget = generic
            .find(".with_budget(|budget| transaction.lower_production_target(budget))")
            .unwrap();
        let capture = generic
            .find("engineering_capture_v1::prepare_kir(&lowered)")
            .unwrap();
        assert!(v3 < collection && collection < budget && budget < capture);
        assert_eq!(
            generic
                .matches("engineering_capture_v1::prepare_kir(")
                .count(),
            1
        );
    }
}
