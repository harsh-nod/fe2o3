use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

pub(in crate::collector::production_importer_v1::numerical_policy_v1::defined_body_v1::matrix) fn check_import<
    'tcx,
>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    imported: &ConstructedProductionSemanticMirV1,
) {
    let mir = &imported.semantic_mir;
    let roster = capture(tcx, plan, &imported.kernel_contexts, mir).unwrap();
    assert_eq!(
        roster.rows.len(),
        2,
        "both actual scoped A/B constructor providers"
    );
    let mut roles = BTreeSet::new();
    for row in &roster.rows {
        roles.insert(match row.role {
            SemanticMfmaOperandRoleV1::A => 0,
            SemanticMfmaOperandRoleV1::B => 1,
        });
        assert_eq!(row.root, mir.roots()[0]);
        assert!(
            roster
                .find(mir.semantic_sha256(), row.root, row.wrapper, row.checked)
                .is_some()
        );
        assert!(
            roster
                .find(
                    mir.semantic_sha256(),
                    SemanticFunctionIdV1::from_index(u32::MAX),
                    row.wrapper,
                    row.checked
                )
                .is_none()
        );
        assert!(
            roster
                .find(mir.semantic_sha256(), row.root, row.wrapper, row.wrapper)
                .is_none()
        );
        assert_ne!(row.wrapper, row.checked);
        let checked_instance = plan.function_producers()[row.checked.index() as usize].instance;
        let definition = trusted_device_items::reviewed_provider_semantic_definition_v1(
            tcx,
            checked_instance.def_id(),
        )
        .expect("actual checked provider retains its original reviewed definition");
        assert_eq!(
            definition.canonical_definition_path,
            "fe2o3_device::tensor::GlobalBf16MfmaMatrix::checked"
        );
        assert!(trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            checked_instance.def_id(),
            CHECKED,
        ));
        assert!(!trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            checked_instance.def_id(),
            "fe2o3_device::GlobalBf16MfmaMatrix::checked",
        ));
        assert!(!trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            plan.function_producers()[row.wrapper.index() as usize].instance.def_id(),
            CHECKED,
        ));
        assert!(trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
            tcx,
            checked_instance.def_id(),
        )
        .unwrap());
        let original = &mir.functions()[row.wrapper.index() as usize];
        let checked = &mir.functions()[row.checked.index() as usize];
        assert_eq!(original.abi().source_input_types(), row.inputs);
        assert_eq!(checked.abi().source_input_types(), &row.inputs[1..]);
        assert_eq!(original.abi().source_output_type(), row.output_types[3]);
        assert_eq!(checked.abi().source_output_type(), row.output_types[3]);
        assert_ne!(
            row.output_types[1], row.output_types[3],
            "payload is not its Result"
        );
        assert_eq!(
            row.identity.kernel_brand(),
            rustc_type_identity_v1(
                tcx,
                validate_source(
                    tcx,
                    plan.function_producers()[row.wrapper.index() as usize].instance
                )
                .unwrap()
                .unwrap()
                .pair
                .identity
                .root
                .ty
            )
        );

        let mut swapped = mir.functions().to_vec();
        let mut blocks = original.blocks().to_vec();
        let retained = &plan.body_producers()[row.wrapper.index() as usize];
        assert_eq!(retained.function, row.wrapper);
        let mut planned_calls = plan.direct_call_producers().iter().filter(|call| call.caller == row.wrapper);
        let planned_call = planned_calls.next().expect("original forwarding call recipe");
        assert!(planned_calls.next().is_none());
        assert_eq!(planned_call.callee, row.checked);
        assert_eq!(planned_call.block, 0, "original native wrapper entry");
        let call_block = retained.raw_to_semantic_blocks[planned_call.block as usize];
        assert_eq!(original.entry(), call_block);
        let call_index = call_block.index() as usize;
        assert_eq!(retained.blocks[call_index].rustc_block, planned_call.block);
        assert_eq!(blocks.len(), 2, "exact original forwarding wrapper");
        let first = &blocks[call_index];
        let SemanticTerminatorKindV1::Call(call) = first.terminator().kind() else {
            panic!("original forwarding call");
        };
        assert!(matches!(
            mir.callables()[call.callee().index() as usize],
            SemanticCallableDeclV1::Defined { function } if function == row.checked
        ));
        let mut arguments = call.arguments().to_vec();
        arguments.swap(1, 2); // same usize type, different source coordinate
        let call = SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
            call.callee(),
            arguments,
            call.variadic_argument_abis().to_vec(),
            call.destination().cloned(),
            call.unwind(),
        )
        .unwrap();
        blocks[call_index] = SemanticBasicBlockV1::new(
            first.identity(),
            first.source(),
            first.statements().to_vec(),
            SemanticTerminatorV1::new(
                first.terminator().source(),
                SemanticTerminatorKindV1::Call(call),
            ),
        )
        .unwrap();
        swapped[row.wrapper.index() as usize] = with_blocks(original, blocks);
        assert_replay_rejected(validate_canonical(
            tcx,
            plan,
            mir.types(),
            &swapped,
            mir.callables(),
            &imported.kernel_contexts,
        ));

        let foreign = InertSemanticMirRequestV1::new_with_callables(
            mir.target(),
            mir.types().to_vec(),
            mir.allocations().to_vec(),
            mir.statics().to_vec(),
            mir.vtables().to_vec(),
            swapped,
            mir.callables().to_vec(),
            mir.roots().to_vec(),
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        assert_ne!(foreign.semantic_sha256(), mir.semantic_sha256());
        assert!(
            roster
                .find(
                    foreign.semantic_sha256(),
                    row.root,
                    row.wrapper,
                    row.checked
                )
                .is_none(),
            "same labels do not authenticate a different canonical subject"
        );

        let mut erased = mir.functions().to_vec();
        let mut blocks = checked.blocks().to_vec();
        let branch = blocks
            .iter()
            .position(|block| {
                matches!(
                    block.terminator().kind(),
                    SemanticTerminatorKindV1::SwitchInt { .. }
                )
            })
            .expect("actual checked constructor retains a success/error branch");
        let original_block = &blocks[branch];
        blocks[branch] = SemanticBasicBlockV1::new(
            original_block.identity(),
            original_block.source(),
            original_block.statements().to_vec(),
            SemanticTerminatorV1::new(
                original_block.terminator().source(),
                SemanticTerminatorKindV1::Unreachable,
            ),
        )
        .unwrap();
        erased[row.checked.index() as usize] = with_blocks(checked, blocks);
        assert_replay_rejected(validate_canonical(
            tcx,
            plan,
            mir.types(),
            &erased,
            mir.callables(),
            &imported.kernel_contexts,
        ));
    }
    assert_eq!(roles, BTreeSet::from([0, 1]));
}

fn assert_replay_rejected(result: Result<Vec<ConstructorRowV1>, ProductionSemanticImportErrorV1>) {
    assert!(
        matches!(
            result,
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "global matrix original constructor/helper body replay"
            ))
        ),
        "mutation must fail exact original-body replay"
    );
}

fn with_blocks(
    body: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    assert!(body.defined_capability_contract().is_none());
    SemanticFunctionDeclV1::new(
        body.identity(),
        body.role(),
        body.item_definition_identity(),
        body.monomorphization_identity(),
        body.generic_type_arguments_identity(),
        body.const_generic_arguments_identity(),
        body.source(),
        body.abi().clone(),
        body.locals().to_vec(),
        body.entry(),
        blocks,
    )
    .unwrap()
}
