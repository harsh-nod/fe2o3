fn validate_publication_v6(
    summary: Option<ProductionMiddleEndPublicationSummaryV6>,
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV6> {
    let Some(summary) = summary else {
        return Ok(());
    };
    let [write, ready, request, acquire, guard, read] = summary.sites;
    if summary.payload_origin == 0
        || summary.flags_origin == 0
        || summary.payload_origin == summary.flags_origin
        || summary.maximum_invocations != 256
        || summary.potentially_conflicting_cell_pairs != 128
        || summary.discharged_cell_pairs != 128
        || summary.unresolved_cell_pairs != 0
        || summary
            .sites
            .iter()
            .any(|site| site.block >= 1024 || site.operation >= 8192)
        || write.block != ready.block
        || write.operation >= ready.operation
        || request.block != acquire.block
        || request.block != guard.block
        || request.block != read.block
        || write.block == read.block
        || request.operation >= acquire.operation
        || acquire.operation >= guard.operation
        || guard.operation >= read.operation
    {
        return Err(ProductionMiddleEndEvidenceCodecErrorV6::InvalidPublication);
    }
    Ok(())
}

fn require_publication_roster_v6(
    counts: [usize; 3],
    proof_present: bool,
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV6> {
    if counts != if proof_present { [2, 1, 1] } else { [0, 0, 0] } {
        return Err(ProductionMiddleEndEvidenceCodecErrorV6::PublicationReportMismatch);
    }
    Ok(())
}

fn publication_from_owner_v6(
    ranked: &ProductionRankedKernelLoweringInputV1,
) -> Result<Option<ProductionMiddleEndPublicationSummaryV6>, ProductionMiddleEndEvidenceCodecErrorV6>
{
    let mut counts = [0_usize; 3];
    for operation in ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
    {
        let index = match operation {
            ProductionRankedOperationV1::PublicationAtomicStoreU32 { .. } => 0,
            ProductionRankedOperationV1::PublicationAtomicLoadU32 { .. } => 1,
            ProductionRankedOperationV1::PublicationReadGuard { .. } => 2,
            _ => continue,
        };
        counts[index] = counts[index]
            .checked_add(1)
            .ok_or(ProductionMiddleEndEvidenceCodecErrorV6::PublicationReportMismatch)?;
    }
    let proof = ranked.race_report().static_publication();
    require_publication_roster_v6(counts, proof.is_some())?;
    let Some(proof) = proof else {
        return Ok(None);
    };
    let mut sites = [ProductionMiddleEndPublicationSiteV6 {
        block: 0,
        operation: 0,
    }; 6];
    for (target, site) in sites.iter_mut().zip(proof.sites()) {
        target.block = u32::try_from(site.block())
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV6::InvalidPublication)?;
        target.operation = u32::try_from(site.operation())
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV6::InvalidPublication)?;
    }
    let summary = ProductionMiddleEndPublicationSummaryV6 {
        payload_origin: proof.payload_origin(),
        flags_origin: proof.flags_origin(),
        sites,
        maximum_invocations: proof.maximum_invocations(),
        potentially_conflicting_cell_pairs: proof.potentially_conflicting_cell_pairs(),
        discharged_cell_pairs: proof.discharged_cell_pairs(),
        unresolved_cell_pairs: proof.unresolved_cell_pairs(),
    };
    validate_publication_v6(Some(summary))?;
    Ok(Some(summary))
}

fn encode_publication_v6(
    bytes: &mut Vec<u8>,
    summary: Option<ProductionMiddleEndPublicationSummaryV6>,
) {
    let Some(summary) = summary else {
        bytes.extend_from_slice(&[0; PUBLICATION_BYTES_V6]);
        return;
    };
    bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(&summary.payload_origin.to_le_bytes());
    bytes.extend_from_slice(&summary.flags_origin.to_le_bytes());
    for site in summary.sites {
        bytes.extend_from_slice(&site.block.to_le_bytes());
        bytes.extend_from_slice(&site.operation.to_le_bytes());
    }
    for value in [
        summary.maximum_invocations,
        summary.potentially_conflicting_cell_pairs,
        summary.discharged_cell_pairs,
        summary.unresolved_cell_pairs,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn decode_publication_v6(
    reader: &mut ReaderV5<'_>,
) -> Result<Option<ProductionMiddleEndPublicationSummaryV6>, ProductionMiddleEndEvidenceCodecErrorV6>
{
    let kind = reader.u8()?;
    if kind == 0 {
        if reader
            .take(PUBLICATION_BYTES_V6 - 1)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(ProductionMiddleEndEvidenceCodecErrorV6::InvalidPublication);
        }
        return Ok(None);
    }
    if kind != 1 || reader.fixed::<7>()? != [0; 7] {
        return Err(ProductionMiddleEndEvidenceCodecErrorV6::InvalidPublication);
    }
    let payload_origin = reader.u64()?;
    let flags_origin = reader.u64()?;
    let mut sites = [ProductionMiddleEndPublicationSiteV6 {
        block: 0,
        operation: 0,
    }; 6];
    for site in &mut sites {
        site.block = reader.u32()?;
        site.operation = reader.u32()?;
    }
    let summary = ProductionMiddleEndPublicationSummaryV6 {
        payload_origin,
        flags_origin,
        sites,
        maximum_invocations: reader.u32()?,
        potentially_conflicting_cell_pairs: reader.u32()?,
        discharged_cell_pairs: reader.u32()?,
        unresolved_cell_pairs: reader.u32()?,
    };
    validate_publication_v6(Some(summary))?;
    Ok(Some(summary))
}
