use super::*;
use ExecutionCapabilityOperationV1 as Op;
use NumericalPolicyMathOperationV1 as Math;
use SubgroupPartitionOperationV1 as Partition;

const FP32_FUNCTIONS: [F32MathFunction; 13] = [
    F32MathFunction::Sqrt,
    F32MathFunction::FusedMultiplyAdd,
    F32MathFunction::Floor,
    F32MathFunction::Ceil,
    F32MathFunction::Truncate,
    F32MathFunction::RoundTiesEven,
    F32MathFunction::Sin,
    F32MathFunction::Cos,
    F32MathFunction::Exp,
    F32MathFunction::Exp2,
    F32MathFunction::Ln,
    F32MathFunction::Log2,
    F32MathFunction::Log10,
];

fn math_binding() -> NumericalPolicyMathBindingV1 {
    NumericalPolicyMathBindingV1 {
        math_reference: id(0xc0),
        math: id(0xc1),
        policy_reference: id(0xc2),
        capability: id(0xc3),
        bound: id(0xc4),
        bound_reference: id(0xc5),
        policy: id(0xc6),
        kernel_brand: id(0xc7),
        mode: NumericalModeV1::StrictIeee,
    }
}

pub(super) fn catalog() -> Vec<Op> {
    let binding = math_binding();
    let mut operations = vec![
        Op::NumericalPolicyIssue {
            context: id(0x10),
            capability: binding.capability,
            policy: binding.policy,
            mode: binding.mode,
        },
        Op::SubgroupPartition(Partition::Derive {
            subgroup_reference: id(0x2a),
            subgroup: id(0x12),
            epoch: id(0x2b),
            partition: id(0xd0),
            width: 64,
            partition_width: 8,
        }),
        Op::SubgroupPartition(Partition::ReduceSumF32 {
            partition_reference: id(0xd1),
            partition: id(0xd0),
            element: id(0xd2),
            width: 64,
            partition_width: 8,
        }),
        Op::SubgroupPartition(Partition::ReduceMaxF32 {
            partition_reference: id(0xd1),
            partition: id(0xd0),
            element: id(0xd2),
            width: 64,
            partition_width: 8,
        }),
        Op::SubgroupPartition(Partition::BroadcastF32 {
            partition_reference: id(0xd1),
            partition: id(0xd0),
            element: id(0xd2),
            source_lane: id(0xd3),
            width: 64,
            partition_width: 8,
        }),
        Op::SubgroupDeriveBorrowed {
            workgroup_reference: id(0xd4),
            workgroup: id(0x11),
            subgroup: id(0x12),
            width: 64,
        },
        Op::NumericalPolicyMath(Math::MathDerive {
            context: id(0x10),
            binding,
        }),
        Op::NumericalPolicyMath(Math::Bind { binding }),
    ];
    operations.extend(FP32_FUNCTIONS.map(|function| {
        Op::NumericalPolicyMath(Math::F32 {
            binding,
            bound_reference: binding.bound_reference,
            element: id(0xc8),
            function,
        })
    }));
    operations
}

pub(super) fn partition_signature(
    operation: Partition,
) -> (Vec<ExecutionTypeIdentityV1>, ExecutionTypeIdentityV1) {
    match operation {
        Partition::Derive {
            subgroup_reference,
            epoch,
            partition,
            ..
        } => (vec![subgroup_reference, epoch], partition),
        Partition::ReduceSumF32 {
            partition_reference,
            element,
            ..
        }
        | Partition::ReduceMaxF32 {
            partition_reference,
            element,
            ..
        } => (vec![partition_reference, element], element),
        Partition::BroadcastF32 {
            partition_reference,
            element,
            source_lane,
            ..
        } => (vec![partition_reference, element, source_lane], element),
    }
}

pub(super) fn math_signature(
    operation: Math,
) -> (Vec<ExecutionTypeIdentityV1>, ExecutionTypeIdentityV1) {
    match operation {
        Math::MathDerive { context, binding } => (vec![context], binding.math),
        Math::Bind { binding } => (
            vec![binding.math_reference, binding.policy_reference],
            binding.bound,
        ),
        Math::F32 {
            bound_reference,
            element,
            function,
            ..
        } => {
            let mut arguments = vec![bound_reference];
            arguments.extend(std::iter::repeat_n(element, function.arity()));
            (arguments, element)
        }
    }
}

pub(super) fn partition_results(operation: Partition, epoch: [u8; 32]) -> Vec<Type> {
    match operation {
        Partition::Derive {
            partition,
            width,
            partition_width,
            ..
        } => vec![capability_type_at(
            partition,
            ExecutionCapabilityRoleV1::SubgroupPartition {
                width,
                partition_width,
            },
            &Op::SubgroupPartition(operation),
            epoch,
        )],
        Partition::ReduceSumF32 { .. } | Partition::ReduceMaxF32 { .. } | Partition::BroadcastF32 { .. } => vec![Type::F32],
    }
}

pub(super) fn math_results(operation: Math, epoch: [u8; 32]) -> Vec<Type> {
    match operation {
        Math::MathDerive { binding, .. } => vec![capability_type_at(
            binding.math,
            ExecutionCapabilityRoleV1::NumericalPolicyMathSource(binding),
            &Op::NumericalPolicyMath(operation),
            epoch,
        )],
        Math::Bind { binding } => vec![capability_type_at(
            binding.bound,
            ExecutionCapabilityRoleV1::NumericalPolicyMathBound(binding),
            &Op::NumericalPolicyMath(operation),
            epoch,
        )],
        Math::F32 { .. } => vec![Type::F32],
    }
}

impl CatalogModuleBuilder {
    fn partition_inputs(&mut self, subgroup: ExecutionTypeIdentityV1, width: u32) -> Vec<ValueId> {
        let owner = self.workgroup(id(0x11), EPOCH_BEFORE);
        let subgroup = self.emit(
            Op::SubgroupDerive {
                workgroup: id(0x11),
                subgroup,
                width,
            },
            vec![owner],
            EPOCH_BEFORE,
        )[0];
        vec![subgroup, owner]
    }

    pub(super) fn partition_operands(&mut self, operation: Partition) -> (Vec<ValueId>, [u8; 32]) {
        match operation {
            Partition::Derive {
                subgroup, width, ..
            } => (self.partition_inputs(subgroup, width), EPOCH_BEFORE),
            Partition::ReduceSumF32 {
                partition,
                width,
                partition_width,
                ..
            }
            | Partition::ReduceMaxF32 {
                partition,
                width,
                partition_width,
                ..
            }
            | Partition::BroadcastF32 {
                partition,
                width,
                partition_width,
                ..
            } => {
                let inputs = self.partition_inputs(id(0x12), width);
                let receiver = self.emit(
                    Op::SubgroupPartition(Partition::Derive {
                        subgroup_reference: id(0x2a),
                        subgroup: id(0x12),
                        epoch: id(0x2b),
                        partition,
                        width,
                        partition_width,
                    }),
                    inputs,
                    EPOCH_BEFORE,
                )[0];
                let value = self.constant(Type::F32, Constant::F32Bits(0x3f80_0000));
                let mut operands = vec![receiver, value];
                if matches!(operation, Partition::BroadcastF32 { .. }) {
                    operands.push(self.constant(Type::Scalar(ScalarType::U32), Constant::U32(0)));
                }
                (operands, EPOCH_BEFORE)
            }
        }
    }

    fn policy_math_inputs(&mut self, binding: NumericalPolicyMathBindingV1) -> Vec<ValueId> {
        let math = self.emit(
            Op::NumericalPolicyMath(Math::MathDerive {
                context: id(0x10),
                binding,
            }),
            vec![self.context],
            EPOCH_BEFORE,
        )[0];
        let policy = self.emit(
            Op::NumericalPolicyIssue {
                context: id(0x10),
                capability: binding.capability,
                policy: binding.policy,
                mode: binding.mode,
            },
            vec![self.context],
            EPOCH_BEFORE,
        )[0];
        vec![math, policy]
    }

    pub(super) fn math_operands(&mut self, operation: Math) -> (Vec<ValueId>, [u8; 32]) {
        match operation {
            Math::MathDerive { .. } => (vec![self.context], EPOCH_BEFORE),
            Math::Bind { binding } => (self.policy_math_inputs(binding), EPOCH_BEFORE),
            Math::F32 {
                binding, function, ..
            } => {
                let inputs = self.policy_math_inputs(binding);
                let bound = self.emit(
                    Op::NumericalPolicyMath(Math::Bind { binding }),
                    inputs,
                    EPOCH_BEFORE,
                )[0];
                let mut operands = vec![bound];
                for index in 0..function.arity() {
                    operands.push(self.constant(
                        Type::F32,
                        Constant::F32Bits((1.0_f32 + index as f32).to_bits()),
                    ));
                }
                (operands, EPOCH_BEFORE)
            }
        }
    }
}

pub(super) fn partition_capabilities(operation: Partition) -> BTreeSet<TargetCapability> {
    let (width, _) = operation.widths();
    let mut requirements = BTreeSet::from([
        TargetCapability::Subgroups,
        TargetCapability::SubgroupSize(width),
    ]);
    match operation {
        Partition::Derive { .. } => {}
        Partition::ReduceSumF32 { .. } | Partition::ReduceMaxF32 { .. } | Partition::BroadcastF32 { .. } => {
            assert_eq!(width, 64, "catalog partition width");
            requirements.insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
        }
    }
    requirements
}

pub(super) fn math_capabilities(operation: Math) -> BTreeSet<TargetCapability> {
    match operation {
        Math::MathDerive { .. } | Math::Bind { .. } => BTreeSet::new(),
        Math::F32 { .. } => {
            BTreeSet::from([capability(ExecutionCapabilityRequirementV1::Numerical {
                value_type: ScalarType::F32,
                mode: NumericalModeV1::StrictIeee,
            })])
        }
    }
}

pub(super) fn mutate_partition(operation: &mut Partition) {
    match operation {
        Partition::Derive {
            partition_width, ..
        }
        | Partition::ReduceSumF32 {
            partition_width, ..
        }
        | Partition::ReduceMaxF32 {
            partition_width, ..
        }
        | Partition::BroadcastF32 {
            partition_width, ..
        } => *partition_width = 4,
    }
}

pub(super) fn mutate_math(operation: &mut Math) {
    match operation {
        Math::MathDerive { binding, .. } | Math::Bind { binding } | Math::F32 { binding, .. } => {
            binding.policy = id(0xe3)
        }
    }
}

pub(super) fn wire_header(operation: &Op) -> [u8; 2] {
    match operation {
        Op::WorkgroupDerive { .. } => [1, 0],
        Op::SubgroupDerive { .. } => [1, 1],
        Op::LdsAllocateBorrowed { .. } => [3, 30],
        Op::LdsAllocate { .. } => [1, 2],
        Op::LdsInitializeByInvocation { .. } => [1, 3],
        Op::LdsPublish { .. } => [1, 4],
        Op::LdsReadPublished { .. } => [1, 5],
        Op::WorkgroupBarrier { .. } => [1, 6],
        Op::SubgroupBarrier { .. } => [1, 7],
        Op::WorkgroupFence { .. } => [1, 8],
        Op::SubgroupFence { .. } => [1, 9],
        Op::Atomic { .. } => [1, 10],
        Op::WorkgroupCollective { .. } => [1, 11],
        Op::SubgroupCollective { .. } => [1, 12],
        Op::MatrixAccess { .. } => [1, 13],
        Op::AsyncCopy { .. } => [1, 14],
        Op::AsyncWait { .. } => [1, 15],
        Op::RawMemoryBind { .. } => [1, 16],
        Op::PrivateMemoryAllocate { .. } => [1, 17],
        Op::WorkgroupMemoryIndex { .. } => [1, 18],
        Op::WorkgroupMemoryIndexV2 { .. } => [1, 27],
        Op::WorkgroupMemoryIndexIntoDisjoint { .. } => [1, 28],
        Op::WorkgroupMemoryAllocate { .. } => [1, 19],
        Op::WorkgroupMemoryPublish { .. } => [1, 20],
        Op::MemoryLoad { .. } => [1, 21],
        Op::MemoryStore { .. } => [1, 22],
        Op::NumericalPolicyIssue { .. } => [2, 23],
        Op::SubgroupPartition(_) => [2, 24],
        Op::SubgroupDeriveBorrowed { .. } => [3, 25],
        Op::NumericalPolicyMath(_) => [4, 26],
        Op::ReusableLdsConversion(_) => [6, 29],
    }
}

pub(super) fn type_revision(role: &ExecutionCapabilityRoleV1) -> u8 {
    use ExecutionCapabilityRoleV1 as Role;
    match role {
        Role::ReusableWorkgroup | Role::ReusablePhaseCompletion => panic!("phase roles are outside this legacy V13 fixture"),
        Role::KernelAuthority
        | Role::Workgroup
        | Role::Subgroup { .. }
        | Role::Lds { .. }
        | Role::ScopedAtomic { .. }
        | Role::Matrix { .. }
        | Role::PendingAsyncCopy { .. }
        | Role::MemoryView { .. }
        | Role::WorkgroupMemoryIndex
        | Role::EpochTransition
        | Role::UnsafeRawMemoryObligation => 1,
        Role::NumericalPolicy { .. } | Role::SubgroupPartition { .. } => 2,
        Role::BorrowedSubgroup { .. } => 3,
        Role::NumericalPolicyMathSource(_) | Role::NumericalPolicyMathBound(_) => 4,
        Role::ReusableLds { .. } => 6,
    }
}

#[test]
fn nested_partition_and_policy_math_rosters_are_complete() {
    let operations = catalog();
    assert_eq!(operations.len(), 21);
    let partitions = operations
        .iter()
        .filter_map(|op| {
            let Op::SubgroupPartition(operation) = op else {
                return None;
            };
            Some(*operation)
        })
        .collect::<Vec<_>>();
    assert_eq!(partitions.len(), 4);
    assert!(
        partitions
            .iter()
            .any(|op| matches!(op, Partition::Derive { .. }))
    );
    assert!(
        partitions
            .iter()
            .any(|op| matches!(op, Partition::ReduceSumF32 { .. }))
    );
    assert!(
        partitions
            .iter()
            .any(|op| matches!(op, Partition::BroadcastF32 { .. }))
    );
    assert!(partitions.iter().any(|op| matches!(op, Partition::ReduceMaxF32 { .. })));
    let consumers = operations
        .iter()
        .filter_map(|op| {
            let Op::NumericalPolicyMath(Math::F32 { function, .. }) = op else {
                return None;
            };
            Some(*function)
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(consumers, FP32_FUNCTIONS.into_iter().collect());
    assert_eq!(consumers.len(), 13);
    assert!(
        operations
            .iter()
            .any(|op| matches!(op, Op::NumericalPolicyMath(Math::MathDerive { .. })))
    );
    assert!(
        operations
            .iter()
            .any(|op| matches!(op, Op::NumericalPolicyMath(Math::Bind { .. })))
    );
}
