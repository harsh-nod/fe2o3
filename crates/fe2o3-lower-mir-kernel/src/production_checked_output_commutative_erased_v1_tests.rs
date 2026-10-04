// Genuine constructed UnitLocal source/N/E, not ordinary Rust/native admission.
use super::*;
#[path = "production_checked_output_private_cell_erased_v1_tests.rs"]
mod private_cell_tests;

fn bitwise_fixture(roots: usize) -> Fixture6 {
    let fixture = erased_effect_fixture_mode_with_functions(
        true,
        roots,
        None,
        true,
        false,
        true,
        |functions| {
            for (ordinal, function) in functions.iter_mut().take(roots).enumerate() {
                let provenance = SemanticSourceProvenanceV1::unavailable();
                let mut locals = function.locals().to_vec();
                assert_eq!(locals.len(), 8);
                for offset in 0..6 {
                    locals.push(SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256(
                            [210 + (ordinal * 6 + offset) as u8; 32],
                        ),
                        ARRAY_SCALAR,
                        SemanticLocalRoleV1::Temporary,
                        provenance,
                    ));
                }
                let store = |slot, operand| {
                    SemanticStatementV1::new(
                        provenance,
                        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                            place(slot, ARRAY_SCALAR),
                            value(operand, ARRAY_SCALAR),
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    )
                };
                let binary = |destination, operation, left, right| {
                    assignment(
                        destination,
                        ARRAY_SCALAR,
                        SemanticRvalueKindV1::Binary {
                            operation,
                            left,
                            right,
                        },
                    )
                };
                let mut blocks = function.blocks().to_vec();
                let body = &blocks[1];
                let mut statements = body.statements().to_vec();
                let predicate = statements.pop().unwrap();
                // The existing global effect precedes these additions, preserving
                // the independently emitted ranked global-access source coordinate.
                statements.extend([
                    binary(
                        8,
                        SemanticBinaryOpV1::BitAnd,
                        value(4, ARRAY_SCALAR),
                        constant(ARRAY_SCALAR, 13, 4),
                    ),
                    store(12, 8),
                    binary(
                        9,
                        SemanticBinaryOpV1::BitAnd,
                        constant(ARRAY_SCALAR, 13, 4),
                        value(4, ARRAY_SCALAR),
                    ),
                    binary(
                        10,
                        SemanticBinaryOpV1::BitOr,
                        value(8, ARRAY_SCALAR),
                        value(4, ARRAY_SCALAR),
                    ),
                    store(13, 10),
                    binary(
                        11,
                        SemanticBinaryOpV1::BitOr,
                        value(4, ARRAY_SCALAR),
                        value(9, ARRAY_SCALAR),
                    ),
                    store(12, 9),
                    store(13, 11),
                    predicate,
                ]);
                blocks[1] = SemanticBasicBlockV1::new(
                    body.identity(),
                    body.source(),
                    statements,
                    body.terminator().clone(),
                )
                .unwrap();
                let entry = function.kernel_entry().unwrap().clone();
                *function = SemanticFunctionDeclV1::new(
                    function.identity(),
                    function.role(),
                    function.item_definition_identity(),
                    function.monomorphization_identity(),
                    function.generic_type_arguments_identity(),
                    function.const_generic_arguments_identity(),
                    function.source(),
                    function.abi().clone(),
                    locals,
                    function.entry(),
                    blocks,
                )
                .unwrap()
                .with_kernel_entry(entry);
            }
        },
    );
    let FinalFixture {
        source,
        bound,
        checked,
        bound_storage,
        ..
    } = final_fixture_from(fixture, None);
    drop(checked);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(FLOOR + source.retained_storage_floor_v1() + bound_storage)
        .unwrap();
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, &mut budget)
            .unwrap();
    budget.reserve_storage(checked.retained_storage()).unwrap();
    continue6(Fixture5 {
        source,
        bound,
        checked,
        floor: budget.storage(),
        bound_storage,
    })
}

fn prefix7(
    roots: usize,
    mutation: bool,
) -> (
    crate::ProductionOwnedUnitLocalRedundantStoreContinuationV1,
    usize,
) {
    let input = if mutation {
        bitwise_fixture(roots)
    } else {
        redundant_fixture(true, roots, None)
    };
    let floor = input.floor;
    let prefix = admit6(input, WORK, STORAGE).0.unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let (prefix, storage) = prefix
        .continue_redundant_private_stores_v1(&mut budget)
        .unwrap();
    (prefix, floor + storage.retained_storage())
}

#[test]
fn consuming_unit_local_preserves_distinct_original_n_e_prefix_once_and_fresh_k() {
    for roots in [1, 2] {
        for mutation in [false, true] {
            let (prefix, inherited) = prefix7(roots, mutation);
            let pointer = |owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12| {
                owner.canonical().canonical_bytes().as_ptr()
            };
            let original = pointer(prefix.prefix().original_source().executable());
            let erased = pointer(prefix.prefix().erased());
            let old_i = pointer(prefix.prefix().output());
            let actual_j = pointer(prefix.output());
            assert_ne!(
                prefix
                    .prefix()
                    .original_source()
                    .executable()
                    .canonical()
                    .canonical_bytes(),
                prefix.prefix().erased().canonical().canonical_bytes()
            );
            let minimum = prefix.retained_input_storage_floor_v1().unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            let floor = inherited + 71;
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let (mut owner, storage) = prefix
                .continue_commutative_bitwise_cse_v1(&mut budget)
                .unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_eq!(
                pointer(owner.prefix().prefix().original_source().executable()),
                original
            );
            assert_eq!(pointer(owner.prefix().prefix().erased()), erased);
            assert_eq!(pointer(owner.prefix().prefix().output()), old_i);
            assert_eq!(pointer(owner.prefix().output()), actual_j);
            assert_eq!(
                owner.retained_input_storage_floor_v1().unwrap(),
                minimum + storage.retained_storage()
            );
            assert_eq!(owner.continuation().execution().changed(), mutation);
            if mutation {
                assert!(owner.continuation().proved_pairs() >= roots * 2);
            }
            assert_eq!(
                owner.output().canonical().canonical_bytes()
                    != owner.prefix().output().canonical().canonical_bytes(),
                mutation
            );
            let counts = |module: &Module| {
                module
                    .functions
                    .iter()
                    .filter_map(|f| f.body.as_ref())
                    .flat_map(|b| &b.blocks)
                    .flat_map(|b| &b.operations)
                    .filter(|op| {
                        matches!(
                            op.kind,
                            OperationKind::Store { .. }
                                | OperationKind::Load { .. }
                                | OperationKind::Call { .. }
                        )
                    })
                    .count()
            };
            assert_eq!(
                counts(owner.output().module()),
                counts(owner.prefix().output().module())
            );
            assert_eq!(owner.kernels().len(), roots);
            owner.verify_equivalence(&mut budget).unwrap();
            if roots == 2 {
                owner.exercise_fresh_report_order_v1(&mut budget);
            }
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(!owner.grants_artifact_or_launch_authority());
            drop(owner);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn consuming_unit_local_exact_and_one_short_limits_and_unreserved_output() {
    let run = |work_limit, storage_limit| {
        let (prefix, inherited) = prefix7(1, true);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        let floor = inherited + 83;
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
    let (prefix, inherited) = prefix7(1, true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (owner, storage) = prefix
        .continue_commutative_bitwise_cse_v1(&mut budget)
        .unwrap();
    let mut short_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut short = ArgumentBudgetV1::new(&mut short_work, STORAGE);
    let minimum = owner.retained_input_storage_floor_v1().unwrap();
    short.reserve_storage(minimum - 1).unwrap();
    assert!(matches!(
        owner.verify_equivalence(&mut short),
        Err(crate::ProductionCommutativeContinuationErrorV1::Resource(
            AssertOriginResourceV1::Accounting
        ))
    ));
    assert_eq!(short.work(), 0);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    owner.verify_equivalence(&mut budget).unwrap();
}
