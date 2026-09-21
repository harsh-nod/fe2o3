// Transform only unadmitted semantic declarations; every ordinary source gate runs.
fn native_licm_scalar_type_v1(tag: u8, boolean: bool) -> SemanticTypeDeclV1 {
    let size = if boolean { 1 } else { 8 };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            size,
            neutral_scalar_backend_v1(
                SemanticBackendPrimitiveV1::integer(false, (size * 8) as u16, size),
                if boolean { 1 } else { u64::MAX.into() },
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(if boolean {
            SemanticScalarTypeV1::Bool
        } else {
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }
        }),
    )
}

fn native_licm_source_v1(
    functions: &mut [SemanticFunctionDeclV1],
    boolean: SemanticTypeIdV1,
    wide: SemanticTypeIdV1,
    erased: bool,
) {
    for (ordinal, function) in functions.iter_mut().take(2).enumerate() {
        let scalar = function.locals()[if erased { 4 } else { 1 }].ty();
        let slot = if erased { 3 } else { 2 };
        let mut locals = function.locals().to_vec();
        let base = locals.len() as u32;
        let input = if erased { base } else { 1 };
        if erased {
            locals.push(local(200, scalar, SemanticLocalRoleV1::Argument(1)));
        }
        let bound = locals.len() as u32;
        locals.push(local(
            209,
            wide,
            SemanticLocalRoleV1::Argument(if erased { 2 } else { 1 }),
        ));
        let counter = locals.len() as u32;
        for (offset, ty) in [wide, boolean, scalar, boolean, scalar]
            .into_iter()
            .enumerate()
        {
            locals.push(local(
                210 + offset as u8,
                ty,
                SemanticLocalRoleV1::Temporary,
            ));
        }
        let predicate = counter + 1;
        let masked = counter + 2;
        let even = counter + 3;
        let read = counter + 4;
        let abi = {
            let old = function.abi();
            let mut arguments = old.arguments().to_vec();
            let mut ownership = old.source_argument_ownership().to_vec();
            if erased {
                arguments.push(SemanticAbiArgumentV1::source(
                    neutral_plain_direct_abi_value_v1(scalar),
                ));
                ownership.push(SemanticSourceArgumentOwnershipV1::ByValue);
            }
            arguments.push(SemanticAbiArgumentV1::source(
                neutral_plain_direct_abi_value_v1(wide),
            ));
            ownership.push(SemanticSourceArgumentOwnershipV1::ByValue);
            SemanticFunctionAbiV1::from_rustc(
                old.identity(),
                old.layout_identity(),
                old.canon_abi(),
                old.extern_abi(),
                old.c_variadic(),
                old.can_unwind(),
                old.fixed_count() + if erased { 2 } else { 1 },
                arguments,
                old.return_value().clone(),
            )
            .unwrap()
            .with_source_argument_ownership(ownership)
            .unwrap()
        };
        let binary = |destination, ty, operation, left, right| {
            typed_assignment(
                destination,
                ty,
                SemanticRvalueKindV1::Binary {
                    operation,
                    left,
                    right,
                },
            )
        };
        let jump =
            |target| SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, target));
        let condition = |local, yes, no| SemanticTerminatorKindV1::SwitchInt {
            discriminant: typed_operand(local, boolean),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, no),
                )],
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, yes),
            )
            .unwrap(),
        };
        let mut blocks = if erased {
            function.blocks()[..2].to_vec()
        } else {
            Vec::new()
        };
        let start = blocks.len() as u32;
        let tag = 201 + ordinal as u8 * 20;
        let mut entry = if erased {
            function.blocks()[2].statements().to_vec()
        } else {
            Vec::new()
        };
        let global = if erased {
            let SemanticStatementKindV1::Store(store) = entry[0].kind() else {
                panic!("existing genuine global output Store")
            };
            Some(store.destination().clone())
        } else {
            None
        };
        entry.push(typed_assignment(
            counter,
            wide,
            SemanticRvalueKindV1::Use(typed_constant(wide, 0, 8)),
        ));
        blocks.push(block(
            tag,
            entry,
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: typed_operand(input, scalar),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, start + 1),
                    )],
                    cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, start + 2),
                )
                .unwrap(),
            },
        ));
        blocks.push(block(tag + 1, vec![], jump(start + 3)));
        blocks.push(block(tag + 2, vec![], jump(start + 3)));
        blocks.push(block(
            tag + 3,
            vec![binary(
                predicate,
                boolean,
                SemanticBinaryOpV1::LessThan,
                typed_operand(counter, wide),
                typed_operand(bound, wide),
            )],
            condition(predicate, start + 4, start + 7),
        ));
        let mut body = vec![
            binary(
                masked,
                scalar,
                SemanticBinaryOpV1::BitAnd,
                typed_operand(input, scalar),
                typed_constant(scalar, 1, 4),
            ),
            binary(
                even,
                boolean,
                SemanticBinaryOpV1::Equal,
                typed_operand(masked, scalar),
                typed_constant(scalar, 0, 4),
            ),
            statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                typed_place(slot, scalar),
                typed_constant(scalar, 99, 4),
                SemanticVolatilityV1::NonVolatile,
                None,
            ))),
        ];
        if let Some(destination) = global {
            body.push(statement(SemanticStatementKindV1::Store(
                SemanticMemoryStoreV1::new(
                    destination,
                    typed_operand(masked, scalar),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                ),
            )));
        }
        blocks.push(block(tag + 4, body, condition(even, start + 5, start + 6)));
        blocks.push(block(
            tag + 5,
            vec![typed_assignment(
                read,
                scalar,
                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                    typed_place(slot, scalar),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            )],
            jump(start + 6),
        ));
        blocks.push(block(
            tag + 6,
            vec![binary(
                counter,
                wide,
                SemanticBinaryOpV1::Add,
                typed_operand(counter, wide),
                typed_constant(wide, 1, 8),
            )],
            jump(start + 3),
        ));
        blocks.push(block(tag + 7, vec![], SemanticTerminatorKindV1::Return));
        let rebuilt = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            abi,
            locals,
            function.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(function.kernel_entry().unwrap().clone());
        *function = rebuilt;
    }
}

pub(crate) fn with_backend_licm_direct_prefix_v1(
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
                let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
                let wide = SemanticTypeIdV1::from_index(types.len() as u32 + 1);
                types.push(native_licm_scalar_type_v1(239, true));
                types.push(native_licm_scalar_type_v1(240, false));
                native_licm_source_v1(functions, boolean, wide, false);
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

pub(crate) fn with_backend_licm_erased_prefix_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    mutation: bool,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_licm_erased_prefix_order_v1(profile, mutation, false, next);
}

pub(crate) fn with_backend_forwarding_erased_prefix_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    mutation: bool,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_licm_erased_prefix_order_v1(profile, mutation, true, next);
}

fn with_backend_licm_erased_prefix_order_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    mutation: bool,
    global_before_private: bool,
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
        false,
        true,
        |functions| {
            if mutation {
                let boolean = functions[0].locals()[5].ty();
                let wide = functions[0].locals()[6].ty();
                native_licm_source_v1(functions, boolean, wide, true);
                if global_before_private {
                    for function in functions.iter_mut().take(2) {
                        let mut blocks = function.blocks().to_vec();
                        let body = &blocks[6];
                        let mut statements = body.statements().to_vec();
                        assert_eq!(statements.len(), 4);
                        assert!(matches!(
                            statements[2].kind(),
                            SemanticStatementKindV1::Store(_)
                        ));
                        assert!(matches!(
                            statements[3].kind(),
                            SemanticStatementKindV1::Store(_)
                        ));
                        // Change only these unadmitted source occurrences. The
                        // old wrapper never takes this branch; all admission,
                        // source lifetime, erasure and ranked gates below rerun.
                        statements.swap(2, 3);
                        blocks[6] = SemanticBasicBlockV1::new(
                            body.identity(),
                            body.source(),
                            statements,
                            body.terminator().clone(),
                        )
                        .unwrap();
                        let rebuilt = SemanticFunctionDeclV1::new(
                            function.identity(),
                            function.role(),
                            function.item_definition_identity(),
                            function.monomorphization_identity(),
                            function.generic_type_arguments_identity(),
                            function.const_generic_arguments_identity(),
                            function.source(),
                            function.abi().clone(),
                            function.locals().to_vec(),
                            function.entry(),
                            blocks,
                        )
                        .unwrap()
                        .with_kernel_entry(function.kernel_entry().unwrap().clone());
                        *function = rebuilt;
                    }
                }
            }
        },
        |source, bound, ranked, budget| {
            assert_eq!(source.deleted_call_count(), 2);
            assert_eq!(source.deleted_function_count(), 1);
            assert_eq!(
                source
                    .original_source()
                    .semantic_ssa()
                    .source_semantic()
                    .functions()
                    .len(),
                3
            );
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
fn licm_native_noop_source_adapter_preserves_existing_direct_fixture_exactly() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let mut expected = None;
        with_backend_loop_preheaders_direct_prefix_v1(profile, false, |owner, ranked, budget| {
            expected = Some((
                owner.output().canonical().canonical_bytes().to_vec(),
                *ranked.canonical_roster_identity().as_bytes(),
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
            ));
        });
        with_backend_licm_direct_prefix_v1(profile, false, |owner, ranked, budget| {
            assert_eq!(
                expected.take().unwrap(),
                (
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
