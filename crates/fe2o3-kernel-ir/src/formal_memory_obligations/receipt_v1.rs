use std::{error::Error, fmt, mem::size_of, str};

use sha2::{Digest, Sha256};

use super::{
    FormalAllocationParameter, FormalBoundsRequirement, FormalIndexWidth, FormalMemoryAccess,
    FormalMemoryAccessKind, FormalMemoryAnalysisBasis, FormalMemoryObligations,
    FormalParameterKind, InterInvocationConflictRequirement, RuntimeAliasRequirement,
};
use crate::{
    AccessMode, AddressSpace, ByteExpression, FunctionOperationLocation, InvocationRange1d,
    MAX_TEXT_BYTES_V1,
};

/// Canonical formal-memory obligation receipt version.
pub const FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V1: u16 = 1;
/// Additive receipt version admitting compiler-derived write-only allocations.
pub const FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V2: u16 = 2;
/// Extraction-policy version committed by every V1 receipt.
pub const FORMAL_MEMORY_OBLIGATION_POLICY_V1: u16 = 1;
/// Maximum exact bytes in one formal-memory obligation receipt.
pub const MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1: usize = 16 * 1024 * 1024;
/// Independent module-scale cap for any one obligation collection.
///
/// This is deliberately not the Kernel IR per-block operation cap. One formal
/// result can cover operations from many blocks. The byte cap is usually the
/// tighter bound, but this count cap keeps malformed inputs bounded even if a
/// future record encoding becomes smaller.
pub const MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1: usize = 1_048_576;
/// Aggregate module-scale record budget across every obligation collection.
pub const MAX_FORMAL_MEMORY_RECORDS_V1: usize = 1_048_576;
/// Maximum Rust-visible auxiliary vector capacity owned by the V1 decoder.
///
/// This excludes the caller-provided canonical byte buffer and allocator
/// bookkeeping. The decoder charges every vector's actual capacity, without
/// credit for early release, and rejects inputs that would cross this bound.
pub const MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1: usize = 128 * 1024 * 1024;

const MAGIC_V1: [u8; 8] = *b"FE2O3FM\0";
const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/INERT-FORMAL-MEMORY-OBLIGATION-CONTENT/V1\0";
const HEADER_BYTES: usize = 20;

/// Typed identity of exact inert formal-memory obligation content.
///
/// This digest identifies bytes only. It is not producer authentication,
/// proof authority, or evidence that extraction was complete.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertFormalMemoryObligationReceiptIdentityV1([u8; 32]);

impl InertFormalMemoryObligationReceiptIdentityV1 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact canonical bytes for inert formal-memory obligations.
///
/// This content container can be reconstructed by any caller. It does not
/// authenticate its producer, bind itself to a verified KIR identity, record
/// extraction completeness, establish that a runtime binding satisfies the
/// obligations, or carry Verus or other proof authority. A later
/// producer-derived receipt must bind this content identity to the exact KIR,
/// extraction inputs and policy, and complete/incomplete analysis result.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalFormalMemoryObligationReceiptV1 {
    canonical_bytes: Vec<u8>,
    identity: InertFormalMemoryObligationReceiptIdentityV1,
}

impl InertCanonicalFormalMemoryObligationReceiptV1 {
    pub fn from_obligations(
        obligations: &FormalMemoryObligations,
    ) -> Result<Self, FormalMemoryReceiptErrorV1> {
        let canonical_bytes = encode_obligations(obligations)?;
        validate_receipt(&canonical_bytes)?;
        Ok(Self::from_validated_bytes(canonical_bytes))
    }

    pub fn from_canonical_bytes(
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, FormalMemoryReceiptErrorV1> {
        validate_receipt(&canonical_bytes)?;
        Ok(Self::from_validated_bytes(canonical_bytes))
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the exact kernel identifier retained by this validated receipt.
    pub fn kernel_id(&self) -> &str {
        self.binding_names().0
    }

    /// Returns the exact entry-function identifier retained by this validated receipt.
    pub fn entry_id(&self) -> &str {
        self.binding_names().1
    }

    pub const fn identity(&self) -> &InertFormalMemoryObligationReceiptIdentityV1 {
        &self.identity
    }

    pub fn revalidate(&self) -> Result<(), FormalMemoryReceiptErrorV1> {
        validate_receipt(&self.canonical_bytes)?;
        if receipt_identity(&self.canonical_bytes) != self.identity {
            return Err(FormalMemoryReceiptErrorV1::IdentityMismatch);
        }
        Ok(())
    }

    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }

    fn from_validated_bytes(canonical_bytes: Vec<u8>) -> Self {
        let identity = receipt_identity(&canonical_bytes);
        Self {
            canonical_bytes,
            identity,
        }
    }

    fn binding_names(&self) -> (&str, &str) {
        let mut reader = Reader::new(&self.canonical_bytes);
        reader
            .fixed::<HEADER_BYTES>()
            .expect("validated formal-memory receipt retains its complete header");
        let kernel = reader
            .text("kernel ID")
            .expect("validated formal-memory receipt retains its kernel ID");
        let entry = reader
            .text("entry function ID")
            .expect("validated formal-memory receipt retains its entry function ID");
        (kernel, entry)
    }
}

fn encode_obligations(
    obligations: &FormalMemoryObligations,
) -> Result<Vec<u8>, FormalMemoryReceiptErrorV1> {
    preflight_record_counts(ObligationRecordCountsV1::from_obligations(obligations))?;
    let version = if obligations
        .allocations
        .iter()
        .any(|allocation| allocation.access == AccessMode::WriteOnly)
    {
        FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V2
    } else {
        FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V1
    };

    let mut writer = Writer::new();
    writer.bytes(&MAGIC_V1)?;
    writer.u16(version)?;
    writer.u16(FORMAL_MEMORY_OBLIGATION_POLICY_V1)?;
    writer.u16(0)?;
    writer.u16(0)?;
    writer.u32(0)?;
    writer.text("kernel ID", obligations.kernel.as_str())?;
    writer.text("entry function ID", obligations.entry.as_str())?;
    writer.u8(index_width_tag(obligations.index_width))?;
    writer.u8(analysis_basis_tag(obligations.analysis_basis()))?;
    writer.u16(0)?;
    encode_optional_invocations(&mut writer, obligations.invocations)?;

    let mut allocations: Vec<_> = obligations.allocations.iter().collect();
    allocations.sort_unstable();
    writer.count("allocations", allocations.len())?;
    for allocation in allocations {
        encode_allocation(&mut writer, allocation)?;
    }

    let mut accesses: Vec<_> = obligations.accesses.iter().collect();
    accesses.sort_unstable();
    writer.count("accesses", accesses.len())?;
    for access in accesses {
        encode_access(&mut writer, access)?;
    }

    let mut bounds: Vec<_> = obligations.bounds_requirements.iter().collect();
    bounds.sort_unstable();
    writer.count("bounds requirements", bounds.len())?;
    for requirement in bounds {
        encode_bounds(&mut writer, *requirement)?;
    }

    let mut aliases: Vec<_> = obligations.runtime_alias_requirements.iter().collect();
    aliases.sort_unstable();
    writer.count("runtime alias requirements", aliases.len())?;
    for requirement in aliases {
        encode_alias(&mut writer, *requirement)?;
    }

    let mut conflicts: Vec<_> = obligations.inter_invocation_conflicts.iter().collect();
    conflicts.sort_unstable();
    writer.count("inter-invocation conflicts", conflicts.len())?;
    for requirement in conflicts {
        encode_conflict(&mut writer, *requirement)?;
    }

    writer.finish()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObligationRecordCountsV1 {
    allocations: usize,
    accesses: usize,
    bounds: usize,
    aliases: usize,
    conflicts: usize,
}

impl ObligationRecordCountsV1 {
    fn from_obligations(obligations: &FormalMemoryObligations) -> Self {
        Self {
            allocations: obligations.allocations.len(),
            accesses: obligations.accesses.len(),
            bounds: obligations.bounds_requirements.len(),
            aliases: obligations.runtime_alias_requirements.len(),
            conflicts: obligations.inter_invocation_conflicts.len(),
        }
    }

    const fn fields(self) -> [(&'static str, usize); 5] {
        [
            ("allocations", self.allocations),
            ("accesses", self.accesses),
            ("bounds requirements", self.bounds),
            ("runtime alias requirements", self.aliases),
            ("inter-invocation conflicts", self.conflicts),
        ]
    }
}

fn preflight_record_counts(
    counts: ObligationRecordCountsV1,
) -> Result<(), FormalMemoryReceiptErrorV1> {
    let mut aggregate = 0_usize;
    for (field, count) in counts.fields() {
        if count > MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1 {
            return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field,
                actual: count,
                max: MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1,
            });
        }
        aggregate = aggregate
            .checked_add(count)
            .ok_or(FormalMemoryReceiptErrorV1::Overflow {
                field: "aggregate formal-memory record count",
            })?;
    }
    if aggregate > MAX_FORMAL_MEMORY_RECORDS_V1 {
        return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
            field: "aggregate formal-memory record count",
            actual: aggregate,
            max: MAX_FORMAL_MEMORY_RECORDS_V1,
        });
    }
    Ok(())
}

fn encode_allocation(
    writer: &mut Writer,
    allocation: &FormalAllocationParameter,
) -> Result<(), FormalMemoryReceiptErrorV1> {
    writer.u32(allocation.identity.parameter_index)?;
    writer.u32(allocation.value.0)?;
    writer.u8(parameter_kind_tag(allocation.kind))?;
    writer.u8(address_space_tag(allocation.address_space))?;
    writer.u8(access_mode_tag(allocation.access))?;
    writer.u8(0)
}

fn encode_access(
    writer: &mut Writer,
    access: &FormalMemoryAccess,
) -> Result<(), FormalMemoryReceiptErrorV1> {
    encode_location(writer, access.location)?;
    writer.u32(access.allocation.parameter_index)?;
    writer.u8(memory_access_kind_tag(access.kind))?;
    writer.u8(address_space_tag(access.address_space))?;
    encode_byte_expression(writer, access.byte_offset)?;
    writer.u64(access.byte_width)?;
    writer.u64(access.alignment)?;
    encode_invocations(writer, access.invocations)
}

fn encode_bounds(
    writer: &mut Writer,
    requirement: FormalBoundsRequirement,
) -> Result<(), FormalMemoryReceiptErrorV1> {
    encode_location(writer, requirement.location)?;
    writer.u32(requirement.allocation.parameter_index)?;
    writer.u64(requirement.minimum_byte_len)
}

fn encode_alias(
    writer: &mut Writer,
    requirement: RuntimeAliasRequirement,
) -> Result<(), FormalMemoryReceiptErrorV1> {
    writer.u32(requirement.left.parameter_index)?;
    writer.u32(requirement.right.parameter_index)?;
    writer.u64(requirement.left_accessed_bytes.start)?;
    writer.u64(requirement.left_accessed_bytes.end_exclusive)?;
    writer.u64(requirement.right_accessed_bytes.start)?;
    writer.u64(requirement.right_accessed_bytes.end_exclusive)
}

fn encode_conflict(
    writer: &mut Writer,
    requirement: InterInvocationConflictRequirement,
) -> Result<(), FormalMemoryReceiptErrorV1> {
    encode_location(writer, requirement.left)?;
    encode_location(writer, requirement.right)?;
    writer.u32(requirement.allocation.parameter_index)
}

fn encode_location(
    writer: &mut Writer,
    location: FunctionOperationLocation,
) -> Result<(), FormalMemoryReceiptErrorV1> {
    writer.u32(location.block.0)?;
    writer.u64(u64::try_from(location.operation_index).map_err(|_| {
        FormalMemoryReceiptErrorV1::Overflow {
            field: "operation index",
        }
    })?)
}

fn encode_optional_invocations(
    writer: &mut Writer,
    invocations: Option<InvocationRange1d>,
) -> Result<(), FormalMemoryReceiptErrorV1> {
    match invocations {
        Some(invocations) => {
            writer.u8(1)?;
            encode_invocations(writer, invocations)
        }
        None => writer.u8(0),
    }
}

fn encode_invocations(
    writer: &mut Writer,
    invocations: InvocationRange1d,
) -> Result<(), FormalMemoryReceiptErrorV1> {
    writer.u64(invocations.start())?;
    writer.u64(invocations.end_exclusive())
}

fn encode_byte_expression(
    writer: &mut Writer,
    expression: ByteExpression,
) -> Result<(), FormalMemoryReceiptErrorV1> {
    match expression {
        ByteExpression::Affine {
            constant,
            invocation_coefficient,
        } => {
            writer.u8(1)?;
            writer.u8(0)?;
            writer.u16(0)?;
            writer.u64(constant)?;
            writer.u64(invocation_coefficient)
        }
        ByteExpression::Unbounded => {
            writer.u8(2)?;
            writer.u8(0)?;
            writer.u16(0)?;
            writer.u64(0)?;
            writer.u64(0)
        }
    }
}

fn validate_receipt(bytes: &[u8]) -> Result<(), FormalMemoryReceiptErrorV1> {
    if bytes.len() > MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1 {
        return Err(FormalMemoryReceiptErrorV1::TooLarge {
            max: MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1,
        });
    }
    let mut reader = Reader::new(bytes);
    if reader.fixed::<8>()? != MAGIC_V1 {
        return Err(FormalMemoryReceiptErrorV1::InvalidMagic);
    }
    let version = reader.u16()?;
    if !matches!(
        version,
        FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V1 | FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V2
    ) {
        return Err(FormalMemoryReceiptErrorV1::UnknownVersion(version));
    }
    let policy = reader.u16()?;
    if policy != FORMAL_MEMORY_OBLIGATION_POLICY_V1 {
        return Err(FormalMemoryReceiptErrorV1::UnknownPolicy(policy));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(FormalMemoryReceiptErrorV1::UnsupportedFlags(flags));
    }
    reader.reserved_u16("receipt header")?;
    let declared = reader.u32()?;
    let declared = usize::try_from(declared)
        .map_err(|_| FormalMemoryReceiptErrorV1::InvalidLength { declared })?;
    if declared < HEADER_BYTES {
        return Err(FormalMemoryReceiptErrorV1::InvalidLength {
            declared: declared as u32,
        });
    }
    if declared > bytes.len() {
        return Err(FormalMemoryReceiptErrorV1::Truncated);
    }
    if declared < bytes.len() {
        return Err(FormalMemoryReceiptErrorV1::TrailingBytes);
    }

    if reader.text("kernel ID")?.is_empty() {
        return Err(FormalMemoryReceiptErrorV1::InvalidIdentity { field: "kernel ID" });
    }
    if reader.text("entry function ID")?.is_empty() {
        return Err(FormalMemoryReceiptErrorV1::InvalidIdentity {
            field: "entry function ID",
        });
    }
    decode_index_width(reader.u8()?)?;
    decode_analysis_basis(reader.u8()?)?;
    reader.reserved_u16("obligation preamble")?;
    let receipt_invocations = decode_optional_invocations(&mut reader)?;

    let mut allocation_budget = DecoderAllocationBudgetV1::new();

    let allocation_count = reader.count("allocations")?;
    reader.require_fixed_records(allocation_count, ALLOCATION_RECORD_BYTES_V1, "allocations")?;
    let mut previous = None;
    let mut previous_key = None;
    let mut allocations = allocation_budget
        .vector::<AllocationMetadataV1>(allocation_count, "allocation metadata")?;
    let mut allocation_values =
        allocation_budget.vector::<u32>(allocation_count, "allocation value identities")?;
    for _ in 0..allocation_count {
        let record = decode_allocation(&mut reader, version)?;
        if previous_key == Some(record.0) {
            return Err(FormalMemoryReceiptErrorV1::SemanticKeyConflict {
                field: "allocation parameter index",
            });
        }
        enforce_strict_order(&mut previous, record, "allocations")?;
        previous_key = Some(record.0);
        allocations.push(AllocationMetadataV1 {
            parameter_index: record.0,
            address_space: record.3,
            access_mode: record.4,
        });
        allocation_values.push(record.1);
    }
    allocation_values.sort_unstable();
    if allocation_values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(FormalMemoryReceiptErrorV1::SemanticKeyConflict {
            field: "allocation value",
        });
    }
    drop(allocation_values);
    if version == FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V2
        && !allocations
            .iter()
            .any(|allocation| allocation.access_mode == access_mode_tag(AccessMode::WriteOnly))
    {
        return Err(FormalMemoryReceiptErrorV1::NonCanonicalVersion { version });
    }

    let access_count = reader.count("accesses")?;
    reader.require_fixed_records(access_count, ACCESS_RECORD_BYTES_V1, "accesses")?;
    let mut previous = None;
    let mut previous_key = None;
    let mut accesses =
        allocation_budget.vector::<AccessMetadataV1>(access_count, "access metadata")?;
    for _ in 0..access_count {
        let record = decode_access(&mut reader)?;
        if previous_key == Some(record.0) {
            return Err(FormalMemoryReceiptErrorV1::SemanticKeyConflict {
                field: "access location",
            });
        }
        enforce_strict_order(&mut previous, record, "accesses")?;
        previous_key = Some(record.0);
        let allocation = require_allocation(&allocations, record.1, "access allocation")?;
        if record.3 != allocation.address_space {
            return Err(FormalMemoryReceiptErrorV1::InconsistentReference {
                field: "access and allocation address space",
            });
        }
        if record.2 != memory_access_kind_tag(FormalMemoryAccessKind::Read)
            && allocation.access_mode == access_mode_tag(AccessMode::ReadOnly)
        {
            return Err(FormalMemoryReceiptErrorV1::AccessViolation {
                field: "write through read-only allocation",
            });
        }
        if record.2 == memory_access_kind_tag(FormalMemoryAccessKind::Read)
            && allocation.access_mode == access_mode_tag(AccessMode::WriteOnly)
        {
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
        if receipt_invocations != Some(record.7) {
            return Err(FormalMemoryReceiptErrorV1::InvocationInconsistency);
        }
        accesses.push(AccessMetadataV1 {
            operation_index: record.0.1,
            block: record.0.0,
            allocation: record.1,
            kind: record.2,
        });
    }

    let bounds_count = reader.count("bounds requirements")?;
    reader.require_fixed_records(bounds_count, BOUNDS_RECORD_BYTES_V1, "bounds requirements")?;
    let mut previous = None;
    let mut previous_key = None;
    for _ in 0..bounds_count {
        let record = decode_bounds(&mut reader)?;
        if previous_key == Some(record.0) {
            return Err(FormalMemoryReceiptErrorV1::SemanticKeyConflict {
                field: "bounds location",
            });
        }
        enforce_strict_order(&mut previous, record, "bounds requirements")?;
        previous_key = Some(record.0);
        require_allocation(&allocations, record.1, "bounds allocation")?;
        let access = require_access(&accesses, record.0, "bounds access location")?;
        if access.allocation != record.1 {
            return Err(FormalMemoryReceiptErrorV1::InconsistentReference {
                field: "bounds allocation and access location",
            });
        }
        if record.2 == 0 {
            return Err(FormalMemoryReceiptErrorV1::InvalidRange {
                field: "bounds minimum byte length",
            });
        }
    }

    let alias_count = reader.count("runtime alias requirements")?;
    reader.require_fixed_records(
        alias_count,
        ALIAS_RECORD_BYTES_V1,
        "runtime alias requirements",
    )?;
    let mut previous = None;
    let mut previous_key = None;
    for _ in 0..alias_count {
        let record = decode_alias(&mut reader)?;
        let key = (record.0, record.1);
        if previous_key == Some(key) {
            return Err(FormalMemoryReceiptErrorV1::SemanticKeyConflict {
                field: "runtime alias allocation pair",
            });
        }
        enforce_strict_order(&mut previous, record, "runtime alias requirements")?;
        previous_key = Some(key);
        if record.0 >= record.1 {
            return Err(FormalMemoryReceiptErrorV1::InvalidRange {
                field: "runtime alias allocation ordering",
            });
        }
        require_allocation(&allocations, record.0, "runtime alias left allocation")?;
        require_allocation(&allocations, record.1, "runtime alias right allocation")?;
    }

    let conflict_count = reader.count("inter-invocation conflicts")?;
    reader.require_fixed_records(
        conflict_count,
        CONFLICT_RECORD_BYTES_V1,
        "inter-invocation conflicts",
    )?;
    let mut previous = None;
    let mut previous_key = None;
    let mut conflicts = allocation_budget
        .vector::<ConflictRecord>(conflict_count, "inter-invocation conflict metadata")?;
    for _ in 0..conflict_count {
        let record = decode_conflict(&mut reader)?;
        let key = (record.0, record.1);
        if previous_key == Some(key) {
            return Err(FormalMemoryReceiptErrorV1::SemanticKeyConflict {
                field: "inter-invocation conflict location pair",
            });
        }
        enforce_strict_order(&mut previous, record, "inter-invocation conflicts")?;
        previous_key = Some(key);
        conflicts.push(record);
    }
    for (left, right, allocation) in conflicts {
        require_allocation(
            &allocations,
            allocation,
            "inter-invocation conflict allocation",
        )?;
        let left_access = require_access(
            &accesses,
            left,
            "inter-invocation conflict left access location",
        )?;
        let right_access = require_access(
            &accesses,
            right,
            "inter-invocation conflict right access location",
        )?;
        if left_access.allocation != allocation || right_access.allocation != allocation {
            return Err(FormalMemoryReceiptErrorV1::InconsistentReference {
                field: "inter-invocation conflict allocation and access locations",
            });
        }
        if (left_access.kind == memory_access_kind_tag(FormalMemoryAccessKind::Read)
            && right_access.kind == memory_access_kind_tag(FormalMemoryAccessKind::Read))
            || (left_access.kind == memory_access_kind_tag(FormalMemoryAccessKind::Atomic)
                && right_access.kind == memory_access_kind_tag(FormalMemoryAccessKind::Atomic))
        {
            return Err(FormalMemoryReceiptErrorV1::InvalidConflict {
                field: "non-conflicting access pair",
            });
        }
    }

    if !reader.is_finished() {
        return Err(FormalMemoryReceiptErrorV1::TrailingBytes);
    }
    Ok(())
}

fn require_allocation<'metadata>(
    allocations: &'metadata [AllocationMetadataV1],
    allocation: u32,
    field: &'static str,
) -> Result<&'metadata AllocationMetadataV1, FormalMemoryReceiptErrorV1> {
    allocations
        .binary_search_by_key(&allocation, |metadata| metadata.parameter_index)
        .map(|index| &allocations[index])
        .map_err(|_| FormalMemoryReceiptErrorV1::DanglingReference { field })
}

fn require_access<'metadata>(
    accesses: &'metadata [AccessMetadataV1],
    location: LocationRecord,
    field: &'static str,
) -> Result<&'metadata AccessMetadataV1, FormalMemoryReceiptErrorV1> {
    accesses
        .binary_search_by_key(&location, AccessMetadataV1::location)
        .map(|index| &accesses[index])
        .map_err(|_| FormalMemoryReceiptErrorV1::DanglingReference { field })
}

type AllocationRecord = (u32, u32, u8, u8, u8);
type LocationRecord = (u32, u64);
type AccessRecord = (
    LocationRecord,
    u32,
    u8,
    u8,
    (u8, u64, u64),
    u64,
    u64,
    (u64, u64),
);
type BoundsRecord = (LocationRecord, u32, u64);
type AliasRecord = (u32, u32, u64, u64, u64, u64);
type ConflictRecord = (LocationRecord, LocationRecord, u32);

const ALLOCATION_RECORD_BYTES_V1: usize = 12;
const ACCESS_RECORD_BYTES_V1: usize = 70;
const BOUNDS_RECORD_BYTES_V1: usize = 24;
const ALIAS_RECORD_BYTES_V1: usize = 40;
const CONFLICT_RECORD_BYTES_V1: usize = 28;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllocationMetadataV1 {
    parameter_index: u32,
    address_space: u8,
    access_mode: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AccessMetadataV1 {
    operation_index: u64,
    block: u32,
    allocation: u32,
    kind: u8,
}

impl AccessMetadataV1 {
    const fn location(&self) -> LocationRecord {
        (self.block, self.operation_index)
    }
}

struct DecoderAllocationBudgetV1 {
    charged_bytes: usize,
}

impl DecoderAllocationBudgetV1 {
    const fn new() -> Self {
        Self { charged_bytes: 0 }
    }

    fn vector<T>(
        &mut self,
        count: usize,
        field: &'static str,
    ) -> Result<Vec<T>, FormalMemoryReceiptErrorV1> {
        let requested =
            count
                .checked_mul(size_of::<T>())
                .ok_or(FormalMemoryReceiptErrorV1::Overflow {
                    field: "decoder auxiliary allocation",
                })?;
        let requested_total = self.charged_bytes.checked_add(requested).ok_or(
            FormalMemoryReceiptErrorV1::Overflow {
                field: "decoder auxiliary allocation",
            },
        )?;
        if requested_total > MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1 {
            return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field: "decoder auxiliary allocation",
                actual: requested_total,
                max: MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1,
            });
        }

        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| FormalMemoryReceiptErrorV1::DecoderAllocationFailed { field })?;
        let allocated = values.capacity().checked_mul(size_of::<T>()).ok_or(
            FormalMemoryReceiptErrorV1::Overflow {
                field: "decoder auxiliary allocation",
            },
        )?;
        let charged_bytes = self.charged_bytes.checked_add(allocated).ok_or(
            FormalMemoryReceiptErrorV1::Overflow {
                field: "decoder auxiliary allocation",
            },
        )?;
        if charged_bytes > MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1 {
            return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field: "decoder auxiliary allocation",
                actual: charged_bytes,
                max: MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1,
            });
        }
        self.charged_bytes = charged_bytes;
        Ok(values)
    }
}

fn decode_allocation(
    reader: &mut Reader<'_>,
    version: u16,
) -> Result<AllocationRecord, FormalMemoryReceiptErrorV1> {
    let parameter_index = reader.u32()?;
    let value = reader.u32()?;
    let kind = reader.u8()?;
    decode_parameter_kind(kind)?;
    let address_space = reader.u8()?;
    decode_address_space(address_space)?;
    let access = reader.u8()?;
    decode_access_mode(access, version)?;
    reader.reserved_u8("allocation")?;
    Ok((parameter_index, value, kind, address_space, access))
}

fn decode_access(reader: &mut Reader<'_>) -> Result<AccessRecord, FormalMemoryReceiptErrorV1> {
    let location = decode_location(reader)?;
    let allocation = reader.u32()?;
    let kind = reader.u8()?;
    decode_memory_access_kind(kind)?;
    let address_space = reader.u8()?;
    decode_address_space(address_space)?;
    let expression = decode_byte_expression(reader)?;
    let byte_width = reader.u64()?;
    let alignment = reader.u64()?;
    let invocations = decode_invocations(reader)?;
    Ok((
        location,
        allocation,
        kind,
        address_space,
        expression,
        byte_width,
        alignment,
        invocations,
    ))
}

fn decode_bounds(reader: &mut Reader<'_>) -> Result<BoundsRecord, FormalMemoryReceiptErrorV1> {
    Ok((decode_location(reader)?, reader.u32()?, reader.u64()?))
}

fn decode_alias(reader: &mut Reader<'_>) -> Result<AliasRecord, FormalMemoryReceiptErrorV1> {
    let record = (
        reader.u32()?,
        reader.u32()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
    );
    if record.2 >= record.3 || record.4 >= record.5 {
        return Err(FormalMemoryReceiptErrorV1::InvalidRange {
            field: "runtime alias accessed bytes",
        });
    }
    Ok(record)
}

fn decode_conflict(reader: &mut Reader<'_>) -> Result<ConflictRecord, FormalMemoryReceiptErrorV1> {
    Ok((
        decode_location(reader)?,
        decode_location(reader)?,
        reader.u32()?,
    ))
}

fn decode_location(reader: &mut Reader<'_>) -> Result<LocationRecord, FormalMemoryReceiptErrorV1> {
    Ok((reader.u32()?, reader.u64()?))
}

fn decode_optional_invocations(
    reader: &mut Reader<'_>,
) -> Result<Option<(u64, u64)>, FormalMemoryReceiptErrorV1> {
    match reader.u8()? {
        0 => Ok(None),
        1 => decode_invocations(reader).map(Some),
        tag => Err(FormalMemoryReceiptErrorV1::UnknownTag {
            kind: "optional invocation range",
            tag,
        }),
    }
}

fn decode_invocations(reader: &mut Reader<'_>) -> Result<(u64, u64), FormalMemoryReceiptErrorV1> {
    let range = (reader.u64()?, reader.u64()?);
    if range.0 >= range.1 {
        return Err(FormalMemoryReceiptErrorV1::InvalidRange {
            field: "invocation range",
        });
    }
    Ok(range)
}

fn decode_byte_expression(
    reader: &mut Reader<'_>,
) -> Result<(u8, u64, u64), FormalMemoryReceiptErrorV1> {
    let tag = reader.u8()?;
    reader.reserved_u8("byte expression")?;
    reader.reserved_u16("byte expression")?;
    let constant = reader.u64()?;
    let coefficient = reader.u64()?;
    match tag {
        1 => Ok((tag, constant, coefficient)),
        2 if constant == 0 && coefficient == 0 => Ok((tag, constant, coefficient)),
        2 => Err(FormalMemoryReceiptErrorV1::ReservedNonZero {
            field: "unbounded byte expression payload",
        }),
        _ => Err(FormalMemoryReceiptErrorV1::UnknownTag {
            kind: "byte expression",
            tag,
        }),
    }
}

fn enforce_strict_order<T: Ord>(
    previous: &mut Option<T>,
    next: T,
    field: &'static str,
) -> Result<(), FormalMemoryReceiptErrorV1> {
    if previous.as_ref().is_some_and(|value| value >= &next) {
        return Err(FormalMemoryReceiptErrorV1::NonCanonicalOrder { field });
    }
    *previous = Some(next);
    Ok(())
}

fn receipt_identity(bytes: &[u8]) -> InertFormalMemoryObligationReceiptIdentityV1 {
    let mut digest = Sha256::new();
    digest.update((IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(IDENTITY_DOMAIN_V1);
    digest.update(FORMAL_MEMORY_OBLIGATION_POLICY_V1.to_le_bytes());
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    InertFormalMemoryObligationReceiptIdentityV1(digest.finalize().into())
}

const fn index_width_tag(width: FormalIndexWidth) -> u8 {
    match width {
        FormalIndexWidth::Bits32 => 1,
        FormalIndexWidth::Bits64 => 2,
        FormalIndexWidth::Unknown => 3,
    }
}

fn decode_index_width(tag: u8) -> Result<(), FormalMemoryReceiptErrorV1> {
    match tag {
        1..=3 => Ok(()),
        _ => Err(FormalMemoryReceiptErrorV1::UnknownTag {
            kind: "formal index width",
            tag,
        }),
    }
}

const fn analysis_basis_tag(basis: FormalMemoryAnalysisBasis) -> u8 {
    match basis {
        FormalMemoryAnalysisBasis::CompilerDerivedIrWithUnauthenticatedLaunchInputs => 1,
    }
}

fn decode_analysis_basis(tag: u8) -> Result<(), FormalMemoryReceiptErrorV1> {
    match tag {
        1 => Ok(()),
        _ => Err(FormalMemoryReceiptErrorV1::UnknownTag {
            kind: "formal-memory analysis basis",
            tag,
        }),
    }
}

const fn parameter_kind_tag(kind: FormalParameterKind) -> u8 {
    match kind {
        FormalParameterKind::Pointer => 1,
        FormalParameterKind::Slice => 2,
    }
}

fn decode_parameter_kind(tag: u8) -> Result<(), FormalMemoryReceiptErrorV1> {
    match tag {
        1 | 2 => Ok(()),
        _ => Err(FormalMemoryReceiptErrorV1::UnknownTag {
            kind: "formal parameter kind",
            tag,
        }),
    }
}

const fn address_space_tag(address_space: AddressSpace) -> u8 {
    match address_space {
        AddressSpace::Private => 1,
        AddressSpace::Workgroup => 2,
        AddressSpace::Global => 3,
        AddressSpace::Constant => 4,
        AddressSpace::Generic => 5,
    }
}

fn decode_address_space(tag: u8) -> Result<(), FormalMemoryReceiptErrorV1> {
    match tag {
        1..=5 => Ok(()),
        _ => Err(FormalMemoryReceiptErrorV1::UnknownTag {
            kind: "address space",
            tag,
        }),
    }
}

const fn access_mode_tag(access: AccessMode) -> u8 {
    match access {
        AccessMode::ReadOnly => 1,
        AccessMode::ReadWrite => 2,
        AccessMode::WriteOnly => 3,
    }
}

fn decode_access_mode(tag: u8, version: u16) -> Result<(), FormalMemoryReceiptErrorV1> {
    match tag {
        1..=2 => Ok(()),
        3 if version >= FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V2 => Ok(()),
        _ => Err(FormalMemoryReceiptErrorV1::UnknownTag {
            kind: "access mode",
            tag,
        }),
    }
}

const fn memory_access_kind_tag(kind: FormalMemoryAccessKind) -> u8 {
    match kind {
        FormalMemoryAccessKind::Read => 1,
        FormalMemoryAccessKind::Write => 2,
        FormalMemoryAccessKind::Atomic => 3,
    }
}

fn decode_memory_access_kind(tag: u8) -> Result<(), FormalMemoryReceiptErrorV1> {
    match tag {
        1..=3 => Ok(()),
        _ => Err(FormalMemoryReceiptErrorV1::UnknownTag {
            kind: "formal memory access kind",
            tag,
        }),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormalMemoryReceiptErrorV1 {
    TooLarge {
        max: usize,
    },
    LimitExceeded {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    Overflow {
        field: &'static str,
    },
    DecoderAllocationFailed {
        field: &'static str,
    },
    InvalidMagic,
    UnknownVersion(u16),
    NonCanonicalVersion {
        version: u16,
    },
    UnknownPolicy(u16),
    UnsupportedFlags(u16),
    InvalidLength {
        declared: u32,
    },
    Truncated,
    TrailingBytes,
    ReservedNonZero {
        field: &'static str,
    },
    InvalidUtf8 {
        field: &'static str,
    },
    InvalidIdentity {
        field: &'static str,
    },
    UnknownTag {
        kind: &'static str,
        tag: u8,
    },
    InvalidRange {
        field: &'static str,
    },
    InvalidValue {
        field: &'static str,
    },
    SemanticKeyConflict {
        field: &'static str,
    },
    DanglingReference {
        field: &'static str,
    },
    InconsistentReference {
        field: &'static str,
    },
    AccessViolation {
        field: &'static str,
    },
    InvalidConflict {
        field: &'static str,
    },
    InvocationInconsistency,
    NonCanonicalOrder {
        field: &'static str,
    },
    IdentityMismatch,
}

impl fmt::Display for FormalMemoryReceiptErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => {
                write!(formatter, "formal-memory receipt exceeds {max} bytes")
            }
            Self::LimitExceeded { field, actual, max } => {
                write!(
                    formatter,
                    "{field} has {actual} items or bytes; maximum is {max}"
                )
            }
            Self::Overflow { field } => write!(formatter, "{field} does not fit its wire field"),
            Self::DecoderAllocationFailed { field } => {
                write!(formatter, "could not allocate bounded decoder {field}")
            }
            Self::InvalidMagic => formatter.write_str("invalid formal-memory receipt magic"),
            Self::UnknownVersion(version) => {
                write!(formatter, "unknown formal-memory receipt version {version}")
            }
            Self::NonCanonicalVersion { version } => {
                write!(
                    formatter,
                    "noncanonical formal-memory receipt version {version}"
                )
            }
            Self::UnknownPolicy(policy) => write!(
                formatter,
                "unknown formal-memory extraction policy {policy}"
            ),
            Self::UnsupportedFlags(flags) => write!(
                formatter,
                "unsupported formal-memory receipt flags {flags:#x}"
            ),
            Self::InvalidLength { declared } => write!(
                formatter,
                "invalid declared formal-memory receipt length {declared}"
            ),
            Self::Truncated => formatter.write_str("truncated formal-memory receipt"),
            Self::TrailingBytes => formatter.write_str("trailing formal-memory receipt bytes"),
            Self::ReservedNonZero { field } => write!(formatter, "nonzero reserved {field}"),
            Self::InvalidUtf8 { field } => write!(formatter, "invalid UTF-8 in {field}"),
            Self::InvalidIdentity { field } => write!(formatter, "invalid empty {field}"),
            Self::UnknownTag { kind, tag } => write!(formatter, "unknown {kind} tag {tag}"),
            Self::InvalidRange { field } => write!(formatter, "invalid {field}"),
            Self::InvalidValue { field } => write!(formatter, "invalid {field}"),
            Self::SemanticKeyConflict { field } => {
                write!(formatter, "conflicting formal-memory {field}")
            }
            Self::DanglingReference { field } => {
                write!(formatter, "dangling formal-memory {field}")
            }
            Self::InconsistentReference { field } => {
                write!(formatter, "inconsistent formal-memory {field}")
            }
            Self::AccessViolation { field } => {
                write!(formatter, "invalid formal-memory access: {field}")
            }
            Self::InvalidConflict { field } => {
                write!(formatter, "invalid inter-invocation conflict: {field}")
            }
            Self::InvocationInconsistency => formatter.write_str(
                "formal-memory access invocation range does not match the receipt range",
            ),
            Self::NonCanonicalOrder { field } => {
                write!(formatter, "{field} are not in strict canonical order")
            }
            Self::IdentityMismatch => {
                formatter.write_str("formal-memory receipt identity mismatch")
            }
        }
    }
}

impl Error for FormalMemoryReceiptErrorV1 {}

struct Writer {
    bytes: Vec<u8>,
    aggregate_records: usize,
}

impl Writer {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            aggregate_records: 0,
        }
    }

    fn finish(mut self) -> Result<Vec<u8>, FormalMemoryReceiptErrorV1> {
        let length =
            u32::try_from(self.bytes.len()).map_err(|_| FormalMemoryReceiptErrorV1::Overflow {
                field: "receipt length",
            })?;
        self.bytes[16..20].copy_from_slice(&length.to_le_bytes());
        Ok(self.bytes)
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), FormalMemoryReceiptErrorV1> {
        let new_len = self.bytes.len().checked_add(value.len()).ok_or(
            FormalMemoryReceiptErrorV1::Overflow {
                field: "receipt length",
            },
        )?;
        if new_len > MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1 {
            return Err(FormalMemoryReceiptErrorV1::TooLarge {
                max: MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1,
            });
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn text(&mut self, field: &'static str, value: &str) -> Result<(), FormalMemoryReceiptErrorV1> {
        if value.len() > MAX_TEXT_BYTES_V1 {
            return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field,
                actual: value.len(),
                max: MAX_TEXT_BYTES_V1,
            });
        }
        self.u32(
            u32::try_from(value.len())
                .map_err(|_| FormalMemoryReceiptErrorV1::Overflow { field })?,
        )?;
        self.bytes(value.as_bytes())
    }

    fn count(
        &mut self,
        field: &'static str,
        count: usize,
    ) -> Result<(), FormalMemoryReceiptErrorV1> {
        if count > MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1 {
            return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field,
                actual: count,
                max: MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1,
            });
        }
        self.aggregate_records = self.aggregate_records.checked_add(count).ok_or(
            FormalMemoryReceiptErrorV1::Overflow {
                field: "aggregate formal-memory record count",
            },
        )?;
        if self.aggregate_records > MAX_FORMAL_MEMORY_RECORDS_V1 {
            return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field: "aggregate formal-memory record count",
                actual: self.aggregate_records,
                max: MAX_FORMAL_MEMORY_RECORDS_V1,
            });
        }
        self.u32(u32::try_from(count).map_err(|_| FormalMemoryReceiptErrorV1::Overflow { field })?)
    }

    fn u8(&mut self, value: u8) -> Result<(), FormalMemoryReceiptErrorV1> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), FormalMemoryReceiptErrorV1> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), FormalMemoryReceiptErrorV1> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), FormalMemoryReceiptErrorV1> {
        self.bytes(&value.to_le_bytes())
    }
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
    aggregate_records: usize,
}

impl<'bytes> Reader<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            aggregate_records: 0,
        }
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], FormalMemoryReceiptErrorV1> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(FormalMemoryReceiptErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(FormalMemoryReceiptErrorV1::Truncated)?;
        self.offset = end;
        Ok(value.try_into().expect("fixed slice length was checked"))
    }

    fn text(&mut self, field: &'static str) -> Result<&'bytes str, FormalMemoryReceiptErrorV1> {
        let length = self.u32()? as usize;
        if length > MAX_TEXT_BYTES_V1 {
            return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field,
                actual: length,
                max: MAX_TEXT_BYTES_V1,
            });
        }
        let end = self
            .offset
            .checked_add(length)
            .ok_or(FormalMemoryReceiptErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(FormalMemoryReceiptErrorV1::Truncated)?;
        self.offset = end;
        str::from_utf8(value).map_err(|_| FormalMemoryReceiptErrorV1::InvalidUtf8 { field })
    }

    fn count(&mut self, field: &'static str) -> Result<usize, FormalMemoryReceiptErrorV1> {
        let count = self.u32()? as usize;
        if count > MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1 {
            return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field,
                actual: count,
                max: MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1,
            });
        }
        self.aggregate_records = self.aggregate_records.checked_add(count).ok_or(
            FormalMemoryReceiptErrorV1::Overflow {
                field: "aggregate formal-memory record count",
            },
        )?;
        if self.aggregate_records > MAX_FORMAL_MEMORY_RECORDS_V1 {
            return Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field: "aggregate formal-memory record count",
                actual: self.aggregate_records,
                max: MAX_FORMAL_MEMORY_RECORDS_V1,
            });
        }
        Ok(count)
    }

    fn require_fixed_records(
        &self,
        count: usize,
        record_bytes: usize,
        field: &'static str,
    ) -> Result<(), FormalMemoryReceiptErrorV1> {
        let required = count
            .checked_mul(record_bytes)
            .ok_or(FormalMemoryReceiptErrorV1::Overflow { field })?;
        let remaining = self.bytes.len().saturating_sub(self.offset);
        if required > remaining {
            return Err(FormalMemoryReceiptErrorV1::Truncated);
        }
        Ok(())
    }

    fn reserved_u8(&mut self, field: &'static str) -> Result<(), FormalMemoryReceiptErrorV1> {
        if self.u8()? != 0 {
            return Err(FormalMemoryReceiptErrorV1::ReservedNonZero { field });
        }
        Ok(())
    }

    fn reserved_u16(&mut self, field: &'static str) -> Result<(), FormalMemoryReceiptErrorV1> {
        if self.u16()? != 0 {
            return Err(FormalMemoryReceiptErrorV1::ReservedNonZero { field });
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, FormalMemoryReceiptErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, FormalMemoryReceiptErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, FormalMemoryReceiptErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, FormalMemoryReceiptErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    const fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockId, FunctionId, KernelId, ValueId};

    fn location(block: u32, operation_index: usize) -> FunctionOperationLocation {
        FunctionOperationLocation::new(BlockId(block), operation_index)
    }

    fn range(start: u64, end_exclusive: u64) -> InvocationRange1d {
        InvocationRange1d::new(start, end_exclusive).unwrap()
    }

    fn allocation(parameter_index: u32, kind: FormalParameterKind) -> FormalAllocationParameter {
        FormalAllocationParameter {
            identity: super::super::FormalAllocationIdentity { parameter_index },
            value: ValueId(parameter_index + 10),
            kind,
            address_space: AddressSpace::Global,
            access: AccessMode::ReadWrite,
        }
    }

    fn fixture() -> FormalMemoryObligations {
        FormalMemoryObligations {
            kernel: KernelId::new("kernel"),
            entry: FunctionId::new("entry"),
            index_width: FormalIndexWidth::Bits64,
            invocations: Some(range(2, 19)),
            allocations: vec![
                allocation(1, FormalParameterKind::Slice),
                allocation(0, FormalParameterKind::Pointer),
            ],
            accesses: vec![
                FormalMemoryAccess {
                    location: location(2, 8),
                    allocation: super::super::FormalAllocationIdentity { parameter_index: 1 },
                    kind: FormalMemoryAccessKind::Write,
                    address_space: AddressSpace::Global,
                    byte_offset: ByteExpression::Unbounded,
                    byte_width: 8,
                    alignment: 8,
                    invocations: range(2, 19),
                },
                FormalMemoryAccess {
                    location: location(1, 4),
                    allocation: super::super::FormalAllocationIdentity { parameter_index: 0 },
                    kind: FormalMemoryAccessKind::Read,
                    address_space: AddressSpace::Global,
                    byte_offset: ByteExpression::invocation_affine(12, 4),
                    byte_width: 4,
                    alignment: 4,
                    invocations: range(2, 19),
                },
            ],
            bounds_requirements: vec![
                FormalBoundsRequirement {
                    location: location(2, 8),
                    allocation: super::super::FormalAllocationIdentity { parameter_index: 1 },
                    minimum_byte_len: 152,
                },
                FormalBoundsRequirement {
                    location: location(1, 4),
                    allocation: super::super::FormalAllocationIdentity { parameter_index: 0 },
                    minimum_byte_len: 88,
                },
            ],
            runtime_alias_requirements: vec![RuntimeAliasRequirement {
                left: super::super::FormalAllocationIdentity { parameter_index: 0 },
                right: super::super::FormalAllocationIdentity { parameter_index: 1 },
                left_accessed_bytes: super::super::FormalByteRange {
                    start: 20,
                    end_exclusive: 88,
                },
                right_accessed_bytes: super::super::FormalByteRange {
                    start: 2,
                    end_exclusive: 152,
                },
            }],
            inter_invocation_conflicts: vec![InterInvocationConflictRequirement {
                left: location(2, 8),
                right: location(2, 8),
                allocation: super::super::FormalAllocationIdentity { parameter_index: 1 },
            }],
        }
    }

    fn receipt(
        obligations: &FormalMemoryObligations,
    ) -> InertCanonicalFormalMemoryObligationReceiptV1 {
        InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(obligations).unwrap()
    }

    fn assert_mutation_changes(
        baseline: &InertFormalMemoryObligationReceiptIdentityV1,
        mutate: impl FnOnce(&mut FormalMemoryObligations),
    ) {
        let mut obligations = fixture();
        mutate(&mut obligations);
        let mutation = receipt(&obligations);
        mutation.revalidate().unwrap();
        assert_ne!(baseline, mutation.identity());
    }

    fn assert_rejected(
        mutate: impl FnOnce(&mut FormalMemoryObligations),
        expected: FormalMemoryReceiptErrorV1,
    ) {
        let mut obligations = fixture();
        mutate(&mut obligations);
        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(&obligations),
            Err(expected)
        );
    }

    #[test]
    fn receipt_is_deterministic_round_trips_and_rederives() {
        let first = receipt(&fixture());
        let second = receipt(&fixture());
        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
        assert_eq!(first.identity(), second.identity());
        assert_eq!(first.kernel_id(), "kernel");
        assert_eq!(first.entry_id(), "entry");
        first.revalidate().unwrap();

        let recovered = InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(
            first.canonical_bytes().to_vec(),
        )
        .unwrap();
        assert_eq!(recovered.canonical_bytes(), first.canonical_bytes());
        assert_eq!(recovered.identity(), first.identity());
        assert_eq!(recovered.kernel_id(), "kernel");
        assert_eq!(recovered.entry_id(), "entry");
    }

    #[test]
    fn collections_use_canonical_order_but_source_locations_remain_semantic() {
        let baseline = receipt(&fixture());
        let mut permuted = fixture();
        permuted.allocations.reverse();
        permuted.accesses.reverse();
        permuted.bounds_requirements.reverse();
        assert_eq!(
            receipt(&permuted).canonical_bytes(),
            baseline.canonical_bytes()
        );

        let mut source_order_changed = fixture();
        source_order_changed.accesses[0].location.operation_index += 1;
        source_order_changed.bounds_requirements[0]
            .location
            .operation_index += 1;
        source_order_changed.inter_invocation_conflicts[0]
            .left
            .operation_index += 1;
        source_order_changed.inter_invocation_conflicts[0]
            .right
            .operation_index += 1;
        assert_ne!(
            receipt(&source_order_changed).identity(),
            baseline.identity()
        );
    }

    #[test]
    fn identity_covers_every_obligation_field_family() {
        let baseline = *receipt(&fixture()).identity();
        assert_mutation_changes(&baseline, |value| value.kernel = KernelId::new("kernel-2"));
        assert_mutation_changes(&baseline, |value| value.entry = FunctionId::new("entry-2"));
        assert_mutation_changes(&baseline, |value| {
            value.index_width = FormalIndexWidth::Bits32
        });
        assert_mutation_changes(&baseline, |value| {
            let invocations = range(3, 20);
            value.invocations = Some(invocations);
            for access in &mut value.accesses {
                access.invocations = invocations;
            }
        });
        assert_mutation_changes(&baseline, |value| value.allocations[0].value.0 += 1);
        assert_mutation_changes(&baseline, |value| {
            value.allocations[0].kind = FormalParameterKind::Pointer
        });
        assert_mutation_changes(&baseline, |value| {
            value.allocations[0].address_space = AddressSpace::Workgroup;
            value.accesses[0].address_space = AddressSpace::Workgroup;
        });
        assert_mutation_changes(&baseline, |value| {
            value.allocations[1].access = AccessMode::ReadOnly
        });
        assert_mutation_changes(&baseline, |value| {
            let changed = location(2, 9);
            value.accesses[0].location = changed;
            value.bounds_requirements[0].location = changed;
            value.inter_invocation_conflicts[0].left = changed;
            value.inter_invocation_conflicts[0].right = changed;
        });
        assert_mutation_changes(&baseline, |value| {
            let allocation = super::super::FormalAllocationIdentity { parameter_index: 0 };
            value.accesses[0].allocation = allocation;
            value.bounds_requirements[0].allocation = allocation;
            value.inter_invocation_conflicts[0].allocation = allocation;
        });
        assert_mutation_changes(&baseline, |value| {
            value.accesses[1].kind = FormalMemoryAccessKind::Write
        });
        assert_mutation_changes(&baseline, |value| {
            value.accesses[0].address_space = AddressSpace::Generic;
            value.allocations[0].address_space = AddressSpace::Generic;
        });
        assert_mutation_changes(&baseline, |value| {
            value.accesses[0].byte_offset = ByteExpression::constant(9)
        });
        assert_mutation_changes(&baseline, |value| {
            value.accesses[1].byte_offset = ByteExpression::invocation_affine(13, 4)
        });
        assert_mutation_changes(&baseline, |value| {
            value.accesses[1].byte_offset = ByteExpression::invocation_affine(12, 5)
        });
        assert_mutation_changes(&baseline, |value| value.accesses[0].byte_width += 1);
        assert_mutation_changes(&baseline, |value| value.accesses[0].alignment = 16);
        assert_mutation_changes(&baseline, |value| {
            value.bounds_requirements[0].minimum_byte_len += 1
        });
        assert_mutation_changes(&baseline, |value| {
            value.runtime_alias_requirements[0]
                .left_accessed_bytes
                .start += 1
        });
        assert_mutation_changes(&baseline, |value| {
            value.runtime_alias_requirements[0]
                .left_accessed_bytes
                .end_exclusive += 1
        });
        assert_mutation_changes(&baseline, |value| {
            value.runtime_alias_requirements[0]
                .right_accessed_bytes
                .start += 1
        });
        assert_mutation_changes(&baseline, |value| {
            value.runtime_alias_requirements[0]
                .right_accessed_bytes
                .end_exclusive += 1
        });
        assert_mutation_changes(&baseline, |value| {
            value.accesses[1].kind = FormalMemoryAccessKind::Write;
            value.inter_invocation_conflicts[0] = InterInvocationConflictRequirement {
                left: location(1, 4),
                right: location(1, 4),
                allocation: super::super::FormalAllocationIdentity { parameter_index: 0 },
            };
        });
    }

    #[test]
    fn empty_sets_and_count_preflight_obey_module_scale_bounds() {
        let empty = FormalMemoryObligations {
            kernel: KernelId::new("empty"),
            entry: FunctionId::new("entry"),
            index_width: FormalIndexWidth::Unknown,
            invocations: None,
            allocations: vec![],
            accesses: vec![],
            bounds_requirements: vec![],
            runtime_alias_requirements: vec![],
            inter_invocation_conflicts: vec![],
        };
        receipt(&empty).revalidate().unwrap();

        assert_eq!(
            preflight_record_counts(ObligationRecordCountsV1 {
                allocations: MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1 + 1,
                accesses: 0,
                bounds: 0,
                aliases: 0,
                conflicts: 0,
            }),
            Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field: "allocations",
                actual: MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1 + 1,
                max: MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1,
            })
        );
        assert_eq!(
            preflight_record_counts(ObligationRecordCountsV1 {
                allocations: MAX_FORMAL_MEMORY_RECORDS_V1,
                accesses: 1,
                bounds: 0,
                aliases: 0,
                conflicts: 0,
            }),
            Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field: "aggregate formal-memory record count",
                actual: MAX_FORMAL_MEMORY_RECORDS_V1 + 1,
                max: MAX_FORMAL_MEMORY_RECORDS_V1,
            })
        );
    }

    #[test]
    fn semantic_keys_must_be_unique() {
        assert_rejected(
            |value| {
                let mut duplicate = value.allocations[0].clone();
                duplicate.value = ValueId(99);
                value.allocations.push(duplicate);
            },
            FormalMemoryReceiptErrorV1::SemanticKeyConflict {
                field: "allocation parameter index",
            },
        );
        assert_rejected(
            |value| value.allocations[0].value = value.allocations[1].value,
            FormalMemoryReceiptErrorV1::SemanticKeyConflict {
                field: "allocation value",
            },
        );
        assert_rejected(
            |value| {
                let mut duplicate = value.accesses[0].clone();
                duplicate.kind = FormalMemoryAccessKind::Read;
                value.accesses.push(duplicate);
            },
            FormalMemoryReceiptErrorV1::SemanticKeyConflict {
                field: "access location",
            },
        );
        assert_rejected(
            |value| {
                let mut duplicate = value.bounds_requirements[0];
                duplicate.minimum_byte_len += 1;
                value.bounds_requirements.push(duplicate);
            },
            FormalMemoryReceiptErrorV1::SemanticKeyConflict {
                field: "bounds location",
            },
        );
        assert_rejected(
            |value| {
                let mut duplicate = value.runtime_alias_requirements[0];
                duplicate.left_accessed_bytes.start += 1;
                value.runtime_alias_requirements.push(duplicate);
            },
            FormalMemoryReceiptErrorV1::SemanticKeyConflict {
                field: "runtime alias allocation pair",
            },
        );
        assert_rejected(
            |value| {
                let mut duplicate = value.inter_invocation_conflicts[0];
                duplicate.allocation =
                    super::super::FormalAllocationIdentity { parameter_index: 0 };
                value.inter_invocation_conflicts.push(duplicate);
            },
            FormalMemoryReceiptErrorV1::SemanticKeyConflict {
                field: "inter-invocation conflict location pair",
            },
        );
    }

    #[test]
    fn allocation_references_must_resolve() {
        assert_rejected(
            |value| value.accesses[0].allocation.parameter_index = 99,
            FormalMemoryReceiptErrorV1::DanglingReference {
                field: "access allocation",
            },
        );
        assert_rejected(
            |value| value.bounds_requirements[0].allocation.parameter_index = 99,
            FormalMemoryReceiptErrorV1::DanglingReference {
                field: "bounds allocation",
            },
        );
        assert_rejected(
            |value| value.runtime_alias_requirements[0].right.parameter_index = 99,
            FormalMemoryReceiptErrorV1::DanglingReference {
                field: "runtime alias right allocation",
            },
        );
        assert_rejected(
            |value| {
                value.inter_invocation_conflicts[0]
                    .allocation
                    .parameter_index = 99;
            },
            FormalMemoryReceiptErrorV1::DanglingReference {
                field: "inter-invocation conflict allocation",
            },
        );
    }

    #[test]
    fn accesses_must_match_allocation_address_space_and_access_mode() {
        assert_rejected(
            |value| value.accesses[0].address_space = AddressSpace::Generic,
            FormalMemoryReceiptErrorV1::InconsistentReference {
                field: "access and allocation address space",
            },
        );
        assert_rejected(
            |value| value.allocations[0].access = AccessMode::ReadOnly,
            FormalMemoryReceiptErrorV1::AccessViolation {
                field: "write through read-only allocation",
            },
        );
        assert_rejected(
            |value| value.allocations[1].access = AccessMode::WriteOnly,
            FormalMemoryReceiptErrorV1::AccessViolation {
                field: "read through write-only allocation",
            },
        );

        let valid_self_write = receipt(&fixture());
        valid_self_write.revalidate().unwrap();
    }

    #[test]
    fn write_only_allocations_use_additive_v2_and_v1_rejects_tag_three() {
        let mut obligations = fixture();
        obligations.allocations[0].access = AccessMode::WriteOnly;
        let receipt = receipt(&obligations);
        assert_eq!(
            &receipt.canonical_bytes()[8..10],
            &FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V2.to_le_bytes()
        );
        receipt.revalidate().unwrap();
        let recovered = InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(
            receipt.canonical_bytes().to_vec(),
        )
        .unwrap();
        assert_eq!(recovered.canonical_bytes(), receipt.canonical_bytes());

        let mut forged_v1 = receipt.into_canonical_bytes();
        forged_v1[8..10]
            .copy_from_slice(&FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V1.to_le_bytes());
        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(forged_v1),
            Err(FormalMemoryReceiptErrorV1::UnknownTag {
                kind: "access mode",
                tag: 3,
            })
        );
    }

    #[test]
    fn conflict_pairs_must_be_potentially_conflicting() {
        for kind in [FormalMemoryAccessKind::Read, FormalMemoryAccessKind::Atomic] {
            assert_rejected(
                |value| value.accesses[0].kind = kind,
                FormalMemoryReceiptErrorV1::InvalidConflict {
                    field: "non-conflicting access pair",
                },
            );
        }
    }

    #[test]
    fn location_references_must_resolve_and_agree_with_allocations() {
        assert_rejected(
            |value| value.bounds_requirements[0].location = location(99, 1),
            FormalMemoryReceiptErrorV1::DanglingReference {
                field: "bounds access location",
            },
        );
        assert_rejected(
            |value| {
                value.inter_invocation_conflicts[0].left = location(99, 1);
                value.inter_invocation_conflicts[0].right = location(99, 1);
            },
            FormalMemoryReceiptErrorV1::DanglingReference {
                field: "inter-invocation conflict left access location",
            },
        );
        assert_rejected(
            |value| {
                value.bounds_requirements[0].allocation =
                    super::super::FormalAllocationIdentity { parameter_index: 0 };
            },
            FormalMemoryReceiptErrorV1::InconsistentReference {
                field: "bounds allocation and access location",
            },
        );
        assert_rejected(
            |value| {
                value.inter_invocation_conflicts[0] = InterInvocationConflictRequirement {
                    left: location(1, 4),
                    right: location(2, 8),
                    allocation: super::super::FormalAllocationIdentity { parameter_index: 0 },
                };
            },
            FormalMemoryReceiptErrorV1::InconsistentReference {
                field: "inter-invocation conflict allocation and access locations",
            },
        );
    }

    #[test]
    fn widths_alignments_and_half_open_ranges_must_be_valid() {
        assert_rejected(
            |value| value.accesses[0].byte_width = 0,
            FormalMemoryReceiptErrorV1::InvalidValue {
                field: "access byte width",
            },
        );
        for alignment in [0, 3] {
            assert_rejected(
                |value| value.accesses[0].alignment = alignment,
                FormalMemoryReceiptErrorV1::InvalidValue {
                    field: "access alignment",
                },
            );
        }
        assert_rejected(
            |value| value.bounds_requirements[0].minimum_byte_len = 0,
            FormalMemoryReceiptErrorV1::InvalidRange {
                field: "bounds minimum byte length",
            },
        );
        assert_rejected(
            |value| {
                value.runtime_alias_requirements[0]
                    .left_accessed_bytes
                    .start = value.runtime_alias_requirements[0]
                    .left_accessed_bytes
                    .end_exclusive;
            },
            FormalMemoryReceiptErrorV1::InvalidRange {
                field: "runtime alias accessed bytes",
            },
        );
        assert_rejected(
            |value| {
                value.runtime_alias_requirements[0]
                    .right_accessed_bytes
                    .start = value.runtime_alias_requirements[0]
                    .right_accessed_bytes
                    .end_exclusive
                    + 1;
            },
            FormalMemoryReceiptErrorV1::InvalidRange {
                field: "runtime alias accessed bytes",
            },
        );
    }

    #[test]
    fn aliases_must_use_distinct_canonical_allocation_order() {
        assert_rejected(
            |value| {
                value.runtime_alias_requirements[0].right =
                    value.runtime_alias_requirements[0].left;
            },
            FormalMemoryReceiptErrorV1::InvalidRange {
                field: "runtime alias allocation ordering",
            },
        );
        assert_rejected(
            |value| {
                value.runtime_alias_requirements[0].left =
                    super::super::FormalAllocationIdentity { parameter_index: 1 };
                value.runtime_alias_requirements[0].right =
                    super::super::FormalAllocationIdentity { parameter_index: 0 };
            },
            FormalMemoryReceiptErrorV1::InvalidRange {
                field: "runtime alias allocation ordering",
            },
        );
    }

    #[test]
    fn every_access_must_match_the_receipt_invocation_range() {
        assert_rejected(
            |value| value.invocations = None,
            FormalMemoryReceiptErrorV1::InvocationInconsistency,
        );
        assert_rejected(
            |value| value.accesses[0].invocations = range(3, 19),
            FormalMemoryReceiptErrorV1::InvocationInconsistency,
        );
    }

    #[test]
    fn decoder_auxiliary_allocation_has_an_enforced_conservative_bound() {
        let largest_compact_record = size_of::<ConflictRecord>()
            .max(size_of::<AccessMetadataV1>())
            .max(size_of::<AllocationMetadataV1>() + size_of::<u32>());
        assert!(
            MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1
                >= 2 * MAX_FORMAL_MEMORY_RECORDS_V1 * largest_compact_record
        );

        let mut budget = DecoderAllocationBudgetV1 {
            charged_bytes: MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1,
        };
        assert_eq!(
            budget.vector::<u8>(1, "test vector"),
            Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field: "decoder auxiliary allocation",
                actual: MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1 + 1,
                max: MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1,
            })
        );

        let mut overflow_budget = DecoderAllocationBudgetV1::new();
        assert_eq!(
            overflow_budget.vector::<u64>(usize::MAX, "overflow vector"),
            Err(FormalMemoryReceiptErrorV1::Overflow {
                field: "decoder auxiliary allocation",
            })
        );
    }

    fn preamble_end(bytes: &[u8]) -> usize {
        let kernel_len = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
        let entry_len_offset = 24 + kernel_len;
        let entry_len = u32::from_le_bytes(
            bytes[entry_len_offset..entry_len_offset + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        entry_len_offset + 4 + entry_len
    }

    fn allocation_count_offset(bytes: &[u8]) -> usize {
        preamble_end(bytes) + 4 + 17
    }

    #[test]
    fn decoder_rejects_empty_invocation_ranges_and_oversized_counts() {
        let canonical = receipt(&fixture());
        let canonical_bytes = canonical.canonical_bytes();

        let mut top_level_empty = canonical_bytes.to_vec();
        let top_level_start = preamble_end(canonical_bytes) + 5;
        let start = top_level_empty[top_level_start..top_level_start + 8].to_vec();
        top_level_empty[top_level_start + 8..top_level_start + 16].copy_from_slice(&start);
        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(top_level_empty),
            Err(FormalMemoryReceiptErrorV1::InvalidRange {
                field: "invocation range",
            })
        );

        let mut access_empty = canonical_bytes.to_vec();
        let allocation_count_offset = allocation_count_offset(canonical_bytes);
        let allocation_count = u32::from_le_bytes(
            canonical_bytes[allocation_count_offset..allocation_count_offset + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let first_access = allocation_count_offset + 4 + allocation_count * 12 + 4;
        let access_invocation_start = first_access + 54;
        let start = access_empty[access_invocation_start..access_invocation_start + 8].to_vec();
        access_empty[access_invocation_start + 8..access_invocation_start + 16]
            .copy_from_slice(&start);
        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(access_empty),
            Err(FormalMemoryReceiptErrorV1::InvalidRange {
                field: "invocation range",
            })
        );

        let mut impossible_count = canonical_bytes.to_vec();
        impossible_count[allocation_count_offset..allocation_count_offset + 4]
            .copy_from_slice(&1_000_u32.to_le_bytes());
        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(impossible_count),
            Err(FormalMemoryReceiptErrorV1::Truncated)
        );

        let mut excessive_count = canonical_bytes.to_vec();
        excessive_count[allocation_count_offset..allocation_count_offset + 4].copy_from_slice(
            &u32::try_from(MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1 + 1)
                .unwrap()
                .to_le_bytes(),
        );
        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(excessive_count),
            Err(FormalMemoryReceiptErrorV1::LimitExceeded {
                field: "allocations",
                actual: MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1 + 1,
                max: MAX_FORMAL_MEMORY_RECORDS_PER_KIND_V1,
            })
        );
    }

    #[test]
    fn decoder_rejects_truncation_trailing_versions_policies_flags_and_tags() {
        let canonical = receipt(&fixture());
        let bytes = canonical.canonical_bytes();
        for end in 0..bytes.len() {
            assert!(
                InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(
                    bytes[..end].to_vec()
                )
                .is_err(),
                "prefix {end}"
            );
        }

        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(trailing),
            Err(FormalMemoryReceiptErrorV1::TrailingBytes)
        );

        for (offset, mutation, expected) in [
            (
                8,
                2_u16,
                FormalMemoryReceiptErrorV1::NonCanonicalVersion { version: 2 },
            ),
            (10, 2_u16, FormalMemoryReceiptErrorV1::UnknownPolicy(2)),
            (12, 1_u16, FormalMemoryReceiptErrorV1::UnsupportedFlags(1)),
        ] {
            let mut mutated = bytes.to_vec();
            mutated[offset..offset + 2].copy_from_slice(&mutation.to_le_bytes());
            assert_eq!(
                InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(mutated),
                Err(expected)
            );
        }

        let mut unknown_tag = bytes.to_vec();
        unknown_tag[preamble_end(bytes)] = 0;
        assert!(matches!(
            InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(unknown_tag),
            Err(FormalMemoryReceiptErrorV1::UnknownTag {
                kind: "formal index width",
                tag: 0
            })
        ));
    }

    #[test]
    fn decoder_rejects_noncanonical_order_and_oversized_inputs() {
        let canonical = receipt(&fixture());
        let mut bytes = canonical.canonical_bytes().to_vec();
        let allocation_count_offset = allocation_count_offset(&bytes);
        let allocations_offset = allocation_count_offset + 4;
        let first = bytes[allocations_offset..allocations_offset + 12].to_vec();
        let second = bytes[allocations_offset + 12..allocations_offset + 24].to_vec();
        bytes[allocations_offset..allocations_offset + 12].copy_from_slice(&second);
        bytes[allocations_offset + 12..allocations_offset + 24].copy_from_slice(&first);
        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(bytes),
            Err(FormalMemoryReceiptErrorV1::NonCanonicalOrder {
                field: "allocations"
            })
        );

        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(vec![
                0;
                MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1
                    + 1
            ]),
            Err(FormalMemoryReceiptErrorV1::TooLarge {
                max: MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1
            })
        );
    }

    #[test]
    fn valid_field_substitution_changes_identity_and_identity_domains_are_separate() {
        let baseline = receipt(&fixture());
        let mut bytes = baseline.canonical_bytes().to_vec();
        bytes[24] ^= 1;
        let substituted =
            InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(bytes).unwrap();
        assert_ne!(baseline.identity(), substituted.identity());

        let mut digest = Sha256::new();
        digest.update((b"different-domain".len() as u32).to_le_bytes());
        digest.update(b"different-domain");
        digest.update(FORMAL_MEMORY_OBLIGATION_POLICY_V1.to_le_bytes());
        digest.update((baseline.canonical_bytes().len() as u64).to_le_bytes());
        digest.update(baseline.canonical_bytes());
        let other_domain: [u8; 32] = digest.finalize().into();
        assert_ne!(baseline.identity().digest(), &other_domain);
    }
}
