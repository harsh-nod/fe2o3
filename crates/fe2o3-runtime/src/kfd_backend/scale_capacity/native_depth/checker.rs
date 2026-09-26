//! Typed consistency checks, not independent native receipt authentication.

use super::*;

pub(super) fn membership(rows: &[ReceiptRow]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"fe2o3.scale-retained-depth-membership.v1\0");
    hash.update((rows.len() as u64).to_le_bytes());
    for row in rows {
        for value in [row.lane as u64, row.id, row.stream, row.kernel] {
            hash.update(value.to_le_bytes());
        }
        for allocation in row.allocations {
            hash.update(allocation.to_le_bytes());
        }
        hash.update([u8::from(row.predecessor.is_some())]);
        hash.update(row.predecessor.unwrap_or(0).to_le_bytes());
        hash.update([match row.phase {
            None => 0,
            Some(RuntimeComputePipelinePhaseV1::Published) => 1,
            Some(RuntimeComputePipelinePhaseV1::Completed) => 2,
            Some(RuntimeComputePipelinePhaseV1::PhysicallyRetired) => 3,
            Some(RuntimeComputePipelinePhaseV1::Quarantined) => 4,
        }]);
        hash.update(row.native);
        hash.update(row.shape);
    }
    hash.finalize().into()
}

pub(super) fn check_depth(cut: &DepthSnapshot, expected: &Expected) -> Result<(), &'static str> {
    if cut.rows.len() != LANES * DEPTH
        || expected.ids.iter().any(|ids| ids.len() != DEPTH)
        || cut.pipeline_lengths != [DEPTH - 1; LANES]
        || cut.reservations != LANES * DEPTH
    {
        return Err("full two-lane retained depth");
    }
    if expected.streams[0] == expected.streams[1]
        || expected.streams.contains(&0)
        || expected.kernel == 0
        || expected.module == 0
    {
        return Err("expected resource identities");
    }
    let ids: HashSet<_> = expected.ids.iter().flatten().copied().collect();
    let allocations: HashSet<_> = expected.allocations.iter().flatten().copied().collect();
    if ids.len() != LANES * DEPTH
        || ids.contains(&0)
        || allocations.len() != LANES * 3
        || allocations.contains(&0)
    {
        return Err("distinct expected submissions and allocations");
    }
    let mut native = HashSet::with_capacity(cut.rows.len());
    for (lane, lane_rows) in cut.rows.chunks_exact(DEPTH).enumerate() {
        for (ordinal, row) in lane_rows.iter().enumerate() {
            if row.id != expected.ids[lane][ordinal]
                || row.lane != lane
                || row.stream != expected.streams[lane]
                || row.kernel != expected.kernel
                || row.allocations != expected.allocations[lane]
                || row.predecessor
                    != ordinal
                        .checked_sub(1)
                        .map(|index| expected.ids[lane][index])
                || row.phase != (ordinal != 0).then_some(RuntimeComputePipelinePhaseV1::Published)
                || row.shape == [0; 32]
                || row.shape != lane_rows[0].shape
            {
                return Err("receipt owner coordinate or phase");
            }
            if row.native == [0; 32] || !native.insert(row.native) {
                return Err("distinct native receipts");
            }
        }
    }
    if cut.membership != membership(&cut.rows) {
        return Err("receipt membership digest");
    }
    if cut.custody.len() != LANES * 3 {
        return Err("custody roster count");
    }
    for lane in 0..LANES {
        for allocation in expected.allocations[lane] {
            let custody = cut
                .custody
                .get(&allocation)
                .ok_or("missing allocation custody")?;
            if custody.owners.len() != DEPTH
                || custody.capacity != DEPTH
                || custody.counts != [DEPTH, 0]
                || custody.sole_stream != Some(expected.streams[lane])
                || !custody
                    .owners
                    .iter()
                    .zip(&expected.ids[lane])
                    .all(|(owner, &id)| {
                        owner.submission == id
                            && owner.stream == expected.streams[lane]
                            && owner.kind == RuntimeAllocationCustodyKindV1::Compute
                    })
            {
                return Err("exact allocation custody owners");
            }
        }
    }
    let leases = expected
        .streams
        .into_iter()
        .enumerate()
        .map(|(lane, stream)| (stream, lane))
        .collect();
    let tails = expected
        .streams
        .into_iter()
        .enumerate()
        .map(|(lane, stream)| (stream, expected.ids[lane][DEPTH - 1]))
        .collect();
    let dependencies = expected
        .ids
        .iter()
        .flat_map(|ids| ids[..DEPTH - 1].iter().map(|&id| (id, 1)))
        .collect();
    if cut.leases != leases
        || cut.tails != tails
        || cut.dependencies != dependencies
        || cut.modules != HashMap::from([(expected.module, LANES * DEPTH)])
    {
        return Err("runtime index join");
    }
    let usage = cut.usage;
    let bytes = usage.used.get(ResourceKindV1::ControlResidentBytes);
    if usage.poisoned
        || usage.retained_records != 10
        || usage.reserved_records != 0
        || usage.quarantined_records != 0
        || usage.record_capacity < 10
        || bytes == 0
        || bytes > usage.capacity.get(ResourceKindV1::ControlResidentBytes)
        || usage.used != ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes)
    {
        return Err("host table ledger at retained depth");
    }
    Ok(())
}

pub(super) fn check_disposed(usage: ResourceCreditUsageV1) -> Result<(), &'static str> {
    if usage.used != ResourceVectorV1::ZERO
        || usage.reserved_records != 0
        || usage.retained_records != 0
        || usage.quarantined_records != 0
        || usage.poisoned
    {
        return Err("host table disposal is incomplete");
    }
    Ok(())
}

pub(super) fn check_profile(
    events: &[KfdRuntimeProfileEventV1],
    ids: &[Vec<ProfileIdentityV1>; LANES],
    queues: [ProfileIdentityV1; LANES],
    streams: [ProfileIdentityV1; LANES],
    publications: &[KfdRuntimeProfileEventKindV1],
) -> Result<(), &'static str> {
    if ids.iter().any(|ids| ids.len() != DEPTH)
        || publications.len() != LANES * DEPTH
        || queues[0] == queues[1]
        || streams[0] == streams[1]
    {
        return Err("profile expected identities");
    }
    let index: HashMap<_, _> = ids
        .iter()
        .enumerate()
        .flat_map(|(lane, ids)| {
            ids.iter()
                .enumerate()
                .map(move |(ordinal, &id)| (id, (lane, ordinal)))
        })
        .collect();
    if index.len() != LANES * DEPTH {
        return Err("duplicate expected profile dispatch");
    }
    let mut published = [0; LANES];
    let mut completed = [0; LANES];
    let mut released = HashSet::new();
    let mut created = [false; LANES];
    let mut destroyed = [false; LANES];
    let mut prior_sequence = None;
    for event in events {
        if prior_sequence.is_some_and(|prior| event.sequence <= prior) {
            return Err("profile sequence");
        }
        prior_sequence = Some(event.sequence);
        match event.event {
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue } => {
                let lane = queues
                    .iter()
                    .position(|id| *id == queue)
                    .ok_or("foreign queue creation")?;
                if created[lane] {
                    return Err("duplicate queue creation");
                }
                created[lane] = true;
            }
            KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch,
                queue,
                stream,
                ..
            } => {
                let &(lane, ordinal) = index.get(&dispatch).ok_or("foreign publication")?;
                if !created[lane]
                    || destroyed[lane]
                    || queue != queues[lane]
                    || stream != streams[lane]
                    || published[lane] != ordinal
                    || completed != [0; LANES]
                {
                    return Err("publication prefix or lane identity");
                }
                if event.event != publications[lane * DEPTH + ordinal] {
                    return Err("publication differs from retained recipe");
                }
                published[lane] += 1;
            }
            KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. } => {
                let &(lane, ordinal) = index.get(&dispatch).ok_or("foreign completion")?;
                if published != [DEPTH; LANES] || completed[lane] != ordinal || destroyed[lane] {
                    return Err("per-lane completion order");
                }
                completed[lane] += 1;
            }
            KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch } => {
                let first_release = released.insert(dispatch);
                if !index.contains_key(&dispatch) || completed != [DEPTH; LANES] || !first_release {
                    return Err("release before completion or duplicate release");
                }
            }
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue } => {
                let lane = queues
                    .iter()
                    .position(|id| *id == queue)
                    .ok_or("foreign queue destruction")?;
                if !created[lane] || destroyed[lane] || released.len() != LANES * DEPTH {
                    return Err("queue destruction before complete release");
                }
                destroyed[lane] = true;
            }
            _ => {}
        }
    }
    if published != [DEPTH; LANES]
        || completed != [DEPTH; LANES]
        || released.len() != LANES * DEPTH
        || destroyed != [true; LANES]
    {
        return Err("incomplete profile history");
    }
    Ok(())
}
