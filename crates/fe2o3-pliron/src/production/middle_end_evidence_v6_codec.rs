fn identity_v6(preimage: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V6);
    digest.update((preimage.len() as u64).to_le_bytes());
    digest.update(preimage);
    digest.finalize().into()
}

fn encode_record_v6(
    facts: CommonMiddleEndFactsV1,
    ir: &str,
    live_ranked_graph_identity: [u8; 32],
    publication: Option<ProductionMiddleEndPublicationSummaryV6>,
) -> Result<InertProductionMiddleEndEvidenceV6, ProductionMiddleEndEvidenceCodecErrorV6> {
    use ProductionMiddleEndEvidenceCodecErrorV5 as E;
    facts.validate(ir.as_bytes())?;
    if live_ranked_graph_identity == [0; 32] {
        return Err(E::ZeroIdentity.into());
    }
    validate_publication_v6(publication)?;
    if ir.len() > MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V6 {
        return Err(E::RankedIrTooLarge {
            actual: ir.len(),
            limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V6,
        }
        .into());
    }
    let total = FIXED_BYTES_V6
        .checked_add(ir.len())
        .ok_or(E::CounterOverflow)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| E::AllocationFailed)?;
    bytes.extend_from_slice(&MAGIC_V6);
    bytes.extend_from_slice(&6_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(total as u64).to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V6.len() as u16).to_le_bytes());
    bytes.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V6);
    bytes.extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V6.len() as u16).to_le_bytes());
    bytes.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V6);
    bytes.extend_from_slice(&[1, 1, 0, 0]);
    let ranked_ir_range = facts.encode_body(&mut bytes, ir)?;
    bytes.extend_from_slice(&live_ranked_graph_identity);
    encode_publication_v6(&mut bytes, publication);
    let sha256 = identity_v6(&bytes);
    if sha256 == [0; 32] {
        return Err(E::ZeroIdentity.into());
    }
    bytes.extend_from_slice(&sha256);
    if bytes.len() != total {
        return Err(E::NonCanonical.into());
    }
    Ok(InertProductionMiddleEndEvidenceV6 {
        facts,
        live_ranked_graph_identity,
        publication,
        ranked_ir_range,
        identity: ProductionMiddleEndEvidenceIdentityV6 {
            sha256,
            byte_len: total as u64,
        },
        canonical_bytes: bytes.into_boxed_slice(),
    })
}

fn decode_record_v6(
    bytes: &[u8],
) -> Result<InertProductionMiddleEndEvidenceV6, ProductionMiddleEndEvidenceCodecErrorV6> {
    use ProductionMiddleEndEvidenceCodecErrorV5 as E;
    if bytes.len() > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V6 {
        return Err(E::TooLarge {
            actual: bytes.len(),
            limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V6,
        }
        .into());
    }
    let mut reader = ReaderV5::new(bytes);
    if reader.fixed::<8>()? != MAGIC_V6 {
        return Err(E::InvalidMagic.into());
    }
    let version = reader.u16()?;
    if version != 6 {
        return Err(E::UnsupportedVersion(version).into());
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(E::UnsupportedFlags(flags).into());
    }
    let declared = reader.u64()?;
    let total = usize::try_from(declared).map_err(|_| E::InvalidLength(declared))?;
    if total > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V6 {
        return Err(E::TooLarge {
            actual: total,
            limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V6,
        }
        .into());
    }
    if total > bytes.len() {
        return Err(E::Truncated.into());
    }
    if total < bytes.len() {
        return Err(E::TrailingBytes.into());
    }
    if total <= FIXED_BYTES_V6 {
        return Err(E::InvalidLength(declared).into());
    }
    if reader.u32()? != 0 {
        return Err(E::NonzeroReserved.into());
    }
    for (expected, error) in [
        (PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V6, E::InvalidDomain),
        (PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V6, E::InvalidPolicy),
    ] {
        let length = usize::from(reader.u16()?);
        if length != expected.len() || reader.take(length)? != expected {
            return Err(error.into());
        }
    }
    if reader.u8()? != 1 {
        return Err(E::InvalidAssurance.into());
    }
    if reader.u8()? != 1 {
        return Err(E::SemanticOwnerNotRevalidated.into());
    }
    if reader.u16()? != 0 {
        return Err(E::NonzeroReserved.into());
    }
    let (facts, range) = CommonMiddleEndFactsV1::decode_body(&mut reader)?;
    if range.len() > MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V6 {
        return Err(E::RankedIrTooLarge {
            actual: range.len(),
            limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V6,
        }
        .into());
    }
    let live_ranked_graph_identity = reader.fixed::<32>()?;
    if live_ranked_graph_identity == [0; 32] {
        return Err(E::ZeroIdentity.into());
    }
    let publication = decode_publication_v6(&mut reader)?;
    let terminal = reader.offset();
    let digest = reader.fixed::<32>()?;
    if !reader.is_empty() {
        return Err(E::TrailingBytes.into());
    }
    if digest == [0; 32] {
        return Err(E::ZeroIdentity.into());
    }
    if identity_v6(&bytes[..terminal]) != digest {
        return Err(E::IdentityMismatch.into());
    }
    let ir = std::str::from_utf8(&bytes[range]).map_err(|_| E::InvalidRankedIrUtf8)?;
    let record = encode_record_v6(facts, ir, live_ranked_graph_identity, publication)?;
    if record.canonical_bytes() != bytes {
        return Err(E::NonCanonical.into());
    }
    Ok(record)
}
