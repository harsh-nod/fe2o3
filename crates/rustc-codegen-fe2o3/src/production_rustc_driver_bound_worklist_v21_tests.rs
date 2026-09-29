//! Actual original Rust through the fixed private-class Policy9 continuation.
use super::*;
use fe2o3_kernel_ir::{
    AddressSpace, CanonicalFormalLaunchInputV19, ExplicitLaunchExtent, FormalIndexWidth,
    OperationKind,
};
use sha2::{Digest as _, Sha256};
use std::cell::Cell;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::bound_worklist_tests::bound_worklist_child";

fn program(body: &str) -> String {
    format!(
        r#"use fe2o3_device::kernel;
#[inline(never)]
fn observe(value: &u32) {{
    let _observed = *value;
}}
#[inline(never)]
fn private_chain(mut seed: u32) {{
    observe(&seed);
    let loaded = seed;
    let first = loaded ^ 0;
    observe(&seed);
    let second = first ^ 0;
    observe(&seed);
    let third = second ^ 0;
    seed = third;
    observe(&seed);
}}
#[inline(never)]
fn private_plain(seed: u32) {{
    let second = seed;
    observe(&seed);
    observe(&second);
}}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn worklist_first(seed: u32, _word: usize) {{ {body} }}
#[kernel(typed, launch(required = [32, 1, 1], max = [32, 1, 1], max_grid = [5, 1, 1]))]
pub fn worklist_second(seed: u32, _word: usize) {{ private_plain(seed); }}
"#
    )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Census {
    private_allocations: usize,
    private_loads: usize,
    private_stores: usize,
    binary_operations: usize,
    cross_block_binary_dependencies: usize,
}

fn census(module: &fe2o3_kernel_ir::Module) -> Census {
    let mut census = Census {
        private_allocations: 0,
        private_loads: 0,
        private_stores: 0,
        binary_operations: 0,
        cross_block_binary_dependencies: 0,
    };
    for body in module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
    {
        for block in &body.blocks {
            for operation in &block.operations {
                match &operation.kind {
                    OperationKind::Alloca { address_space, .. } => {
                        assert_eq!(*address_space, AddressSpace::Private);
                        census.private_allocations += 1;
                    }
                    OperationKind::Load { access, .. }
                    | OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::ReadValue {
                        access,
                        ..
                    }) => {
                        assert_eq!(access.address_space, AddressSpace::Private);
                        census.private_loads += 1;
                    }
                    OperationKind::Store { access, .. }
                    | OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::WriteValue {
                        access,
                        ..
                    }) => {
                        assert_eq!(access.address_space, AddressSpace::Private);
                        census.private_stores += 1;
                    }
                    OperationKind::Binary { lhs, rhs, .. } => {
                        census.binary_operations += 1;
                        for operand in [lhs, rhs] {
                            census.cross_block_binary_dependencies +=
                                usize::from(body.blocks.iter().any(|producer_block| {
                                    producer_block.id != block.id
                                        && producer_block.operations.iter().any(|producer| {
                                            matches!(producer.kind, OperationKind::Binary { .. })
                                                && producer
                                                    .results
                                                    .iter()
                                                    .any(|result| &result.id == operand)
                                        })
                                }));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    census
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    source: [u8; 32],
    original: [u8; 32],
    output: [u8; 32],
    roots: usize,
    instances: usize,
    before: Census,
    after: Census,
    changed: bool,
    policy: u16,
    unqualified: bool,
    prepared: bool,
    foreign_owner_refused: bool,
    foreign_ledger_refused: bool,
    callback_error_preserved: bool,
}

#[derive(Default)]
struct WorklistCallbacks {
    result: Option<Result<Observation, String>>,
}

impl Callbacks for WorklistCallbacks {
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
                .map_err(|error| format!("independent worklist source: {error:?}"))?;
            start_preparation_observation_v29();
            let completed = Cell::new(false);
            let continuation = transaction()?
                .with_original_source_bound_private_worklist_v21(
                    |source, handoff, roots, _, budget| {
                        let semantic = source.source_semantic(budget)?;
                        assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V35);
                        assert_eq!(source.source_ssa(budget)?.identity(), foreign.identity());
                        assert!(!std::ptr::eq(source.source_ssa(budget)?, &foreign));
                        source.require_kernel_argument_abi_v18(AbiInput { roots }, budget)?;
                        handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                        let root_count = source.root_count(budget)?;
                        assert_eq!(root_count, 2);
                        assert_eq!(roots.len(), root_count);
                        let mut instances = 0;
                        for ordinal in 0..root_count {
                            let (root, _) = source.root(ordinal, budget)?;
                            let inputs = semantic.functions()[root.index() as usize]
                                .abi()
                                .source_input_types();
                            assert_eq!(inputs.len(), 2);
                            assert_eq!(
                                semantic.types()[inputs[1].index() as usize].rust_type_kind(),
                                SemanticRustTypeKindV1::Usize
                            );
                            assert!(matches!(
                                roots[ordinal].arguments[1].kind,
                                AbiKind::Descriptor {
                                    source: SourceTypeDescriptorV3::Usize,
                                    ..
                                }
                            ));
                            let count = source.instance_count(ordinal, budget)?;
                            assert!(count >= 2);
                            instances += count;
                            for instance in 0..count {
                                assert!(source.instance_active(ordinal, instance, budget)?);
                                if let Some((caller, _)) =
                                    source.instance(ordinal, instance, budget)?.1
                                {
                                    assert!(caller < instance);
                                }
                            }
                        }
                        let original = source.canonical(budget)?;
                        let output = handoff.output(budget)?;
                        assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                        assert!(!std::ptr::eq(original, output.owner()));
                        assert_eq!(original.module().kernels.len(), root_count);
                        assert_eq!(output.owner().module().kernels.len(), root_count);
                        let (launches, width) = handoff.formal_context_v21(budget)?;
                        assert_eq!(width, FormalIndexWidth::Bits64);
                        assert_eq!(launches.len(), root_count);
                        let mut extents = Vec::new();
                        for launch in launches {
                            let CanonicalFormalLaunchInputV19::PhysicalEnvelope(
                                ExplicitLaunchExtent::Exact {
                                    rank,
                                    extents: extent,
                                },
                            ) = launch
                            else {
                                panic!("actual descriptor must retain physical geometry");
                            };
                            assert_eq!(*rank, 1);
                            extents.push(*extent);
                        }
                        extents.sort();
                        assert_eq!(extents, [[160, 1, 1], [192, 1, 1]]);
                        let before = census(original.module());
                        let after = census(output.owner().module());
                        assert!(
                            before.private_allocations >= 2
                                && before.private_loads >= 2
                                && before.private_stores >= 2
                        );
                        assert!(
                            after.private_allocations >= 2
                                && after.private_loads >= 2
                                && after.private_stores >= 2
                        );
                        assert_eq!(after.private_allocations, before.private_allocations);
                        assert_eq!(after.private_loads, before.private_loads);
                        assert_eq!(after.private_stores, before.private_stores);
                        assert_eq!(output.execution().policy_version(), 9);
                        assert_eq!(
                            &output.execution().canonical_bytes()[..8],
                            &[9, 0, 1, 0, 2, 0, 18, 0]
                        );
                        assert!(!output.grants_authority());
                        assert!(!handoff.ranked_verification_is_complete());
                        assert!(!handoff.grants_artifact_or_launch_authority());
                        let observation = Observation {
                            source: *source.source_ssa(budget)?.source_semantic_sha256(),
                            original: Sha256::digest(original.canonical_bytes()).into(),
                            output: Sha256::digest(output.owner().canonical_bytes()).into(),
                            roots: root_count,
                            instances,
                            before,
                            after,
                            changed: output.report().passes()[0].changed(),
                            policy: output.execution().policy_version(),
                            unqualified: true,
                            prepared: false,
                            foreign_owner_refused: false,
                            foreign_ledger_refused: false,
                            callback_error_preserved: false,
                        };
                        completed.set(true);
                        Ok(observation)
                    },
                )
                .map_err(|error| format!("original bound worklist: {error:?}"))?;
            assert!(completed.get());
            assert!(
                take_preparation_observation_v29()
                    .expect("actual worklist preparation observed")
                    .materialized
            );
            let mut report = continuation.into_observation();
            report.prepared = true;

            let completed = Cell::new(false);
            let error = refused(
                transaction()?.with_original_source_bound_private_worklist_v21::<(), _>(
                    |source, _, _, _, budget| {
                        let error = source.check_original_source(&foreign, budget).unwrap_err();
                        assert!(matches!(
                            error,
                            SourceError::Binding("foreign original SSA owner")
                        ));
                        completed.set(true);
                        Err(error.into())
                    },
                ),
            );
            assert!(completed.get());
            assert!(matches!(
                error,
                Error::Source(SourceError::Binding("foreign original SSA owner"))
            ));
            report.foreign_owner_refused = true;

            let completed = Cell::new(false);
            let error = refused(
                transaction()?.with_original_source_bound_private_worklist_v21::<(), _>(
                    |_, handoff, _, _, _| {
                        let mut work =
                            fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                        let foreign = Budget::new(&mut work, 20_000_000);
                        let error = match handoff.output(&foreign) {
                            Err(error) => error,
                            Ok(_) => panic!("foreign ledger acquired worklist output"),
                        };
                        assert!(matches!(
                            error,
                            SourceError::Resource(ResourceError::Accounting)
                        ));
                        completed.set(true);
                        Err(error.into())
                    },
                ),
            );
            assert!(completed.get());
            assert!(matches!(
                error,
                Error::Source(SourceError::Resource(ResourceError::Accounting))
            ));
            report.foreign_ledger_refused = true;

            let completed = Cell::new(false);
            let error = refused(
                transaction()?.with_original_source_bound_private_worklist_v21::<(), _>(
                    |source, handoff, _, _, budget| {
                        handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                        assert_eq!(handoff.output(budget)?.execution().policy_version(), 9);
                        completed.set(true);
                        Err(Error::Unsupported(
                            "worklist private completion still requires final admission",
                        ))
                    },
                ),
            );
            assert!(completed.get());
            assert!(matches!(
                error,
                Error::Unsupported("worklist private completion still requires final admission")
            ));
            report.callback_error_preserved = true;
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn bound_worklist_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = WorklistCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("bound worklist callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("bound worklist result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual bound worklist: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src and authentic AMD source dependencies"]
fn actual_original_bound_worklist_preserves_private_memory_and_rewrites_cross_block_chain() {
    run_actual_sources::<Observation>(
        &[
            ("private_chain", "private_chain(seed);"),
            ("private_noop", "private_plain(seed);"),
        ],
        &[(0, 0)],
        CHILD,
        "BOUND_PRIVATE_WORKLIST_V21",
        program,
        |_, _, case, report, _| {
            assert_eq!(report.roots, 2);
            assert!(report.instances >= 4);
            assert_eq!(report.policy, 9);
            assert!(
                report.before.private_allocations >= 2
                    && report.before.private_loads >= 2
                    && report.before.private_stores >= 2
            );
            assert_eq!(
                report.after.private_allocations,
                report.before.private_allocations
            );
            assert_eq!(report.after.private_loads, report.before.private_loads);
            assert_eq!(report.after.private_stores, report.before.private_stores);
            assert_eq!(report.after.binary_operations, 0);
            match case {
                "private_chain" => {
                    assert!(report.before.binary_operations >= 3);
                    assert!(report.before.cross_block_binary_dependencies >= 2);
                    assert!(report.changed);
                    assert_ne!(report.original, report.output);
                }
                "private_noop" => {
                    assert_eq!(report.before.binary_operations, 0);
                    assert!(!report.changed);
                }
                _ => panic!("unexpected actual worklist case"),
            }
            assert!(report.unqualified && report.prepared);
            assert!(
                report.foreign_owner_refused
                    && report.foreign_ledger_refused
                    && report.callback_error_preserved
            );
        },
    );
}

#[test]
fn bound_worklist_actual_fixture_keeps_private_borrows_checked_chain_and_distinct_geometry() {
    // Retain the historical parent identity. The chain is checked against both
    // actual owners; its arithmetic is total XOR, not checked-add tuple syntax.
    let source = program("private_chain(seed);");
    assert_eq!(source.matches("#[kernel(typed,").count(), 2);
    assert_eq!(source.matches("_word: usize").count(), 2);
    assert!(source.contains("fn observe(value: &u32)"));
    assert_eq!(source.matches("observe(&seed);").count(), 5);
    assert!(source.contains("observe(&second);"));
    assert!(
        source
            .contains("let first = loaded ^ 0;\n    observe(&seed);\n    let second = first ^ 0;\n    observe(&seed);\n    let third = second ^ 0;\n    seed = third;")
    );
    assert!(source.contains("seed = second;\n    observe(&seed);"));
    assert!(source.contains("max_grid = [3, 1, 1]"));
    assert!(source.contains("max_grid = [5, 1, 1]"));
    assert!(!source.contains("unsafe"));
    assert!(CHILD.ends_with("::bound_worklist_tests::bound_worklist_child"));
}
