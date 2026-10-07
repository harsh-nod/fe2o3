use super::*;

pub(super) fn validate_d2d_binding_v1(binding: R23D2dBindingV1) -> Result<(), R23D2dErrorV1> {
    validate_d2d_allocation_v1(binding.source)?;
    validate_d2d_allocation_v1(binding.destination)?;
    let source = binding.source.allocation;
    let destination = binding.destination.allocation;
    if source.owner == destination.owner
        || source.allocation == destination.allocation
        || source.mapping == destination.mapping
        || binding.source.backing_identity == binding.destination.backing_identity
        || source.allocation.vm != destination.allocation.vm
        || binding.queue.logical_queue.vm != source.allocation.vm
        || binding.queue.logical_queue.generation.0 == 0
        || binding.queue.occurrence == 0
        || binding.queue.native_queue_id >= R18_KFD_PROCESS_QUEUE_ID_LIMIT_V1
        || binding.queue.engine_id != R23_D2D_NATIVE_H2D_ENGINE_ID_V1
        || gpu_va_ranges_overlap_v1(
            binding.source.mapped_gpu_va,
            binding.destination.mapped_gpu_va,
        )
    {
        return Err(R23D2dErrorV1::InvalidBinding);
    }
    Ok(())
}

pub(super) fn validate_d2d_allocation_v1(
    binding: R23D2dAllocationBindingV1,
) -> Result<(), R23D2dErrorV1> {
    if binding.allocation.owner.0 == 0
        || binding.allocation.allocation.vm.device.generation.0 == 0
        || binding.allocation.allocation.vm.id.0 == 0
        || binding.allocation.allocation.id.0 == 0
        || binding.allocation.allocation.generation.0 == 0
        || binding.allocation.mapping.allocation != binding.allocation.allocation
        || binding.allocation.mapping.id.0 == 0
        || binding.attachment_generation == 0
        || binding.pool_generation == 0
        || binding.backing_identity == 0
        || binding.logical_byte_len == 0
        || binding.logical_byte_len > binding.physical_byte_len
        || binding.physical_byte_len > R17_PERSISTENT_NATIVE_ALLOCATION_BYTES_V1
        || binding.mapped_gpu_va.base == 0
        || binding.mapped_gpu_va.byte_len != binding.physical_byte_len
        || binding.mapped_gpu_va.checked_end().is_none()
    {
        return Err(R23D2dErrorV1::InvalidBinding);
    }
    Ok(())
}

pub(super) fn validate_d2d_request_v1(
    binding: R23D2dBindingV1,
    request: R23D2dCopyRequestV1,
) -> Result<(), R23D2dErrorV1> {
    if request.byte_len == 0
        || request.source_range.byte_len != request.byte_len
        || request.destination_range.byte_len != request.byte_len
        || request.source_range.checked_end().is_none_or(|end| {
            end > binding.source.logical_byte_len || end > binding.source.physical_byte_len
        })
        || request.destination_range.checked_end().is_none_or(|end| {
            end > binding.destination.logical_byte_len
                || end > binding.destination.physical_byte_len
        })
    {
        return Err(R23D2dErrorV1::InvalidRequest);
    }
    Ok(())
}

pub(super) const fn gpu_va_ranges_overlap_v1(left: GpuVaRangeV1, right: GpuVaRangeV1) -> bool {
    match (left.checked_end(), right.checked_end()) {
        (Some(left_end), Some(right_end)) => left.base < right_end && right.base < left_end,
        _ => true,
    }
}

pub(super) const fn lease_key_v1(
    binding: R23D2dAllocationBindingV1,
    role: R23D2dLeaseRoleV1,
    range: R18ByteRangeV1,
    generation: u64,
) -> R23D2dLeaseKeyV1 {
    R23D2dLeaseKeyV1 {
        allocation: binding.allocation,
        attachment_generation: binding.attachment_generation,
        pool_generation: binding.pool_generation,
        backing_identity: binding.backing_identity,
        role,
        range,
        generation,
    }
}

pub(super) fn r23_dependencies_match_v1(
    expected: &[R20DependencyV1],
    observed: &[R20DependencyObservationV1],
) -> bool {
    expected.len() == observed.len()
        && expected.iter().all(|dependency| {
            observed
                .iter()
                .filter(|observation| observation.dependency == *dependency)
                .count()
                == 1
        })
}

pub(super) fn reject_r23_spurious_metadata_v1(
    metadata: Option<&R23D2dAggregateCompletionMetadataV1>,
) -> Result<(), R23D2dErrorV1> {
    if metadata.is_some() {
        Err(R23D2dErrorV1::InvalidObservation)
    } else {
        Ok(())
    }
}

pub(super) fn into_quarantined_parts(
    custody: R23MoveOnlyD2dCustodyV1,
) -> (
    R23AllocationAuthorityV1,
    R23AllocationAuthorityV1,
    Option<R23MoveOnlyLeasePairV1>,
) {
    match custody {
        R23MoveOnlyD2dCustodyV1::Device(source, destination)
        | R23MoveOnlyD2dCustodyV1::Ready(source, destination) => (source, destination, None),
        R23MoveOnlyD2dCustodyV1::Prepared {
            source,
            destination,
            leases,
        }
        | R23MoveOnlyD2dCustodyV1::Published {
            source,
            destination,
            leases,
        }
        | R23MoveOnlyD2dCustodyV1::Frontier {
            source,
            destination,
            leases,
        } => (source, destination, Some(leases)),
        R23MoveOnlyD2dCustodyV1::Quarantined {
            source,
            destination,
            leases,
        } => (source, destination, leases),
    }
}
