use super::*;

#[path = "production_checked_output_admission_policy5_v1_tests.rs"]
mod policy5_tests;
#[path = "production_checked_output_admission_policy6_v1_tests.rs"]
mod policy6_tests;

fn retained_scalar_source() -> ProductionPreRankedKirOwnerV1 {
    retained_scalar_source_with_reads(1)
}

fn retained_scalar_source_with_reads(reads: usize) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = fixture_with_blocks_and_symbol(
        Fixture::ElidedBounds,
        false,
        |_, _| {
            let slot = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(5), vec![], U32).unwrap();
            let mut statements = vec![SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    slot.clone(),
                    constant(U32, 99, 4),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            )];
            if reads > 1 {
                statements.insert(
                    0,
                    SemanticStatementV1::new(
                        SemanticSourceProvenanceV1::unavailable(),
                        SemanticStatementKindV1::StorageLive(slot.local()),
                    ),
                );
            }
            for _ in 0..reads {
                statements.push(assignment(
                    6,
                    U32,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(slot.clone())),
                ));
            }
            if reads > 1 {
                statements.push(SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::StorageDead(slot.local()),
                ));
            }
            vec![block(31, statements, SemanticTerminatorKindV1::Return)]
        },
        |_| "private_array_relation".to_owned(),
        &[U32, U32],
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
fn general_policy3_private_repeated_source_reads_reuse_the_lifetime_index() {
    let receipt = array_output_ranked_receipt_v1(retained_scalar_source_with_reads(8));
    assert_eq!(
        private_counts(receipt.materialized.executable().module()),
        (1, 1, 8)
    );
    with_prepared(prepare(receipt, Profile::Gfx942, None), |input, budget| {
        let floor = budget.storage();
        let owner =
            AdmittedOutput::try_admit_general_v1(input.receipt, input.bound, input.output, budget)
                .unwrap();
        assert_eq!(private_counts(owner.output().module()), (1, 1, 8));
        owner.verify_equivalence(budget).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

fn private_counts(module: &Module) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            for operation in &block.operations {
                match operation.kind {
                    OperationKind::Alloca {
                        address_space: AddressSpace::Private,
                        ..
                    } => counts.0 += 1,
                    OperationKind::Store { access, .. }
                        if access.address_space == AddressSpace::Private =>
                    {
                        counts.1 += 1
                    }
                    OperationKind::Load { access, .. }
                        if access.address_space == AddressSpace::Private =>
                    {
                        counts.2 += 1
                    }
                    _ => {}
                }
            }
        }
    }
    counts
}

#[test]
fn general_policy3_private_array_read_write_uses_genuine_source_and_actual_output() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let receipt = array_output_ranked_receipt_v1(array_owner(ArrayCase::RetainedValueRead));
        assert_eq!(
            private_counts(receipt.materialized.executable().module()),
            (1, 9, 1)
        );
        with_prepared(prepare(receipt, profile, None), |input, budget| {
            let identity = *input.output.owner().canonical().identity();
            let owner = AdmittedOutput::try_admit_general_v1(
                input.receipt,
                input.bound,
                input.output,
                budget,
            )
            .unwrap();
            assert_eq!(*owner.output().canonical().identity(), identity);
            assert_eq!(private_counts(owner.output().module()), (1, 9, 1));
            assert!(
                owner.kernels()[0].accesses().is_empty(),
                "formal report excludes Private, independently checked by R2"
            );
            owner.verify_equivalence(budget).unwrap();
            assert!(!owner.grants_artifact_or_launch_authority());
        });
    }
}

#[test]
fn general_policy3_private_scalar_slot_keeps_exact_store_load_pointer() {
    let source = retained_scalar_source();
    assert_eq!(private_counts(source.executable().module()), (1, 1, 1));
    with_prepared(
        prepare(
            array_output_ranked_receipt_v1(source),
            Profile::Gfx942,
            None,
        ),
        |input, budget| {
            let owner = AdmittedOutput::try_admit_general_v1(
                input.receipt,
                input.bound,
                input.output,
                budget,
            )
            .unwrap();
            assert_eq!(private_counts(owner.output().module()), (1, 1, 1));
            let mut store = None;
            let mut load = None;
            for operation in owner.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
            {
                match operation.kind {
                    OperationKind::Store { pointer, .. } => store = Some(pointer),
                    OperationKind::Load { pointer, .. } => load = Some(pointer),
                    _ => {}
                }
            }
            assert!(
                store.is_some() && store == load,
                "genuine source candidate for a nonzero exact-pointer forwarding row"
            );
            owner.verify_equivalence(budget).unwrap();
        },
    );
}

#[test]
fn general_policy4_private_source_has_a_real_nonzero_forwarding_row_and_fresh_o() {
    let Prepared {
        receipt,
        bound,
        output,
        source_storage,
        bound_storage,
        ..
    } = prepare(
        array_output_ranked_receipt_v1(retained_scalar_source()),
        Profile::Gfx942,
        None,
    );
    drop(output);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(FLOOR + source_storage + bound_storage)
        .unwrap();
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, &mut budget)
            .unwrap();
    assert_eq!(
        checked.forwarding_rows().len(),
        1,
        "actual source slot, not a no-op policy receipt"
    );
    assert_eq!(
        private_counts(checked.intermediate_policy3().owner().module()),
        (1, 1, 1)
    );
    assert_eq!(private_counts(checked.owner().module()), (1, 1, 0));
    let output_identity = *checked.owner().canonical().identity();
    let retained = checked.retained_storage();
    budget.reserve_storage(retained).unwrap();
    let floor = budget.storage();
    let owner = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
        receipt,
        bound,
        checked,
        &mut budget,
    )
    .unwrap();
    assert_eq!(*owner.output().canonical().identity(), output_identity);
    assert_eq!(private_counts(owner.output().module()), (1, 1, 0));
    assert!(owner.kernels()[0].accesses().is_empty());
    owner.verify_equivalence(&mut budget).unwrap();
    assert!(!owner.grants_artifact_or_launch_authority());
    assert_eq!(budget.storage(), floor);
}

#[test]
fn general_policy3_private_source_store_value_substitution_is_not_admitted() {
    let input = prepare(
        array_output_ranked_receipt_v1(retained_scalar_source()),
        Profile::Gfx942,
        Some(|module| {
            for operation in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
                if let OperationKind::Constant(Constant::U32(value)) = &mut operation.kind {
                    if *value == 99 {
                        *value = 98;
                        return;
                    }
                }
            }
            panic!("actual stored scalar constant");
        }),
    );
    with_prepared(input, |input, budget| {
        assert!(matches!(
            AdmittedOutput::try_admit_general_v1(input.receipt, input.bound, input.output, budget),
            Err(AdmissionError::Coordinates(_))
        ));
    });
}

#[test]
fn general_policy3_private_budget_failure_restores_transferred_input_floor() {
    let input = prepare(
        array_output_ranked_receipt_v1(array_owner(ArrayCase::RetainedValueRead)),
        Profile::Gfx942,
        None,
    );
    let floor = FLOOR + input.source_storage + input.bound_storage + input.output_storage;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(11);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let result =
        AdmittedOutput::try_admit_general_v1(input.receipt, input.bound, input.output, &mut budget);
    // Source attachment now shares this ledger and exhausts it before the
    // later coordinate and private-memory checks can run.
    assert!(
        matches!(
            result,
            Err(AdmissionError::Source(
                crate::ProductionSemanticKirErrorV1::MirPlironTranslation(
                    crate::ProductionMirPlironTranslationErrorV1::ResourceLimit
                )
            ))
        ),
        "expected the exact caller-budget refusal during source attachment: {result:?}"
    );
    assert_eq!(budget.storage(), floor);
}
