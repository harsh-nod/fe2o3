//! Additive inert runtime-read receipt format. Decoding never grants live authority.

use super::super::super::FormalRuntimeSliceReadDomainV1;
use super::*;

pub const FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V4: u16 = 4;
pub const FORMAL_MEMORY_OBLIGATION_POLICY_V3: u16 = 3;
const IDENTITY_DOMAIN_V4: &[u8] = b"FE2O3/INERT-FORMAL-MEMORY-OBLIGATION-CONTENT/V4\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalMemoryReceiptEncodingV4 {
    LegacyV1,
    LegacyV2,
    GuardedV3,
    RuntimeBoundedV4,
}

impl FormalMemoryReceiptEncodingV4 {
    pub const fn wire_version(self) -> u16 {
        match self {
            Self::LegacyV1 => 1,
            Self::LegacyV2 => 2,
            Self::GuardedV3 => 3,
            Self::RuntimeBoundedV4 => 4,
        }
    }

    pub const fn extraction_policy(self) -> u16 {
        match self {
            Self::LegacyV1 | Self::LegacyV2 => 1,
            Self::GuardedV3 => 2,
            Self::RuntimeBoundedV4 => 3,
        }
    }
}

/// Descriptive inputs only, never an authenticated runtime allocation or launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalMemoryReceiptMetadataV4 {
    encoding: FormalMemoryReceiptEncodingV4,
    index_width: FormalIndexWidth,
    analysis_basis: FormalMemoryAnalysisBasis,
    invocations: Option<InvocationRange1d>,
}

impl FormalMemoryReceiptMetadataV4 {
    pub const fn encoding(self) -> FormalMemoryReceiptEncodingV4 {
        self.encoding
    }
    pub const fn index_width(self) -> FormalIndexWidth {
        self.index_width
    }
    pub const fn analysis_basis(self) -> FormalMemoryAnalysisBasis {
        self.analysis_basis
    }
    pub const fn invocations(self) -> Option<InvocationRange1d> {
        self.invocations
    }

    fn legacy(metadata: FormalMemoryReceiptMetadataV3) -> Self {
        let encoding = match metadata.encoding() {
            FormalMemoryReceiptEncodingV3::LegacyV1 => FormalMemoryReceiptEncodingV4::LegacyV1,
            FormalMemoryReceiptEncodingV3::LegacyV2 => FormalMemoryReceiptEncodingV4::LegacyV2,
            FormalMemoryReceiptEncodingV3::GuardedV3 => FormalMemoryReceiptEncodingV4::GuardedV3,
        };
        Self {
            encoding,
            index_width: metadata.index_width(),
            analysis_basis: metadata.analysis_basis(),
            invocations: metadata.invocations(),
        }
    }
}

/// Canonical V4 content containing at least one runtime-indexed slice read.
///
/// Any caller may reconstruct this content. Its numeric SSA coordinates do not
/// authenticate a source, verified graph, bounds proof, complete extraction,
/// artifact, producer, target, or runtime binding.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalFormalMemoryObligationReceiptV4 {
    canonical_bytes: Vec<u8>,
    identity: [u8; 32],
    metadata: FormalMemoryReceiptMetadataV4,
}

impl InertCanonicalFormalMemoryObligationReceiptV4 {
    pub fn from_obligations(obligations: &FormalMemoryObligations) -> ResultV3<Self> {
        let mut meter = meter_v4(default_work_limit()?)?;
        let canonical_bytes = encode_revision(obligations, WireRevision::RuntimeV4, &mut meter)?;
        Self::validate_owned(canonical_bytes, &mut meter)
    }

    pub fn from_canonical_bytes(canonical_bytes: Vec<u8>) -> ResultV3<Self> {
        Self::validate_owned(canonical_bytes, &mut meter_v4(default_work_limit()?)?)
    }

    fn validate_owned(canonical_bytes: Vec<u8>, meter: &mut CodecMeter) -> ResultV3<Self> {
        validate_revision(&canonical_bytes, WireRevision::RuntimeV4, meter)?;
        let metadata = read_metadata_v4(&canonical_bytes)?;
        let identity = identity_v4(&canonical_bytes);
        Ok(Self {
            canonical_bytes,
            identity,
            metadata,
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub fn kernel_id(&self) -> &str {
        binding_names(&self.canonical_bytes).0
    }
    pub fn entry_id(&self) -> &str {
        binding_names(&self.canonical_bytes).1
    }
    pub const fn identity_digest(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn metadata(&self) -> FormalMemoryReceiptMetadataV4 {
        self.metadata
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub fn revalidate(&self) -> ResultV3<()> {
        validate_revision(
            &self.canonical_bytes,
            WireRevision::RuntimeV4,
            &mut meter_v4(default_work_limit()?)?,
        )?;
        if read_metadata_v4(&self.canonical_bytes)? != self.metadata
            || identity_v4(&self.canonical_bytes) != self.identity
        {
            return Err(FormalMemoryReceiptErrorV1::IdentityMismatch);
        }
        Ok(())
    }
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }
}

/// Explicit additive facade. Older entry points do not silently accept V4.
#[derive(Debug, Eq, PartialEq)]
pub enum InertFormalMemoryReceiptFormatV4 {
    Legacy(InertCanonicalFormalMemoryObligationReceiptV1),
    Guarded(InertCanonicalFormalMemoryObligationReceiptV3),
    RuntimeBounded(InertCanonicalFormalMemoryObligationReceiptV4),
}

impl InertFormalMemoryReceiptFormatV4 {
    pub fn from_current_obligations(obligations: &FormalMemoryObligations) -> ResultV3<Self> {
        let counts = ObligationRecordCountsV1::from_obligations(obligations);
        preflight_record_counts(counts)?;
        let mut meter = meter_v4(default_work_limit()?)?;
        let count = counts
            .accesses
            .checked_add(counts.bounds)
            .ok_or_else(overflow)?;
        meter.charge(count)?;
        let runtime = obligations
            .accesses
            .iter()
            .any(|row| matches!(row.domain, FormalAccessDomainV1::RuntimeSliceReadBounded(_)))
            || obligations.bounds_requirements.iter().any(|row| {
                matches!(
                    row.kind,
                    FormalBoundsKindV1::RuntimeSliceElementAtGuardedIndex(_)
                )
            });
        if runtime {
            let bytes = encode_revision(obligations, WireRevision::RuntimeV4, &mut meter)?;
            InertCanonicalFormalMemoryObligationReceiptV4::validate_owned(bytes, &mut meter)
                .map(Self::RuntimeBounded)
        } else {
            InertFormalMemoryReceiptFormatV3::from_current_obligations(obligations)
                .map(Self::legacy)
        }
    }

    pub fn decode_current(bytes: Vec<u8>) -> ResultV3<Self> {
        if bytes.len() > MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1 {
            return Err(FormalMemoryReceiptErrorV1::TooLarge {
                max: MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1,
            });
        }
        let mut reader = Reader::new(&bytes);
        if reader.fixed::<8>()? != MAGIC_V1 {
            return Err(FormalMemoryReceiptErrorV1::InvalidMagic);
        }
        match reader.u16()? {
            1..=3 => InertFormalMemoryReceiptFormatV3::decode_current(bytes).map(Self::legacy),
            4 => InertCanonicalFormalMemoryObligationReceiptV4::from_canonical_bytes(bytes)
                .map(Self::RuntimeBounded),
            version => Err(FormalMemoryReceiptErrorV1::UnknownVersion(version)),
        }
    }

    fn legacy(receipt: InertFormalMemoryReceiptFormatV3) -> Self {
        match receipt {
            InertFormalMemoryReceiptFormatV3::Legacy(row) => Self::Legacy(row),
            InertFormalMemoryReceiptFormatV3::Guarded(row) => Self::Guarded(row),
        }
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        match self {
            Self::Legacy(row) => row.canonical_bytes(),
            Self::Guarded(row) => row.canonical_bytes(),
            Self::RuntimeBounded(row) => row.canonical_bytes(),
        }
    }
    pub fn kernel_id(&self) -> &str {
        binding_names(self.canonical_bytes()).0
    }
    pub fn entry_id(&self) -> &str {
        binding_names(self.canonical_bytes()).1
    }
    pub fn identity_digest(&self) -> &[u8; 32] {
        match self {
            Self::Legacy(row) => row.identity().digest(),
            Self::Guarded(row) => row.identity_digest(),
            Self::RuntimeBounded(row) => row.identity_digest(),
        }
    }
    pub fn metadata(&self) -> FormalMemoryReceiptMetadataV4 {
        match self {
            Self::Legacy(row) => FormalMemoryReceiptMetadataV4::legacy(
                read_metadata(row.canonical_bytes()).expect("validated legacy metadata"),
            ),
            Self::Guarded(row) => FormalMemoryReceiptMetadataV4::legacy(row.metadata()),
            Self::RuntimeBounded(row) => row.metadata(),
        }
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub fn revalidate(&self) -> ResultV3<()> {
        match self {
            Self::Legacy(row) => row.revalidate(),
            Self::Guarded(row) => row.revalidate(),
            Self::RuntimeBounded(row) => row.revalidate(),
        }
    }
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        match self {
            Self::Legacy(row) => row.into_canonical_bytes(),
            Self::Guarded(row) => row.into_canonical_bytes(),
            Self::RuntimeBounded(row) => row.into_canonical_bytes(),
        }
    }
}

fn meter_v4(limit: usize) -> ResultV3<CodecMeter> {
    let mut meter = CodecMeter::new(limit)?;
    // V3's shared codec headers remain charged; add the V4-owned transport
    // headers conservatively. Heap payloads use the same actual-capacity meter.
    let extra = size_of::<InertCanonicalFormalMemoryObligationReceiptV4>()
        + size_of::<InertFormalMemoryReceiptFormatV4>()
        + size_of::<FormalMemoryReceiptMetadataV4>();
    let total = meter
        .allocations
        .charged_bytes
        .checked_add(extra)
        .ok_or_else(overflow)?;
    if total > MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1 {
        return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
            field: "decoder auxiliary allocation",
            actual: total,
            max: MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1,
        });
    }
    meter.allocations.charged_bytes = total;
    Ok(meter)
}

fn read_metadata_v4(bytes: &[u8]) -> ResultV3<FormalMemoryReceiptMetadataV4> {
    let mut reader = Reader::new(bytes);
    reader.fixed::<8>()?;
    let version = reader.u16()?;
    if version != FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V4 {
        return Err(FormalMemoryReceiptErrorV1::UnknownVersion(version));
    }
    let policy = reader.u16()?;
    if policy != FORMAL_MEMORY_OBLIGATION_POLICY_V3 {
        return Err(FormalMemoryReceiptErrorV1::UnknownPolicy(policy));
    }
    reader.fixed::<8>()?;
    reader.text("kernel ID")?;
    reader.text("entry function ID")?;
    let index_width = match reader.u8()? {
        1 => FormalIndexWidth::Bits32,
        2 => FormalIndexWidth::Bits64,
        3 => FormalIndexWidth::Unknown,
        tag => {
            return Err(FormalMemoryReceiptErrorV1::UnknownTag {
                kind: "formal index width",
                tag,
            });
        }
    };
    decode_analysis_basis(reader.u8()?)?;
    reader.reserved_u16("obligation preamble")?;
    let invocations = decode_optional_invocations(&mut reader)?
        .map(|(start, end)| InvocationRange1d::new(start, end).expect("decoded nonempty range"));
    Ok(FormalMemoryReceiptMetadataV4 {
        encoding: FormalMemoryReceiptEncodingV4::RuntimeBoundedV4,
        index_width,
        analysis_basis: FormalMemoryAnalysisBasis::CompilerDerivedIrWithUnauthenticatedLaunchInputs,
        invocations,
    })
}

fn identity_v4(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((IDENTITY_DOMAIN_V4.len() as u32).to_le_bytes());
    digest.update(IDENTITY_DOMAIN_V4);
    digest.update(FORMAL_MEMORY_OBLIGATION_POLICY_V3.to_le_bytes());
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

pub(super) fn encode_runtime_domain(
    writer: &mut Writer,
    domain: FormalRuntimeSliceReadDomainV1,
) -> ResultV3<()> {
    let FormalGuardedPathV1::TrueEdge {
        source,
        ordinal,
        target,
    } = domain.path
    else {
        return Err(invalid("runtime read requires a true edge"));
    };
    writer.u8(2)?;
    writer.u32(domain.allocation.parameter_index)?;
    for value in [
        domain.slice,
        domain.index,
        domain.guard_index,
        domain.length,
        domain.predicate,
        domain.pointer,
    ] {
        writer.u32(value.0)?;
    }
    writer.u64(domain.element_bytes)?;
    writer.u8(1)?;
    writer.u32(source.0)?;
    writer.u64(u64::try_from(ordinal).map_err(|_| overflow())?)?;
    writer.u32(target.0)
}

pub(super) fn decode_runtime_domain(reader: &mut Reader<'_>) -> ResultV3<FormalAccessDomainV1> {
    let allocation = FormalAllocationIdentity {
        parameter_index: reader.u32()?,
    };
    let slice = ValueId(reader.u32()?);
    let index = ValueId(reader.u32()?);
    let guard_index = ValueId(reader.u32()?);
    let length = ValueId(reader.u32()?);
    let predicate = ValueId(reader.u32()?);
    let pointer = ValueId(reader.u32()?);
    let element_bytes = reader.u64()?;
    if !matches!(element_bytes, 1 | 2 | 4 | 8) {
        return Err(FormalMemoryReceiptErrorV1::InvalidValue {
            field: "runtime read scalar element width",
        });
    }
    match reader.u8()? {
        1 => {}
        0 => return Err(invalid("runtime read requires a true edge")),
        tag => {
            return Err(FormalMemoryReceiptErrorV1::UnknownTag {
                kind: "runtime read path",
                tag,
            });
        }
    }
    let path = FormalGuardedPathV1::TrueEdge {
        source: BlockId(reader.u32()?),
        ordinal: usize::try_from(reader.u64()?).map_err(|_| overflow())?,
        target: BlockId(reader.u32()?),
    };
    // SSA equality/transport belongs to fresh verified-graph extraction, not
    // this inert codec. In particular, index == guard_index is commonplace.
    Ok(FormalAccessDomainV1::RuntimeSliceReadBounded(
        FormalRuntimeSliceReadDomainV1 {
            allocation,
            slice,
            index,
            guard_index,
            length,
            predicate,
            pointer,
            element_bytes,
            path,
        },
    ))
}

pub(super) fn validate_runtime_access(
    width: u8,
    allocation: AllocationRecord,
    access: AccessRecord,
    domain: FormalRuntimeSliceReadDomainV1,
) -> ResultV3<()> {
    if width != 2
        || allocation.2 != 2
        || allocation.1 != domain.slice.0
        || access.1 != domain.allocation.parameter_index
        || access.2 != 1
        || access.3 != 3
        || access.4 != (2, 0, 0)
        || access.5 != domain.element_bytes
        || access.6 > domain.element_bytes
    {
        return Err(invalid("runtime read recipe and allocation"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "receipt_v4_tests.rs"]
mod tests;
