fn validate_receipt_v5(
    bytes: &[u8],
    fields: FormalFieldsV5,
    publication: Option<&FormalMemoryStaticPublicationSummaryV5>,
) -> FormalResultV5<()> {
    use ProductionFormalMemoryEvidenceErrorV5 as E;
    // The public receipt parser authenticates every record and its canonical
    // order before this bounded cursor extracts the existing fixed-width roster.
    let receipt =
        InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(bytes.to_vec())
            .map_err(|error| E::Receipt(error.to_string()))?;
    if receipt.identity().digest() != &fields.receipt_identity {
        return Err(E::InvalidIdentity);
    }
    let mut r = FormalReaderV5::new(receipt.canonical_bytes());
    if r.fixed::<8>()? != *b"FE2O3FM\0"
        || !matches!(r.u16()?, 1 | 2)
        || r.u16()? != fe2o3_kernel_ir::FORMAL_MEMORY_OBLIGATION_POLICY_V1
        || r.u16()? != 0
        || r.u16()? != 0
        || r.u32()? as usize != bytes.len()
    {
        return Err(E::InvalidAdmission);
    }
    for _ in 0..2 {
        let length = r.u32()? as usize;
        r.take(length)?;
    }
    r.u8()?;
    r.u8()?;
    if r.u16()? != 0 || r.u8()? != 1 || r.u64()? != 0 || r.u64()? != fields.witness {
        return Err(E::InvalidAdmission);
    }
    for width in [12usize, 70, 24, 40] {
        let count = r.u32()? as usize;
        r.take(count.checked_mul(width).ok_or(E::Limit)?)?;
    }
    let count = r.u32()?;
    if count != fields.raw_conflicts {
        return Err(E::InvalidAdmission);
    }
    let discharges = publication.map_or(&[][..], |proof| &proof.discharges);
    if count as usize != discharges.len() {
        return Err(E::InvalidPublication);
    }
    for item in discharges {
        if read_location_v5(&mut r)? != item.left
            || read_location_v5(&mut r)? != item.right
            || r.u32()? != item.allocation
        {
            return Err(E::InvalidPublication);
        }
    }
    r.finish()
}
