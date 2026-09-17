#[derive(Clone, Copy)]
struct FormalFieldsV5 {
    source_identity: [u8; 32],
    ranked_identity: [u8; 32],
    kir: ProductionCanonicalKernelIrIdentityV1,
    receipt_identity: [u8; 32],
    witness: u64,
    raw_conflicts: u32,
    discharged: u32,
}

fn encode_v5(
    fields: FormalFieldsV5,
    publication: Option<&FormalMemoryStaticPublicationSummaryV5>,
    receipt: &[u8],
) -> FormalResultV5<Vec<u8>> {
    validate_fields_v5(fields, publication)?;
    let payload = publication
        .map(encode_publication_v5)
        .transpose()?
        .unwrap_or_default();
    let size = HEADER_BYTES_V5
        .checked_add(payload.len())
        .and_then(|n| n.checked_add(receipt.len()))
        .filter(|n| *n <= MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V5)
        .ok_or(ProductionFormalMemoryEvidenceErrorV5::Limit)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| ProductionFormalMemoryEvidenceErrorV5::Limit)?;
    bytes.extend_from_slice(&MAGIC_V5);
    put16_v5(&mut bytes, FORMAL_MEMORY_ADMISSION_EVIDENCE_VERSION_V5);
    put16_v5(&mut bytes, FORMAL_MEMORY_ADMISSION_EVIDENCE_POLICY_V5);
    put32_v5(&mut bytes, 0);
    put32_v5(&mut bytes, size as u32);
    bytes.extend_from_slice(&fields.source_identity);
    bytes.extend_from_slice(&fields.ranked_identity);
    put16_v5(
        &mut bytes,
        match fields.kir.version() {
            ProductionCanonicalKernelIrVersionV1::V8 => 8,
            ProductionCanonicalKernelIrVersionV1::V9 => 9,
            ProductionCanonicalKernelIrVersionV1::V11 => 11,
        },
    );
    put16_v5(&mut bytes, 0);
    put64_v5(&mut bytes, fields.kir.canonical_length());
    bytes.extend_from_slice(fields.kir.digest());
    bytes.extend_from_slice(&fields.receipt_identity);
    put64_v5(&mut bytes, fields.witness);
    put16_v5(&mut bytes, 1);
    put16_v5(&mut bytes, 1);
    for value in [
        0,
        fields.raw_conflicts,
        fields.discharged,
        0,
        payload.len() as u32,
        receipt.len() as u32,
    ] {
        put32_v5(&mut bytes, value);
    }
    bytes.extend_from_slice(&payload);
    bytes.extend_from_slice(receipt);
    if bytes.len() != size {
        return Err(ProductionFormalMemoryEvidenceErrorV5::InvalidLength);
    }
    Ok(bytes)
}

fn decode_v5(bytes: &[u8]) -> FormalResultV5<InertCanonicalFormalMemoryAdmissionEvidenceV5> {
    use ProductionFormalMemoryEvidenceErrorV5 as E;
    if bytes.len() > MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V5 {
        return Err(E::Limit);
    }
    let mut reader = FormalReaderV5::new(bytes);
    if reader.fixed::<8>()? != MAGIC_V5
        || reader.u16()? != 5
        || reader.u16()? != 1
        || reader.u32()? != 0
    {
        return Err(E::InvalidHeader);
    }
    if reader.u32()? as usize != bytes.len() {
        return Err(E::InvalidLength);
    }
    let source_identity = reader.fixed()?;
    let ranked_identity = reader.fixed()?;
    let version = match reader.u16()? {
        8 => ProductionCanonicalKernelIrVersionV1::V8,
        9 => ProductionCanonicalKernelIrVersionV1::V9,
        11 => ProductionCanonicalKernelIrVersionV1::V11,
        _ => return Err(E::InvalidHeader),
    };
    if reader.u16()? != 0 {
        return Err(E::InvalidHeader);
    }
    let length = reader.u64()?;
    let digest = reader.fixed()?;
    let kir = ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(version, digest, length);
    let receipt_identity = reader.fixed()?;
    let witness = reader.u64()?;
    if reader.u16()? != 1 || reader.u16()? != 1 || reader.u32()? != 0 {
        return Err(E::InvalidAdmission);
    }
    let raw_conflicts = reader.u32()?;
    let discharged = reader.u32()?;
    if reader.u32()? != 0 {
        return Err(E::InvalidAdmission);
    }
    let publication_length = reader.u32()? as usize;
    let receipt_length = reader.u32()? as usize;
    let publication = if publication_length == 0 {
        None
    } else {
        Some(decode_publication_v5(reader.take(publication_length)?)?)
    };
    let receipt_start = reader.offset;
    let receipt_bytes = reader.take(receipt_length)?;
    reader.finish()?;
    let fields = FormalFieldsV5 {
        source_identity,
        ranked_identity,
        kir,
        receipt_identity,
        witness,
        raw_conflicts,
        discharged,
    };
    validate_fields_v5(fields, publication.as_ref())?;
    validate_receipt_v5(receipt_bytes, fields, publication.as_ref())?;
    let canonical = encode_v5(fields, publication.as_ref(), receipt_bytes)?;
    if canonical != bytes {
        return Err(E::InvalidHeader);
    }
    let mut hash = Sha256::new();
    hash.update(DOMAIN_V5);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    let identity = hash.finalize().into();
    Ok(InertCanonicalFormalMemoryAdmissionEvidenceV5 {
        canonical_bytes: canonical.into_boxed_slice(),
        identity,
        source_semantic_identity: source_identity,
        ranked_graph_identity: ranked_identity,
        canonical_kernel_ir: kir,
        receipt_identity,
        receipt_range: receipt_start..receipt_start + receipt_length,
        witness,
        raw_conflicts,
        discharged_conflicts: discharged,
        publication,
    })
}

fn validate_fields_v5(
    fields: FormalFieldsV5,
    publication: Option<&FormalMemoryStaticPublicationSummaryV5>,
) -> FormalResultV5<()> {
    use ProductionFormalMemoryEvidenceErrorV5 as E;
    if [
        fields.source_identity,
        fields.ranked_identity,
        *fields.kir.digest(),
        fields.receipt_identity,
    ]
    .contains(&[0; 32])
        || fields.kir.canonical_length() == 0
    {
        return Err(E::InvalidIdentity);
    }
    if fields.witness == 0 || fields.raw_conflicts != fields.discharged {
        return Err(E::InvalidAdmission);
    }
    match publication {
        Some(proof) => {
            validate_publication_v5(proof)?;
            if fields.witness != 256 || proof.discharges.len() != fields.discharged as usize {
                return Err(E::InvalidPublication);
            }
        }
        None if fields.raw_conflicts != 0 => return Err(E::InvalidAdmission),
        None => (),
    }
    Ok(())
}

fn put16_v5(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
fn put32_v5(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
fn put64_v5(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct FormalReaderV5<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> FormalReaderV5<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> FormalResultV5<&'a [u8]> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(ProductionFormalMemoryEvidenceErrorV5::Limit)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionFormalMemoryEvidenceErrorV5::InvalidLength)?;
        self.offset = end;
        Ok(bytes)
    }
    fn fixed<const N: usize>(&mut self) -> FormalResultV5<[u8; N]> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionFormalMemoryEvidenceErrorV5::InvalidLength)
    }
    fn u8(&mut self) -> FormalResultV5<u8> {
        Ok(self.fixed::<1>()?[0])
    }
    fn u16(&mut self) -> FormalResultV5<u16> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }
    fn u32(&mut self) -> FormalResultV5<u32> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }
    fn u64(&mut self) -> FormalResultV5<u64> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }
    fn finish(self) -> FormalResultV5<()> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ProductionFormalMemoryEvidenceErrorV5::InvalidLength)
        }
    }
}
