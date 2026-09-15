fn lower_subgroup_partition_operation_v1(
    types: &[SemanticTypeDeclV1],
    operation: fe2o3_mir_model::semantic_mir_v1::SemanticSubgroupPartitionOperationV1,
) -> Result<fe2o3_kernel_ir::SubgroupPartitionOperationV1, ProductionSemanticKirErrorV1> {
    use fe2o3_kernel_ir::SubgroupPartitionOperationV1 as K;
    use fe2o3_mir_model::semantic_mir_v1::SemanticSubgroupPartitionOperationV1 as M;
    if !operation.is_well_formed() {
        return Err(unsupported(
            0,
            None,
            None,
            "invalid physical subgroup or partition width",
        ));
    }
    let id = |ty| execution_type_identity_v1(types, ty);
    Ok(match operation {
        M::Derive {
            subgroup_reference,
            subgroup,
            epoch,
            partition,
            width,
            partition_width,
        } => K::Derive {
            subgroup_reference: id(subgroup_reference)?,
            subgroup: id(subgroup)?,
            epoch: id(epoch)?,
            partition: id(partition)?,
            width,
            partition_width,
        },
        M::ReduceSumF32 {
            partition_reference,
            partition,
            element,
            width,
            partition_width,
        } => {
            if execution_scalar_v1(types, element)? != ScalarType::F32 {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "partition reduction requires exact f32",
                ));
            }
            K::ReduceSumF32 {
                partition_reference: id(partition_reference)?,
                partition: id(partition)?,
                element: id(element)?,
                width,
                partition_width,
            }
        }
        M::ReduceMaxF32 {
            partition_reference,
            partition,
            element,
            width,
            partition_width,
        } => {
            if execution_scalar_v1(types, element)? != ScalarType::F32 {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "partition reduction requires exact f32",
                ));
            }
            K::ReduceMaxF32 {
                partition_reference: id(partition_reference)?,
                partition: id(partition)?,
                element: id(element)?,
                width,
                partition_width,
            }
        }
        M::BroadcastF32 {
            partition_reference,
            partition,
            element,
            source_lane,
            width,
            partition_width,
        } => {
            if execution_scalar_v1(types, element)? != ScalarType::F32
                || execution_scalar_v1(types, source_lane)? != ScalarType::U32
            {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "partition broadcast requires exact f32/u32",
                ));
            }
            K::BroadcastF32 {
                partition_reference: id(partition_reference)?,
                partition: id(partition)?,
                element: id(element)?,
                source_lane: id(source_lane)?,
                width,
                partition_width,
            }
        }
    })
}

fn subgroup_partition_transport_role_v1(
    contract: SemanticExecutionCapabilityContractV1,
) -> Option<ExecutionCapabilityRoleV1> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticSubgroupPartitionOperationV1 as P;
    match contract.operation() {
        SemanticExecutionCapabilityOperationV1::WorkgroupDerive { .. } => {
            Some(ExecutionCapabilityRoleV1::Workgroup)
        }
        SemanticExecutionCapabilityOperationV1::SubgroupDerive { width, .. } => {
            Some(ExecutionCapabilityRoleV1::Subgroup { width })
        }
        SemanticExecutionCapabilityOperationV1::SubgroupPartition(P::Derive {
            width,
            partition_width,
            ..
        }) => Some(ExecutionCapabilityRoleV1::SubgroupPartition {
            width,
            partition_width,
        }),
        _ => None,
    }
}

fn subgroup_partition_transport_matches_v1(
    contract: SemanticExecutionCapabilityContractV1,
    capability: &ExecutionCapabilityTypeV1,
) -> bool {
    let source = contract.provenance();
    capability.is_complete()
        && capability.provenance.kernel_binding == *source.kernel_binding().as_bytes()
        && capability.provenance.frontend_unit == *source.frontend_unit().as_bytes()
        && capability.provenance.kernel_marker == *source.kernel_marker().as_bytes()
        && capability.provenance.target_brand == *source.target_brand().as_bytes()
        && capability.provenance.launch_brand == *source.launch_brand().as_bytes()
        && capability.provenance.issuance == *source.issuance().as_bytes()
        && capability.workgroup_brand == contract.workgroup_brand().map(|id| *id.as_bytes())
        && capability.epoch == contract.epoch_before().map(|id| *id.as_bytes())
        && contract.epoch_after().is_none()
        && subgroup_partition_transport_role_v1(contract).as_ref() == Some(&capability.role)
}

fn subgroup_partition_transport_type_v1(
    types: &[SemanticTypeDeclV1],
    semantic_type: SemanticTypeIdV1,
    contract: SemanticExecutionCapabilityContractV1,
    context: Option<&KernelContextTypeV1>,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    let context = context.ok_or_else(|| {
        unsupported(
            0,
            None,
            None,
            "subgroup partition SSA transport lacks authenticated kernel context",
        )
    })?;
    let source = contract.provenance();
    if semantic_type != contract.signature().output()
        || context.kernel_marker() != source.kernel_marker().as_bytes()
        || context.target() != source.target_brand().as_bytes()
        || context.launch() != source.launch_brand().as_bytes()
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let capability = ExecutionCapabilityTypeV1 {
        source_type: execution_type_identity_v1(types, semantic_type)?,
        provenance: ExecutionCapabilityProvenanceV1 {
            root: context.root().clone(),
            kernel_binding: *source.kernel_binding().as_bytes(),
            frontend_unit: *source.frontend_unit().as_bytes(),
            kernel_marker: *source.kernel_marker().as_bytes(),
            target_brand: *source.target_brand().as_bytes(),
            launch_brand: *source.launch_brand().as_bytes(),
            issuance: *source.issuance().as_bytes(),
        },
        workgroup_brand: contract.workgroup_brand().map(|id| *id.as_bytes()),
        epoch: contract.epoch_before().map(|id| *id.as_bytes()),
        role: subgroup_partition_transport_role_v1(contract)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
    };
    if !subgroup_partition_transport_matches_v1(contract, &capability) {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(Type::ExecutionCapability(capability))
}

fn register_subgroup_partition_transports_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    bindings: &mut BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let contracts = callables
        .iter()
        .filter_map(|callable| match callable {
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            } => Some(*contract),
            _ => None,
        })
        .collect::<Vec<_>>();
    for contract in &contracts {
        if subgroup_partition_transport_role_v1(*contract).is_none()
            || !contracts.iter().any(|partition| {
                matches!(
                    partition.operation(),
                    SemanticExecutionCapabilityOperationV1::SubgroupPartition(_)
                        | SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed { .. }
                ) && partition.provenance() == contract.provenance()
                    && partition.workgroup_brand() == contract.workgroup_brand()
                    && partition.epoch_before() == contract.epoch_before()
            })
        {
            continue;
        }
        let semantic_type = contract.signature().output();
        insert_compiler_issued_ssa_binding_v1(
            bindings,
            semantic_type,
            SemanticPromotedBindingV1::SubgroupPartitionAuthority {
                contract: *contract,
                source_type: execution_type_identity_v1(types, semantic_type)?,
            },
        )?;
    }
    BorrowedSubgroupTransportV1::register(types, callables, bindings)
}

#[cfg(test)]
mod subgroup_partition_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    include!("subgroup_partition_01/ordered_max_tests.rs");

    fn types() -> Vec<SemanticTypeDeclV1> {
        (0..7)
            .map(|index| {
                let shape = match index {
                    5 => SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
                    6 => SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32,
                    }),
                    _ => SemanticTypeShapeV1::Aggregate(
                        SemanticAggregateTypeV1::new(vec![]).unwrap(),
                    ),
                };
                SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256([index + 1; 32]),
                    SemanticLayoutIdentityV1::from_sha256([index + 1; 32]),
                    SemanticTypeLayoutV1::new(
                        Some(if index >= 5 { 4 } else { 0 }),
                        if index >= 5 { 4 } else { 1 },
                    )
                    .unwrap(),
                    shape,
                )
            })
            .collect()
    }

    fn contract() -> SemanticExecutionCapabilityContractV1 {
        let ty = SemanticTypeIdV1::from_index;
        SemanticExecutionCapabilityContractV1::new(
            SemanticExecutionCapabilityOperationV1::SubgroupPartition(
                SemanticSubgroupPartitionOperationV1::Derive {
                    subgroup_reference: ty(0),
                    subgroup: ty(1),
                    epoch: ty(2),
                    partition: ty(3),
                    width: 64,
                    partition_width: 16,
                },
            ),
            SemanticExecutionCapabilitySignatureV1::new(&[ty(0), ty(2)], ty(3)).unwrap(),
            SemanticKernelCapabilityProvenanceV1::new(
                SemanticFunctionIdV1::from_index(0),
                SemanticKernelBindingIdentityV1::from_sha256([10; 32]),
                SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([11; 32]),
                SemanticTypeIdentityV1::from_sha256([12; 32]),
                SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([13; 32]),
                SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([14; 32]),
                SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([15; 32]),
            )
            .unwrap(),
            SemanticTypeIdentityV1::from_sha256([16; 32]),
            SemanticTypeIdentityV1::from_sha256([17; 32]),
            None,
            SemanticFunctionIdentityV1::from_sha256([18; 32]),
        )
        .unwrap()
    }

    #[test]
    fn partition_lowering_preserves_nominal_types_and_exact_scalars() {
        let types = types();
        let ty = SemanticTypeIdV1::from_index;
        let operation = SemanticSubgroupPartitionOperationV1::BroadcastF32 {
            partition_reference: ty(4),
            partition: ty(3),
            element: ty(5),
            source_lane: ty(6),
            width: 64,
            partition_width: 16,
        };
        let lowered = lower_subgroup_partition_operation_v1(&types, operation).unwrap();
        assert_eq!(
            lowered,
            fe2o3_kernel_ir::SubgroupPartitionOperationV1::BroadcastF32 {
                partition_reference: execution_type_identity_v1(&types, ty(4)).unwrap(),
                partition: execution_type_identity_v1(&types, ty(3)).unwrap(),
                element: execution_type_identity_v1(&types, ty(5)).unwrap(),
                source_lane: execution_type_identity_v1(&types, ty(6)).unwrap(),
                width: 64,
                partition_width: 16,
            }
        );
        for (element, source_lane, partition_width) in [(5, 5, 16), (6, 6, 16), (5, 6, 0)] {
            let bad = SemanticSubgroupPartitionOperationV1::BroadcastF32 {
                partition_reference: ty(4),
                partition: ty(3),
                element: ty(element),
                source_lane: ty(source_lane),
                width: 64,
                partition_width,
            };
            assert!(lower_subgroup_partition_operation_v1(&types, bad).is_err());
        }
    }

    #[test]
    fn partition_ssa_transport_rejects_fabricated_or_rebranded_authority() {
        let types = types();
        let contract = contract();
        let semantic_type = contract.signature().output();
        let context = KernelContextTypeV1::new("partition_entry", [12; 32], [13; 32], [14; 32]);
        let expected =
            subgroup_partition_transport_type_v1(&types, semantic_type, contract, Some(&context))
                .unwrap();
        let binding = SemanticPromotedBindingV1::SubgroupPartitionAuthority {
            contract,
            source_type: execution_type_identity_v1(&types, semantic_type).unwrap(),
        };
        let transport = SemanticPromotedTransportV1::Semantic(binding);
        assert!(
            binding
                .transport_values(&SemanticValueBindingV1::Aggregate(vec![]))
                .is_err()
        );
        assert!(
            binding
                .transport_values(&SemanticValueBindingV1::CollectiveContext)
                .is_err()
        );
        assert!(
            subgroup_partition_transport_type_v1(&types, semantic_type, contract, None).is_err()
        );
        let good = SemanticValueBindingV1::Value {
            id: ValueId(7),
            ty: expected.clone(),
        };
        assert_eq!(
            transport
                .transport_values(&good, std::slice::from_ref(&expected))
                .unwrap(),
            vec![(ValueId(7), expected.clone())]
        );
        let restored = transport
            .binding_from_transport(
                &types,
                semantic_type,
                &[ValueDef::new(ValueId(8), expected.clone())],
                std::slice::from_ref(&expected),
            )
            .unwrap();
        assert_eq!(
            binding.transport_values(&restored).unwrap(),
            vec![(ValueId(8), expected.clone())]
        );
        for mutate in [
            (|cap: &mut ExecutionCapabilityTypeV1| cap.epoch = Some([99; 32]))
                as fn(&mut ExecutionCapabilityTypeV1),
            |cap| cap.workgroup_brand = Some([99; 32]),
            |cap| cap.provenance.root = fe2o3_kernel_ir::FunctionId::new("other"),
            |cap| cap.provenance.issuance = [99; 32],
            |cap| cap.source_type = ExecutionTypeIdentityV1::new([99; 32]),
            |cap| {
                cap.role = ExecutionCapabilityRoleV1::SubgroupPartition {
                    width: 64,
                    partition_width: 32,
                }
            },
        ] {
            let Type::ExecutionCapability(mut capability) = expected.clone() else {
                unreachable!()
            };
            mutate(&mut capability);
            let bad = SemanticValueBindingV1::Value {
                id: ValueId(7),
                ty: Type::ExecutionCapability(capability),
            };
            assert!(
                transport
                    .transport_values(&bad, std::slice::from_ref(&expected))
                    .is_err()
            );
        }
    }
}
