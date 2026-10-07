//! Closed physical roles over genuine allocations, not caller-supplied addresses.
use super::super::super::projection_residual_mlp_tiles_v1 as projection;
use super::*;

pub(super) const GUARDED_IMAGE: [u8; 32] = [
    0xde, 0x88, 0x0d, 0xce, 0xbf, 0x79, 0x42, 0x5c, 0xcb, 0x55, 0x5c, 0x1a, 0x2b, 0xdc, 0xa3, 0xff,
    0x78, 0xb9, 0x26, 0xf7, 0xb2, 0x06, 0x2d, 0x06, 0xb9, 0x18, 0x76, 0x3d, 0xa4, 0xde, 0x0f, 0x66,
];
pub(super) const GUARD_SYMBOL: &str = "ferric_qwen3_mlp_state_guard_v2";
pub(super) const R2_SYMBOL: &str = "ferric_qwen3_tp2_guarded_projection_residual_bf16_v2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OutputPolicy {
    Strict,
    ExactOwnResidual,
}

pub(super) fn validate_policy(group: &Gfx950EngineeringPeerGroupV1) -> Result<()> {
    if group.contexts.len() != 2
        || group.contexts.iter().any(|context| {
            context.ordered_batch_poisoned
                || context.raw_timestamps_enabled
                || context.performance.is_some_and(|p| {
                    p.cache_kernel_admission || p.operational_currentness || p.profile
                })
        })
    {
        return Err(
            "paired guarded MLP requires two healthy full-currentness unprofiled queues".into(),
        );
    }
    Ok(())
}

pub(super) fn validate_owners(
    group: &mut Gfx950EngineeringPeerGroupV1,
    owners: &[CombinedMlpStateV1; 2],
) -> Result<u64> {
    let generation = owners[0].generation;
    if generation == 0
        || owners[1].generation != generation
        || owners[0].buffer.id == owners[1].buffer.id
    {
        return Err("paired guarded MLP generation or duplicate owner".into());
    }
    for (rank, owner) in owners.iter().enumerate() {
        if owner.owner_rank() != rank {
            return Err("paired guarded MLP ordered owner rank".into());
        }
        owner.local_id(group)?;
        require_activation(owner.activation, Activation::Ready)?;
        require_initial(&owner.observe(group)?, generation)?;
    }
    Ok(generation)
}

pub(super) fn guarded_metadata(value: &KernelMetadataV1, validator: bool) -> Result<()> {
    let slices = if validator { 1 } else { 6 };
    let explicit = if validator { 24 } else { 104 };
    if value.symbol != if validator { GUARD_SYMBOL } else { R2_SYMBOL }
        || value.object_sha256 != GUARDED_IMAGE
        || value.kernarg_bytes != explicit + 256
        || value.kernarg_alignment != 8
        || value.group_segment_bytes != 0
        || value.private_segment_bytes != 0
        || value.wavefront_size != 64
        || value.implicit_argument_offset != Some(explicit)
        || value.implicit_argument_bytes != 256
        || value.explicit_arguments.len() != slices * 2 + 2
    {
        return Err("paired guarded MLP exact guarded image ABI".into());
    }
    for (i, arg) in value.explicit_arguments.iter().enumerate() {
        let (offset, bytes, pointer) = if i < slices * 2 {
            ((i * 8) as u32, 8, i % 2 == 0)
        } else {
            ((slices * 16 + (i - slices * 2) * 4) as u32, 4, false)
        };
        if (arg.offset, arg.bytes, arg.global_buffer) != (offset, bytes, pointer)
            || arg.access.is_some()
            || arg.pointee_alignment.is_some()
        {
            return Err("paired guarded MLP exact guarded argument roster".into());
        }
    }
    Ok(())
}

fn region(
    group: &Gfx950EngineeringPeerGroupV1,
    token: Gfx950EngineeringPeerBufferV1,
    owner: usize,
    requested: usize,
    combined: bool,
) -> Result<profile::OwnedRegion> {
    let record = group.validate_token(token)?;
    if token.owner != owner
        || token.bytes != requested as u64
        || record.kind
            != if combined {
                BufferKind::CombinedMlpStateV1
            } else {
                BufferKind::PublicVram
            }
        || record.mapping.phase != Phase::PeersMapped
    {
        return Err("paired guarded MLP typed allocation role".into());
    }
    let allocation = group.contexts[owner]
        .buffers
        .get(&record.local_id)
        .ok_or("paired guarded MLP allocation missing")?;
    let value = profile::OwnedRegion {
        buffer: token.id,
        base: allocation.va,
        requested: allocation.requested,
        backing: allocation.backing,
    };
    validate_region(value, requested)?;
    Ok(value)
}

pub(super) fn validate_region(region: profile::OwnedRegion, requested: usize) -> Result<()> {
    if region.buffer == 0
        || region.base == 0
        || !region.base.is_multiple_of(PAGE_BYTES as u64)
        || region.requested != requested
        || region.backing < requested
        || !region.backing.is_multiple_of(PAGE_BYTES)
        || region.base.checked_add(region.backing as u64).is_none()
    {
        return Err("paired guarded MLP genuine allocation extent".into());
    }
    Ok(())
}

fn overlap(left: profile::OwnedRegion, right: profile::OwnedRegion) -> bool {
    left.buffer == right.buffer
        || (left.base < right.base + right.backing as u64
            && right.base < left.base + left.backing as u64)
}

pub(super) fn validate_regions(
    mlp: &[[profile::OwnedRegion; 11]; 2],
    partials: &[profile::OwnedRegion; 2],
    residuals: &[profile::OwnedRegion; 2],
    outputs: &[profile::OwnedRegion; 2],
) -> Result<()> {
    validate_regions_with_policy(mlp, partials, residuals, outputs, OutputPolicy::Strict)
}

pub(super) fn validate_regions_with_policy(
    mlp: &[[profile::OwnedRegion; 11]; 2],
    partials: &[profile::OwnedRegion; 2],
    residuals: &[profile::OwnedRegion; 2],
    outputs: &[profile::OwnedRegion; 2],
    policy: OutputPolicy,
) -> Result<()> {
    let mut seen = Vec::new();
    for row in mlp {
        for (i, &value) in row.iter().enumerate() {
            validate_region(
                value,
                if i == 10 {
                    COMBINED_BYTES
                } else {
                    profile::EXTENTS[i]
                },
            )?;
            if seen.iter().any(|&prior| overlap(prior, value)) {
                return Err("paired guarded MLP allocation alias within/across ranks".into());
            }
            seen.push(value);
        }
    }
    for &value in partials {
        validate_region(value, 16384)?;
    }
    for &value in residuals.iter().chain(outputs) {
        validate_region(value, 8192)?;
    }
    if overlap(partials[0], partials[1]) {
        return Err("paired guarded MLP input partials alias".into());
    }
    for &read in partials.iter().chain(residuals) {
        for row in mlp {
            for (i, &root) in row.iter().enumerate() {
                if (i == 0 || profile::role_access(i) != BufferAccessV1::Read)
                    && overlap(read, root)
                {
                    return Err("paired guarded MLP R1 input aliases asynchronous writer".into());
                }
            }
        }
    }
    seen.extend(partials);
    if policy == OutputPolicy::Strict {
        seen.extend(residuals);
    }
    for (rank, &output) in outputs.iter().enumerate() {
        // Never exempt an MLP root, including a read-only weight aliased by the
        // residual. R2 may reuse only the exact old R1 residual allocation.
        if seen.iter().any(|&prior| overlap(prior, output)) {
            return Err("paired guarded MLP final output aliases retained input/state".into());
        }
        if policy == OutputPolicy::ExactOwnResidual {
            let own = residuals[rank];
            if overlap(output, residuals[1 - rank])
                || (output.buffer, output.base, output.requested, output.backing)
                    != (own.buffer, own.base, own.requested, own.backing)
            {
                return Err("paired guarded MLP output is not exact own residual".into());
            }
        }
        seen.push(output);
    }
    Ok(())
}

pub(super) fn guarded_arguments(generation: u64, validator: bool) -> Vec<u8> {
    let mut bytes = vec![0; if validator { 280 } else { 360 }];
    let lengths: &[u64] = if validator {
        &[552]
    } else {
        &[4096, 4096, 4096, 4096, 4, 4]
    };
    for (i, length) in lengths.iter().enumerate() {
        bytes[i * 16 + 8..i * 16 + 16].copy_from_slice(&length.to_le_bytes());
    }
    let at = lengths.len() * 16;
    bytes[at..at + 4].copy_from_slice(&(generation as u32).to_le_bytes());
    bytes[at + 4..at + 8].copy_from_slice(&((generation >> 32) as u32).to_le_bytes());
    bytes
}

pub(super) fn prepare(
    group: &mut Gfx950EngineeringPeerGroupV1,
    owners: &[CombinedMlpStateV1; 2],
    inputs: &Inputs<'_>,
    timeout_ms: u32,
) -> Result<[[PreparedDispatch; 4]; 2]> {
    prepare_with_policy(group, owners, inputs, timeout_ms, OutputPolicy::Strict)
}

pub(super) fn prepare_exact_own_residual(
    group: &mut Gfx950EngineeringPeerGroupV1,
    owners: &[CombinedMlpStateV1; 2],
    inputs: &Inputs<'_>,
    timeout_ms: u32,
) -> Result<[[PreparedDispatch; 4]; 2]> {
    prepare_with_policy(
        group,
        owners,
        inputs,
        timeout_ms,
        OutputPolicy::ExactOwnResidual,
    )
}

fn prepare_with_policy(
    group: &mut Gfx950EngineeringPeerGroupV1,
    owners: &[CombinedMlpStateV1; 2],
    inputs: &Inputs<'_>,
    timeout_ms: u32,
    policy: OutputPolicy,
) -> Result<[[PreparedDispatch; 4]; 2]> {
    prepare_with_currentness(
        group,
        owners,
        inputs,
        timeout_ms,
        policy,
        &mut scoped_currentness::Currentness::Full,
    )
}

pub(super) fn prepare_with_currentness(
    group: &mut Gfx950EngineeringPeerGroupV1,
    owners: &[CombinedMlpStateV1; 2],
    inputs: &Inputs<'_>,
    timeout_ms: u32,
    policy: OutputPolicy,
    currentness: &mut scoped_currentness::Currentness<'_>,
) -> Result<[[PreparedDispatch; 4]; 2]> {
    if inputs.projection_sha256 == [0; 32] || inputs.mlp_sha256 == [0; 32] {
        return Err("paired guarded MLP missing R1/MLP image binding".into());
    }
    let empty = profile::OwnedRegion {
        buffer: 0,
        base: 0,
        requested: 0,
        backing: 0,
    };
    let mut mlp = [[empty; 11]; 2];
    for rank in 0..2 {
        for (i, root) in inputs.ranks[rank].mlp_roots.iter().enumerate() {
            mlp[rank][i] = region(group, *root, rank, profile::EXTENTS[i], false)?;
        }
        mlp[rank][10] = region(group, owners[rank].buffer, rank, COMBINED_BYTES, true)?;
        for (stage, kernel) in inputs.ranks[rank].kernels.iter().enumerate() {
            let retained = group.contexts[rank]
                .kernels
                .get(&kernel.id)
                .ok_or("paired guarded MLP retained kernel missing")?;
            if kernel.group != group.incarnation
                || kernel.rank != rank
                || kernel.metadata != retained.metadata
            {
                return Err("paired guarded MLP kernel rank/group/retained identity".into());
            }
            match stage {
                0 => projection::validate_projection_metadata(
                    &retained.metadata,
                    inputs.projection_sha256,
                )?,
                1 => profile::validate_metadata(
                    &retained.metadata,
                    inputs.mlp_sha256,
                    profile::SYMBOL,
                )?,
                2 | 3 => guarded_metadata(&retained.metadata, stage == 2)?,
                _ => unreachable!(),
            }
        }
    }
    let partials = [
        region(group, inputs.partials[0], 0, 16384, false)?,
        region(group, inputs.partials[1], 1, 16384, false)?,
    ];
    let residuals = [
        region(group, inputs.ranks[0].residual_input, 0, 8192, false)?,
        region(group, inputs.ranks[1].residual_input, 1, 8192, false)?,
    ];
    let outputs = [
        region(group, inputs.ranks[0].output, 0, 8192, false)?,
        region(group, inputs.ranks[1].output, 1, 8192, false)?,
    ];
    match policy {
        OutputPolicy::Strict => validate_regions(&mlp, &partials, &residuals, &outputs)?,
        OutputPolicy::ExactOwnResidual => {
            validate_regions_with_policy(&mlp, &partials, &residuals, &outputs, policy)?
        }
    }
    let mut result = Vec::with_capacity(2);
    for rank in 0..2 {
        let input = &inputs.ranks[rank];
        let r1 = (0..10)
            .map(|slot| match slot {
                0..=7 => inputs.partials[if slot < 2 { slot } else { 0 }].pointer(
                    slot as u32 * 16,
                    0,
                    if slot < 2 { 16384 } else { 0 },
                    BufferAccessV1::Read,
                ),
                8 => input
                    .residual_input
                    .pointer(128, 0, 8192, BufferAccessV1::Read),
                _ => input.mlp_roots[0].pointer(144, 0, 8192, BufferAccessV1::Write),
            })
            .collect::<Vec<_>>();
        let mut mlp_pointers = input
            .mlp_roots
            .iter()
            .enumerate()
            .map(|(i, root)| {
                root.pointer(
                    i as u32 * 8,
                    0,
                    profile::EXTENTS[i] as u64,
                    profile::role_access(i),
                )
            })
            .collect::<Vec<_>>();
        mlp_pointers.push(owners[rank].regions(group)?.mlp_prefix(rank)?);
        let guard = vec![owners[rank].regions(group)?.validator(rank)?];
        let r2 = vec![
            inputs.ranks[0].mlp_roots[9].pointer(0, 0, 16384, BufferAccessV1::Read),
            inputs.ranks[1].mlp_roots[9].pointer(16, 0, 16384, BufferAccessV1::Read),
            input.mlp_roots[0].pointer(32, 0, 8192, BufferAccessV1::Read),
            input.output.pointer(48, 0, 8192, BufferAccessV1::Write),
            owners[0].regions(group)?.r2_guard(rank, 64)?,
            owners[1].regions(group)?.r2_guard(rank, 80)?,
        ];
        let generation = owners[rank].generation;
        let args = [
            projection::projection_bytes(),
            vec![0; profile::KERNARG_BYTES],
            guarded_arguments(generation, true),
            guarded_arguments(generation, false),
        ];
        let mut prepared = Vec::with_capacity(4);
        for (stage, (bytes, pointers)) in args
            .into_iter()
            .zip([r1, mlp_pointers, guard, r2])
            .enumerate()
        {
            prepared.push(group.prepare_peer_dispatch_currentness(
                input.kernels[stage],
                bytes,
                [64, 1, 1],
                [if stage == 2 { 64 } else { 4096 }, 1, 1],
                &pointers,
                timeout_ms,
                currentness,
            )?);
        }
        result.push(
            prepared
                .try_into()
                .map_err(|_| "paired guarded MLP stage cardinality")?,
        );
    }
    result
        .try_into()
        .map_err(|_| "paired guarded MLP rank cardinality".into())
}
