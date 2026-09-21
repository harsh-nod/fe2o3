// Constructed semantic source only. Never edit an admitted source or KIR graph.
fn native_preheader_loop_source_v1(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut [SemanticFunctionDeclV1],
) {
    let function = &functions[0];
    let selector = function.locals()[1].ty();
    let boolean = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
    types.push(canonical_assertion_graph_tests::assertion_types()[1].clone());
    let scalar = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
    types.push(canonical_assertion_graph_tests::assertion_types()[3].clone());
    let set = |value| {
        typed_assignment(
            2,
            scalar,
            SemanticRvalueKindV1::Use(typed_constant(scalar, value, 8)),
        )
    };
    let jump = |target| SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, target));
    let blocks = vec![
        block(201, vec![set(0)], zero_switch(1, selector, 1, 2)),
        block(202, vec![], jump(3)),
        block(203, vec![], jump(3)),
        block(
            204,
            vec![typed_assignment(
                3,
                boolean,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: typed_operand(2, scalar),
                    right: typed_constant(scalar, 3, 8),
                },
            )],
            zero_switch(3, boolean, 5, 4),
        ),
        block(
            205,
            vec![typed_assignment(
                2,
                scalar,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: typed_operand(2, scalar),
                    right: typed_constant(scalar, 1, 8),
                },
            )],
            jump(3),
        ),
        block(206, vec![], SemanticTerminatorKindV1::Return),
    ];
    let mut locals = function.locals().to_vec();
    locals[2] = local(208, scalar, SemanticLocalRoleV1::Temporary);
    locals.push(local(209, boolean, SemanticLocalRoleV1::Temporary));
    let replacement = SemanticFunctionDeclV1::new(
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
    .with_kernel_entry(function.kernel_entry().unwrap().clone());
    functions[0] = replacement;
}

pub(crate) fn with_backend_loop_preheaders_direct_prefix_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    mutation: bool,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy6V1,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_checked_ranked_bound_types_functions_v1(
        profile,
        None,
        |types, functions| {
            if mutation {
                native_preheader_loop_source_v1(types, functions);
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

#[test]
fn loop_preheaders_native_identity_source_fixture_keeps_existing_no_store_source_exact() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let mut original = None;
        with_backend_checked_output_policy6_roster_v1(profile, |owner, ranked, budget| {
            original = Some((
                owner
                    .source_semantic_kir()
                    .pre_ranked_executable()
                    .unwrap()
                    .canonical()
                    .canonical_bytes()
                    .to_vec(),
                owner.output().canonical().canonical_bytes().to_vec(),
                *ranked.canonical_roster_identity().as_bytes(),
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
            ));
        });
        with_backend_loop_preheaders_direct_prefix_v1(profile, false, |owner, ranked, budget| {
            assert_eq!(
                original.take().unwrap(),
                (
                    owner
                        .source_semantic_kir()
                        .pre_ranked_executable()
                        .unwrap()
                        .canonical()
                        .canonical_bytes()
                        .to_vec(),
                    owner.output().canonical().canonical_bytes().to_vec(),
                    *ranked.canonical_roster_identity().as_bytes(),
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage()
                )
            );
        });
    }
}
