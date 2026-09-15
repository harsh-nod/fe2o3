// Admitted mixed component source, not authenticated Matrix/kernel issuance.
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/numerical_policy_math_01/ssa_fixture.rs"
));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MixedMutation117 {
    Live,
    MissingMatrixCall,
    MathOwnerDead,
    PolicyOwnerDead,
}

impl MixedMutation117 {
    pub(super) fn dead_owner(self) -> Option<u32> {
        match self {
            Self::MathOwnerDead => Some(3),
            Self::PolicyOwnerDead => Some(4),
            Self::Live | Self::MissingMatrixCall => None,
        }
    }
}

fn mixed_rebuild117(
    original: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let rebuilt = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = original.kernel_entry() {
        rebuilt.with_kernel_entry(entry.clone())
    } else {
        rebuilt
    }
}

fn mixed_block117(
    original: &SemanticBasicBlockV1,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        statements,
        SemanticTerminatorV1::new(original.terminator().source(), terminator),
    )
    .unwrap()
}

fn mixed_assign117(
    destination: u32,
    result: u32,
    kind: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        location(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(destination, result),
            SemanticRvalueV1::new(ty(result), kind),
        )),
    )
}

fn mixed_borrow117(
    destination: u32,
    reference: u32,
    owner: u32,
    owned: u32,
) -> SemanticStatementV1 {
    mixed_assign117(
        destination,
        reference,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(owner, owned),
        },
    )
}

fn mixed_bind_abi117(template: &SemanticFunctionAbiV1) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([240; 32]),
        template.layout_identity(),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![ty(13), ty(7)],
        ty(14),
        [13, 7]
            .into_iter()
            .zip(template.arguments())
            .map(|(id, argument)| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty(id),
                    argument.value().mode().clone(),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(ty(14), template.return_value().mode().clone()),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow; 2])
    .unwrap()
}

fn mixed_access_abi117(layout: SemanticLayoutIdentityV1) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([241; 32]),
        layout,
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![ty(15), ty(16)],
        ty(12),
        [15, 16]
            .into_iter()
            .map(|id| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty(id),
                    SemanticAbiPassModeV1::Ignore,
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(ty(12), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap()
}

pub(super) fn mixed_source117(mutation: MixedMutation117) -> AdmittedInertSemanticMirV1 {
    try_mixed_source117(mutation).unwrap()
}

pub(super) fn try_mixed_source117(
    mutation: MixedMutation117,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    let base = full_source(false, false);
    assert_eq!(
        (
            base.types().len(),
            base.functions().len(),
            base.callables().len()
        ),
        (12, 4, 8)
    );
    let SemanticDefinedCapabilityContractV1::KernelMathDerive(original_getter) = base.functions()
        [1]
    .defined_capability_contract()
    .copied()
    .unwrap() else {
        unreachable!()
    };
    let SemanticDefinedCapabilityContractV1::PolicyMathBind(original_bind) = base.functions()[3]
        .defined_capability_contract()
        .copied()
        .unwrap()
    else {
        unreachable!()
    };
    let identity = SemanticDefinedMatrixIdentityV1::new(
        original_bind.provenance(),
        original_bind.policy(),
        original_bind.kernel_brand(),
        SemanticTypeIdentityV1::from_sha256([243; 32]),
        SemanticTypeIdentityV1::from_sha256([244; 32]),
    )
    .unwrap();
    let mut types = base.types().to_vec();
    let zst = |tag| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            base.types()[3].layout().clone(),
            base.types()[3].shape().clone(),
        )
    };
    types.push(zst(230)); // 12: Matrix, distinct from Math and Policy.
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([231; 32]),
            SemanticLayoutIdentityV1::from_sha256([231; 32]),
            base.types()[6].layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(12),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(base.types()[6].abi_properties().clone()),
    );
    let pointer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([232; 32]),
            SemanticLayoutIdentityV1::from_sha256([232; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::ScalarPair {
                    first: pointer,
                    second: pointer,
                },
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8, 16, 16], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![ty(13), ty(7), ty(8), ty(8)]).unwrap(),
            ),
        )
        .with_rustc_abi_properties(base.types()[9].abi_properties().clone()),
    );
    types.extend([zst(233), zst(234)]); // 15: subgroup; 16: epoch.

    let copy = |id, t| SemanticOperandV1::Copy(place(id, t));
    let marker = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(8),
            SemanticConstantValueV1::ZeroSized,
        ))
    };
    let matrix_bind = function(
        240,
        mixed_bind_abi117(base.functions()[3].abi()),
        vec![
            local(240, 14, SemanticLocalRoleV1::Return),
            local(241, 13, SemanticLocalRoleV1::Argument(0)),
            local(242, 7, SemanticLocalRoleV1::Argument(1)),
        ],
        vec![block(
            240,
            vec![mixed_assign117(
                0,
                14,
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::Aggregate,
                    vec![copy(1, 13), copy(2, 7), marker(), marker()],
                )
                .unwrap(),
            )],
            SemanticTerminatorKindV1::Return,
        )],
        false,
    );
    let access = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::MatrixAccess {
            subgroup: ty(15),
            epoch: ty(16),
            matrix: ty(12),
            subgroup_brand: identity.matrix_brand(),
            width: 64,
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(15), ty(16)], ty(12)).unwrap(),
        identity.provenance(),
        identity.execution_brand(),
        identity.epoch(),
        None,
        SemanticFunctionIdentityV1::from_sha256([241; 32]),
    )
    .unwrap();
    let mut callables = base.callables()[..4].to_vec();
    callables.push(SemanticCallableDeclV1::defined(
        SemanticFunctionIdV1::from_index(4),
    ));
    callables.extend_from_slice(&base.callables()[4..]);
    callables.push(terminal(
        241,
        mixed_access_abi117(base.functions()[3].abi().layout_identity()),
        SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: access },
    ));

    let root = &base.functions()[0];
    let mut locals = root.locals().to_vec();
    for id in 12..17 {
        locals.push(local(
            240 + (id - 12) as u8,
            id,
            SemanticLocalRoleV1::Temporary,
        ));
    }
    let mut blocks = root.blocks().to_vec();
    blocks[0] = mixed_block117(
        &blocks[0],
        blocks[0].statements().to_vec(),
        call(5, vec![], 1, 1, 1),
    );
    let missing = mutation == MixedMutation117::MissingMatrixCall;
    blocks[2] = mixed_block117(
        &blocks[2],
        blocks[2].statements().to_vec(),
        call(7, vec![copy(2, 2)], 4, 5, if missing { 3 } else { 6 }),
    );
    let SemanticTerminatorKindV1::Call(consumer) = root.blocks()[4].terminator().kind() else {
        unreachable!()
    };
    let mut use_statements = blocks[4].statements().to_vec();
    assert_eq!(use_statements.len(), 1);
    if let Some(owner) = mutation.dead_owner() {
        use_statements.push(SemanticStatementV1::new(
            location(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(owner)),
        ));
    }
    blocks[4] = mixed_block117(
        &blocks[4],
        use_statements,
        call(8, consumer.arguments().to_vec(), 9, 11, 5),
    );
    if !missing {
        assert_eq!(blocks[3].statements().len(), 2);
        // The same original Policy Borrow now supplies both checked Bind calls.
        blocks[3] = mixed_block117(
            &blocks[3],
            vec![blocks[3].statements()[0].clone()],
            blocks[3].terminator().kind().clone(),
        );
        let empty =
            || SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Aggregate, vec![]).unwrap();
        blocks.push(block(
            246,
            vec![
                mixed_assign117(15, 15, empty()),
                mixed_assign117(16, 16, empty()),
            ],
            call(9, vec![copy(15, 15), copy(16, 16)], 12, 12, 7),
        ));
        blocks.push(block(
            247,
            vec![mixed_borrow117(13, 13, 12, 12), mixed_borrow117(6, 7, 4, 5)],
            call(4, vec![copy(13, 13), copy(6, 7)], 14, 14, 3),
        ));
    }

    let mut functions = base.functions().to_vec();
    functions[0] = mixed_rebuild117(root, locals, blocks);
    let bridge = &base.functions()[2];
    let mut bridge_blocks = bridge.blocks().to_vec();
    bridge_blocks[0] = mixed_block117(
        &bridge_blocks[0],
        bridge_blocks[0].statements().to_vec(),
        call(6, vec![], 1, 4, 1),
    );
    functions[2] = mixed_rebuild117(bridge, bridge.locals().to_vec(), bridge_blocks);
    functions.push(matrix_bind);
    // Strip old attachments before recomputing commitments over the final roster.
    for index in [1, 3] {
        let original = &base.functions()[index];
        functions[index] = mixed_rebuild117(
            original,
            original.locals().to_vec(),
            original.blocks().to_vec(),
        );
    }
    let getter = SemanticKernelMathDeriveV1::for_defined_function(
        SemanticFunctionIdV1::from_index(1),
        &functions,
        &callables,
        &types,
        original_getter.types(),
        original_getter.provenance(),
        original_getter.kernel_brand(),
    )
    .unwrap();
    let math_bind = SemanticPolicyMathBindV1::for_defined_function(
        SemanticFunctionIdV1::from_index(3),
        &functions,
        &callables,
        &types,
        original_bind.types(),
        original_bind.provenance(),
        original_bind.policy(),
        original_bind.kernel_brand(),
    )
    .unwrap();
    let matrix_bind = SemanticPolicyMatrixBindV1::for_defined_function(
        SemanticFunctionIdV1::from_index(4),
        &functions,
        &callables,
        &types,
        SemanticPolicyMatrixBindTypesV1::new([13, 12, 7, 5, 14].map(ty)),
        identity,
    )
    .unwrap();
    for (index, contract) in [
        (
            1,
            SemanticDefinedCapabilityContractV1::KernelMathDerive(getter),
        ),
        (
            3,
            SemanticDefinedCapabilityContractV1::PolicyMathBind(math_bind),
        ),
        (
            4,
            SemanticDefinedCapabilityContractV1::PolicyMatrixBind(matrix_bind),
        ),
    ] {
        functions[index] = functions[index]
            .clone()
            .with_defined_capability_contract(contract)
            .unwrap();
    }
    InertSemanticMirRequestV1::new_with_callables(
        base.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        base.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
}
