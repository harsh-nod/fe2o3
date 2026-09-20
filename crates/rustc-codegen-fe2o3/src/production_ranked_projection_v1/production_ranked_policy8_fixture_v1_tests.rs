// Modify only unadmitted source declarations; all source/SSA/ranked checks run.
fn policy8_bitwise_source_v1(functions: &mut [SemanticFunctionDeclV1], erased: bool) {
    for (ordinal, function) in functions.iter_mut().take(2).enumerate() {
        let scalar = function.locals()[if erased { 4 } else { 1 }].ty();
        let input = if erased { 4 } else { 1 };
        let mut locals = function.locals().to_vec();
        let base = locals.len() as u32;
        for offset in 0..6 {
            locals.push(local(
                210 + ordinal as u8 * 6 + offset,
                scalar,
                SemanticLocalRoleV1::Temporary,
            ));
        }
        let binary = |destination, operation, left, right| {
            typed_assignment(
                destination,
                scalar,
                SemanticRvalueKindV1::Binary {
                    operation,
                    left,
                    right,
                },
            )
        };
        let store = |slot, value| {
            statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                typed_place(slot, scalar),
                typed_operand(value, scalar),
                SemanticVolatilityV1::NonVolatile,
                None,
            )))
        };
        let mut blocks = function.blocks().to_vec();
        let block_index = usize::from(erased);
        let body = &blocks[block_index];
        let mut statements = body.statements().to_vec();
        // The UnitLocal fixture's existing global access keeps its exact site.
        let predicate = erased.then(|| statements.pop().unwrap());
        statements.extend([
            binary(
                base,
                SemanticBinaryOpV1::BitAnd,
                typed_operand(input, scalar),
                typed_constant(scalar, 13, 4),
            ),
            store(base + 4, base),
            binary(
                base + 1,
                SemanticBinaryOpV1::BitAnd,
                typed_constant(scalar, 13, 4),
                typed_operand(input, scalar),
            ),
            binary(
                base + 2,
                SemanticBinaryOpV1::BitOr,
                typed_operand(base, scalar),
                typed_operand(input, scalar),
            ),
            store(base + 5, base + 2),
            binary(
                base + 3,
                SemanticBinaryOpV1::BitOr,
                typed_operand(input, scalar),
                typed_operand(base + 1, scalar),
            ),
            store(base + 4, base + 1),
            store(base + 5, base + 3),
        ]);
        statements.extend(predicate);
        blocks[block_index] = SemanticBasicBlockV1::new(
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
}

pub(crate) fn with_backend_policy8_direct_prefix_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    mutation: bool,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy6V1,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_checked_ranked_bound_functions_v1(
        profile,
        Some(2),
        |functions| {
            if mutation {
                policy8_bitwise_source_v1(functions, false);
            }
        },
        |receipt, bound, ranked, budget| {
            let checked =
                fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, budget)
                    .unwrap();
            budget.reserve_storage(checked.retained_storage()).unwrap();
            let checked = fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(
                &bound, checked, budget,
            )
            .unwrap();
            let storage = checked.retained_storage();
            budget.reserve_storage(storage).unwrap();
            let floor = budget.storage();
            let owner =
                fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy6V1::try_admit_v1(
                    receipt, bound, checked, budget,
                )
                .unwrap();
            next(owner, ranked, budget);
            assert_eq!(budget.storage(), floor);
            budget.release_storage(storage).unwrap();
        },
    );
}

pub(crate) fn with_backend_policy8_erased_prefix_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    mutation: bool,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    canonical_assertion_graph_tests::with_backend_erased_functions_roster_v1(
        false,
        2,
        profile,
        true,
        true,
        |functions| {
            if mutation {
                policy8_bitwise_source_v1(functions, true);
            }
        },
        |source, bound, ranked, budget| {
            assert_eq!(source.deleted_call_count(), 2);
            let checked =
                fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, budget)
                    .unwrap();
            budget.reserve_storage(checked.retained_storage()).unwrap();
            let checked = fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(
                &bound, checked, budget,
            )
            .unwrap();
            let storage = checked.retained_storage();
            budget.reserve_storage(storage).unwrap();
            let floor = budget.storage();
            let owner = fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1::try_admit_v1(source, bound, checked, budget).unwrap();
            next(owner, ranked, budget);
            assert_eq!(budget.storage(), floor);
            budget.release_storage(storage).unwrap();
        },
    );
}

#[test]
fn policy8_identity_source_transform_preserves_exact_old_fixture_bytes_and_work() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let mut direct = None;
        with_backend_policy7_direct_prefix_v1(profile, true, |owner, ranked, budget| {
            direct = Some((
                owner.output().canonical().canonical_bytes().to_vec(),
                owner
                    .source_semantic_kir()
                    .pre_ranked_executable()
                    .unwrap()
                    .canonical()
                    .canonical_bytes()
                    .to_vec(),
                *ranked.canonical_roster_identity().as_bytes(),
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
            ));
        });
        with_backend_policy8_direct_prefix_v1(profile, false, |owner, ranked, budget| {
            assert_eq!(
                direct.unwrap(),
                (
                    owner.output().canonical().canonical_bytes().to_vec(),
                    owner
                        .source_semantic_kir()
                        .pre_ranked_executable()
                        .unwrap()
                        .canonical()
                        .canonical_bytes()
                        .to_vec(),
                    *ranked.canonical_roster_identity().as_bytes(),
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage()
                )
            );
        });
        let mut erased = None;
        with_backend_policy7_erased_prefix_v1(profile, true, |owner, ranked, budget| {
            erased = Some((
                owner.output().canonical().canonical_bytes().to_vec(),
                owner
                    .original_source()
                    .executable()
                    .canonical()
                    .canonical_bytes()
                    .to_vec(),
                *ranked.canonical_roster_identity().as_bytes(),
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
            ));
        });
        with_backend_policy8_erased_prefix_v1(profile, false, |owner, ranked, budget| {
            assert_eq!(
                erased.unwrap(),
                (
                    owner.output().canonical().canonical_bytes().to_vec(),
                    owner
                        .original_source()
                        .executable()
                        .canonical()
                        .canonical_bytes()
                        .to_vec(),
                    *ranked.canonical_roster_identity().as_bytes(),
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage()
                )
            );
        });
    }
}
