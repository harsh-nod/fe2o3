use super::*;

pub(in crate::kfd_backend) fn profile_semantic_contract_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    geometry: KfdProfileLaunchV1,
) -> Option<KfdProfileSemanticContractV1> {
    match semantic_launch {
        KfdRuntimeSemanticLaunchV1::Ordinary => None,
        KfdRuntimeSemanticLaunchV1::Atomic(contract) => Some(KfdProfileSemanticContractV1::Atomic(
            KfdProfileAtomicContractV1 {
                operation: match contract.operation {
                    RuntimeAtomicOperationV1::Add => KfdProfileAtomicOperationV1::Add,
                    RuntimeAtomicOperationV1::Minimum => KfdProfileAtomicOperationV1::Minimum,
                    RuntimeAtomicOperationV1::Maximum => KfdProfileAtomicOperationV1::Maximum,
                    RuntimeAtomicOperationV1::BitwiseAnd => KfdProfileAtomicOperationV1::BitwiseAnd,
                    RuntimeAtomicOperationV1::BitwiseOr => KfdProfileAtomicOperationV1::BitwiseOr,
                    RuntimeAtomicOperationV1::BitwiseXor => KfdProfileAtomicOperationV1::BitwiseXor,
                    RuntimeAtomicOperationV1::Exchange => KfdProfileAtomicOperationV1::Exchange,
                    RuntimeAtomicOperationV1::CompareExchange => {
                        KfdProfileAtomicOperationV1::CompareExchange
                    }
                },
                scope: profile_memory_scope_v1(contract.scope),
                order: profile_memory_order_v1(contract.order),
                failure_order: contract.failure_order.map(profile_memory_order_v1),
                weak: contract.weak,
                geometry,
            },
        )),
        KfdRuntimeSemanticLaunchV1::Collective(contract) => Some(
            KfdProfileSemanticContractV1::Collective(KfdProfileCollectiveContractV1 {
                operation: match contract.operation {
                    crate::RuntimeCollectiveOperationV1::Barrier => {
                        KfdProfileCollectiveOperationV1::Barrier
                    }
                    crate::RuntimeCollectiveOperationV1::Broadcast => {
                        KfdProfileCollectiveOperationV1::Broadcast
                    }
                    crate::RuntimeCollectiveOperationV1::ReduceSum => {
                        KfdProfileCollectiveOperationV1::ReduceSum
                    }
                    crate::RuntimeCollectiveOperationV1::ReduceMinimum => {
                        KfdProfileCollectiveOperationV1::ReduceMinimum
                    }
                    crate::RuntimeCollectiveOperationV1::ReduceMaximum => {
                        KfdProfileCollectiveOperationV1::ReduceMaximum
                    }
                    crate::RuntimeCollectiveOperationV1::AllReduceSum => {
                        KfdProfileCollectiveOperationV1::AllReduceSum
                    }
                    crate::RuntimeCollectiveOperationV1::InclusiveScanSum => {
                        KfdProfileCollectiveOperationV1::InclusiveScanSum
                    }
                },
                scope: profile_memory_scope_v1(contract.scope),
                order: profile_memory_order_v1(contract.order),
                participants: contract.participants,
                geometry,
            }),
        ),
    }
}

pub(in crate::kfd_backend) const fn profile_memory_scope_v1(
    scope: RuntimeMemoryScopeV1,
) -> KfdProfileMemoryScopeV1 {
    match scope {
        RuntimeMemoryScopeV1::Workgroup => KfdProfileMemoryScopeV1::Workgroup,
        RuntimeMemoryScopeV1::Device => KfdProfileMemoryScopeV1::Device,
        RuntimeMemoryScopeV1::System => KfdProfileMemoryScopeV1::System,
    }
}

pub(in crate::kfd_backend) const fn profile_memory_order_v1(
    order: RuntimeMemoryOrderV1,
) -> KfdProfileMemoryOrderV1 {
    match order {
        RuntimeMemoryOrderV1::Relaxed => KfdProfileMemoryOrderV1::Relaxed,
        RuntimeMemoryOrderV1::Acquire => KfdProfileMemoryOrderV1::Acquire,
        RuntimeMemoryOrderV1::Release => KfdProfileMemoryOrderV1::Release,
        RuntimeMemoryOrderV1::AcquireRelease => KfdProfileMemoryOrderV1::AcquireRelease,
        RuntimeMemoryOrderV1::SequentiallyConsistent => {
            KfdProfileMemoryOrderV1::SequentiallyConsistent
        }
    }
}

pub(in crate::kfd_backend) fn dispatch_shape_sha256_v1(
    launch: &BackendLaunchV1<'_>,
    semantic_launch: KfdRuntimeSemanticLaunchV1,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.runtime.kfd.recycled-dispatch-shape.v1\0");
    digest.update(launch.kernel.to_le_bytes());
    for value in launch.geometry.grid {
        digest.update(value.to_le_bytes());
    }
    for value in launch.geometry.workgroup {
        digest.update(value.to_le_bytes());
    }
    digest.update(launch.geometry.dynamic_shared_bytes.to_le_bytes());
    digest.update((launch.explicit_kernarg.len() as u64).to_le_bytes());
    digest.update(launch.explicit_kernarg);
    digest.update((launch.bindings.len() as u64).to_le_bytes());
    for binding in launch.bindings {
        digest.update(binding.region.allocation.to_le_bytes());
        digest.update([match binding.region.access {
            RuntimeAccessV1::Read => 1,
            RuntimeAccessV1::Write => 2,
            RuntimeAccessV1::ReadWrite => 3,
        }]);
        digest.update(binding.region.byte_offset.to_le_bytes());
        digest.update(binding.region.byte_len.to_le_bytes());
        digest.update(binding.kernarg_byte_offset.to_le_bytes());
    }
    match semantic_launch {
        KfdRuntimeSemanticLaunchV1::Ordinary => digest.update([0]),
        KfdRuntimeSemanticLaunchV1::Atomic(contract) => {
            digest.update([1, atomic_operation_tag_v1(contract.operation)]);
            digest.update([memory_scope_tag_v1(contract.scope)]);
            digest.update([memory_order_tag_v1(contract.order)]);
            digest.update([contract
                .failure_order
                .map_or(0, |order| memory_order_tag_v1(order).saturating_add(1))]);
            digest.update([u8::from(contract.weak)]);
        }
        KfdRuntimeSemanticLaunchV1::Collective(contract) => {
            digest.update([2, collective_operation_tag_v1(contract.operation)]);
            digest.update([memory_scope_tag_v1(contract.scope)]);
            digest.update([memory_order_tag_v1(contract.order)]);
            digest.update(contract.participants.to_le_bytes());
        }
    }
    digest.finalize().into()
}

pub(in crate::kfd_backend) const fn atomic_operation_tag_v1(
    operation: RuntimeAtomicOperationV1,
) -> u8 {
    match operation {
        RuntimeAtomicOperationV1::Add => 0,
        RuntimeAtomicOperationV1::Minimum => 1,
        RuntimeAtomicOperationV1::Maximum => 2,
        RuntimeAtomicOperationV1::BitwiseAnd => 3,
        RuntimeAtomicOperationV1::BitwiseOr => 4,
        RuntimeAtomicOperationV1::BitwiseXor => 5,
        RuntimeAtomicOperationV1::Exchange => 6,
        RuntimeAtomicOperationV1::CompareExchange => 7,
    }
}

pub(in crate::kfd_backend) const fn collective_operation_tag_v1(
    operation: crate::RuntimeCollectiveOperationV1,
) -> u8 {
    match operation {
        crate::RuntimeCollectiveOperationV1::Barrier => 0,
        crate::RuntimeCollectiveOperationV1::Broadcast => 1,
        crate::RuntimeCollectiveOperationV1::ReduceSum => 2,
        crate::RuntimeCollectiveOperationV1::ReduceMinimum => 3,
        crate::RuntimeCollectiveOperationV1::ReduceMaximum => 4,
        crate::RuntimeCollectiveOperationV1::AllReduceSum => 5,
        crate::RuntimeCollectiveOperationV1::InclusiveScanSum => 6,
    }
}

pub(in crate::kfd_backend) const fn memory_scope_tag_v1(scope: RuntimeMemoryScopeV1) -> u8 {
    match scope {
        RuntimeMemoryScopeV1::Workgroup => 0,
        RuntimeMemoryScopeV1::Device => 1,
        RuntimeMemoryScopeV1::System => 2,
    }
}

pub(in crate::kfd_backend) const fn memory_order_tag_v1(order: RuntimeMemoryOrderV1) -> u8 {
    match order {
        RuntimeMemoryOrderV1::Relaxed => 0,
        RuntimeMemoryOrderV1::Acquire => 1,
        RuntimeMemoryOrderV1::Release => 2,
        RuntimeMemoryOrderV1::AcquireRelease => 3,
        RuntimeMemoryOrderV1::SequentiallyConsistent => 4,
    }
}

pub(in crate::kfd_backend) const fn atomic_contract_is_legal_v1(
    contract: RuntimeAtomicLaunchContractV1,
) -> bool {
    match (contract.operation, contract.failure_order) {
        (RuntimeAtomicOperationV1::CompareExchange, Some(failure)) => {
            compare_exchange_orders_are_legal_v1(contract.order, failure)
        }
        (RuntimeAtomicOperationV1::CompareExchange, None) => false,
        (_, None) => !contract.weak,
        (_, Some(_)) => false,
    }
}

pub(in crate::kfd_backend) const fn compare_exchange_orders_are_legal_v1(
    success: RuntimeMemoryOrderV1,
    failure: RuntimeMemoryOrderV1,
) -> bool {
    match success {
        RuntimeMemoryOrderV1::Relaxed => matches!(failure, RuntimeMemoryOrderV1::Relaxed),
        RuntimeMemoryOrderV1::Acquire => matches!(
            failure,
            RuntimeMemoryOrderV1::Relaxed | RuntimeMemoryOrderV1::Acquire
        ),
        RuntimeMemoryOrderV1::Release => matches!(failure, RuntimeMemoryOrderV1::Relaxed),
        RuntimeMemoryOrderV1::AcquireRelease => matches!(
            failure,
            RuntimeMemoryOrderV1::Relaxed | RuntimeMemoryOrderV1::Acquire
        ),
        RuntimeMemoryOrderV1::SequentiallyConsistent => matches!(
            failure,
            RuntimeMemoryOrderV1::Relaxed
                | RuntimeMemoryOrderV1::Acquire
                | RuntimeMemoryOrderV1::SequentiallyConsistent
        ),
    }
}

pub(in crate::kfd_backend) fn complete_workgroup_geometry_v1(
    geometry: crate::RuntimeLaunchGeometryV1,
) -> bool {
    geometry
        .grid
        .into_iter()
        .zip(geometry.workgroup)
        .all(|(grid, workgroup)| {
            workgroup != 0 && grid >= workgroup && grid.is_multiple_of(workgroup)
        })
}

pub(in crate::kfd_backend) fn workgroup_participants_v1(
    geometry: crate::RuntimeLaunchGeometryV1,
) -> Option<u64> {
    geometry
        .workgroup
        .into_iter()
        .try_fold(1_u64, |product, value| {
            product.checked_mul(u64::from(value))
        })
}

pub(in crate::kfd_backend) const fn atomic_profile_is_admissible_v1(
    profile: KfdRuntimeAtomicExecutionProfileV1,
) -> bool {
    if matches!(profile.scope, RuntimeMemoryScopeV1::System) {
        return false;
    }
    match (profile.operation, profile.failure_order) {
        (RuntimeAtomicOperationV1::CompareExchange, Some(failure)) => {
            compare_exchange_orders_are_legal_v1(profile.order, failure)
        }
        (RuntimeAtomicOperationV1::CompareExchange, None) => false,
        (_, None) => !profile.weak,
        (_, Some(_)) => false,
    }
}

pub(in crate::kfd_backend) const fn collective_profile_is_admissible_v1(
    profile: KfdRuntimeCollectiveExecutionProfileV1,
) -> bool {
    matches!(profile.scope, RuntimeMemoryScopeV1::Workgroup)
}
