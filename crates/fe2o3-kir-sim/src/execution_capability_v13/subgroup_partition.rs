use super::*;
use fe2o3_kernel_ir::{
    ExecutionCapabilityRoleV1 as Role, SubgroupPartitionOperationV1 as P, WaveF32ReductionKindV1,
};

const MAX_PARTITIONS: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Binding {
    width: u32,
    partition_width: u32,
    brand: [u8; 32],
    epoch: [u8; 32],
}

#[derive(Default)]
pub(super) struct Bindings(Vec<(ValueId, Binding)>);

impl Bindings {
    pub(super) fn scratch_bytes(&self) -> Result<usize, ExecutionCapabilityProjectionErrorV13> {
        self.0
            .capacity()
            .checked_mul(size_of::<(ValueId, Binding)>())
            .ok_or(ExecutionCapabilityProjectionErrorV13::AllocationFailure)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    partition: P,
    types: &BTreeMap<ValueId, Type>,
    aliases: &[(ValueId, ValueId)],
    promoted: &BTreeMap<ValueId, Type>,
    bindings: &mut Bindings,
    workgroup: Option<fe2o3_kernel_ir::WorkgroupSize>,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = || {
        ExecutionCapabilityProjectionErrorV13::Incomplete(
            IncompleteExecutionCapabilityOperationV13::SubgroupPartition,
        )
    };
    if !contract.is_complete()
        || contract.operation
            != fe2o3_kernel_ir::ExecutionCapabilityOperationV1::SubgroupPartition(partition)
    {
        return Err(incomplete());
    }
    let (width, partition_width) = partition.widths();
    let binding = Binding {
        width,
        partition_width,
        brand: contract.workgroup_brand.ok_or_else(incomplete)?,
        epoch: contract.epoch_before.ok_or_else(incomplete)?,
    };
    let matches_role = |value: &ValueId, role| {
        matches!(types.get(value),
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
            return Err(incomplete());
        };
        if !(matches_role(subgroup, Role::Subgroup { width })
            || matches!(types.get(subgroup), Some(Type::ExecutionCapability(c))
                if matches!(c.role, Role::BorrowedSubgroup { width: w, .. } if w == width)
                    && matches_role(subgroup, c.role.clone())))
            || !matches_role(workgroup, Role::Workgroup)
        {
            return Err(incomplete());
        }
        require_only_logical_results(&results)?;
        let [result] = results.as_slice() else {
            return Err(incomplete());
        };
        let Type::ExecutionCapability(capability) = &result.ty else {
            return Err(incomplete());
        };
        if capability.source_type != source_type
            || !matches_role(
                &result.id,
                Role::SubgroupPartition {
                    width,
                    partition_width,
                },
            )
        {
            return Err(incomplete());
        }
        if bindings.0.len() == MAX_PARTITIONS
            || bindings.0.iter().any(|(value, _)| *value == result.id)
        {
            return Err(incomplete());
        }
        bindings
            .0
            .try_reserve(1)
            .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
        bindings.0.push((result.id, binding));
        return Ok(Vec::new());
    }
    let receiver = contract.operands.first().ok_or_else(incomplete)?;
    if !bindings.0.contains(&(*receiver, binding))
        || !matches_role(
            receiver,
            Role::SubgroupPartition {
                width,
                partition_width,
            },
        )
    {
        return Err(incomplete());
    }
    let workgroup = workgroup.ok_or_else(incomplete)?;
    let wave = wave_width(width).ok_or_else(incomplete)?;
    if workgroup.y != 1 || workgroup.z != 1 || workgroup.x == 0 || workgroup.x % width != 0 {
        return Err(incomplete());
    }
    let physical = physical_operands(contract, types, aliases, promoted)?;
    let scalar = |value: &ValueId, ty| {
        types.get(value).or_else(|| promoted.get(value)) == Some(&Type::Scalar(ty))
    };
    let kind = match (partition, physical.as_slice()) {
        (P::ReduceSumF32 { .. }, [value]) if scalar(value, ScalarType::F32) => {
            WaveOperationKind::ReduceF32 {
                value: *value,
                tile_width: partition_width,
                kind: WaveF32ReductionKindV1::Sum,
            }
        }
        (P::ReduceMaxF32 { .. }, [value]) if scalar(value, ScalarType::F32) => {
            WaveOperationKind::ReduceF32 {
                value: *value,
                tile_width: partition_width,
                kind: WaveF32ReductionKindV1::Maximum,
            }
        }
        (P::BroadcastF32 { .. }, [value, lane])
            if scalar(value, ScalarType::F32) && scalar(lane, ScalarType::U32) =>
        {
            WaveOperationKind::BroadcastF32 {
                value: *value,
                source_lane: *lane,
                tile_width: partition_width,
            }
        }
        _ => return Err(incomplete()),
    };
    let result = sole_scalar_result(&results, ScalarType::F32).ok_or_else(incomplete)?;
    Ok(vec![Operation::effect_free(
        ValueDef::new(result, Type::Scalar(ScalarType::F32)),
        OperationKind::Wave(WaveOperation::full(kind, wave)),
    )])
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
    fn partition_projection_retains_scalar_and_lane_without_masks() {
        let module = fixture::module_fixture();
        verify_module(&module).unwrap();
        let function = &module.functions[0];
        let body = function.body.as_ref().unwrap();
        let operations = &body.blocks[0].operations;
        let types = value_types(&function.signature, body).unwrap();
        let mut bindings = Bindings::default();
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
            output.extend(
                project(
                    operation.results.clone(),
                    contract,
                    partition,
                    &types,
                    &[],
                    &BTreeMap::new(),
                    &mut bindings,
                    Some(WorkgroupSize::new(64, 1, 1)),
                )
                .unwrap(),
            );
        }
        assert_eq!(bindings.0.len(), 1);
        assert!(bindings.scratch_bytes().unwrap() >= size_of::<(ValueId, Binding)>());
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
    }

    #[test]
    fn partition_projection_rejects_missing_epoch_and_incomplete_participation() {
        let module = fixture::module_fixture();
        let function = &module.functions[0];
        let body = function.body.as_ref().unwrap();
        let operations = &body.blocks[0].operations;
        let types = value_types(&function.signature, body).unwrap();
        let derive = &operations[3];
        let OperationKind::ExecutionCapability(original) = &derive.kind else {
            unreachable!()
        };
        for mutate in [
            (|contract: &mut ExecutionCapabilityOpV1| contract.operands.clear())
                as fn(&mut ExecutionCapabilityOpV1),
            |contract| contract.operands.swap(0, 1),
            |contract| contract.epoch_before = Some([99; 32]),
        ] {
            let mut contract = original.clone();
            mutate(&mut contract);
            let ExecutionCapabilityOperationV1::SubgroupPartition(partition) = contract.operation
            else {
                unreachable!()
            };
            assert!(
                project(
                    derive.results.clone(),
                    &contract,
                    partition,
                    &types,
                    &[],
                    &BTreeMap::new(),
                    &mut Bindings::default(),
                    Some(WorkgroupSize::new(64, 1, 1))
                )
                .is_err()
            );
        }
        let ExecutionCapabilityOperationV1::SubgroupPartition(partition) = original.operation
        else {
            unreachable!()
        };
        let mut bindings = Bindings::default();
        project(
            derive.results.clone(),
            original,
            partition,
            &types,
            &[],
            &BTreeMap::new(),
            &mut bindings,
            None,
        )
        .unwrap();
        for workgroup in [
            None,
            Some(WorkgroupSize::new(16, 1, 1)),
            Some(WorkgroupSize::new(65, 1, 1)),
            Some(WorkgroupSize::new(32, 2, 1)),
        ] {
            let reduce = &operations[6];
            let OperationKind::ExecutionCapability(contract) = &reduce.kind else {
                unreachable!()
            };
            let ExecutionCapabilityOperationV1::SubgroupPartition(partition) = contract.operation
            else {
                unreachable!()
            };
            assert!(
                project(
                    reduce.results.clone(),
                    contract,
                    partition,
                    &types,
                    &[],
                    &BTreeMap::new(),
                    &mut bindings,
                    workgroup
                )
                .is_err()
            );
        }
    }
}
