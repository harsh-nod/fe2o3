//! Explicit inert guarded-domain wire format. No live-module or runtime authority.

use super::super::{
    FormalAccessDomainV1, FormalAliasRegionV1, FormalAllocationIdentity, FormalBoundsKindV1,
    FormalGuardedPathV1, FormalSliceBoundedDomainV1,
};
use super::*;
use crate::verification_index_v1::{
    verification_bounded_sort_by_v1, verification_ceil_log2_v1, verification_find_last_by_v1,
};
use crate::{
    BlockId, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrVerificationResourceErrorV1, CanonicalKernelIrWorkBudgetV1, ValueId,
};
use std::cmp::Ordering;

pub const FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V3: u16 = 3;
pub const FORMAL_MEMORY_OBLIGATION_POLICY_V2: u16 = 2;
const IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/INERT-FORMAL-MEMORY-OBLIGATION-CONTENT/V3\0";
type ResultV3<T> = Result<T, FormalMemoryReceiptErrorV1>;

/// Closed format choice. It identifies inert extraction semantics, not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalMemoryReceiptEncodingV3 {
    LegacyV1,
    LegacyV2,
    GuardedV3,
}

impl FormalMemoryReceiptEncodingV3 {
    pub const fn wire_version(self) -> u16 {
        match self {
            Self::LegacyV1 => 1,
            Self::LegacyV2 => 2,
            Self::GuardedV3 => 3,
        }
    }
    pub const fn extraction_policy(self) -> u16 {
        match self {
            Self::LegacyV1 | Self::LegacyV2 => 1,
            Self::GuardedV3 => 2,
        }
    }
}

/// Descriptive extraction inputs; this does not authenticate a runtime launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalMemoryReceiptMetadataV3 {
    encoding: FormalMemoryReceiptEncodingV3,
    index_width: FormalIndexWidth,
    analysis_basis: FormalMemoryAnalysisBasis,
    invocations: Option<InvocationRange1d>,
}

impl FormalMemoryReceiptMetadataV3 {
    pub const fn encoding(self) -> FormalMemoryReceiptEncodingV3 {
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
}

/// V3 bytes retain conditional domains, symbolic bounds and whole-allocation aliases.
/// Anyone may construct these inert bytes. No verified KIR, completeness, producer,
/// reference, functional, target or runtime authority is established by decoding.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalFormalMemoryObligationReceiptV3 {
    canonical_bytes: Vec<u8>,
    identity: [u8; 32],
    metadata: FormalMemoryReceiptMetadataV3,
}

impl InertCanonicalFormalMemoryObligationReceiptV3 {
    pub fn from_obligations(obligations: &FormalMemoryObligations) -> ResultV3<Self> {
        let mut meter = CodecMeter::new(default_work_limit()?)?;
        let canonical_bytes = encode_v3(obligations, &mut meter)?;
        let metadata = validate_v3(&canonical_bytes, &mut meter)?;
        let identity = identity_v3(&canonical_bytes);
        Ok(Self {
            canonical_bytes,
            identity,
            metadata,
        })
    }
    pub fn from_canonical_bytes(canonical_bytes: Vec<u8>) -> ResultV3<Self> {
        let mut meter = CodecMeter::new(default_work_limit()?)?;
        let metadata = validate_v3(&canonical_bytes, &mut meter)?;
        let identity = identity_v3(&canonical_bytes);
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
    pub const fn metadata(&self) -> FormalMemoryReceiptMetadataV3 {
        self.metadata
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub fn revalidate(&self) -> ResultV3<()> {
        let mut meter = CodecMeter::new(default_work_limit()?)?;
        if validate_v3(&self.canonical_bytes, &mut meter)? != self.metadata
            || identity_v3(&self.canonical_bytes) != self.identity
        {
            return Err(FormalMemoryReceiptErrorV1::IdentityMismatch);
        }
        Ok(())
    }
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }
}

/// Explicit current-format facade; legacy constructors/decoders remain unchanged.
#[derive(Debug, Eq, PartialEq)]
pub enum InertFormalMemoryReceiptFormatV3 {
    Legacy(InertCanonicalFormalMemoryObligationReceiptV1),
    Guarded(InertCanonicalFormalMemoryObligationReceiptV3),
}

impl InertFormalMemoryReceiptFormatV3 {
    pub fn from_current_obligations(obligations: &FormalMemoryObligations) -> ResultV3<Self> {
        preflight_record_counts(ObligationRecordCountsV1::from_obligations(obligations))?;
        let guarded = obligations
            .accesses
            .iter()
            .any(|row| row.domain != FormalAccessDomainV1::LaunchEnvelope)
            || obligations
                .bounds_requirements
                .iter()
                .any(|row| row.minimum_byte_len().is_none())
            || obligations.runtime_alias_requirements.iter().any(|row| {
                row.left_accessed_bytes().is_none() || row.right_accessed_bytes().is_none()
            });
        if guarded {
            InertCanonicalFormalMemoryObligationReceiptV3::from_obligations(obligations)
                .map(Self::Guarded)
        } else {
            InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(obligations)
                .map(Self::Legacy)
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
            1 | 2 => InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(bytes)
                .map(Self::Legacy),
            3 => InertCanonicalFormalMemoryObligationReceiptV3::from_canonical_bytes(bytes)
                .map(Self::Guarded),
            version => Err(FormalMemoryReceiptErrorV1::UnknownVersion(version)),
        }
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        match self {
            Self::Legacy(row) => row.canonical_bytes(),
            Self::Guarded(row) => row.canonical_bytes(),
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
        }
    }
    pub fn metadata(&self) -> FormalMemoryReceiptMetadataV3 {
        match self {
            Self::Guarded(row) => row.metadata(),
            Self::Legacy(row) => {
                read_metadata(row.canonical_bytes()).expect("validated legacy receipt metadata")
            }
        }
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub fn revalidate(&self) -> ResultV3<()> {
        match self {
            Self::Legacy(row) => row.revalidate(),
            Self::Guarded(row) => row.revalidate(),
        }
    }
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        match self {
            Self::Legacy(row) => row.into_canonical_bytes(),
            Self::Guarded(row) => row.into_canonical_bytes(),
        }
    }
}

fn binding_names(bytes: &[u8]) -> (&str, &str) {
    let mut reader = Reader::new(bytes);
    reader.fixed::<HEADER_BYTES>().expect("validated header");
    let kernel = reader.text("kernel ID").expect("validated kernel");
    let entry = reader.text("entry function ID").expect("validated entry");
    (kernel, entry)
}

fn read_metadata(bytes: &[u8]) -> ResultV3<FormalMemoryReceiptMetadataV3> {
    let mut reader = Reader::new(bytes);
    reader.fixed::<8>()?;
    let encoding = match reader.u16()? {
        1 => FormalMemoryReceiptEncodingV3::LegacyV1,
        2 => FormalMemoryReceiptEncodingV3::LegacyV2,
        3 => FormalMemoryReceiptEncodingV3::GuardedV3,
        version => return Err(FormalMemoryReceiptErrorV1::UnknownVersion(version)),
    };
    let policy = reader.u16()?;
    if policy != encoding.extraction_policy() {
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
    Ok(FormalMemoryReceiptMetadataV3 {
        encoding,
        index_width,
        analysis_basis: FormalMemoryAnalysisBasis::CompilerDerivedIrWithUnauthenticatedLaunchInputs,
        invocations,
    })
}

fn identity_v3(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((IDENTITY_DOMAIN_V3.len() as u32).to_le_bytes());
    digest.update(IDENTITY_DOMAIN_V3);
    digest.update(FORMAL_MEMORY_OBLIGATION_POLICY_V2.to_le_bytes());
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn overflow() -> FormalMemoryReceiptErrorV1 {
    FormalMemoryReceiptErrorV1::Overflow {
        field: "guarded receipt resources",
    }
}
fn default_work_limit() -> ResultV3<usize> {
    let rows = MAX_FORMAL_MEMORY_RECORDS_V1
        .checked_add(1)
        .ok_or_else(overflow)?;
    rows.checked_mul(verification_ceil_log2_v1(rows) + 1)
        .and_then(|n| n.checked_mul(128))
        .and_then(|n| n.checked_add(4 * MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1))
        .ok_or_else(overflow)
}

struct CodecMeter {
    work: CanonicalKernelIrWorkBudgetV1,
    allocations: DecoderAllocationBudgetV1,
}
impl CodecMeter {
    fn new(limit: usize) -> ResultV3<Self> {
        let fixed = size_of::<SortedRows<'_>>()
            + size_of::<Writer>()
            + 3 * size_of::<Vec<u8>>()
            + size_of::<Self>()
            + size_of::<InertCanonicalFormalMemoryObligationReceiptV3>()
            + size_of::<InertFormalMemoryReceiptFormatV3>()
            + 2 * size_of::<Reader<'_>>();
        if fixed > MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1 {
            return Err(overflow());
        }
        Ok(Self {
            work: CanonicalKernelIrWorkBudgetV1::new(limit),
            allocations: DecoderAllocationBudgetV1 {
                charged_bytes: fixed,
            },
        })
    }
    fn charge(&mut self, amount: usize) -> ResultV3<()> {
        self.work
            .charge_work(amount)
            .map_err(|error| FormalMemoryReceiptErrorV1::LimitExceeded {
                field: "guarded receipt work",
                actual: error.actual(),
                max: error.limit(),
            })
    }
    fn vector<T>(&mut self, count: usize, field: &'static str) -> ResultV3<Vec<T>> {
        self.allocations.vector(count, field)
    }
    fn resource(error: CanonicalKernelIrVerificationResourceErrorV1) -> FormalMemoryReceiptErrorV1 {
        match error {
            CanonicalKernelIrVerificationResourceErrorV1::Work(error) => {
                FormalMemoryReceiptErrorV1::LimitExceeded {
                    field: "guarded receipt work",
                    actual: error.actual(),
                    max: error.limit(),
                }
            }
            _ => overflow(),
        }
    }
    fn sort<T>(
        &mut self,
        rows: &mut [T],
        width: usize,
        compare: impl FnMut(&T, &T) -> Ordering,
    ) -> ResultV3<()> {
        verification_bounded_sort_by_v1(
            rows,
            width,
            &mut CanonicalKernelIrVerificationResourceBudgetV1::new(&mut self.work, 0),
            compare,
        )
        .map_err(Self::resource)
    }
    fn find<T>(
        &mut self,
        rows: &[T],
        width: usize,
        compare: impl FnMut(&T) -> Ordering,
    ) -> ResultV3<usize> {
        verification_find_last_by_v1(
            rows,
            width,
            &mut CanonicalKernelIrVerificationResourceBudgetV1::new(&mut self.work, 0),
            compare,
        )
        .map_err(Self::resource)?
        .ok_or(FormalMemoryReceiptErrorV1::DanglingReference {
            field: "guarded receipt row",
        })
    }
}

struct SortedRows<'a> {
    allocations: Vec<&'a FormalAllocationParameter>,
    accesses: Vec<&'a FormalMemoryAccess>,
    bounds: Vec<&'a FormalBoundsRequirement>,
    aliases: Vec<&'a RuntimeAliasRequirement>,
    conflicts: Vec<&'a InterInvocationConflictRequirement>,
}

fn domain_bytes(domain: FormalAccessDomainV1) -> usize {
    match domain {
        FormalAccessDomainV1::LaunchEnvelope => 1,
        FormalAccessDomainV1::SliceBounded(domain) => match domain.path {
            FormalGuardedPathV1::ExplicitPredicate => 38,
            _ => 54,
        },
    }
}
fn region_bytes(region: FormalAliasRegionV1) -> usize {
    match region {
        FormalAliasRegionV1::FixedBytes(_) => 17,
        _ => 1,
    }
}

fn exact_bytes(obligations: &FormalMemoryObligations, meter: &mut CodecMeter) -> ResultV3<usize> {
    let counts = ObligationRecordCountsV1::from_obligations(obligations);
    preflight_record_counts(counts)?;
    let rows = counts
        .fields()
        .iter()
        .try_fold(0usize, |n, (_, c)| n.checked_add(*c).ok_or_else(overflow))?;
    meter.charge(
        32usize
            .checked_add(4usize.checked_mul(rows).ok_or_else(overflow)?)
            .ok_or_else(overflow)?,
    )?;
    let mut bytes: usize = HEADER_BYTES
        + 8
        + 4
        + if obligations.invocations.is_some() {
            17
        } else {
            1
        }
        + 20;
    for (field, name) in [
        ("kernel ID", obligations.kernel.as_str()),
        ("entry function ID", obligations.entry.as_str()),
    ] {
        if name.is_empty() {
            return Err(FormalMemoryReceiptErrorV1::InvalidIdentity { field });
        }
        if name.len() > MAX_TEXT_BYTES_V1 {
            return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field,
                actual: name.len(),
                max: MAX_TEXT_BYTES_V1,
            });
        }
        bytes = bytes.checked_add(name.len()).ok_or_else(overflow)?;
    }
    for _ in &obligations.allocations {
        bytes = bytes.checked_add(12).ok_or_else(overflow)?;
    }
    for row in &obligations.accesses {
        bytes = bytes
            .checked_add(70 + domain_bytes(row.domain))
            .ok_or_else(overflow)?;
    }
    for row in &obligations.bounds_requirements {
        let width = match row.kind {
            FormalBoundsKindV1::FixedMinimumBytes(_) => 25,
            FormalBoundsKindV1::SliceElementAtGuardedIndex(domain) => {
                17 + domain_bytes(FormalAccessDomainV1::SliceBounded(domain))
            }
        };
        bytes = bytes.checked_add(width).ok_or_else(overflow)?;
    }
    for row in &obligations.runtime_alias_requirements {
        bytes = bytes
            .checked_add(8 + region_bytes(row.left_region()) + region_bytes(row.right_region()))
            .ok_or_else(overflow)?;
    }
    for _ in &obligations.inter_invocation_conflicts {
        bytes = bytes.checked_add(28).ok_or_else(overflow)?;
    }
    if bytes > MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1 {
        return Err(FormalMemoryReceiptErrorV1::TooLarge {
            max: MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1,
        });
    }
    Ok(bytes)
}

fn copy_rows<'a, T>(rows: &'a [T], meter: &mut CodecMeter) -> ResultV3<Vec<&'a T>> {
    let mut out = meter.vector(rows.len(), "guarded encoder order")?;
    out.extend(rows.iter());
    Ok(out)
}

fn encode_v3(obligations: &FormalMemoryObligations, meter: &mut CodecMeter) -> ResultV3<Vec<u8>> {
    let length = exact_bytes(obligations, meter)?;
    let mut rows = SortedRows {
        allocations: copy_rows(&obligations.allocations, meter)?,
        accesses: copy_rows(&obligations.accesses, meter)?,
        bounds: copy_rows(&obligations.bounds_requirements, meter)?,
        aliases: copy_rows(&obligations.runtime_alias_requirements, meter)?,
        conflicts: copy_rows(&obligations.inter_invocation_conflicts, meter)?,
    };
    meter.sort(&mut rows.allocations, 1, |a, b| a.identity.cmp(&b.identity))?;
    meter.sort(&mut rows.accesses, 2, |a, b| a.location.cmp(&b.location))?;
    meter.sort(&mut rows.bounds, 2, |a, b| a.location.cmp(&b.location))?;
    meter.sort(&mut rows.aliases, 2, |a, b| {
        (a.left, a.right).cmp(&(b.left, b.right))
    })?;
    meter.sort(&mut rows.conflicts, 4, |a, b| {
        (a.left, a.right).cmp(&(b.left, b.right))
    })?;
    meter.charge(length)?;
    let mut writer = Writer {
        bytes: meter.vector(length, "guarded receipt bytes")?,
        aggregate_records: 0,
    };
    writer.bytes(&MAGIC_V1)?;
    writer.u16(3)?;
    writer.u16(2)?;
    writer.u16(0)?;
    writer.u16(0)?;
    writer.u32(0)?;
    writer.text("kernel ID", obligations.kernel.as_str())?;
    writer.text("entry function ID", obligations.entry.as_str())?;
    writer.u8(index_width_tag(obligations.index_width))?;
    writer.u8(analysis_basis_tag(obligations.analysis_basis()))?;
    writer.u16(0)?;
    encode_optional_invocations(&mut writer, obligations.invocations)?;
    writer.count("allocations", rows.allocations.len())?;
    for row in rows.allocations {
        meter.charge(32)?;
        encode_allocation(&mut writer, row)?;
    }
    writer.count("accesses", rows.accesses.len())?;
    for row in rows.accesses {
        meter.charge(32)?;
        encode_access(&mut writer, row)?;
        encode_domain(&mut writer, row.domain)?;
    }
    writer.count("bounds requirements", rows.bounds.len())?;
    for row in rows.bounds {
        meter.charge(32)?;
        encode_location(&mut writer, row.location)?;
        writer.u32(row.allocation.parameter_index)?;
        match row.kind {
            FormalBoundsKindV1::FixedMinimumBytes(bytes) => {
                writer.u8(0)?;
                writer.u64(bytes)?;
            }
            FormalBoundsKindV1::SliceElementAtGuardedIndex(domain) => {
                writer.u8(1)?;
                encode_domain(&mut writer, FormalAccessDomainV1::SliceBounded(domain))?;
            }
        }
    }
    writer.count("runtime alias requirements", rows.aliases.len())?;
    for row in rows.aliases {
        meter.charge(32)?;
        writer.u32(row.left.parameter_index)?;
        writer.u32(row.right.parameter_index)?;
        encode_region(&mut writer, row.left_region())?;
        encode_region(&mut writer, row.right_region())?;
    }
    writer.count("inter-invocation conflicts", rows.conflicts.len())?;
    for row in rows.conflicts {
        meter.charge(32)?;
        encode_conflict(&mut writer, *row)?;
    }
    if writer.bytes.len() != length {
        return Err(FormalMemoryReceiptErrorV1::InconsistentReference {
            field: "guarded receipt exact length",
        });
    }
    writer.finish()
}

fn encode_domain(writer: &mut Writer, domain: FormalAccessDomainV1) -> ResultV3<()> {
    match domain {
        FormalAccessDomainV1::LaunchEnvelope => writer.u8(0),
        FormalAccessDomainV1::SliceBounded(domain) => {
            writer.u8(1)?;
            writer.u32(domain.allocation.parameter_index)?;
            for value in [
                domain.slice,
                domain.index,
                domain.length,
                domain.predicate,
                domain.selected_offset,
                domain.pointer,
            ] {
                writer.u32(value.0)?;
            }
            writer.u64(domain.element_bytes)?;
            match domain.path {
                FormalGuardedPathV1::ExplicitPredicate => writer.u8(0),
                FormalGuardedPathV1::TrueEdge {
                    source,
                    ordinal,
                    target,
                } => {
                    writer.u8(1)?;
                    writer.u32(source.0)?;
                    writer.u64(u64::try_from(ordinal).map_err(|_| overflow())?)?;
                    writer.u32(target.0)
                }
            }
        }
    }
}
fn encode_region(writer: &mut Writer, region: FormalAliasRegionV1) -> ResultV3<()> {
    match region {
        FormalAliasRegionV1::FixedBytes(range) => {
            writer.u8(0)?;
            writer.u64(range.start())?;
            writer.u64(range.end_exclusive())
        }
        FormalAliasRegionV1::WholeFormalAllocation => writer.u8(1),
    }
}

#[derive(Clone, Copy)]
struct AllocationV3 {
    record: AllocationRecord,
    guarded: bool,
    accessed: bool,
    writes: bool,
}
#[derive(Clone, Copy)]
struct AccessV3 {
    record: AccessRecord,
    domain: FormalAccessDomainV1,
    bound_seen: bool,
}

fn invalid(field: &'static str) -> FormalMemoryReceiptErrorV1 {
    FormalMemoryReceiptErrorV1::InconsistentReference { field }
}

fn decode_domain(reader: &mut Reader<'_>) -> ResultV3<FormalAccessDomainV1> {
    match reader.u8()? {
        0 => Ok(FormalAccessDomainV1::LaunchEnvelope),
        1 => {
            let allocation = FormalAllocationIdentity {
                parameter_index: reader.u32()?,
            };
            let slice = ValueId(reader.u32()?);
            let index = ValueId(reader.u32()?);
            let length = ValueId(reader.u32()?);
            let predicate = ValueId(reader.u32()?);
            let selected_offset = ValueId(reader.u32()?);
            let pointer = ValueId(reader.u32()?);
            let element_bytes = reader.u64()?;
            if !matches!(element_bytes, 1 | 2 | 4 | 8 | 16) {
                return Err(FormalMemoryReceiptErrorV1::InvalidValue {
                    field: "guarded scalar element width",
                });
            }
            let path = match reader.u8()? {
                0 => FormalGuardedPathV1::ExplicitPredicate,
                1 => FormalGuardedPathV1::TrueEdge {
                    source: BlockId(reader.u32()?),
                    ordinal: usize::try_from(reader.u64()?).map_err(|_| overflow())?,
                    target: BlockId(reader.u32()?),
                },
                tag => {
                    return Err(FormalMemoryReceiptErrorV1::UnknownTag {
                        kind: "guarded path",
                        tag,
                    });
                }
            };
            let values = [slice, index, length, predicate, selected_offset, pointer];
            for i in 0..values.len() {
                if values[i + 1..].contains(&values[i]) {
                    return Err(invalid("guarded recipe distinct value definitions"));
                }
            }
            Ok(FormalAccessDomainV1::SliceBounded(
                FormalSliceBoundedDomainV1 {
                    allocation,
                    slice,
                    index,
                    length,
                    predicate,
                    selected_offset,
                    pointer,
                    element_bytes,
                    path,
                },
            ))
        }
        tag => Err(FormalMemoryReceiptErrorV1::UnknownTag {
            kind: "access domain",
            tag,
        }),
    }
}

fn decode_region(reader: &mut Reader<'_>) -> ResultV3<FormalAliasRegionV1> {
    match reader.u8()? {
        0 => {
            let start = reader.u64()?;
            let end_exclusive = reader.u64()?;
            if start >= end_exclusive {
                return Err(FormalMemoryReceiptErrorV1::InvalidRange {
                    field: "runtime alias accessed bytes",
                });
            }
            Ok(FormalAliasRegionV1::FixedBytes(
                super::super::FormalByteRange {
                    start,
                    end_exclusive,
                },
            ))
        }
        1 => Ok(FormalAliasRegionV1::WholeFormalAllocation),
        tag => Err(FormalMemoryReceiptErrorV1::UnknownTag {
            kind: "runtime alias region",
            tag,
        }),
    }
}

fn strict_key<T: Ord>(previous: &mut Option<T>, key: T, field: &'static str) -> ResultV3<()> {
    if previous.as_ref() == Some(&key) {
        return Err(FormalMemoryReceiptErrorV1::SemanticKeyConflict { field });
    }
    enforce_strict_order(previous, key, field)
}

fn validate_v3(bytes: &[u8], meter: &mut CodecMeter) -> ResultV3<FormalMemoryReceiptMetadataV3> {
    if bytes.len() > MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1 {
        return Err(FormalMemoryReceiptErrorV1::TooLarge {
            max: MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1,
        });
    }
    // Cover parsing, the later identity hash and the bounded typed metadata reread.
    meter.charge(
        bytes
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(32))
            .ok_or_else(overflow)?,
    )?;
    let mut reader = Reader::new(bytes);
    if reader.fixed::<8>()? != MAGIC_V1 {
        return Err(FormalMemoryReceiptErrorV1::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != 3 {
        return Err(FormalMemoryReceiptErrorV1::UnknownVersion(version));
    }
    let policy = reader.u16()?;
    if policy != 2 {
        return Err(FormalMemoryReceiptErrorV1::UnknownPolicy(policy));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(FormalMemoryReceiptErrorV1::UnsupportedFlags(flags));
    }
    reader.reserved_u16("receipt header")?;
    let declared = reader.u32()?;
    if declared < HEADER_BYTES as u32 {
        return Err(FormalMemoryReceiptErrorV1::InvalidLength { declared });
    }
    match (declared as usize).cmp(&bytes.len()) {
        Ordering::Less => return Err(FormalMemoryReceiptErrorV1::TrailingBytes),
        Ordering::Greater => return Err(FormalMemoryReceiptErrorV1::Truncated),
        _ => {}
    }
    for field in ["kernel ID", "entry function ID"] {
        if reader.text(field)?.is_empty() {
            return Err(FormalMemoryReceiptErrorV1::InvalidIdentity { field });
        }
    }
    let width = reader.u8()?;
    decode_index_width(width)?;
    decode_analysis_basis(reader.u8()?)?;
    reader.reserved_u16("obligation preamble")?;
    let invocations = decode_optional_invocations(&mut reader)?;
    let count = reader.count("allocations")?;
    reader.require_fixed_records(count, 12, "allocations")?;
    let mut allocations = meter.vector::<AllocationV3>(count, "guarded allocation metadata")?;
    let mut values = meter.vector::<u32>(count, "guarded allocation values")?;
    let mut previous = None;
    for _ in 0..count {
        meter.charge(16)?;
        let record = decode_allocation(&mut reader, 3)?;
        strict_key(&mut previous, record.0, "allocation parameter index")?;
        values.push(record.1);
        allocations.push(AllocationV3 {
            record,
            guarded: false,
            accessed: false,
            writes: false,
        });
    }
    meter.sort(&mut values, 1, Ord::cmp)?;
    meter.charge(values.len())?;
    if values.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err(FormalMemoryReceiptErrorV1::SemanticKeyConflict {
            field: "allocation value",
        });
    }
    drop(values);

    let count = reader.count("accesses")?;
    reader.require_fixed_records(count, 71, "accesses")?;
    let mut accesses = meter.vector::<AccessV3>(count, "guarded access metadata")?;
    let mut previous = None;
    let mut has_guarded = false;
    for _ in 0..count {
        meter.charge(64)?;
        let record = decode_access(&mut reader)?;
        let domain = decode_domain(&mut reader)?;
        strict_key(&mut previous, record.0, "access location")?;
        let index = meter.find(&allocations, 1, |row| row.record.0.cmp(&record.1))?;
        let allocation = &mut allocations[index];
        if allocation.record.3 != record.3 {
            return Err(invalid("access and allocation address space"));
        }
        if record.2 != 1 && allocation.record.4 == 1 {
            return Err(FormalMemoryReceiptErrorV1::AccessViolation {
                field: "write through read-only allocation",
            });
        }
        if record.2 == 1 && allocation.record.4 == 3 {
            return Err(FormalMemoryReceiptErrorV1::AccessViolation {
                field: "read through write-only allocation",
            });
        }
        if record.5 == 0 {
            return Err(FormalMemoryReceiptErrorV1::InvalidValue {
                field: "access byte width",
            });
        }
        if !record.6.is_power_of_two() {
            return Err(FormalMemoryReceiptErrorV1::InvalidValue {
                field: "access alignment",
            });
        }
        if invocations != Some(record.7) {
            return Err(FormalMemoryReceiptErrorV1::InvocationInconsistency);
        }
        if let FormalAccessDomainV1::SliceBounded(domain) = domain {
            if width == 3
                || allocation.record.2 != 2
                || allocation.record.1 != domain.slice.0
                || record.1 != domain.allocation.parameter_index
                || record.2 == 3
                || record.4 != (1, 0, domain.element_bytes)
                || record.5 != domain.element_bytes
            {
                return Err(invalid("guarded access recipe and allocation"));
            }
            allocation.guarded = true;
            has_guarded = true;
        }
        allocation.accessed = true;
        allocation.writes |= record.2 != 1;
        accesses.push(AccessV3 {
            record,
            domain,
            bound_seen: false,
        });
    }

    let count = reader.count("bounds requirements")?;
    reader.require_fixed_records(count, 25, "bounds requirements")?;
    let mut previous = None;
    for _ in 0..count {
        meter.charge(32)?;
        let location = decode_location(&mut reader)?;
        let allocation = reader.u32()?;
        strict_key(&mut previous, location, "bounds location")?;
        let index = meter.find(&accesses, 2, |row| row.record.0.cmp(&location))?;
        let access = &mut accesses[index];
        if access.record.1 != allocation {
            return Err(invalid("bounds allocation and access location"));
        }
        match reader.u8()? {
            0 => {
                let minimum = reader.u64()?;
                if minimum == 0 {
                    return Err(FormalMemoryReceiptErrorV1::InvalidRange {
                        field: "bounds minimum byte length",
                    });
                }
                if access.domain != FormalAccessDomainV1::LaunchEnvelope {
                    return Err(invalid("guarded access requires symbolic bounds"));
                }
            }
            1 => {
                let domain = decode_domain(&mut reader)?;
                if !matches!(domain, FormalAccessDomainV1::SliceBounded(_))
                    || access.domain != domain
                {
                    return Err(invalid("bounds and access complete guarded domain"));
                }
            }
            tag => {
                return Err(FormalMemoryReceiptErrorV1::UnknownTag {
                    kind: "bounds kind",
                    tag,
                });
            }
        }
        access.bound_seen = true;
    }
    meter.charge(accesses.len())?;
    if accesses
        .iter()
        .any(|row| row.domain != FormalAccessDomainV1::LaunchEnvelope && !row.bound_seen)
    {
        return Err(FormalMemoryReceiptErrorV1::DanglingReference {
            field: "guarded access symbolic bound",
        });
    }

    let count = reader.count("runtime alias requirements")?;
    reader.require_fixed_records(count, 10, "runtime alias requirements")?;
    let mut previous = None;
    for _ in 0..count {
        meter.charge(24)?;
        let left = reader.u32()?;
        let right = reader.u32()?;
        let left_region = decode_region(&mut reader)?;
        let right_region = decode_region(&mut reader)?;
        strict_key(
            &mut previous,
            (left, right),
            "runtime alias allocation pair",
        )?;
        if left >= right {
            return Err(FormalMemoryReceiptErrorV1::InvalidRange {
                field: "runtime alias allocation ordering",
            });
        }
        let left = meter.find(&allocations, 1, |row| row.record.0.cmp(&left))?;
        let right = meter.find(&allocations, 1, |row| row.record.0.cmp(&right))?;
        for (allocation, region) in [
            (&allocations[left], left_region),
            (&allocations[right], right_region),
        ] {
            if !allocation.accessed
                || allocation.guarded
                    != matches!(region, FormalAliasRegionV1::WholeFormalAllocation)
            {
                return Err(invalid("runtime alias region and guarded allocation"));
            }
        }
        if !allocations[left].writes && !allocations[right].writes {
            return Err(invalid("runtime alias read-only pair"));
        }
    }

    let count = reader.count("inter-invocation conflicts")?;
    reader.require_fixed_records(count, 28, "inter-invocation conflicts")?;
    let mut previous = None;
    for _ in 0..count {
        meter.charge(24)?;
        let (left, right, allocation) = decode_conflict(&mut reader)?;
        strict_key(
            &mut previous,
            (left, right),
            "inter-invocation conflict location pair",
        )?;
        let left = meter.find(&accesses, 2, |row| row.record.0.cmp(&left))?;
        let right = meter.find(&accesses, 2, |row| row.record.0.cmp(&right))?;
        let a = accesses[left].record;
        let b = accesses[right].record;
        if a.1 != allocation || b.1 != allocation {
            return Err(invalid(
                "inter-invocation conflict allocation and access locations",
            ));
        }
        if (a.2 == 1 && b.2 == 1) || (a.2 == 3 && b.2 == 3) {
            return Err(FormalMemoryReceiptErrorV1::InvalidConflict {
                field: "non-conflicting access pair",
            });
        }
    }
    if !reader.is_finished() {
        return Err(FormalMemoryReceiptErrorV1::TrailingBytes);
    }
    if !has_guarded {
        return Err(FormalMemoryReceiptErrorV1::NonCanonicalVersion { version: 3 });
    }
    read_metadata(bytes)
}

#[cfg(test)]
#[path = "receipt_v3_tests.rs"]
mod tests;
