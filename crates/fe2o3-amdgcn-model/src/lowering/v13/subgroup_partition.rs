use super::*;
use fe2o3_kernel_ir::{SubgroupPartitionOperationV1 as P, WaveF32ReductionKindV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PartitionBinding {
    width: u32,
    partition_width: u32,
    brand: [u8; 32],
    epoch: [u8; 32],
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    module: &Module,
    operation: &Operation,
    contract: &ExecutionCapabilityOpV1,
    partition: P,
    original_types: &BTreeMap<ValueId, Type>,
    lowered_types: &mut BTreeMap<ValueId, Type>,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
    wave_width: &mut Option<WaveWidth>,
    output: &mut Vec<Operation>,
) -> Result<(), LoweringErrors> {
    let reject = || {
        incomplete(
            module,
            "partition lost its exact receiver, epoch, or scalar transport",
        )
    };
    if !contract.is_complete()
        || contract.operation != ExecutionCapabilityOperationV1::SubgroupPartition(partition)
    {
        return Err(reject());
    }
    let (width, partition_width) = partition.widths();
    let wave = require_wave(module, width, wave_width)?;
    let binding = PartitionBinding {
        width,
        partition_width,
        brand: contract.workgroup_brand.ok_or_else(reject)?,
        epoch: contract.epoch_before.ok_or_else(reject)?,
    };
    let matches_role = |value: &ValueId, role| {
        matches!(original_types.get(value),
        Some(Type::ExecutionCapability(capability))
            if capability.is_complete() && capability.role == role
                && capability.provenance == contract.provenance
                && capability.workgroup_brand == contract.workgroup_brand
                && capability.epoch == contract.epoch_before)
    };
    if let P::Derive {
        partition: source_type,
        ..
    } = partition
    {
        let [subgroup, workgroup] = contract.operands.as_slice() else {
            return Err(reject());
        };
        if !matches!(aliases.get(subgroup), Some(ExecutionAliasV1::Erased))
            || !matches!(aliases.get(workgroup), Some(ExecutionAliasV1::Erased))
            || !(matches_role(subgroup, ExecutionCapabilityRoleV1::Subgroup { width })
                || matches!(original_types.get(subgroup), Some(Type::ExecutionCapability(c))
                    if matches!(c.role, ExecutionCapabilityRoleV1::BorrowedSubgroup { width: w, .. } if w == width)
                        && matches_role(subgroup, c.role.clone())))
            || !matches_role(workgroup, ExecutionCapabilityRoleV1::Workgroup)
        {
            return Err(reject());
        }
        let result = sole_capability_result(module, operation)?;
        let Type::ExecutionCapability(capability) = &result.ty else {
            return Err(reject());
        };
        if capability.source_type != source_type
            || !matches_role(
                &result.id,
                ExecutionCapabilityRoleV1::SubgroupPartition {
                    width,
                    partition_width,
                },
            )
        {
            return Err(reject());
        }
        aliases.insert(result.id, ExecutionAliasV1::Partition(binding));
        return Ok(());
    }
    let receiver = contract.operands.first().ok_or_else(reject)?;
    if aliases.get(receiver) != Some(&ExecutionAliasV1::Partition(binding))
        || !matches_role(
            receiver,
            ExecutionCapabilityRoleV1::SubgroupPartition {
                width,
                partition_width,
            },
        )
    {
        return Err(reject());
    }
    let physical = physical_operands(contract, original_types, lowered_types, aliases);
    let kind = match (partition, physical.as_slice()) {
        (P::ReduceSumF32 { .. }, [(value, Type::Scalar(ScalarType::F32))]) => {
            WaveOperationKind::ReduceF32 {
                value: *value,
                tile_width: partition_width,
                kind: WaveF32ReductionKindV1::Sum,
            }
        }
        (P::ReduceMaxF32 { .. }, [(value, Type::Scalar(ScalarType::F32))]) => {
            WaveOperationKind::ReduceF32 {
                value: *value,
                tile_width: partition_width,
                kind: WaveF32ReductionKindV1::Maximum,
            }
        }
        (
            P::BroadcastF32 { .. },
            [
                (value, Type::Scalar(ScalarType::F32)),
                (lane, Type::Scalar(ScalarType::U32)),
            ],
        ) => WaveOperationKind::BroadcastF32 {
            value: *value,
            source_lane: *lane,
            tile_width: partition_width,
        },
        _ => return Err(reject()),
    };
    let [result] = operation.results.as_slice() else {
        return Err(reject());
    };
    if result.ty != Type::Scalar(ScalarType::F32) {
        return Err(reject());
    }
    // The caller requires canonical verification and convergence/participation analysis.
    // The original lane SSA is retained, so Wave verification rechecks its actual bound.
    output.push(Operation::effect_free(
        result.clone(),
        OperationKind::Wave(WaveOperation::full(kind, wave)),
    ));
    lowered_types.insert(result.id, result.ty.clone());
    Ok(())
}

#[cfg(test)]
#[path = "subgroup_partition/ordered_max_tests.rs"]
mod ordered_max_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::*;

    #[allow(dead_code)]
    mod fixture {
        use super::*;
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fe2o3-kernel-ir/src/execution_capability_v1/subgroup_partition/fixture.rs"
        ));
        pub(super) fn module_fixture() -> Module {
            module()
        }
    }

    #[test]
    fn partition_backend_retains_original_scalar_and_lane_edges() {
        let module = fixture::module_fixture();
        verify_module(&module).unwrap();
        let operations = &module.functions[0].body.as_ref().unwrap().blocks[0].operations;
        let types = value_types(&module.functions[0]);
        let mut lowered = types.clone();
        let mut aliases = BTreeMap::from([
            (ValueId(1), ExecutionAliasV1::Erased),
            (ValueId(2), ExecutionAliasV1::Erased),
        ]);
        let mut wave = None;
        let mut output = Vec::new();
        for index in [3, 6, 7] {
            let operation = &operations[index];
            let OperationKind::ExecutionCapability(contract) = &operation.kind else {
                unreachable!()
            };
            let ExecutionCapabilityOperationV1::SubgroupPartition(partition) = contract.operation
            else {
                unreachable!()
            };
            lower(
                &module,
                operation,
                contract,
                partition,
                &types,
                &mut lowered,
                &mut aliases,
                &mut wave,
                &mut output,
            )
            .unwrap();
        }
        assert_eq!(wave, Some(WaveWidth::Wave64));
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].results, operations[6].results);
        assert_eq!(output[1].results, operations[7].results);
        assert_eq!(
            output[0].kind,
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::ReduceF32 {
                    value: ValueId(4),
                    tile_width: 16,
                    kind: WaveF32ReductionKindV1::Sum
                },
                WaveWidth::Wave64,
            ))
        );
        assert_eq!(
            output[1].kind,
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::BroadcastF32 {
                    value: ValueId(6),
                    source_lane: ValueId(5),
                    tile_width: 16
                },
                WaveWidth::Wave64,
            ))
        );
        assert!(matches!(
            aliases.get(&ValueId(3)),
            Some(ExecutionAliasV1::Partition(_))
        ));
    }

    #[test]
    fn partition_backend_rejects_unissued_or_substituted_receivers() {
        let module = fixture::module_fixture();
        let operations = &module.functions[0].body.as_ref().unwrap().blocks[0].operations;
        let types = value_types(&module.functions[0]);
        for index in [3, 6, 7] {
            let operation = &operations[index];
            let OperationKind::ExecutionCapability(contract) = &operation.kind else {
                unreachable!()
            };
            let ExecutionCapabilityOperationV1::SubgroupPartition(partition) = contract.operation
            else {
                unreachable!()
            };
            assert!(
                lower(
                    &module,
                    operation,
                    contract,
                    partition,
                    &types,
                    &mut types.clone(),
                    &mut BTreeMap::new(),
                    &mut None,
                    &mut Vec::new()
                )
                .is_err()
            );
        }
        let operation = &operations[3];
        let OperationKind::ExecutionCapability(original) = &operation.kind else {
            unreachable!()
        };
        for mutate in [
            (|contract: &mut ExecutionCapabilityOpV1| contract.operands.clear())
                as fn(&mut ExecutionCapabilityOpV1),
            |contract| contract.operands.swap(0, 1),
            |contract| contract.epoch_before = Some([99; 32]),
            |contract| contract.provenance.issuance = [99; 32],
        ] {
            let mut contract = original.clone();
            mutate(&mut contract);
            let ExecutionCapabilityOperationV1::SubgroupPartition(partition) = contract.operation
            else {
                unreachable!()
            };
            let mut aliases = BTreeMap::from([
                (ValueId(1), ExecutionAliasV1::Erased),
                (ValueId(2), ExecutionAliasV1::Erased),
            ]);
            assert!(
                lower(
                    &module,
                    operation,
                    &contract,
                    partition,
                    &types,
                    &mut types.clone(),
                    &mut aliases,
                    &mut None,
                    &mut Vec::new()
                )
                .is_err()
            );
        }
    }
}
