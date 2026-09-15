// Synthetic source extension for secondary-role tests, not production authority.
include!("mixed_math_fixture117.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SecondaryPolicyMutation119 {
    Live,
    Reordered,
    DuplicatePolicySameReference,
    // Duplicate the local6 Borrow definition so its candidate map entry is removed.
    DuplicatePolicySameReferenceUnmapped,
    // A second Borrow of owner local4, not a second Policy issuer/owner.
    DuplicatePolicyDistinctReference,
    LatePolicyAddressEscape,
    LateMathAddressEscape,
}

impl SecondaryPolicyMutation119 {
    pub(super) fn leaf_fields(self) -> (u32, u32) {
        match self {
            Self::Reordered => (1, 0),
            _ => (0, 1),
        }
    }

    pub(super) fn escaped_reference(self) -> Option<(u32, u32)> {
        match self {
            Self::LatePolicyAddressEscape => Some((20, 7)),
            Self::LateMathAddressEscape => Some((19, 10)),
            _ => None,
        }
    }
}

pub(super) fn secondary_policy_source119(
    mutation: SecondaryPolicyMutation119,
) -> AdmittedInertSemanticMirV1 {
    let source = try_secondary_policy_source119(mutation).unwrap();
    let expansion =
        fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticCallExpansionV1::try_new(
            &source,
            Default::default(),
        )
        .unwrap();
    expansion.verify_replay(&source).unwrap();
    source
}

// Fallible admission only; secondary_policy_source119 additionally checks replay.
// Type17: [10,7], [7,10], or [10,7,7], alignment8 and consecutive 8-byte fields.
// Locals17/18: carrier, 19: &MathBound, 20: &Policy, 21: second Matrix bound.
// Local22 is the second &Policy or the late AddressOf result (raw pointer type18).
// Capture/forward/Math extraction/Policy extraction remain at root4:1/2/3/4.
// Late AddressOf is at root5:0, after the second MatrixBind and Math consumer.
pub(super) fn try_secondary_policy_source119(
    mutation: SecondaryPolicyMutation119,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    let base = try_mixed_source117(MixedMutation117::Live)?;
    let root = &base.functions()[0];
    assert_eq!(
        (
            base.types().len(),
            base.functions().len(),
            base.callables().len(),
            root.locals().len(),
            root.blocks().len(),
        ),
        (17, 5, 10, 17, 8),
    );
    let duplicate = matches!(
        mutation,
        SecondaryPolicyMutation119::DuplicatePolicySameReference
            | SecondaryPolicyMutation119::DuplicatePolicySameReferenceUnmapped
            | SecondaryPolicyMutation119::DuplicatePolicyDistinctReference
    );
    let distinct = mutation == SecondaryPolicyMutation119::DuplicatePolicyDistinctReference;
    let unmapped = mutation == SecondaryPolicyMutation119::DuplicatePolicySameReferenceUnmapped;
    let (math_field, policy_field) = mutation.leaf_fields();
    let mut fields = vec![ty(10), ty(7)];
    if mutation == SecondaryPolicyMutation119::Reordered {
        fields.swap(0, 1);
    }
    if duplicate {
        fields.push(ty(7));
    }
    let mut types = base.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([235; 32]),
        SemanticLayoutIdentityV1::from_sha256([235; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(fields.len() as u64 * 8),
            8,
            SemanticAggregateLayoutV1::new(
                (0..fields.len()).map(|index| index as u64 * 8).collect(),
                vec![],
            )?,
        )?,
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields)?),
    ));
    if let Some((_, reference)) = mutation.escaped_reference() {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([236; 32]),
            SemanticLayoutIdentityV1::from_sha256([236; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
                false,
            )?,
            SemanticTypeShapeV1::Pointer(SemanticPointerTypeV1::new_with_kind(
                ty(reference),
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )?),
        ));
    }
    let mut locals = root.locals().to_vec();
    for (index, value) in [17, 17, 10, 7, 14].into_iter().enumerate() {
        locals.push(local(
            250 + index as u8,
            value,
            SemanticLocalRoleV1::Temporary,
        ));
    }
    if distinct {
        locals.push(local(255, 7, SemanticLocalRoleV1::Temporary));
    } else if mutation.escaped_reference().is_some() {
        locals.push(local(255, 18, SemanticLocalRoleV1::Temporary));
    }

    let mut blocks = root.blocks().to_vec();
    if distinct || unmapped {
        let mut statements = blocks[7].statements().to_vec();
        assert_eq!(statements.len(), 2);
        statements.push(mixed_borrow117(if distinct { 22 } else { 6 }, 7, 4, 5));
        blocks[7] = mixed_block117(
            &blocks[7],
            statements,
            blocks[7].terminator().kind().clone(),
        );
    }
    let copy = |id, value| SemanticOperandV1::Copy(place(id, value));
    let mut operands = vec![copy(8, 10), copy(6, 7)];
    if mutation == SecondaryPolicyMutation119::Reordered {
        operands.swap(0, 1);
    }
    if duplicate {
        operands.push(copy(if distinct { 22 } else { 6 }, 7));
    }
    let mut statements = blocks[4].statements().to_vec();
    assert_eq!(statements.len(), 1);
    statements.push(mixed_assign117(
        17,
        17,
        SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Aggregate, operands)?,
    ));
    statements.push(mixed_assign117(
        18,
        17,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(17, 17))),
    ));
    for (destination, value, field) in [(19, 10, math_field), (20, 7, policy_field)] {
        let projected = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(18),
            vec![SemanticProjectionV1::new(
                SemanticProjectionKindV1::Field(field),
                ty(value),
            )?],
            ty(value),
        )?;
        statements.push(mixed_assign117(
            destination,
            value,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected)),
        ));
    }
    let SemanticTerminatorKindV1::Call(consumer) = root.blocks()[4].terminator().kind() else {
        unreachable!("117 Math consumer")
    };
    assert_eq!(consumer.callee().index(), 8);
    assert_eq!(consumer.arguments().len(), 2);
    let mut arguments = consumer.arguments().to_vec();
    arguments[0] = copy(19, 10);
    // Keep the original Math terminal's scalar, destination, continuation and unwind.
    blocks.push(block(
        248,
        vec![],
        SemanticTerminatorKindV1::Call(SemanticDirectCallV1::new_callable(
            consumer.callee(),
            arguments,
            consumer.destination().cloned(),
            consumer.unwind(),
        )?),
    ));
    blocks[4] = mixed_block117(
        &blocks[4],
        statements,
        call(4, vec![copy(13, 13), copy(20, 7)], 21, 14, 8),
    );
    if let Some((reference, value)) = mutation.escaped_reference() {
        assert!(blocks[5].statements().is_empty());
        assert!(matches!(
            blocks[5].terminator().kind(),
            SemanticTerminatorKindV1::Return
        ));
        blocks[5] = mixed_block117(
            &blocks[5],
            vec![mixed_assign117(
                22,
                18,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Immutable,
                    place: place(reference, value),
                },
            )],
            blocks[5].terminator().kind().clone(),
        );
    }

    let mut functions = base.functions().to_vec();
    functions[0] = mixed_rebuild117(root, locals, blocks);
    let originals = [1, 3, 4].map(|index| {
        (
            index,
            *base.functions()[index]
                .defined_capability_contract()
                .unwrap(),
        )
    });
    // Strip all old attachments, then compute every commitment over the final roster.
    for &(index, _) in &originals {
        let original = &base.functions()[index];
        functions[index] = mixed_rebuild117(
            original,
            original.locals().to_vec(),
            original.blocks().to_vec(),
        );
    }
    let mut renewed = Vec::with_capacity(originals.len());
    for (index, original) in originals {
        let contract = match original {
            SemanticDefinedCapabilityContractV1::KernelMathDerive(old) => {
                SemanticDefinedCapabilityContractV1::KernelMathDerive(
                    SemanticKernelMathDeriveV1::for_defined_function(
                        old.function(),
                        &functions,
                        base.callables(),
                        &types,
                        old.types(),
                        old.provenance(),
                        old.kernel_brand(),
                    )?,
                )
            }
            SemanticDefinedCapabilityContractV1::PolicyMathBind(old) => {
                SemanticDefinedCapabilityContractV1::PolicyMathBind(
                    SemanticPolicyMathBindV1::for_defined_function(
                        old.function(),
                        &functions,
                        base.callables(),
                        &types,
                        old.types(),
                        old.provenance(),
                        old.policy(),
                        old.kernel_brand(),
                    )?,
                )
            }
            SemanticDefinedCapabilityContractV1::PolicyMatrixBind(old) => {
                SemanticDefinedCapabilityContractV1::PolicyMatrixBind(
                    SemanticPolicyMatrixBindV1::for_defined_function(
                        old.function(),
                        &functions,
                        base.callables(),
                        &types,
                        old.types(),
                        old.identity(),
                    )?,
                )
            }
            _ => unreachable!("117 getter and Bind contracts"),
        };
        renewed.push((index, contract));
    }
    for (index, contract) in renewed {
        functions[index] = functions[index]
            .clone()
            .with_defined_capability_contract(contract)?;
    }
    // Original identities are retained; admission derives the changed source's digest.
    InertSemanticMirRequestV1::new_with_callables(
        base.target(),
        types,
        base.allocations().to_vec(),
        base.statics().to_vec(),
        base.vtables().to_vec(),
        functions,
        base.callables().to_vec(),
        base.roots().to_vec(),
    )?
    .admit_current_production(SemanticMirLimitsV1::default())
}
