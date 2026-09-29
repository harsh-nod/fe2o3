//! Actual original Rust to checked adopted V18 output to parsed target LLVM IR.
#[path = "production_rustc_driver_bound_target_v19_tests.rs"]
mod bound_target_tests;
use super::*;
use crate::production_pipeline::source_owned_v29::target_result::ClosedScalarTargetLlvmErrorV29 as LlvmError;
use crate::production_pipeline::source_owned_v29::target_result::tests::{Mode, genuine_case};
use std::cell::Cell;
use std::io::Write;
use std::process::Stdio;

const LLVM_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::target_llvm_tests::source_owned_target_llvm_child";
const REFUSAL_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::target_llvm_tests::source_owned_dynamic_target_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum Outcome {
    Llvm {
        source: [u8; 32],
        original: [u8; 32],
        optimized: [u8; 32],
        optimized_identity: [u8; 32],
        optimized_bytes: usize,
        changed: bool,
        target: String,
        text: String,
        work: usize,
        peak: usize,
    },
    DynamicRefused,
}

struct LlvmCallbacks {
    refuse_dynamic: bool,
    result: Option<Result<Outcome, String>>,
}

fn paid_text(text: &str, budget: &mut Budget<'_>) -> Result<String, Error> {
    budget.charge_work(text.len())?;
    budget.reserve_storage(text.len())?;
    let mut copied = String::new();
    copied
        .try_reserve_exact(text.len())
        .map_err(|_| Error::Resource(ResourceError::Allocation))?;
    budget.reserve_storage(
        copied
            .capacity()
            .checked_sub(text.len())
            .ok_or(ResourceError::Accounting)?,
    )?;
    copied.push_str(text);
    Ok(copied)
}

impl Callbacks for LlvmCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            // Independent re-observation of the same actual rustc target, not
            // a profile inferred from source text, kernel names or CLI labels.
            let expected = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
                .and_then(|target| target.authenticate_import_session(tcx))
                .map_err(|error| format!("actual target: {error:?}"))?.profile();
            if self.refuse_dynamic {
                let calls = Cell::new(0);
                let error = transaction()?
                    .with_source_owned_scalar_target_llvm_v29::<(), _>(|_, _, _, _| {
                        calls.set(calls.get() + 1);
                        Ok(())
                    })
                    .unwrap_err();
                assert!(
                    matches!(
                        error,
                        Error::TargetLlvm(LlvmError::Unsupported("dynamic formal launch geometry"))
                    ),
                    "{error:?}"
                );
                assert_eq!(calls.get(), 0);
                return Ok(Outcome::DynamicRefused);
            }
            let result = transaction()?
                .with_source_owned_scalar_target_llvm_v29(|source, handoff, native, budget| {
                    let original = source.canonical(budget)?;
                    let output = handoff.output(budget)?;
                    assert_eq!(
                        source.source_semantic(budget)?.wire_version(),
                        SemanticMirWireVersionV1::V29
                    );
                    assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                    handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                    assert_eq!(native.target(budget)?, expected);
                    assert_eq!(original.module().kernels.len(), 2);
                    assert!(original.module().kernels.iter().all(|root| {
                        root.domain.extents().all(|extent| {
                            matches!(extent, fe2o3_kernel_ir::LaunchExtent::Static(_))
                        })
                    }));
                    let text = native.llvm_ir(budget)?;
                    assert!(!text.is_empty());
                    budget
                        .charge_work(text.len().checked_mul(2).ok_or(ResourceError::Arithmetic)?)?;
                    let cpu_claim = match expected {
                        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942 => {
                            "\"target-cpu\"=\"gfx942\""
                        }
                        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950 => {
                            "\"target-cpu\"=\"gfx950\""
                        }
                    };
                    assert!(text.contains(cpu_claim));
                    // The existing emitter deliberately withholds operation
                    // probes for multiple definitions. Bind its exact absence
                    // record to this output in the unmetered parent process.
                    assert_eq!(output.owner().module().functions.len(), 2);
                    assert!(output.owner().module().functions[0].body.is_some());
                    assert!(output.owner().module().functions[1].body.is_some());
                    assert_eq!(
                        output.owner().identity().canonical_length(),
                        u64::try_from(output.owner().canonical_bytes().len()).unwrap()
                    );
                    // This owned copy is only a test transport for the exact text
                    // to the parent process's pinned LLVM parser/verification gate.
                    let copied = paid_text(text, budget)?;
                    let target = paid_text(expected.device_target(), budget)?;
                    Ok(Outcome::Llvm {
                        source: *source.source_ssa(budget)?.source_semantic_sha256(),
                        original: Sha256::digest(original.canonical_bytes()).into(),
                        optimized: Sha256::digest(output.owner().canonical_bytes()).into(),
                        optimized_identity: *output.owner().identity().digest(),
                        optimized_bytes: output.owner().canonical_bytes().len(),
                        changed: output.owner().canonical_bytes() != output.input_audit_bytes(),
                        target,
                        text: copied,
                        work: budget.work(),
                        peak: budget.peak_storage(),
                    })
                })
                .map_err(|error| format!("source-owned target LLVM: {error:?}"))?;

            #[repr(align(256))]
            struct Owned<'a> {
                drops: &'a Cell<usize>,
                bytes: [u8; 16_384],
                panic: bool,
            }
            impl Owned<'_> {
                fn touch(&self) {
                    assert_eq!(self.bytes[0], 7);
                }
            }
            impl Drop for Owned<'_> {
                fn drop(&mut self) {
                    self.drops.set(self.drops.get() + 1);
                    if self.panic {
                        std::panic::panic_any(0x1763_u32);
                    }
                }
            }
            let drops = Cell::new(0);
            let owned = Owned {
                drops: &drops,
                bytes: [7; 16_384],
                panic: false,
            };
            let error = transaction()?
                .with_source_owned_scalar_target_llvm_v29::<(), _>(move |_, _, _, _| {
                    owned.touch();
                    Err(Error::Unsupported("selected actual LLVM callback refusal"))
                })
                .unwrap_err();
            assert!(matches!(
                error,
                Error::Unsupported("selected actual LLVM callback refusal")
            ));
            assert_eq!(drops.get(), 1);
            let owned = Owned {
                drops: &drops,
                bytes: [7; 16_384],
                panic: true,
            };
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                transaction()
                    .unwrap()
                    .with_source_owned_scalar_target_llvm_v29(move |_, _, _, _| {
                        owned.touch();
                        Ok(())
                    })
            }))
            .unwrap_err();
            assert_eq!(*panic.downcast::<u32>().unwrap(), 0x1763);
            assert_eq!(drops.get(), 2);

            let error = transaction()?
                .with_source_owned_scalar_target_llvm_v29::<(), _>(|_, _, native, _| {
                    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                    let foreign = Budget::new(&mut work, 20_000_000);
                    native.llvm_ir(&foreign)?;
                    panic!("foreign ledger reached target LLVM text")
                })
                .unwrap_err();
            assert!(
                matches!(
                    error,
                    Error::Source(SourceError::Resource(ResourceError::Accounting))
                ),
                "{error:?}"
            );

            for mode in [
                Mode::Success,
                Mode::ExactStorage,
                Mode::ShortStorage,
                Mode::WorkRefusal,
                Mode::ForeignEntry,
                Mode::RestoredFloor,
            ] {
                // Each mode receives a fresh real source/output handoff. The
                // helper cannot supply an owner, profile or completion claim.
                let control = transaction()?.with_source_owned_scalar_handoff_v29(
                    |source, handoff, budget| {
                        genuine_case(source, handoff, expected, budget, mode).map_err(Error::from)
                    },
                );
                if matches!(mode, Mode::Success | Mode::ExactStorage) {
                    control
                        .map_err(|error| format!("genuine native control {mode:?}: {error:?}"))?;
                } else {
                    let error = control.unwrap_err();
                    let mut cause: &(dyn std::error::Error + 'static) = &error;
                    let resource = loop {
                        if let Some(resource) = cause.downcast_ref::<ResourceError>() {
                            break resource;
                        }
                        cause = cause
                            .source()
                            .unwrap_or_else(|| panic!("missing typed {mode:?} refusal: {error:?}"));
                    };
                    assert!(
                        matches!(
                            (mode, resource),
                            (Mode::ShortStorage, ResourceError::Storage(_))
                                | (Mode::WorkRefusal, ResourceError::Work(_))
                                | (
                                    Mode::ForeignEntry | Mode::RestoredFloor,
                                    ResourceError::Accounting
                                )
                        ),
                        "wrong {mode:?} refusal: {error:?}"
                    );
                }
            }
            Ok(result)
        })());
        Compilation::Stop
    }
}

fn child(refuse_dynamic: bool) {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = LlvmCallbacks {
        refuse_dynamic,
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual target LLVM callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("target LLVM result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual target LLVM: {result:?}");
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn source_owned_target_llvm_child() {
    child(false);
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn source_owned_dynamic_target_child() {
    child(true);
}

// This checks the emitter's exact fixed two-root record, not LLVM validity or
// source authority. The caller must still run the pinned parser/verification gate.
fn two_root_absence_matches(text: &str, digest: &[u8; 32], bytes: usize, target: &str) -> bool {
    let digest: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    let record = format!(
        "!2 = !{{!\"multiple_defined_bodies\", !\"sha256:{digest}\", !\"kir-version:18\", i64 {bytes}, !\"target:{target}\"}}"
    );
    text.lines()
        .filter(|line| line.starts_with("!fe2o3.semantic_anchor"))
        .eq(["!fe2o3.semantic_anchor.absence.v1 = !{!2}"])
        && text
            .lines()
            .filter(|line| line.starts_with("!2 ="))
            .eq([record.as_str()])
        && !text.contains("!fe2o3.semantic_anchor.v1")
        && !text.contains("@llvm.pseudoprobe")
        && !text.contains("!llvm.pseudo_probe_desc")
}

#[test]
fn two_root_anchor_absence_oracle_binds_reason_output_identity_and_target_without_active_claims() {
    let digest = [0x17; 32];
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        let text = format!(
            "!fe2o3.semantic_anchor.absence.v1 = !{{!2}}\n!2 = !{{!\"multiple_defined_bodies\", !\"sha256:{}\", !\"kir-version:18\", i64 123, !\"target:{target}\"}}\n",
            "17".repeat(32)
        );
        assert!(two_root_absence_matches(&text, &digest, 123, target));
        assert!(!two_root_absence_matches(&text, &[0x18; 32], 123, target));
        assert!(!two_root_absence_matches(&text, &digest, 124, target));
        assert!(!two_root_absence_matches(
            &text,
            &digest,
            123,
            "gfx942:xnack+"
        ));
        for forged in [
            text.replace("multiple_defined_bodies", "no_operations"),
            text.replace("kir-version:18", "kir-version:12"),
            text.replace("absence.v1", "v1"),
            text.replace("!{!2}", "!{!3}"),
            text.replace("!2 =", "!3 ="),
            format!("{text}!fe2o3.semantic_anchor.v1 = !{{!3}}\n"),
            format!("{text}declare void @llvm.pseudoprobe(i64, i64, i32, i64)\n"),
            format!("{text}!llvm.pseudo_probe_desc = !{{!3}}\n"),
            format!("{text}{text}"),
            String::new(),
        ] {
            assert!(!two_root_absence_matches(&forged, &digest, 123, target));
        }
    }
}

#[test]
fn two_root_anchor_absence_distinguishes_canonical_identity_from_raw_bytes_digest() {
    // Frozen V18 identity framing, independent of the emitter and owner helper.
    // This is a metadata-oracle input, not an admitted executable module.
    let bytes = b"canonical identity oracle";
    let domain = b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V18\0";
    let mut hash = Sha256::new();
    hash.update(u32::try_from(domain.len()).unwrap().to_le_bytes());
    hash.update(domain);
    hash.update(1_u16.to_le_bytes());
    hash.update(u64::try_from(bytes.len()).unwrap().to_le_bytes());
    hash.update(bytes);
    let identity: [u8; 32] = hash.finalize().into();
    let raw: [u8; 32] = Sha256::digest(bytes).into();
    assert_ne!(identity, raw);
    let hex: String = identity.iter().map(|byte| format!("{byte:02x}")).collect();
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        let text = format!(
            "!fe2o3.semantic_anchor.absence.v1 = !{{!2}}\n!2 = !{{!\"multiple_defined_bodies\", !\"sha256:{hex}\", !\"kir-version:18\", i64 {}, !\"target:{target}\"}}\n",
            bytes.len()
        );
        assert!(two_root_absence_matches(
            &text,
            &identity,
            bytes.len(),
            target
        ));
        assert!(!two_root_absence_matches(&text, &raw, bytes.len(), target));
        let foreign = if target.starts_with("gfx942") {
            "gfx950:xnack-"
        } else {
            "gfx942:xnack-"
        };
        assert!(!two_root_absence_matches(
            &text,
            &identity,
            bytes.len(),
            foreign
        ));
    }
}

fn parse_and_verify_target_llvm(text: &str) {
    let opt = PathBuf::from(
        env::var_os("FE2O3_OPT").expect("FE2O3_OPT must name the explicit pinned LLVM 22 verifier"),
    );
    assert!(
        opt.is_absolute() && opt.is_file(),
        "explicit LLVM tool path"
    );
    let version = output(clean_command(&opt).arg("--version"));
    let version = String::from_utf8(version.stdout).unwrap();
    assert!(
        version
            .lines()
            .any(|line| line.contains("LLVM version 22.0.0git")),
        "pinned LLVM 22.0.0git required: {version}"
    );
    let mut child = clean_command(&opt)
        .args(["-passes=verify", "-disable-output", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit pinned LLVM parser/verifier");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
    let verified = child
        .wait_with_output()
        .expect("wait for LLVM parser/verifier");
    assert!(
        verified.status.success(),
        "LLVM parse/verification failed: {}",
        String::from_utf8_lossy(&verified.stderr)
    );
}

fn static_source(body: &str) -> String {
    format!(
        r#"use fe2o3_device::kernel;
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn scalar_first(mut seed: u32) {{ {body} }}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn scalar_second(mut seed: u32) {{ {body} }}
"#
    )
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies, source compilation and explicit FE2O3_OPT"]
fn actual_scalar_changed_and_noop_reach_verified_target_llvm_for_both_authenticated_targets() {
    for (body, profile, changed) in [("seed = 7_u32;", (0, 0), true), ("", (3, 2), false)] {
        run_actual_sources::<Outcome>(
            &[("scalar", body), ("scalar", body)],
            &[profile],
            LLVM_CHILD,
            "SOURCE_OWNED_TARGET_LLVM",
            static_source,
            |_, _, label, result, previous| {
                let Outcome::Llvm {
                    original,
                    optimized,
                    optimized_identity,
                    optimized_bytes,
                    changed: actual_changed,
                    text,
                    work,
                    peak,
                    target,
                    ..
                } = &result
                else {
                    panic!("required target LLVM result")
                };
                assert_eq!(*actual_changed, changed);
                assert_eq!(original != optimized, changed);
                assert!(matches!(target.as_str(), "gfx942:xnack-" | "gfx950:xnack-"));
                assert!(*work < 500_000_000 && *peak <= 20_000_000);
                assert!(
                    two_root_absence_matches(text, optimized_identity, *optimized_bytes, target),
                    "exact V18 output/target-bound multiple-body anchor absence required"
                );
                assert!(!two_root_absence_matches(
                    text,
                    optimized,
                    *optimized_bytes,
                    target
                ));
                parse_and_verify_target_llvm(text);
                if let Some(prior) = previous.get(label) {
                    assert_eq!(&result, prior);
                } else {
                    previous.insert(label.to_owned(), result);
                }
            },
        );
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_dynamic_source_refuses_target_llvm_without_substituting_launch_extent() {
    run_actual_sources::<Outcome>(
        &[("dynamic", "")],
        &[(0, 0)],
        REFUSAL_CHILD,
        "SOURCE_OWNED_DYNAMIC_LLVM_REFUSAL",
        scalar_source,
        |_, _, _, result, _| assert_eq!(result, Outcome::DynamicRefused),
    );
}

#[path = "production_rustc_driver_scalar_cfg_target_v18_tests.rs"]
mod scalar_cfg_tests;
