fn publication_from_live_v5(
    live: Option<&crate::production_formal_memory_v1::ProductionStaticPublicationDischargeV1>,
) -> FormalResultV5<Option<FormalMemoryStaticPublicationSummaryV5>> {
    use crate::production_formal_memory_v1::ProductionStaticConflictReasonV1 as Reason;
    let Some(live) = live else {
        return Ok(None);
    };
    let source = &live.source;
    if !source.full_physical_workgroups
        || source
            .events
            .iter()
            .any(|event| event.semantic_statement.is_some())
    {
        return Err(ProductionFormalMemoryEvidenceErrorV5::InvalidPublication);
    }
    let events = source.events.map(|event| FormalMemoryPublicationEventV5 {
        semantic_block: event.semantic_block,
        semantic_ordinal: event.semantic_ordinal,
        kir: location_from_live_v5(event.kir_location),
        ranked: [event.ranked_block, event.ranked_operation],
    });
    let mut ranked_sites = [[0; 2]; 6];
    for (out, site) in ranked_sites.iter_mut().zip(live.ranked.sites()) {
        *out = [
            u32::try_from(site.block())
                .map_err(|_| ProductionFormalMemoryEvidenceErrorV5::Limit)?,
            u32::try_from(site.operation())
                .map_err(|_| ProductionFormalMemoryEvidenceErrorV5::Limit)?,
        ];
    }
    let mut discharges = live
        .conflicts
        .iter()
        .map(|entry| FormalMemoryConflictDischargeV5 {
            left: location_from_live_v5(entry.conflict.left()),
            right: location_from_live_v5(entry.conflict.right()),
            allocation: entry.conflict.allocation().parameter_index(),
            reason: match entry.reason {
                Reason::ProducerInjectivity => {
                    FormalMemoryStaticConflictReasonV5::ProducerInjectivity
                }
                Reason::PublishedReadHappensBefore => {
                    FormalMemoryStaticConflictReasonV5::PublishedReadHappensBefore
                }
            },
        })
        .collect::<Vec<_>>();
    discharges.sort_unstable();
    let proof = FormalMemoryStaticPublicationSummaryV5 {
        semantic_function: source.semantic_function.index(),
        selected_root: source.selected_root.index(),
        source_arguments: [source.payload_source_argument, source.flags_source_argument],
        values: [
            source.payload_parameter.0,
            source.flags_parameter.0,
            source.producer_index.0,
            source.consumer_index.0,
            source.acquire.0,
            source.guard_predicate.0,
        ],
        payload_parameter_index: live.payload_parameter_index,
        global_extents: source.global_extents,
        workgroup_extents: source.workgroup_extents,
        events,
        origins: [live.ranked.payload_origin(), live.ranked.flags_origin()],
        ranked_sites,
        ranked_counts: [
            live.ranked.maximum_invocations(),
            live.ranked.potentially_conflicting_cell_pairs(),
            live.ranked.discharged_cell_pairs(),
            live.ranked.unresolved_cell_pairs(),
        ],
        discharges: discharges.into_boxed_slice(),
    };
    validate_publication_v5(&proof)?;
    Ok(Some(proof))
}

fn location_from_live_v5(
    location: fe2o3_kernel_ir::FunctionOperationLocation,
) -> FormalMemoryPublicationLocationV5 {
    FormalMemoryPublicationLocationV5 {
        block: location.block.0,
        operation: location.operation_index as u64,
    }
}

fn validate_publication_v5(proof: &FormalMemoryStaticPublicationSummaryV5) -> FormalResultV5<()> {
    use ProductionFormalMemoryEvidenceErrorV5::InvalidPublication as Invalid;
    if proof.global_extents != [256, 1, 1]
        || proof.workgroup_extents != [128, 1, 1]
        || proof.ranked_counts != [256, 128, 128, 0]
        || proof.source_arguments[0] == proof.source_arguments[1]
        || proof.origins != proof.source_arguments.map(|arg| u64::from(arg) + 1)
        || proof.values[0] == proof.values[1]
        || proof.events.map(|event| event.semantic_ordinal) != [0, 1, 0, 1, 2]
        || proof
            .ranked_sites
            .iter()
            .any(|site| site[0] >= 1024 || site[1] >= 8192)
    {
        return Err(Invalid);
    }
    let [w, p, q, a, r] = proof.events;
    if w.semantic_block != p.semantic_block
        || q.semantic_block != a.semantic_block
        || q.semantic_block != r.semantic_block
        || w.semantic_block == q.semantic_block
        || w.kir.block != p.kir.block
        || w.kir.operation >= p.kir.operation
        || q.kir.block != a.kir.block
        || q.kir.block != r.kir.block
        || q.kir.operation >= a.kir.operation
        || a.kir.operation >= r.kir.operation
        || w.kir.block == q.kir.block
    {
        return Err(Invalid);
    }
    for (event, index) in proof.events.iter().zip([0, 1, 2, 3, 5]) {
        if event.ranked != proof.ranked_sites[index] {
            return Err(Invalid);
        }
    }
    let sites = proof.ranked_sites;
    if sites[0][0] != sites[1][0]
        || sites[0][1] >= sites[1][1]
        || sites[0][0] == sites[2][0]
        || sites[2..]
            .windows(2)
            .any(|pair| pair[0][0] != pair[1][0] || pair[0][1] >= pair[1][1])
    {
        return Err(Invalid);
    }
    if proof.discharges.windows(2).any(|pair| {
        (pair[0].left, pair[0].right, pair[0].allocation)
            >= (pair[1].left, pair[1].right, pair[1].allocation)
    }) {
        return Err(Invalid);
    }
    for item in &proof.discharges {
        let correct = match item.reason {
            FormalMemoryStaticConflictReasonV5::ProducerInjectivity => {
                item.left == w.kir && item.right == w.kir
            }
            FormalMemoryStaticConflictReasonV5::PublishedReadHappensBefore => {
                (item.left == w.kir && item.right == r.kir)
                    || (item.left == r.kir && item.right == w.kir)
            }
        };
        if !correct || item.allocation != proof.payload_parameter_index {
            return Err(Invalid);
        }
    }
    Ok(())
}

fn encode_publication_v5(
    proof: &FormalMemoryStaticPublicationSummaryV5,
) -> FormalResultV5<Vec<u8>> {
    validate_publication_v5(proof)?;
    let size = proof
        .discharges
        .len()
        .checked_mul(32)
        .and_then(|n| n.checked_add(PUBLICATION_FIXED_BYTES_V5))
        .filter(|n| *n <= MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V5)
        .ok_or(ProductionFormalMemoryEvidenceErrorV5::Limit)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| ProductionFormalMemoryEvidenceErrorV5::Limit)?;
    put16_v5(&mut bytes, 1);
    put16_v5(&mut bytes, 0);
    for value in [proof.semantic_function, proof.selected_root]
        .into_iter()
        .chain(proof.source_arguments)
        .chain(proof.values)
        .chain([proof.payload_parameter_index])
    {
        put32_v5(&mut bytes, value);
    }
    for value in proof
        .global_extents
        .into_iter()
        .chain(proof.workgroup_extents)
    {
        put64_v5(&mut bytes, value);
    }
    bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
    for event in proof.events {
        put32_v5(&mut bytes, event.semantic_block);
        put32_v5(&mut bytes, 0);
        put32_v5(&mut bytes, 0);
        put32_v5(&mut bytes, event.semantic_ordinal);
        put_location_v5(&mut bytes, event.kir);
        for value in event.ranked {
            put32_v5(&mut bytes, value);
        }
    }
    for value in proof.origins {
        put64_v5(&mut bytes, value);
    }
    for value in proof
        .ranked_sites
        .into_iter()
        .flatten()
        .chain(proof.ranked_counts)
        .chain([proof.discharges.len() as u32])
    {
        put32_v5(&mut bytes, value);
    }
    for item in &proof.discharges {
        put_location_v5(&mut bytes, item.left);
        put_location_v5(&mut bytes, item.right);
        put32_v5(&mut bytes, item.allocation);
        bytes.extend_from_slice(&[item.reason as u8, 0, 0, 0]);
    }
    if bytes.len() != size {
        return Err(ProductionFormalMemoryEvidenceErrorV5::InvalidLength);
    }
    Ok(bytes)
}

fn decode_publication_v5(bytes: &[u8]) -> FormalResultV5<FormalMemoryStaticPublicationSummaryV5> {
    use ProductionFormalMemoryEvidenceErrorV5 as E;
    let mut r = FormalReaderV5::new(bytes);
    if r.u16()? != 1 || r.u16()? != 0 {
        return Err(E::InvalidPublication);
    }
    let semantic_function = r.u32()?;
    let selected_root = r.u32()?;
    let source_arguments = [r.u32()?, r.u32()?];
    let values = [r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?];
    let payload_parameter_index = r.u32()?;
    let global_extents = [r.u64()?, r.u64()?, r.u64()?];
    let workgroup_extents = [r.u64()?, r.u64()?, r.u64()?];
    if r.fixed::<8>()? != [1, 0, 0, 0, 0, 0, 0, 0] {
        return Err(E::InvalidPublication);
    }
    let mut events = [FormalMemoryPublicationEventV5 {
        semantic_block: 0,
        semantic_ordinal: 0,
        kir: FormalMemoryPublicationLocationV5 {
            block: 0,
            operation: 0,
        },
        ranked: [0; 2],
    }; 5];
    for event in &mut events {
        event.semantic_block = r.u32()?;
        if r.u32()? != 0 || r.u32()? != 0 {
            return Err(E::InvalidPublication);
        }
        event.semantic_ordinal = r.u32()?;
        event.kir = read_location_v5(&mut r)?;
        event.ranked = [r.u32()?, r.u32()?];
    }
    let origins = [r.u64()?, r.u64()?];
    let mut ranked_sites = [[0; 2]; 6];
    for site in &mut ranked_sites {
        *site = [r.u32()?, r.u32()?];
    }
    let ranked_counts = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
    let count = r.u32()? as usize;
    if count.checked_mul(32) != Some(bytes.len().saturating_sub(r.offset)) {
        return Err(E::InvalidLength);
    }
    let mut discharges = Vec::new();
    discharges.try_reserve_exact(count).map_err(|_| E::Limit)?;
    for _ in 0..count {
        let left = read_location_v5(&mut r)?;
        let right = read_location_v5(&mut r)?;
        let allocation = r.u32()?;
        let reason = match r.u8()? {
            1 => FormalMemoryStaticConflictReasonV5::ProducerInjectivity,
            2 => FormalMemoryStaticConflictReasonV5::PublishedReadHappensBefore,
            _ => return Err(E::InvalidPublication),
        };
        if r.fixed::<3>()? != [0; 3] {
            return Err(E::InvalidPublication);
        }
        discharges.push(FormalMemoryConflictDischargeV5 {
            left,
            right,
            allocation,
            reason,
        });
    }
    r.finish()?;
    let proof = FormalMemoryStaticPublicationSummaryV5 {
        semantic_function,
        selected_root,
        source_arguments,
        values,
        payload_parameter_index,
        global_extents,
        workgroup_extents,
        events,
        origins,
        ranked_sites,
        ranked_counts,
        discharges: discharges.into_boxed_slice(),
    };
    validate_publication_v5(&proof)?;
    Ok(proof)
}

fn put_location_v5(bytes: &mut Vec<u8>, value: FormalMemoryPublicationLocationV5) {
    put32_v5(bytes, value.block);
    put64_v5(bytes, value.operation);
}
fn read_location_v5(
    reader: &mut FormalReaderV5<'_>,
) -> FormalResultV5<FormalMemoryPublicationLocationV5> {
    Ok(FormalMemoryPublicationLocationV5 {
        block: reader.u32()?,
        operation: reader.u64()?,
    })
}
