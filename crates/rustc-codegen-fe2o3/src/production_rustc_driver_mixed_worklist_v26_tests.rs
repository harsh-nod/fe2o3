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
use fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorKindV1;
use std::cell::Cell;
use std::process::Stdio;

#[path = "production_rustc_driver_mixed_entry_boundary_v27_tests.rs"]
mod boundary_tests;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_worklist_tests::mixed_worklist_child";
const COPY: &str = r#"
    if i >= input.len() { return; }
    *slot = input[i];
"#;
const DISJOINT_COPY: &str = r#"
    let disjoint = thread::index_1d().into_disjoint();
    let disjoint_i = disjoint.get();
    if i >= input.len() || disjoint_i >= input.len() { return; }
    *slot = input[disjoint_i];
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
const REPEATED: &str = r#"
    if i >= input.len() { return; }
    let element = &input[i];
    let first = read_outer(element);
    let second = read_outer(element);
    *slot = first ^ second;
"#;
const MUTABLE_STORE: &str = "store_outer(slot, seed);";
const MUTABLE_RMW: &str = r#"
    if i >= input.len() { return; }
    update_outer(slot, &input[i]);
"#;

fn program(body: &str) -> String {
    program_roots(&[("mixed", body)])
}

fn program_roots(roots: &[(&str, &str)]) -> String {
    let mut source = String::from(
        r#"use fe2o3_device::{DisjointSlice, kernel, thread};
#[inline(never)]
fn read_leaf(value: &u32) -> u32 { *value }
#[inline(never)]
fn read_outer(value: &u32) -> u32 { read_leaf(value) }
#[inline(never)]
fn store_leaf(value: &mut u32, input: u32) { *value = input; }
#[inline(never)]
fn store_outer(value: &mut u32, input: u32) { store_leaf(value, input); }
#[inline(never)]
fn update_leaf(value: &mut u32, input: &u32) { *value = *value ^ *input; }
#[inline(never)]
fn update_outer(value: &mut u32, input: &u32) { update_leaf(value, input); }
"#,
    );
    for (name, body) in roots {
        source.push_str(&format!(
            r#"#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn {name}(input: &[u32], mut output: DisjointSlice<u32>, seed: u32, _word: usize) {{
    let index = thread::index_1d();
    let i = index.get();
    let Some(slot) = output.get_mut(index) else {{ return; }};
    {body}
}}
"#
        ));
    }
    source
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    source: [u8; 32],
    original: [u8; 32],
    output: [u8; 32],
    target: String,
    llvm: String,
    roots: usize,
    instances: usize,
    instances_per_root: [usize; 2],
    index_reader_calls: [usize; 2],
    accesses_per_root: [[usize; 2]; 2],
    helper_accesses: [usize; 2],
    repeated_helper_access: bool,
    shared_root_helper_access: bool,
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
    missing_root_refused: bool,
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

pub(super) fn paid_text(text: &str, budget: &mut Budget<'_>) -> Result<String, Error> {
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
                        let semantic = source.source_semantic(budget)?;
                        let mut index_reader_calls = [0; 2];
                        for function in semantic.functions() {
                            budget.charge_work(1)?;
                            for block in function.blocks() {
                                budget.charge_work(1)?;
                                let callee = match block.terminator().kind() {
                                    SemanticTerminatorKindV1::Call(call) => call.callee(),
                                    SemanticTerminatorKindV1::TailCall(call) => call.callee(),
                                    _ => continue,
                                };
                                match &semantic.callables()[callee.index() as usize] {
                                    SemanticCallableDeclV1::CompilerIntrinsic {
                                        operation:
                                            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                                                ..
                                            },
                                        ..
                                    } => index_reader_calls[0] += 1,
                                    SemanticCallableDeclV1::CompilerIntrinsic {
                                        operation:
                                            SemanticCompilerIntrinsicOperationV1::DisjointIndexGet {
                                                ..
                                            },
                                        ..
                                    } => index_reader_calls[1] += 1,
                                    _ => {}
                                }
                            }
                        }
                        let root_count = source.root_count(budget)?;
                        assert!((1..=2).contains(&root_count));
                        assert_eq!(roots.len(), root_count);
                        let mut instances_per_root = [0; 2];
                        for (ordinal, root) in roots.iter().enumerate() {
                            instances_per_root[ordinal] = source.instance_count(ordinal, budget)?;
                            assert_eq!(root.arguments.len(), 4);
                            assert!(matches!(
                                root.arguments[0].kind,
                                AbiKind::Descriptor {
                                    source: SourceTypeDescriptorV3::SharedSlice(_),
                                    ..
                                }
                            ));
                            assert!(matches!(
                                root.arguments[1].kind,
                                AbiKind::Descriptor {
                                    source: SourceTypeDescriptorV3::DisjointSlice(_),
                                    ..
                                }
                            ));
                            assert!(matches!(
                                root.arguments[3].kind,
                                AbiKind::Descriptor {
                                    source: SourceTypeDescriptorV3::Usize,
                                    ..
                                }
                            ));
                        }
                        let (launches, width) = handoff.launch_context(budget)?;
                        assert_eq!(width, FormalIndexWidth::Bits64);
                        assert_eq!(launches.len(), root_count);
                        for launch in launches {
                            assert_eq!(
                                *launch,
                                ExplicitLaunchExtent::Exact {
                                    rank: 1,
                                    extents: [192, 1, 1],
                                }
                            );
                        }
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
                        assert_eq!(premises.len(), 2 * root_count);
                        let mut counts = [0; 2];
                        let mut accesses_per_root = [[0; 2]; 2];
                        let mut declared_slices = [[false; 2]; 2];
                        let mut unused = 0;
                        for premise in premises {
                            assert!(premise.root() < root_count);
                            assert!(premise.original_argument() < 2);
                            let root = premise.root();
                            let argument = usize::try_from(premise.original_argument()).unwrap();
                            assert!(!declared_slices[root][argument]);
                            declared_slices[root][argument] = true;
                            assert_eq!(premise.launch(), launches[root]);
                            assert_eq!(premise.index_width(), width);
                            assert!(!premise.grants_artifact_or_launch_authority());
                            let accesses = premise.access_counts();
                            unused += usize::from(accesses == [0, 0]);
                            counts[0] += accesses[0];
                            counts[1] += accesses[1];
                            accesses_per_root[root][0] += accesses[0];
                            accesses_per_root[root][1] += accesses[1];
                        }
                        assert!(
                            declared_slices[..root_count]
                                .iter()
                                .all(|row| *row == [true; 2])
                        );
                        let occurrences = handoff.runtime_occurrences(budget)?;
                        assert_eq!(occurrences.len(), counts.iter().sum::<usize>());
                        let mut helper_accesses = [0; 2];
                        let mut repeated_helper_access = false;
                        let mut shared_root_helper_access = false;
                        for (ordinal, row) in occurrences.iter().enumerate() {
                            assert!(row.premise_index() < premises.len());
                            assert!(row.requires_address_formation_domain());
                            assert!(!row.grants_artifact_or_launch_authority());
                            let root = premises[row.premise_index()].root();
                            let instance = row.original_instance();
                            assert!(instance < instances_per_root[root]);
                            assert!(source.instance_active(root, instance, budget)?);
                            let (function, caller) = source.instance(root, instance, budget)?;
                            if caller.is_some() {
                                helper_accesses[usize::from(row.domain().writing())] += 1;
                            }
                            for previous in &occurrences[..ordinal] {
                                assert_ne!(row.original_operation(), previous.original_operation());
                                assert_ne!(row.output_operation(), previous.output_operation());
                                let previous_root = premises[previous.premise_index()].root();
                                let previous_instance = previous.original_instance();
                                let (previous_function, previous_caller) =
                                    source.instance(previous_root, previous_instance, budget)?;
                                if caller.is_some()
                                    && previous_caller.is_some()
                                    && function == previous_function
                                {
                                    repeated_helper_access |=
                                        root == previous_root && instance != previous_instance;
                                    shared_root_helper_access |= root != previous_root;
                                }
                            }
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
                            roots: root_count,
                            instances: instances_per_root.iter().sum(),
                            instances_per_root,
                            index_reader_calls,
                            accesses_per_root,
                            helper_accesses,
                            repeated_helper_access,
                            shared_root_helper_access,
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
                            missing_root_refused: false,
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
                        assert!((1..=2).contains(&roots.len()));
                        let changed: [AbiRoot<'_>; 2] = std::array::from_fn(|ordinal| {
                            let root = &roots[ordinal.min(roots.len() - 1)];
                            AbiRoot {
                                kernel_binding: root.kernel_binding,
                                export: root.export,
                                arguments: &root.arguments
                                    [..root.arguments.len() - usize::from(ordinal == 0)],
                                explicit_argument_bytes: root.explicit_argument_bytes,
                                kernarg_alignment_bytes: root.kernarg_alignment_bytes,
                            }
                        });
                        handoff.check_original_argument_abi_v26(
                            AbiInput {
                                roots: &changed[..roots.len()],
                            },
                            budget,
                        )?;
                        panic!("incomplete original ABI acquired mixed output custody")
                    },
                ),
            );
            assert!(matches!(error, Error::Source(SourceError::Binding(_))));
            report.incomplete_abi_refused = true;
            if report.roots > 1 {
                let reached = Cell::new(false);
                let error = refused(
                    transaction()?.with_original_source_conditional_mixed_worklist_v26::<(), _>(
                        |_, handoff, roots, _, budget| {
                            reached.set(true);
                            handoff.check_original_argument_abi_v26(
                                AbiInput {
                                    roots: &roots[..roots.len() - 1],
                                },
                                budget,
                            )?;
                            panic!("missing original root acquired mixed output custody")
                        },
                    ),
                );
                assert!(reached.get());
                assert!(matches!(error, Error::Source(SourceError::Binding(_))));
                report.missing_root_refused = true;
            }

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
            ("disjoint-copy", DISJOINT_COPY),
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
            assert_eq!(report.roots, 1);
            assert_eq!(report.policy, 9);
            assert_eq!(report.writes, 1);
            assert_eq!(report.occurrences, report.reads + report.writes);
            assert_eq!(
                report.index_reader_calls,
                [1, usize::from(case == "disjoint-copy")],
                "retained actual Rust calls must exercise the selected shared index reader"
            );
            match case {
                "copy" | "disjoint-copy" | "nested" => assert_eq!(report.reads, 1),
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

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD dependencies and FE2O3_OPT LLVM 22"]
fn actual_original_mixed_reference_calls_keep_repeated_mutable_and_multiroot_ownership() {
    let repeated = program(REPEATED);
    let mutable_store = program(MUTABLE_STORE);
    let mutable_rmw = program(MUTABLE_RMW);
    let multiple_roots = program_roots(&[("mixed_a", NESTED), ("mixed_b", NESTED)]);
    run_actual_sources::<Observation>(
        &[
            ("repeated", &repeated),
            ("mutable-store", &mutable_store),
            ("mutable-rmw", &mutable_rmw),
            ("multiple-roots", &multiple_roots),
        ],
        &[(0, 0)],
        CHILD,
        "CONDITIONAL_MIXED_REFERENCE_MATRIX_V26",
        str::to_owned,
        |_, _, case, report, _| {
            let (root_count, per_root, helper_accesses, minimum_instances) = match case {
                "repeated" => (1, [2, 1], [2, 0], 5),
                "mutable-store" => (1, [0, 1], [0, 1], 3),
                "mutable-rmw" => (1, [2, 1], [2, 1], 3),
                "multiple-roots" => (2, [1, 1], [2, 0], 3),
                _ => panic!("unexpected reference matrix case"),
            };
            assert!(report.target.starts_with("gfx942") || report.target.starts_with("gfx950"));
            assert_eq!(report.policy, 9);
            assert_eq!(report.roots, root_count);
            assert_eq!(report.index_reader_calls, [root_count, 0]);
            assert_eq!(
                (report.reads, report.writes),
                (root_count * per_root[0], root_count)
            );
            assert_eq!(report.occurrences, report.reads + report.writes);
            assert_eq!(report.helper_accesses, helper_accesses);
            assert_eq!(report.unused_slices, usize::from(case == "mutable-store"));
            for root in 0..root_count {
                assert_eq!(report.accesses_per_root[root], per_root);
                assert!(report.instances_per_root[root] >= minimum_instances);
            }
            if case == "repeated" {
                assert!(report.repeated_helper_access);
            }
            if case == "multiple-roots" {
                assert!(report.shared_root_helper_access);
                assert!(report.missing_root_refused);
            }
            assert!(report.prepared && report.conditional_only);
            assert!(report.foreign_owner_refused && report.foreign_ledger_refused);
            assert!(report.incomplete_abi_refused && report.callback_error_preserved);
            assert!(report.exact_and_short_storage && report.zero_work_refused);
            assert!(report.target_controls_checked && report.worker_input_controls_checked);
            parse_and_verify_target_llvm(&report.llvm);
        },
    );
}

pub(super) fn parse_and_verify_target_llvm(text: &str) {
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
fn mixed_disjoint_index_reader_uses_public_safe_construction_and_a_live_indexed_read() {
    let source = program(DISJOINT_COPY);
    assert!(source.contains("let disjoint = thread::index_1d().into_disjoint();"));
    assert!(source.contains("let disjoint_i = disjoint.get();"));
    assert!(source.contains("if i >= input.len() || disjoint_i >= input.len() { return; }"));
    assert!(source.contains("*slot = input[disjoint_i];"));
    assert!(source.contains("output.get_mut(index)"));
    assert_eq!(source.matches("#[kernel(typed,").count(), 1);
    assert!(!source.contains("unsafe") && !source.contains("*mut") && !source.contains("*const"));
}

#[test]
fn mixed_reference_matrix_uses_safe_actual_calls_and_distinct_kernel_roots() {
    let repeated = program(REPEATED);
    assert_eq!(repeated.matches("read_outer(element)").count(), 2);
    assert!(repeated.contains("let element = &input[i]"));
    let store = program(MUTABLE_STORE);
    assert!(store.contains("store_outer(slot, seed)"));
    assert!(store.contains("fn store_leaf(value: &mut u32, input: u32)"));
    let rmw = program(MUTABLE_RMW);
    assert!(rmw.contains("update_outer(slot, &input[i])"));
    assert!(rmw.contains("fn update_leaf(value: &mut u32, input: &u32)"));
    let roots = program_roots(&[("mixed_a", NESTED), ("mixed_b", NESTED)]);
    assert_eq!(roots.matches("#[kernel(typed,").count(), 2);
    assert_eq!(roots.matches("fn read_leaf(value: &u32)").count(), 1);
    assert_eq!(roots.matches("*slot = read_outer(&input[i])").count(), 2);
    assert!(roots.contains("pub fn mixed_a(") && roots.contains("pub fn mixed_b("));
    for source in [&repeated, &store, &rmw, &roots] {
        assert!(
            !source.contains("unsafe") && !source.contains("*mut") && !source.contains("*const")
        );
        assert_eq!(
            source.matches("output.get_mut(index)").count(),
            source.matches("#[kernel(typed,").count()
        );
    }
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
