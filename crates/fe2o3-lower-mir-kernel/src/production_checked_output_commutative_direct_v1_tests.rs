// Constructed genuine source through SSA/N/the complete prefix, not Rustc or native evidence.
use super::*;
#[path = "production_checked_output_private_cell_direct_v1_tests.rs"]
mod private_cell_tests;
use crate::{
    ProductionCommutativeContinuationErrorV1 as CError,
    ProductionOwnedRedundantStoreContinuationV1 as Prefix7,
};

fn bitwise_source(trap: bool) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = fixture_with_blocks_and_symbol(
        Fixture::ElidedBounds,
        false,
        |_, _| {
            let store = |slot, operand| {
                SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        place(slot, U64),
                        value(operand, U64),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                )
            };
            let binary = |destination, op, left, right| {
                assignment(
                    destination,
                    U64,
                    SemanticRvalueKindV1::Binary {
                        operation: op,
                        left,
                        right,
                    },
                )
            };
            let mut statements = vec![
                assignment(
                    2,
                    U64,
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: value(1, SLICE_REF),
                    },
                ),
                assignment(
                    6,
                    U64,
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: value(2, U64),
                    },
                ),
                binary(
                    7,
                    SemanticBinaryOpV1::BitAnd,
                    value(6, U64),
                    constant(U64, 13, 8),
                ),
                store(5, 7),
                binary(
                    8,
                    SemanticBinaryOpV1::BitAnd,
                    constant(U64, 13, 8),
                    value(6, U64),
                ),
                binary(9, SemanticBinaryOpV1::BitOr, value(7, U64), value(6, U64)),
                store(11, 9),
                binary(10, SemanticBinaryOpV1::BitOr, value(6, U64), value(8, U64)),
                store(5, 8),
                store(11, 10),
            ];
            if trap {
                statements.push(assignment(
                    4,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: value(8, U64),
                        right: constant(U64, 8, 8),
                    },
                ));
                vec![
                    block(
                        31,
                        statements,
                        SemanticTerminatorKindV1::Assert {
                            condition: value(4, BOOL),
                            expected: true,
                            message: SemanticAssertMessageV1::BoundsCheck {
                                length: constant(U64, 8, 8),
                                index: value(8, U64),
                            },
                            target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                            unwind: SemanticUnwindActionV1::Unreachable,
                        },
                    ),
                    block(32, vec![], SemanticTerminatorKindV1::Return),
                ]
            } else {
                vec![block(31, statements, SemanticTerminatorKindV1::Return)]
            }
        },
        |_| "private_array_relation".to_owned(),
        &[U64; 7],
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

fn prefix7(profile: Profile, mutation: bool, trap: bool) -> (Prefix7, usize) {
    let input = fixture6_from_source(
        profile,
        if mutation {
            bitwise_source(trap)
        } else {
            source(None, 3)
        },
    );
    let floor = input.floor;
    let prefix = admit6(input, WORK, STORAGE).0.unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let (prefix, added) = prefix
        .continue_redundant_private_stores_v1(&mut budget)
        .unwrap();
    (prefix, floor + added.retained_storage())
}

fn operations(module: &Module) -> impl Iterator<Item = &fe2o3_kernel_ir::Operation> {
    module
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|b| &b.blocks)
        .flat_map(|b| &b.operations)
}

#[test]
fn constructed_bitwise_fixture_materializes_exact_index_to_u64_bridge_and_typed_stores() {
    use fe2o3_kernel_ir::{BinaryOp, CastKind, ScalarType, Type};
    for trap in [false, true] {
        let source = bitwise_source(trap);
        let module = source.executable().module();
        let bridges = operations(module)
            .filter_map(|op| match &op.kind {
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value,
                    to,
                } if *to == Type::Scalar(ScalarType::U64) => Some((op, *value)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(bridge, input)] = bridges.as_slice() else {
            panic!("requires one explicit fixed-U64 representation bridge");
        };
        assert!(operations(module).any(|op| matches!(op.kind, OperationKind::SliceLength { .. })
            && matches!(op.results.as_slice(), [result] if result.id == *input && result.ty == Type::INDEX)));
        assert!(
            matches!(bridge.results.as_slice(), [result] if result.ty == Type::Scalar(ScalarType::U64))
        );
        let mut bitwise = 0;
        let mut stores = 0;
        for op in operations(module) {
            if matches!(
                op.kind,
                OperationKind::Binary {
                    op: BinaryOp::BitAnd | BinaryOp::BitOr,
                    ..
                }
            ) {
                bitwise += 1;
                assert!(
                    matches!(op.results.as_slice(), [result] if result.ty == Type::Scalar(ScalarType::U64))
                );
            }
            if let OperationKind::Store { value, .. } = op.kind {
                stores += 1;
                assert!(operations(module).any(|producer|
                    matches!(producer.results.as_slice(), [result] if result.id == value && result.ty == Type::Scalar(ScalarType::U64))));
            }
        }
        assert_eq!((bitwise, stores), (4, 4));
    }
}

#[test]
fn consuming_direct_actual_j_nested_mutation_and_noop_preserve_original_allocations_both_targets() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let (prefix, inherited) = prefix7(profile, mutation, false);
            let pointers = (
                prefix
                    .prefix()
                    .source_semantic_kir()
                    .pre_ranked_executable()
                    .unwrap()
                    .canonical()
                    .canonical_bytes()
                    .as_ptr(),
                prefix
                    .prefix()
                    .bound()
                    .canonical()
                    .canonical_bytes()
                    .as_ptr(),
                prefix
                    .prefix()
                    .output()
                    .canonical()
                    .canonical_bytes()
                    .as_ptr(),
                prefix.output().canonical().canonical_bytes().as_ptr(),
            );
            let minimum = prefix.retained_input_storage_floor_v1().unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            let floor = inherited + 137;
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let (owner, storage) = prefix
                .continue_commutative_bitwise_cse_v1(&mut budget)
                .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                storage.retained_storage(),
                owner.additional_retained_storage_v1()
            );
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_eq!(
                owner.retained_input_storage_floor_v1().unwrap(),
                minimum + storage.retained_storage()
            );
            let retained = owner.prefix();
            assert_eq!(
                pointers,
                (
                    retained
                        .prefix()
                        .source_semantic_kir()
                        .pre_ranked_executable()
                        .unwrap()
                        .canonical()
                        .canonical_bytes()
                        .as_ptr(),
                    retained
                        .prefix()
                        .bound()
                        .canonical()
                        .canonical_bytes()
                        .as_ptr(),
                    retained
                        .prefix()
                        .output()
                        .canonical()
                        .canonical_bytes()
                        .as_ptr(),
                    retained.output().canonical().canonical_bytes().as_ptr(),
                )
            );
            assert_eq!(owner.continuation().execution().changed(), mutation);
            assert_eq!(
                owner.output().canonical().canonical_bytes()
                    != retained.output().canonical().canonical_bytes(),
                mutation
            );
            if mutation {
                assert!(owner.continuation().proved_pairs() >= 2);
            }
            assert_eq!(
                private_counts(owner.output().module()),
                private_counts(retained.output().module())
            );
            assert_eq!(owner.kernels().len(), owner.output().module().kernels.len());
            assert!(!owner.grants_artifact_or_launch_authority());
            owner.verify_equivalence(&mut budget).unwrap();
            assert!(budget.work_ledger_identity_v1() == ledger);
            drop(owner);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn source_authorized_trap_is_retained_while_its_predicate_operand_is_substituted() {
    let (prefix, inherited) = prefix7(Profile::Gfx942, true, true);
    let trap_count = |module: &Module| {
        operations(module).filter(|op| matches!(&op.kind,
        OperationKind::Call { callee, arguments } if matches!(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments), Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap)))).count()
    };
    assert!(
        trap_count(prefix.output().module()) > 0,
        "requires an actual retained source assertion trap"
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (owner, storage) = prefix
        .continue_commutative_bitwise_cse_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(
        trap_count(owner.output().module()),
        trap_count(owner.prefix().output().module())
    );
    let (before, bs) = fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(
        owner.prefix().output(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(bs.retained_storage()).unwrap();
    let (after, os) =
        fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(owner.output(), &mut budget)
            .unwrap();
    budget.reserve_storage(os.retained_storage()).unwrap();
    let rows = owner.continuation().occurrences().candidate();
    let mut changed_predicates = 0;
    for (row, output) in rows.operations.iter().zip(after.operations()) {
        if !matches!(output.operation.kind, OperationKind::Compare { .. }) {
            continue;
        }
        let fe2o3_kernel_ir::CanonicalKirOperationOriginV1::Retained(origin) = row.origin else {
            panic!("retained predicate");
        };
        let input = before
            .operations()
            .iter()
            .find(|op| op.coordinate == origin)
            .unwrap();
        changed_predicates += usize::from(input.operation != output.operation);
    }
    assert!(
        changed_predicates > 0,
        "the source trap predicate must actually consume a substituted bitwise definition"
    );
    owner.verify_equivalence(&mut budget).unwrap();

    // A still-well-formed reversed trap branch is not the admitted substitution.
    let mut hostile = owner.output().module().clone();
    let branch = hostile
        .functions
        .iter_mut()
        .filter_map(|f| f.body.as_mut())
        .flat_map(|b| &mut b.blocks)
        .find_map(|b| match b.terminator.as_mut() {
            Some(fe2o3_kernel_ir::Terminator::ConditionalBranch {
                then_target,
                then_arguments,
                else_target,
                else_arguments,
                ..
            }) => Some((then_target, then_arguments, else_target, else_arguments)),
            _ => None,
        })
        .unwrap();
    std::mem::swap(branch.0, branch.2);
    std::mem::swap(branch.1, branch.3);
    let (hostile, hs) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&hostile, &mut budget).unwrap();
    budget.reserve_storage(hs.retained_storage()).unwrap();
    let (hostile_inventory, is) =
        fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&hostile, &mut budget).unwrap();
    budget.reserve_storage(is.retained_storage()).unwrap();
    assert!(matches!(
        fe2o3_kernel_analysis::check_canonical_kir_commutative_bitwise_cse_v1(
            &before,
            &hostile_inventory,
            rows,
            &mut budget,
        ),
        Err(
            fe2o3_kernel_analysis::CanonicalKirCommutativeBitwiseCseErrorV1::Rule(
                "exact CFG successor occurrence"
            )
        )
    ));
}

#[test]
fn consuming_direct_exact_and_one_short_construction_and_replay() {
    let run = |work_limit, storage_limit| {
        let (prefix, inherited) = prefix7(Profile::Gfx942, true, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        let floor = inherited + 97;
        budget.reserve_storage(floor).unwrap();
        let result = prefix.continue_commutative_bitwise_cse_v1(&mut budget);
        let ok = result.is_ok();
        drop(result);
        assert_eq!(budget.storage(), floor);
        (ok, budget.work(), budget.peak_storage())
    };
    let (ok, work, peak) = run(WORK, STORAGE);
    assert!(ok && run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
    assert!(!run(0, peak).0);
    let (prefix, inherited) = prefix7(Profile::Gfx942, true, false);
    let mut initial_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut initial = AssertOriginBudgetV1::new(&mut initial_work, STORAGE);
    initial.reserve_storage(inherited).unwrap();
    let (owner, storage) = prefix
        .continue_commutative_bitwise_cse_v1(&mut initial)
        .unwrap();
    let floor = inherited + storage.retained_storage() + 43;
    let replay = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = owner.verify_equivalence(&mut budget);
        assert_eq!(budget.storage(), floor);
        (result.is_ok(), budget.work(), budget.peak_storage())
    };
    let (ok, work, peak) = replay(WORK, STORAGE);
    assert!(ok && replay(work, peak).0);
    assert!(!replay(work - 1, peak).0);
    assert!(!replay(work, peak - 1).0);
    let minimum = owner.retained_input_storage_floor_v1().unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(minimum - 1).unwrap();
    assert!(matches!(
        owner.verify_equivalence(&mut budget),
        Err(CError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), minimum - 1);
}
