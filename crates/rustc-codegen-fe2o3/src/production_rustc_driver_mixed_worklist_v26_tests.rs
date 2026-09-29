//! Ordinary Rust through the source-bound conditional mixed Policy9 continuation.
use super::*;
use crate::production_pipeline::source_owned_v29::target_result::{
    mixed_v26::{
        check_and_lower_mixed_target_llvm_v26,
        tests::genuine_mixed_case,
        worker_input_v26::tests::{Mode as WorkerMode, genuine_worker_input_case},
    },
    tests::Mode,
};
use fe2o3_kernel_ir::{ExplicitLaunchExtent, FormalIndexWidth};
use fe2o3_lower_mir_kernel::ProductionConditionalMixedOutputHandoffV26 as MixedHandoff;
use std::cell::Cell;
use std::io::Write as _;
use std::process::Stdio;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_worklist_tests::mixed_worklist_child";
const COPY: &str = r#"
    if i >= input.len() { return; }
    *slot = input[i];
"#;
const STORE: &str = "*slot = seed;";
const RMW: &str = r#"
    if i >= input.len() { return; }
    *slot = *slot ^ input[i];
"#;
const NESTED: &str = r#"
    if i >= input.len() { return; }
    *slot = read_outer(&input[i]);
"#;

fn program(body: &str) -> String {
    format!(
        r#"use fe2o3_device::{{DisjointSlice, kernel, thread}};
#[inline(never)]
fn read_leaf(value: &u32) -> u32 {{ *value }}
#[inline(never)]
fn read_outer(value: &u32) -> u32 {{ read_leaf(value) }}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn mixed(input: &[u32], mut output: DisjointSlice<u32>, seed: u32, _word: usize) {{
    let index = thread::index_1d();
    let i = index.get();
    let Some(slot) = output.get_mut(index) else {{ return; }};
    {body}
}}
"#
    )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    source: [u8; 32],
    original: [u8; 32],
    output: [u8; 32],
    target: String,
    llvm: String,
    instances: usize,
    reads: usize,
    writes: usize,
    unused_slices: usize,
    occurrences: usize,
    policy: u16,
    prepared: bool,
    conditional_only: bool,
    foreign_owner_refused: bool,
    foreign_ledger_refused: bool,
    incomplete_abi_refused: bool,
    callback_error_preserved: bool,
    exact_and_short_storage: bool,
    zero_work_refused: bool,
    target_controls_checked: bool,
    worker_input_controls_checked: bool,
}

#[derive(Default)]
struct MixedCallbacks {
    result: Option<Result<Observation, String>>,
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

impl Callbacks for MixedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let foreign = transaction()?
                .original_source_ssa_for_test_v18()
                .map_err(|error| format!("independent mixed source: {error:?}"))?;
            start_preparation_observation_v29();
            let continuation = transaction()?
                .with_original_source_conditional_mixed_worklist_v26(
                    |source, handoff, roots, target, budget| {
                        assert_eq!(
                            source.source_semantic(budget)?.wire_version(),
                            SemanticMirWireVersionV1::V35
                        );
                        assert_eq!(source.source_ssa(budget)?.identity(), foreign.identity());
                        assert!(!std::ptr::eq(source.source_ssa(budget)?, &foreign));
                        handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                        handoff.check_original_argument_abi_v26(AbiInput { roots }, budget)?;
                        assert_eq!(source.root_count(budget)?, 1);
                        assert_eq!(roots.len(), 1);
                        assert_eq!(roots[0].arguments.len(), 4);
                        assert!(matches!(
                            roots[0].arguments[0].kind,
                            AbiKind::Descriptor {
                                source: SourceTypeDescriptorV3::SharedSlice(_),
                                ..
                            }
                        ));
                        assert!(matches!(
                            roots[0].arguments[1].kind,
                            AbiKind::Descriptor {
                                source: SourceTypeDescriptorV3::DisjointSlice(_),
                                ..
                            }
                        ));
                        assert!(matches!(
                            roots[0].arguments[3].kind,
                            AbiKind::Descriptor {
                                source: SourceTypeDescriptorV3::Usize,
                                ..
                            }
                        ));
                        let (launches, width) = handoff.launch_context(budget)?;
                        assert_eq!(width, FormalIndexWidth::Bits64);
                        assert_eq!(
                            launches,
                            &[ExplicitLaunchExtent::Exact {
                                rank: 1,
                                extents: [192, 1, 1],
                            }]
                        );
                        let original = source.canonical(budget)?;
                        let output = handoff.output(budget)?;
                        assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                        assert!(!std::ptr::eq(original, output.owner()));
                        assert_eq!(output.execution().policy_version(), 9);
                        assert!(!output.grants_authority());
                        assert!(!handoff.runtime_requirements_are_discharged());
                        assert!(!handoff.ranked_verification_is_complete());
                        assert!(!handoff.grants_artifact_or_launch_authority());
                        let premises = handoff.runtime_premises(budget)?;
                        assert_eq!(premises.len(), 2);
                        let mut counts = [0; 2];
                        let mut unused = 0;
                        for premise in premises {
                            assert_eq!(premise.root(), 0);
                            assert!(premise.original_argument() < 2);
                            assert_eq!(premise.launch(), launches[0]);
                            assert_eq!(premise.index_width(), width);
                            assert!(!premise.grants_artifact_or_launch_authority());
                            let accesses = premise.access_counts();
                            unused += usize::from(accesses == [0, 0]);
                            counts[0] += accesses[0];
                            counts[1] += accesses[1];
                        }
                        let occurrences = handoff.runtime_occurrences(budget)?;
                        assert_eq!(occurrences.len(), counts.iter().sum::<usize>());
                        for row in occurrences {
                            assert!(row.premise_index() < premises.len());
                            assert!(row.requires_address_formation_domain());
                            assert!(!row.grants_artifact_or_launch_authority());
                        }
                        let native =
                            check_and_lower_mixed_target_llvm_v26(source, handoff, target, budget)?;
                        assert_eq!(native.target(budget)?, target);
                        let text = native.llvm_ir(budget)?;
                        assert!(text.contains("kir-version:18"));
                        assert!(!text.contains("kir-version:12"));
                        // A paid copy only transports exact text to the parent
                        // LLVM parser; it does not transport source authority.
                        let llvm = paid_text(text, budget)?;
                        native.discard(budget)?;
                        Ok(Observation {
                            source: *source.source_ssa(budget)?.source_semantic_sha256(),
                            original: Sha256::digest(original.canonical_bytes()).into(),
                            output: Sha256::digest(output.owner().canonical_bytes()).into(),
                            target: paid_text(target.device_target(), budget)?,
                            llvm,
                            instances: source.instance_count(0, budget)?,
                            reads: counts[0],
                            writes: counts[1],
                            unused_slices: unused,
                            occurrences: occurrences.len(),
                            policy: output.execution().policy_version(),
                            prepared: false,
                            conditional_only: true,
                            foreign_owner_refused: false,
                            foreign_ledger_refused: false,
                            incomplete_abi_refused: false,
                            callback_error_preserved: false,
                            exact_and_short_storage: false,
                            zero_work_refused: false,
                            target_controls_checked: false,
                            worker_input_controls_checked: false,
                        })
                    },
                )
                .map_err(|error| format!("original mixed worklist: {error:?}"))?;
            assert!(
                take_preparation_observation_v29()
                    .expect("actual mixed preparation observed")
                    .materialized
            );
            let mut report = continuation.into_observation();
            report.prepared = true;

            let error = refused(
                transaction()?.with_original_source_conditional_mixed_worklist_v26::<(), _>(
                    |_, handoff, _, _, budget| {
                        handoff.check_original_source(&foreign, budget)?;
                        panic!("equal source bytes acquired mixed output custody")
                    },
                ),
            );
            assert!(matches!(error, Error::Source(SourceError::Binding(_))));
            report.foreign_owner_refused = true;

            let error = refused(
                transaction()?.with_original_source_conditional_mixed_worklist_v26::<(), _>(
                    |_, handoff, _, _, _| {
                        let mut work =
                            fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                        let foreign = Budget::new(&mut work, 20_000_000);
                        match handoff.output(&foreign) {
                            Err(error) => Err(error.into()),
                            Ok(_) => panic!("foreign ledger acquired mixed output"),
                        }
                    },
                ),
            );
            assert!(matches!(
                error,
                Error::Source(SourceError::Resource(ResourceError::Accounting))
            ));
            report.foreign_ledger_refused = true;

            let error = refused(
                transaction()?.with_original_source_conditional_mixed_worklist_v26::<(), _>(
                    |_, handoff, roots, _, budget| {
                        let root = &roots[0];
                        let changed = [AbiRoot {
                            kernel_binding: root.kernel_binding,
                            export: root.export,
                            arguments: &root.arguments[..root.arguments.len() - 1],
                            explicit_argument_bytes: root.explicit_argument_bytes,
                            kernarg_alignment_bytes: root.kernarg_alignment_bytes,
                        }];
                        handoff.check_original_argument_abi_v26(
                            AbiInput { roots: &changed },
                            budget,
                        )?;
                        panic!("incomplete original ABI acquired mixed output custody")
                    },
                ),
            );
            assert!(matches!(error, Error::Source(_)));
            report.incomplete_abi_refused = true;

            let reached = Cell::new(false);
            let error = refused(
                transaction()?.with_original_source_conditional_mixed_worklist_v26::<(), _>(
                    |source, handoff, _, _, budget| {
                        handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                        reached.set(true);
                        Err(Error::Unsupported(
                            "mixed final admission remains conditional",
                        ))
                    },
                ),
            );
            assert!(reached.get());
            assert!(matches!(
                error,
                Error::Unsupported("mixed final admission remains conditional")
            ));
            report.callback_error_preserved = true;

            fn peak<'view, 'source>(
                _: &'view fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'source>,
                _: &MixedHandoff<'view, 'source>,
                _: &[AbiRoot<'_>],
                _: fe2o3_amd_target::ProductionAmdTargetProfileV1,
                budget: &mut Budget<'_>,
            ) -> Result<usize, Error> {
                Ok(budget.peak_storage())
            }
            // Keep the exact function item and return type in all limit runs.
            let measured = transaction()?
                .with_original_source_conditional_mixed_test_limits_v26(
                    500_000_000,
                    20_000_000,
                    peak,
                )
                .map_err(|error| format!("mixed storage measurement: {error:?}"))?
                .into_observation();
            let exact = transaction()?
                .with_original_source_conditional_mixed_test_limits_v26(500_000_000, measured, peak)
                .map_err(|error| format!("mixed exact storage: {error:?}"))?
                .into_observation();
            assert_eq!(exact, measured);
            refused(
                transaction()?.with_original_source_conditional_mixed_test_limits_v26(
                    500_000_000,
                    measured - 1,
                    peak,
                ),
            );
            report.exact_and_short_storage = true;
            let error = refused(
                transaction()?
                    .with_original_source_conditional_mixed_test_limits_v26(0, 20_000_000, peak),
            );
            assert!(matches!(error, Error::Resource(ResourceError::Work(_))));
            report.zero_work_refused = true;
            for mode in [
                Mode::Success,
                Mode::ExactStorage,
                Mode::ShortStorage,
                Mode::WorkRefusal,
                Mode::ForeignEntry,
                Mode::RestoredFloor,
            ] {
                let result = transaction()?.with_original_source_conditional_mixed_worklist_v26(
                    |source, handoff, _, target, budget| {
                        genuine_mixed_case(source, handoff, target, budget, mode)?;
                        Ok(())
                    },
                );
                if matches!(mode, Mode::Success | Mode::ExactStorage) {
                    result
                        .map_err(|error| format!("mixed target {mode:?}: {error:?}"))?
                        .into_observation();
                } else {
                    assert!(matches!(refused(result), Error::TargetLlvm(_)));
                }
            }
            report.target_controls_checked = true;
            for mode in [
                WorkerMode::Success,
                WorkerMode::WrongTarget,
                WorkerMode::WrongBinding,
                WorkerMode::WrongType,
                WorkerMode::IncompleteAbi,
                WorkerMode::ForeignLedger,
                WorkerMode::ExactStorage,
                WorkerMode::ShortStorage,
                WorkerMode::RestoredFloor,
                WorkerMode::WorkRefusal,
            ] {
                let result = transaction()?.with_original_source_conditional_mixed_worklist_v26(
                    |source, handoff, roots, target, budget| {
                        genuine_worker_input_case(source, handoff, roots, target, budget, mode)?;
                        Ok(())
                    },
                );
                if matches!(mode, WorkerMode::Success | WorkerMode::ExactStorage) {
                    result
                        .map_err(|error| format!("mixed Worker input {mode:?}: {error:?}"))?
                        .into_observation();
                } else {
                    assert!(matches!(refused(result), Error::MixedWorkerInput(_)));
                }
            }
            report.worker_input_controls_checked = true;
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn mixed_worklist_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = MixedCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("mixed worklist callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("mixed worklist result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual mixed worklist: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD dependencies and FE2O3_OPT LLVM 22"]
fn actual_original_mixed_worklist_keeps_shared_store_rmw_and_nested_helper_contracts() {
    run_actual_sources::<Observation>(
        &[
            ("copy", COPY),
            ("store", STORE),
            ("rmw", RMW),
            ("nested", NESTED),
        ],
        &[(0, 0)],
        CHILD,
        "CONDITIONAL_MIXED_WORKLIST_V26",
        program,
        |_, _, case, report, _| {
            assert!(report.target.starts_with("gfx942") || report.target.starts_with("gfx950"));
            assert_eq!(report.policy, 9);
            assert_eq!(report.writes, 1);
            assert_eq!(report.occurrences, report.reads + report.writes);
            match case {
                "copy" | "nested" => assert_eq!(report.reads, 1),
                "store" => assert_eq!(report.reads, 0),
                "rmw" => assert_eq!(report.reads, 2),
                _ => panic!("unexpected actual mixed case"),
            }
            assert_eq!(report.unused_slices, usize::from(case == "store"));
            if case == "nested" {
                assert!(report.instances >= 3);
            }
            assert!(report.prepared && report.conditional_only);
            assert!(report.foreign_owner_refused && report.foreign_ledger_refused);
            assert!(report.incomplete_abi_refused && report.callback_error_preserved);
            assert!(report.exact_and_short_storage && report.zero_work_refused);
            assert!(report.target_controls_checked);
            assert!(report.worker_input_controls_checked);
            parse_and_verify_target_llvm(&report.llvm);
        },
    );
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

#[test]
fn mixed_worklist_actual_fixture_has_safe_bounds_disjoint_writes_and_nested_references() {
    let source = program(NESTED);
    assert!(source.contains("input: &[u32]"));
    assert!(source.contains("output: DisjointSlice<u32>"));
    assert!(source.contains("thread::index_1d()"));
    assert!(source.contains("output.get_mut(index)"));
    assert!(source.contains("if i >= input.len() { return; }"));
    assert!(source.contains("fn read_outer(value: &u32)"));
    assert!(source.contains("read_leaf(value)"));
    assert!(source.contains("read_outer(&input[i])"));
    assert!(source.contains("max_grid = [3, 1, 1]"));
    assert!(!source.contains("unsafe"));
    assert!(CHILD.ends_with("::mixed_worklist_tests::mixed_worklist_child"));
}

#[test]
fn mixed_worker_input_refusal_keeps_its_typed_backend_error() {
    use crate::production_pipeline::source_owned_v29::target_result::mixed_v26::worker_input_v26::MixedWorkerInputErrorV26;
    let error = Error::from(MixedWorkerInputErrorV26::Mismatch("original root ordinal"));
    assert!(matches!(
        error,
        Error::MixedWorkerInput(MixedWorkerInputErrorV26::Mismatch("original root ordinal"))
    ));
    assert!(std::error::Error::source(&error).is_some());
}
