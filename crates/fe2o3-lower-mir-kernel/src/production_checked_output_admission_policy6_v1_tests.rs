// Child of genuine semantic-MIR/private-memory fixtures, not ordinary Rust.
use super::*;
type Final6 = crate::ProductionCheckedOutputOwnerPolicy6V1;
type Error6 = crate::ProductionCheckedOutputAdmissionErrorPolicy6V1;
type Checked6 = fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1;
#[path = "production_checked_output_redundant_store_direct_v1_tests.rs"]
mod redundant_store_tests;

#[test]
fn policy6_error_keeps_prefix_identity_without_growing_the_inline_result() {
    type Prefix = crate::ProductionCheckedOutputAdmissionErrorPolicy5V1;
    assert!(std::mem::size_of::<Error6>() <= std::mem::size_of::<Prefix>());
    let prefix = Box::new(Prefix::Admission(
        crate::ProductionCheckedOutputAdmissionErrorPolicy3V1::PrivateAddressR2,
    ));
    let expected = prefix.to_string();
    let pointer = prefix.as_ref() as *const Prefix;
    let error = Error6::Prefix(prefix);
    assert_eq!(error.to_string(), expected);
    let source = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<Prefix>()
        .unwrap();
    assert!(std::ptr::eq(source, pointer));
}

struct Fixture6 {
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    bound: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: Checked6,
    floor: usize,
    source_storage: usize,
    bound_storage: usize,
}
fn fixture6(profile: Profile) -> Fixture6 {
    fixture6_from_source(profile, retained_scalar_source_with_reads(3))
}

fn fixture6_from_source(profile: Profile, source: ProductionPreRankedKirOwnerV1) -> Fixture6 {
    let Prepared {
        receipt,
        bound,
        output,
        source_storage,
        bound_storage,
        ..
    } = prepare(array_output_ranked_receipt_v1(source), profile, None);
    drop(output);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let retained = FLOOR + source_storage + bound_storage;
    budget.reserve_storage(retained).unwrap();
    let prefix =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, &mut budget)
            .unwrap();
    let prefix_storage = prefix.retained_storage();
    let prefix_record = prefix.execution().canonical_bytes().to_vec();
    let old_output = prefix.owner().canonical().canonical_bytes().to_vec();
    budget.reserve_storage(prefix_storage).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let before = budget.work();
    let checked = fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(
        &bound,
        prefix,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), retained);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(budget.work() > before);
    assert_eq!(
        checked.intermediate_policy5().execution().canonical_bytes(),
        prefix_record.as_slice()
    );
    assert_eq!(
        checked
            .intermediate_policy5()
            .owner()
            .canonical()
            .canonical_bytes(),
        old_output
    );
    budget.reserve_storage(checked.retained_storage()).unwrap();
    Fixture6 {
        receipt,
        bound,
        checked,
        floor: budget.storage(),
        source_storage,
        bound_storage,
    }
}
fn admit6(input: Fixture6, work: usize, storage: usize) -> (Result<Final6, Error6>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
    budget.reserve_storage(input.floor).unwrap();
    let result = Final6::try_admit_v1(input.receipt, input.bound, input.checked, &mut budget);
    assert_eq!(budget.storage(), input.floor);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn direct_policy6_retains_actual_prefix_and_fresh_i_source_admission_both_targets() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let input = fixture6(profile);
        let original = input.receipt.materialized.executable().module().clone();
        let final_i = input.checked.owner().canonical().canonical_bytes().to_vec();
        let expected_retained = input.source_storage + input.checked.retained_storage();
        assert_eq!(input.floor, FLOOR + expected_retained + input.bound_storage);
        let floor = input.floor;
        let owner = admit6(input, WORK, STORAGE).0.unwrap();
        assert_eq!(owner.source_semantic_kir().module(), &original);
        assert_eq!(owner.output().canonical().canonical_bytes(), final_i);
        assert!(std::ptr::eq(owner.output(), owner.checked_output().owner()));
        assert_eq!(owner.checked_output().execution().policy_version(), 6);
        assert_eq!(
            owner
                .checked_output()
                .intermediate_policy5()
                .execution()
                .policy_version(),
            5
        );
        assert_eq!(
            private_counts(owner.source_semantic_kir().module()),
            (1, 1, 3)
        );
        assert_eq!(private_counts(owner.output().module()), (1, 1, 0));
        assert_eq!(
            owner.retained_input_storage_floor_v1().unwrap(),
            expected_retained
        );
        assert!(!owner.grants_artifact_or_launch_authority());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        owner.verify_equivalence(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn direct_policy6_exact_final_admission_budgets_preserve_the_inherited_floor() {
    let input = fixture6(Profile::Gfx942);
    let floor = input.floor;
    let (owner, work, peak) = admit6(input, WORK, STORAGE);
    drop(owner.unwrap());
    assert!(peak > floor);
    for (work, storage, expected) in [
        (work, peak, true),
        (work - 1, peak, false),
        (work, peak - 1, false),
    ] {
        let input = fixture6(Profile::Gfx942);
        assert_eq!(input.floor, floor);
        assert_eq!(admit6(input, work, storage).0.is_ok(), expected);
    }
}

fn integer_identity_before_assert_source() -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = fixture_with_blocks_and_symbol(
        Fixture::ElidedBounds,
        false,
        |_, _| {
            vec![
                block(
                    31,
                    vec![
                        assignment(
                            2,
                            U64,
                            SemanticRvalueKindV1::Unary {
                                operation: SemanticUnaryOpV1::PointerMetadata,
                                operand: value(1, SLICE_REF),
                            },
                        ),
                        assignment(
                            5,
                            U32,
                            SemanticRvalueKindV1::Cast {
                                kind: SemanticCastKindV1::Integer,
                                operand: value(2, U64),
                            },
                        ),
                        assignment(
                            6,
                            U32,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::BitXor,
                                left: value(5, U32),
                                right: constant(U32, 0, 4),
                            },
                        ),
                        SemanticStatementV1::new(
                            SemanticSourceProvenanceV1::unavailable(),
                            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                                place(7, U32),
                                value(6, U32),
                                SemanticVolatilityV1::NonVolatile,
                                None,
                            )),
                        ),
                        assignment(3, U64, SemanticRvalueKindV1::Use(constant(U64, 0, 8))),
                        assignment(
                            4,
                            BOOL,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: value(3, U64),
                                right: value(2, U64),
                            },
                        ),
                    ],
                    SemanticTerminatorKindV1::Assert {
                        condition: value(4, BOOL),
                        expected: true,
                        message: SemanticAssertMessageV1::BoundsCheck {
                            length: value(2, U64),
                            index: value(3, U64),
                        },
                        target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(32, vec![], SemanticTerminatorKindV1::Return),
            ]
        },
        |_| "private_array_relation".to_owned(),
        &[U32, U32, U32],
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

#[test]
fn direct_policy6_genuine_identity_removal_moves_trap_inventory_ordinal_not_source_origin() {
    use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    use fe2o3_kernel_ir::CanonicalKirOperationOriginV1;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let input = fixture6_from_source(profile, integer_identity_before_assert_source());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(input.floor).unwrap();
        {
            let (old, storage) = CanonicalKirInventoryV1::derive(
                input.checked.intermediate_policy5().owner(),
                &mut budget,
            )
            .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let (new, storage) =
                CanonicalKirInventoryV1::derive(input.checked.owner(), &mut budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let is_identity = |operation: &fe2o3_kernel_ir::Operation| {
                matches!(
                    operation.kind,
                    OperationKind::Binary {
                        op: fe2o3_kernel_ir::BinaryOp::BitXor,
                        ..
                    }
                )
            };
            assert_eq!(
                old.operations()
                    .iter()
                    .filter(|row| is_identity(row.operation))
                    .count(),
                1
            );
            assert_eq!(
                new.operations()
                    .iter()
                    .filter(|row| is_identity(row.operation))
                    .count(),
                0
            );
            assert_ne!(
                input
                    .checked
                    .intermediate_policy5()
                    .owner()
                    .canonical()
                    .canonical_bytes(),
                input.checked.owner().canonical().canonical_bytes()
            );
            let trap = fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap.operation(None);
            let traps: Vec<_> = new
                .operations()
                .iter()
                .enumerate()
                .filter(|(_, row)| row.operation == &trap)
                .collect();
            assert_eq!(traps.len(), 1, "genuine bounds-assert trap must survive");
            let (ordinal, actual) = traps[0];
            let row = &input
                .checked
                .continuation()
                .occurrences()
                .candidate()
                .operations[ordinal];
            assert_eq!(row.output, actual.coordinate);
            let CanonicalKirOperationOriginV1::Retained(origin) = row.origin else {
                panic!("trap must have a retained qualified O origin");
            };
            let old_ordinal = old
                .operations()
                .iter()
                .position(|row| row.coordinate == origin)
                .unwrap();
            assert_eq!(old.operations()[old_ordinal].operation, actual.operation);
            // The synthetic one-op trap block stays unchanged. Earlier DCE
            // changes its flattened ordinal, so joining by that ordinal is wrong.
            assert_ne!(old_ordinal, ordinal);
            assert_ne!(old.operations()[ordinal].operation, actual.operation);
        }
        budget
            .release_storage(budget.storage() - input.floor)
            .unwrap();
        let floor = input.floor;
        let owner = admit6(input, WORK, STORAGE).0.unwrap();
        owner.verify_equivalence(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(private_counts(owner.output().module()), (1, 1, 0));
    }
}

#[path = "production_checked_output_local_order_direct_v1_tests.rs"]
mod local_order_tests;
