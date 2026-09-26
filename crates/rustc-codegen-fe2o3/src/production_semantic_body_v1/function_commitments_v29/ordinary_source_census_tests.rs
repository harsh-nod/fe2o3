//! Inert constructor-custody tests; real rustc classification has a driver test.
use super::*;
use crate::collector::workgroup_scope_custody_v29::{PendingWorkgroupScopesV29, ScopeCallableV29};

fn ordinary_enabled(
    semantic: &AdmittedInertSemanticMirV1,
) -> ProductionSemanticBodyRequestOwnerV1<'static> {
    let mut owner = owner(semantic.functions().len() as u32);
    owner
        .enable_source_function_commitments_v1(
            semantic.types(),
            semantic.callables(),
            semantic.functions().len(),
            semantic
                .functions()
                .iter()
                .enumerate()
                .map(|(index, function)| expected(index as u32, function)),
        )
        .unwrap();
    owner
}

fn capture_complete(
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'_>,
    semantic: &AdmittedInertSemanticMirV1,
) {
    let encoding = owner.function_commitments.as_ref().unwrap().encoding();
    let declarations = owner
        .totals
        .declaration_tables_commitment_v29(
            semantic.types(),
            semantic.callables(),
            encoding,
            owner.limits,
        )
        .unwrap();
    let mut scopes = PendingWorkgroupScopesV29::new_with_encoding(
        vec![ScopeCallableV29::Ordinary; semantic.callables().len()],
        declarations,
        semantic.target(),
        semantic.functions().len(),
        encoding,
    )
    .unwrap();
    for (index, function) in semantic.functions().iter().enumerate() {
        let function_id = SemanticFunctionIdV1::from_index(index as u32);
        let commitment = owner
            .capture_function_commitment_v29(function_id, function)
            .unwrap()
            .unwrap();
        let mut events = Vec::new();
        for (block, data) in function.blocks().iter().enumerate() {
            scopes
                .capture(
                    function_id,
                    SemanticBlockIdV1::from_index(block as u32),
                    data.statements().len(),
                    data.terminator().kind(),
                    &mut events,
                    |amount| owner.charge(SemanticMirResourceV1::ValidationWork, amount),
                )
                .unwrap();
        }
        let prepared_scopes = scopes
            .prepare(function_id, events, |amount| {
                owner.charge(SemanticMirResourceV1::ValidationWork, amount)
            })
            .unwrap();
        let prepared_function = owner
            .function_commitments
            .as_mut()
            .unwrap()
            .prepare(commitment)
            .unwrap();
        prepared_function.publish();
        prepared_scopes.publish();
    }
    owner.workgroup_scopes = Some(scopes);
}

fn readmit(
    semantic: &AdmittedInertSemanticMirV1,
    functions: Vec<SemanticFunctionDeclV1>,
) -> AdmittedInertSemanticMirV1 {
    InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn replace_blocks(
    function: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
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
}

#[test]
fn ordinary_source_census_complete_constructor_keeps_minimal_source_profile_and_sha() {
    for count in [1, 3] {
        let semantic = admitted(&vec![7; count], false);
        let original_sha = *semantic.semantic_sha256().as_bytes();
        let original_profile = semantic.wire_version();
        assert_ne!(original_profile, SemanticMirWireVersionV1::V29);
        let mut owner = ordinary_enabled(&semantic);
        capture_complete(&mut owner, &semantic);
        let receipt = owner.seal_context_entries(&semantic).unwrap();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget =
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 64);
        let source = receipt
            .materialization_source_v29(&semantic, &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(source.semantic_sha256(), &original_sha);
        assert_eq!(semantic.wire_version(), original_profile);
        assert_eq!(source.classes().len(), semantic.callables().len());
        assert!(source.roots().is_empty());
        assert!(source.events().is_empty());
    }
}

#[test]
fn ordinary_source_census_rejects_same_typed_body_and_unreachable_statement_changes() {
    let baseline = admitted(&[7], false);
    let original = &baseline.functions()[0];
    let unreachable = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([210; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        original.blocks()[0].statements().to_vec(),
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Unreachable,
        ),
    )
    .unwrap();
    let mut blocks = original.blocks().to_vec();
    blocks.push(unreachable);
    let semantic = readmit(&baseline, vec![replace_blocks(original, blocks)]);
    for changed_block in [0, 1] {
        let mut owner = ordinary_enabled(&semantic);
        capture_complete(&mut owner, &semantic);
        let mut blocks = semantic.functions()[0].blocks().to_vec();
        let block = &blocks[changed_block];
        blocks[changed_block] = SemanticBasicBlockV1::new(
            block.identity(),
            block.source(),
            function(0, 11).blocks()[0].statements().to_vec(),
            block.terminator().clone(),
        )
        .unwrap();
        let changed = readmit(
            &semantic,
            vec![replace_blocks(&semantic.functions()[0], blocks)],
        );
        assert!(matches!(
            owner.seal_context_entries(&changed),
            Err(ProductionSemanticBodyErrorV1::IdentityTableMismatch {
                table: "function commitment changed body",
            })
        ));
    }
    let mut control = ordinary_enabled(&semantic);
    capture_complete(&mut control, &semantic);
    assert!(control.seal_context_entries(&semantic).is_ok());
}

#[test]
fn ordinary_source_census_missing_scope_or_body_is_not_empty_census_authority() {
    let semantic = admitted(&[7, 7], false);
    for missing_scope in [false, true] {
        let mut owner = ordinary_enabled(&semantic);
        capture_complete(&mut owner, &semantic);
        if missing_scope {
            owner.workgroup_scopes = None;
        } else {
            owner.function_commitments.as_mut().unwrap().rows[1].captured = None;
        }
        assert!(owner.seal_context_entries(&semantic).is_err());
    }
    let mut owner = ordinary_enabled(&semantic);
    capture_complete(&mut owner, &semantic);
    assert!(owner.seal_context_entries(&semantic).is_ok());
}

#[test]
fn ordinary_source_census_complete_capture_does_not_accept_wrong_original_profile() {
    let ordinary = admitted(&[7], false);
    let mut owner = ordinary_enabled(&ordinary);
    capture_complete(&mut owner, &ordinary);
    assert!(owner.seal_context_entries(&admitted(&[7], true)).is_err());
}

#[test]
fn ordinary_source_census_selection_capture_and_seal_share_exact_work_counter() {
    let semantic = admitted(&[7, 7], false);
    let mut owner = ordinary_enabled(&semantic);
    capture_complete(&mut owner, &semantic);
    let before = owner.totals.validation_work;
    let encoding = owner.function_commitments.as_ref().unwrap().encoding();
    let mut independent = ConstructionTotalsV1::default();
    owner
        .function_commitments
        .take()
        .unwrap()
        .verify(&semantic, &mut independent, owner.limits)
        .unwrap();
    independent
        .declaration_tables_commitment_v29(
            semantic.types(),
            semantic.callables(),
            encoding,
            owner.limits,
        )
        .unwrap();
    let scopes = owner.workgroup_scopes.take().unwrap();
    let declarations = canonical_declaration_tables_commitment_v1(
        semantic.types(),
        semantic.callables(),
        encoding.wire_version(),
        owner.limits,
        &mut |_| Ok(()),
    )
    .unwrap();
    crate::collector::RetainedContextEntriesV29::seal_with_scopes(
        vec![],
        Some((scopes, declarations)),
        &semantic,
        |amount| independent.charge(SemanticMirResourceV1::ValidationWork, amount, owner.limits),
    )
    .unwrap();
    let required = before + independent.validation_work;
    for maximum in [required - 1, required] {
        let mut owner = ordinary_enabled(&semantic);
        capture_complete(&mut owner, &semantic);
        owner.limits = owner
            .limits
            .with_limit(SemanticMirResourceV1::ValidationWork, maximum)
            .unwrap();
        assert_eq!(
            owner.seal_context_entries(&semantic).is_ok(),
            maximum == required
        );
    }
}

#[test]
fn ordinary_source_census_nominal_usize_keeps_real_v35_source_owner() {
    let original = admitted(&[7], false);
    let function = &original.functions()[0];
    let ty = SemanticTypeDeclV1::new(
        original.types()[0].identity(),
        original.types()[0].layout_identity(),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_rustc_layout_is_noundef(true),
    )
    .with_rust_type_kind(SemanticRustTypeKindV1::Usize);
    let block = &function.blocks()[0];
    let statement = SemanticStatementV1::new(
        block.source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(0),
                vec![],
                SemanticTypeIdV1::from_index(0),
            )
            .unwrap(),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(0),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    SemanticTypeIdV1::from_index(0),
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 8).unwrap()),
                ))),
            ),
        )),
    );
    let function = replace_blocks(
        function,
        vec![
            SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                vec![statement],
                block.terminator().clone(),
            )
            .unwrap(),
        ],
    );
    let semantic = InertSemanticMirRequestV1::new(
        original.target(),
        vec![ty],
        vec![],
        vec![],
        vec![],
        vec![function],
        original.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V35);
    let sha = *semantic.semantic_sha256().as_bytes();
    let mut owner = ordinary_enabled(&semantic);
    capture_complete(&mut owner, &semantic);
    let receipt = owner.seal_context_entries(&semantic).unwrap();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget =
        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    let source = receipt
        .materialization_source_v29(&semantic, &mut budget)
        .unwrap()
        .unwrap();
    assert_eq!(source.semantic_sha256(), &sha);
    assert!(source.events().is_empty());
}

#[test]
fn ordinary_source_census_saturating_v30_uses_complete_digest_without_upgrading_source() {
    let original = admitted(&[7], false);
    let operation = SemanticSaturatingIntegerOpV1::Add;
    let value = original.functions()[0].abi().return_value().clone();
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([202; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![value.clone(), value.clone()],
        value,
    )
    .unwrap();
    let binding = SemanticNonBodyCallableBindingV1::new(
        SemanticFunctionIdentityV1::from_sha256([203; 32]),
        SemanticItemDefinitionIdentityV1::from_sha256([204; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([205; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([206; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([207; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
    );
    let mut callables = original.callables().to_vec();
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding,
        operation: SemanticCompilerIntrinsicOperationV1::SaturatingInteger(operation),
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([208; 32]),
    });
    let original_function = &original.functions()[0];
    let block = &original_function.blocks()[0];
    let ty = SemanticTypeIdV1::from_index(0);
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        [7, 11]
            .map(|value| {
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
                ))
            })
            .to_vec(),
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], ty).unwrap(),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let function = replace_blocks(
        original_function,
        vec![
            SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                vec![],
                SemanticTerminatorV1::new(block.source(), SemanticTerminatorKindV1::Call(call)),
            )
            .unwrap(),
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([209; 32]),
                block.source(),
                vec![],
                block.terminator().clone(),
            )
            .unwrap(),
        ],
    );
    let semantic = InertSemanticMirRequestV1::new_with_callables(
        original.target(),
        original.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        original.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V30);
    let original_sha = *semantic.semantic_sha256().as_bytes();
    let entries = [
        ProductionSemanticCallableOwnerEntryV1::defined(
            instance(0),
            SemanticCallableIdV1::from_index(0),
        ),
        ProductionSemanticCallableOwnerEntryV1::terminal(
            instance(1),
            ProductionTerminalExpansionV1::RustcSaturatingInteger(operation),
            SemanticCallableIdV1::from_index(1),
        ),
    ];
    let mut owner =
        ProductionSemanticBodyRequestOwnerV1::new(SemanticMirLimitsV1::default(), 1, &entries)
            .unwrap();
    owner
        .enable_source_function_commitments_v1(
            semantic.types(),
            semantic.callables(),
            1,
            std::iter::once(expected(0, &semantic.functions()[0])),
        )
        .unwrap();
    assert_eq!(
        owner
            .function_commitments
            .as_ref()
            .unwrap()
            .encoding()
            .wire_version(),
        SemanticMirWireVersionV1::V33
    );
    capture_complete(&mut owner, &semantic);
    let receipt = owner.seal_context_entries(&semantic).unwrap();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget =
        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    let source = receipt
        .materialization_source_v29(&semantic, &mut budget)
        .unwrap()
        .unwrap();
    assert_eq!(source.semantic_sha256(), &original_sha);
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V30);
}
