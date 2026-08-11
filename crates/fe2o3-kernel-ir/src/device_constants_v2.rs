//! Inert V2 model for device constants, statics, and relocations.
//!
//! This module is intentionally not exported by `fe2o3-kernel-ir`. It defines a
//! bounded, canonical interchange model that can later be connected to the
//! semantic type graph and lowering only after those trust boundaries exist.

use std::collections::{BTreeSet, VecDeque};

pub const DEVICE_CONSTANTS_V2_MAGIC: [u8; 4] = *b"F2C2";
pub const DEVICE_CONSTANTS_V2_VERSION: u16 = 2;

const HEADER_BYTES: u64 = 20;
const ALLOCATION_HEADER_BYTES: u64 = 74;
const VALIDITY_REGION_BYTES: u64 = 12;
const RELOCATION_BYTES: u64 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphLimits {
    pub max_allocations: u32,
    pub max_relocations: u32,
    pub max_validity_regions: u32,
    pub max_total_allocation_bytes: u64,
    pub max_encoded_bytes: u64,
    pub max_relocation_depth: u32,
    pub max_alignment: u32,
}

impl Default for GraphLimits {
    fn default() -> Self {
        Self {
            max_allocations: 4_096,
            max_relocations: 65_536,
            max_validity_regions: 65_536,
            max_total_allocation_bytes: 64 * 1024 * 1024,
            max_encoded_bytes: 96 * 1024 * 1024,
            max_relocation_depth: 4_096,
            max_alignment: 65_536,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AllocationId(pub u32);

/// Opaque semantic identity for later type-graph resolution.
///
/// The domain and digest are commitments, not proof that a type exists or that
/// its layout is valid. This lane never accepts serialized type definitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticTypeId {
    pub schema_version: u16,
    pub domain: [u8; 16],
    pub digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AllocationKind {
    Constant = 0,
    Static = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Mutability {
    ReadOnly = 0,
    Mutable = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AddressSpace {
    Constant = 0,
    Global = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ValidityClass {
    Bytes = 0,
    PaddingZero = 1,
    Bool = 2,
    NonZero = 3,
    Pointer = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValidityRegion {
    pub offset: u32,
    pub len: u32,
    pub class: ValidityClass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ProvenancePolicy {
    SharedReadOnly = 0,
    Unique = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum CapabilityPolicy {
    ReadOnly = 0,
    ReadWrite = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Relocation {
    pub source_offset: u32,
    pub width: u8,
    pub target: AllocationId,
    pub addend: i64,
    pub provenance: ProvenancePolicy,
    pub capability: CapabilityPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allocation {
    pub id: AllocationId,
    pub semantic_type: SemanticTypeId,
    pub kind: AllocationKind,
    pub alignment: u32,
    pub mutability: Mutability,
    pub address_space: AddressSpace,
    pub bytes: Vec<u8>,
    pub validity: Vec<ValidityRegion>,
    pub relocations: Vec<Relocation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceConstantGraphV2 {
    pub allocations: Vec<Allocation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    Allocations,
    Relocations,
    ValidityRegions,
    AllocationBytes,
    EncodedBytes,
    RelocationDepth,
    Alignment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationError {
    ResourceLimit {
        resource: Resource,
        observed: u64,
        limit: u64,
    },
    ArithmeticOverflow,
    NonCanonicalAllocationId {
        position: u32,
        actual: AllocationId,
    },
    InvalidSemanticTypeId(AllocationId),
    InvalidAllocationPolicy(AllocationId),
    InvalidAlignment(AllocationId),
    NonCanonicalValidityOrder(AllocationId),
    ValidityCoverageGap(AllocationId),
    ValidityCoverageOverlap(AllocationId),
    InvalidValidityWidth(AllocationId),
    InvalidPadding(AllocationId),
    InvalidBitPattern(AllocationId),
    NonCanonicalRelocationOrder(AllocationId),
    RelocationOverlap(AllocationId),
    RelocationOutOfBounds(AllocationId),
    InvalidRelocationWidth(AllocationId),
    UnalignedRelocation(AllocationId),
    UnknownRelocationTarget {
        source: AllocationId,
        target: AllocationId,
    },
    TargetAddendOutOfBounds {
        source: AllocationId,
        target: AllocationId,
    },
    RelocationWithoutPointerRegion(AllocationId),
    PointerRegionWithoutRelocation(AllocationId),
    IntegerForgedPointer(AllocationId),
    CapabilityMismatch {
        source: AllocationId,
        target: AllocationId,
    },
    AmbiguousMutableOrGlobalAlias(AllocationId),
    UnsupportedRelocationCycle,
    EncodingSizeOverflow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    ResourceLimit {
        resource: Resource,
        observed: u64,
        limit: u64,
    },
    Truncated,
    BadMagic,
    UnsupportedVersion(u16),
    NonZeroReserved,
    LengthMismatch,
    UnknownTag,
    LengthOverflow,
    Graph(ValidationError),
    NonCanonicalEncoding,
}

impl From<ValidationError> for DecodeError {
    fn from(value: ValidationError) -> Self {
        Self::Graph(value)
    }
}

impl DeviceConstantGraphV2 {
    pub fn validate(&self, limits: &GraphLimits) -> Result<(), ValidationError> {
        check_limit(
            Resource::Allocations,
            usize_to_u64(self.allocations.len())?,
            u64::from(limits.max_allocations),
        )?;

        let mut total_bytes = 0_u64;
        let mut total_regions = 0_u64;
        let mut total_relocations = 0_u64;

        for (position, allocation) in self.allocations.iter().enumerate() {
            let expected =
                u32::try_from(position).map_err(|_| ValidationError::ArithmeticOverflow)?;
            if allocation.id != AllocationId(expected) {
                return Err(ValidationError::NonCanonicalAllocationId {
                    position: expected,
                    actual: allocation.id,
                });
            }
            total_bytes = total_bytes
                .checked_add(usize_to_u64(allocation.bytes.len())?)
                .ok_or(ValidationError::ArithmeticOverflow)?;
            total_regions = total_regions
                .checked_add(usize_to_u64(allocation.validity.len())?)
                .ok_or(ValidationError::ArithmeticOverflow)?;
            total_relocations = total_relocations
                .checked_add(usize_to_u64(allocation.relocations.len())?)
                .ok_or(ValidationError::ArithmeticOverflow)?;
        }

        check_limit(
            Resource::AllocationBytes,
            total_bytes,
            limits.max_total_allocation_bytes,
        )?;
        check_limit(
            Resource::ValidityRegions,
            total_regions,
            u64::from(limits.max_validity_regions),
        )?;
        check_limit(
            Resource::Relocations,
            total_relocations,
            u64::from(limits.max_relocations),
        )?;

        let encoded_len = encoded_len(self)?;
        check_limit(
            Resource::EncodedBytes,
            encoded_len,
            limits.max_encoded_bytes,
        )?;

        let mut incoming_unique = vec![0_u32; self.allocations.len()];
        let mut indegree = vec![0_u32; self.allocations.len()];
        let mut edges = vec![Vec::<usize>::new(); self.allocations.len()];

        for allocation in &self.allocations {
            validate_allocation_shape(allocation, limits)?;
            validate_validity(allocation)?;
            validate_relocation_order(allocation)?;

            let mut pointer_regions = allocation
                .validity
                .iter()
                .filter(|region| region.class == ValidityClass::Pointer)
                .map(|region| {
                    let width = u8::try_from(region.len)
                        .map_err(|_| ValidationError::InvalidValidityWidth(allocation.id))?;
                    Ok((region.offset, width))
                })
                .collect::<Result<BTreeSet<_>, ValidationError>>()?;

            for relocation in &allocation.relocations {
                validate_relocation_source(allocation, relocation, &mut pointer_regions)?;
                let target_index = usize::try_from(relocation.target.0)
                    .map_err(|_| ValidationError::ArithmeticOverflow)?;
                let target = self.allocations.get(target_index).ok_or(
                    ValidationError::UnknownRelocationTarget {
                        source: allocation.id,
                        target: relocation.target,
                    },
                )?;
                if target.id != relocation.target {
                    return Err(ValidationError::UnknownRelocationTarget {
                        source: allocation.id,
                        target: relocation.target,
                    });
                }
                validate_target_addend(allocation.id, relocation, target)?;
                validate_capability(allocation.id, relocation, target)?;

                let target_requires_unique = target.mutability == Mutability::Mutable
                    || target.address_space == AddressSpace::Global;
                if target_requires_unique {
                    incoming_unique[target_index] = incoming_unique[target_index]
                        .checked_add(1)
                        .ok_or(ValidationError::ArithmeticOverflow)?;
                    if incoming_unique[target_index] > 1 {
                        return Err(ValidationError::AmbiguousMutableOrGlobalAlias(target.id));
                    }
                }

                indegree[target_index] = indegree[target_index]
                    .checked_add(1)
                    .ok_or(ValidationError::ArithmeticOverflow)?;
                edges[usize::try_from(allocation.id.0)
                    .map_err(|_| ValidationError::ArithmeticOverflow)?]
                .push(target_index);
            }

            if !pointer_regions.is_empty() {
                return Err(ValidationError::PointerRegionWithoutRelocation(
                    allocation.id,
                ));
            }
        }

        validate_acyclic_depth(&edges, &indegree, limits.max_relocation_depth)
    }

    pub fn encode_canonical(&self, limits: &GraphLimits) -> Result<Vec<u8>, ValidationError> {
        self.validate(limits)?;
        let encoded_size = encoded_len(self)?;
        let capacity =
            usize::try_from(encoded_size).map_err(|_| ValidationError::EncodingSizeOverflow)?;
        let body_len = encoded_size
            .checked_sub(HEADER_BYTES)
            .ok_or(ValidationError::EncodingSizeOverflow)?;
        let allocation_count = u32::try_from(self.allocations.len())
            .map_err(|_| ValidationError::EncodingSizeOverflow)?;

        let mut output = Vec::with_capacity(capacity);
        output.extend_from_slice(&DEVICE_CONSTANTS_V2_MAGIC);
        put_u16(&mut output, DEVICE_CONSTANTS_V2_VERSION);
        put_u16(&mut output, 0);
        put_u32(&mut output, allocation_count);
        put_u64(&mut output, body_len);

        for allocation in &self.allocations {
            put_u32(&mut output, allocation.id.0);
            put_u16(&mut output, allocation.semantic_type.schema_version);
            output.push(allocation.kind as u8);
            output.push(allocation.mutability as u8);
            output.push(allocation.address_space as u8);
            output.push(0);
            put_u32(&mut output, allocation.alignment);
            put_u32(
                &mut output,
                u32::try_from(allocation.bytes.len())
                    .map_err(|_| ValidationError::EncodingSizeOverflow)?,
            );
            put_u32(
                &mut output,
                u32::try_from(allocation.validity.len())
                    .map_err(|_| ValidationError::EncodingSizeOverflow)?,
            );
            put_u32(
                &mut output,
                u32::try_from(allocation.relocations.len())
                    .map_err(|_| ValidationError::EncodingSizeOverflow)?,
            );
            output.extend_from_slice(&allocation.semantic_type.domain);
            output.extend_from_slice(&allocation.semantic_type.digest);
            output.extend_from_slice(&allocation.bytes);

            for region in &allocation.validity {
                put_u32(&mut output, region.offset);
                put_u32(&mut output, region.len);
                output.push(region.class as u8);
                output.extend_from_slice(&[0; 3]);
            }
            for relocation in &allocation.relocations {
                put_u32(&mut output, relocation.source_offset);
                output.push(relocation.width);
                output.push(relocation.provenance as u8);
                output.push(relocation.capability as u8);
                output.push(0);
                put_u32(&mut output, relocation.target.0);
                put_i64(&mut output, relocation.addend);
            }
        }

        debug_assert_eq!(output.len(), capacity);
        Ok(output)
    }

    pub fn decode_canonical(input: &[u8], limits: &GraphLimits) -> Result<Self, DecodeError> {
        check_decode_limit(
            Resource::EncodedBytes,
            usize_to_u64_decode(input.len())?,
            limits.max_encoded_bytes,
        )?;
        let mut cursor = Cursor::new(input);
        if cursor.read_array::<4>()? != DEVICE_CONSTANTS_V2_MAGIC {
            return Err(DecodeError::BadMagic);
        }
        let version = cursor.read_u16()?;
        if version != DEVICE_CONSTANTS_V2_VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }
        if cursor.read_u16()? != 0 {
            return Err(DecodeError::NonZeroReserved);
        }
        let allocation_count = cursor.read_u32()?;
        check_decode_limit(
            Resource::Allocations,
            u64::from(allocation_count),
            u64::from(limits.max_allocations),
        )?;
        let body_len = cursor.read_u64()?;
        let actual_body_len = input
            .len()
            .checked_sub(usize::try_from(HEADER_BYTES).map_err(|_| DecodeError::LengthOverflow)?)
            .ok_or(DecodeError::Truncated)?;
        if body_len != usize_to_u64_decode(actual_body_len)? {
            return Err(DecodeError::LengthMismatch);
        }

        let allocation_capacity =
            usize::try_from(allocation_count).map_err(|_| DecodeError::LengthOverflow)?;
        let mut allocations = Vec::with_capacity(allocation_capacity);
        let mut total_bytes = 0_u64;
        let mut total_regions = 0_u64;
        let mut total_relocations = 0_u64;

        for _ in 0..allocation_count {
            let id = AllocationId(cursor.read_u32()?);
            let schema_version = cursor.read_u16()?;
            let kind = decode_kind(cursor.read_u8()?)?;
            let mutability = decode_mutability(cursor.read_u8()?)?;
            let address_space = decode_address_space(cursor.read_u8()?)?;
            if cursor.read_u8()? != 0 {
                return Err(DecodeError::NonZeroReserved);
            }
            let alignment = cursor.read_u32()?;
            let byte_count = cursor.read_u32()?;
            let region_count = cursor.read_u32()?;
            let relocation_count = cursor.read_u32()?;
            let domain = cursor.read_array::<16>()?;
            let digest = cursor.read_array::<32>()?;

            total_bytes = checked_decode_total(
                Resource::AllocationBytes,
                total_bytes,
                u64::from(byte_count),
                limits.max_total_allocation_bytes,
            )?;
            total_regions = checked_decode_total(
                Resource::ValidityRegions,
                total_regions,
                u64::from(region_count),
                u64::from(limits.max_validity_regions),
            )?;
            total_relocations = checked_decode_total(
                Resource::Relocations,
                total_relocations,
                u64::from(relocation_count),
                u64::from(limits.max_relocations),
            )?;

            let bytes = cursor
                .take(usize::try_from(byte_count).map_err(|_| DecodeError::LengthOverflow)?)?
                .to_vec();
            let mut validity = Vec::with_capacity(
                usize::try_from(region_count).map_err(|_| DecodeError::LengthOverflow)?,
            );
            for _ in 0..region_count {
                let offset = cursor.read_u32()?;
                let len = cursor.read_u32()?;
                let class = decode_validity(cursor.read_u8()?)?;
                if cursor.read_array::<3>()? != [0; 3] {
                    return Err(DecodeError::NonZeroReserved);
                }
                validity.push(ValidityRegion { offset, len, class });
            }

            let mut relocations = Vec::with_capacity(
                usize::try_from(relocation_count).map_err(|_| DecodeError::LengthOverflow)?,
            );
            for _ in 0..relocation_count {
                let source_offset = cursor.read_u32()?;
                let width = cursor.read_u8()?;
                let provenance = decode_provenance(cursor.read_u8()?)?;
                let capability = decode_capability(cursor.read_u8()?)?;
                if cursor.read_u8()? != 0 {
                    return Err(DecodeError::NonZeroReserved);
                }
                let target = AllocationId(cursor.read_u32()?);
                let addend = cursor.read_i64()?;
                relocations.push(Relocation {
                    source_offset,
                    width,
                    target,
                    addend,
                    provenance,
                    capability,
                });
            }

            allocations.push(Allocation {
                id,
                semantic_type: SemanticTypeId {
                    schema_version,
                    domain,
                    digest,
                },
                kind,
                alignment,
                mutability,
                address_space,
                bytes,
                validity,
                relocations,
            });
        }

        if !cursor.is_finished() {
            return Err(DecodeError::LengthMismatch);
        }
        let graph = Self { allocations };
        graph.validate(limits)?;
        let canonical = graph.encode_canonical(limits)?;
        if canonical != input {
            return Err(DecodeError::NonCanonicalEncoding);
        }
        Ok(graph)
    }
}

fn validate_allocation_shape(
    allocation: &Allocation,
    limits: &GraphLimits,
) -> Result<(), ValidationError> {
    let semantic = allocation.semantic_type;
    if semantic.schema_version == 0
        || semantic.domain.iter().all(|byte| *byte == 0)
        || semantic.digest.iter().all(|byte| *byte == 0)
    {
        return Err(ValidationError::InvalidSemanticTypeId(allocation.id));
    }

    if allocation.alignment == 0
        || !allocation.alignment.is_power_of_two()
        || allocation.alignment > limits.max_alignment
    {
        if allocation.alignment > limits.max_alignment {
            return Err(ValidationError::ResourceLimit {
                resource: Resource::Alignment,
                observed: u64::from(allocation.alignment),
                limit: u64::from(limits.max_alignment),
            });
        }
        return Err(ValidationError::InvalidAlignment(allocation.id));
    }

    let policy_is_valid = match (
        allocation.kind,
        allocation.mutability,
        allocation.address_space,
    ) {
        (AllocationKind::Constant, Mutability::ReadOnly, AddressSpace::Constant)
        | (AllocationKind::Static, Mutability::ReadOnly, AddressSpace::Constant)
        | (AllocationKind::Static, Mutability::ReadOnly, AddressSpace::Global)
        | (AllocationKind::Static, Mutability::Mutable, AddressSpace::Global) => true,
        (AllocationKind::Constant, _, _)
        | (AllocationKind::Static, Mutability::Mutable, AddressSpace::Constant) => false,
    };
    if !policy_is_valid {
        return Err(ValidationError::InvalidAllocationPolicy(allocation.id));
    }
    Ok(())
}

fn validate_validity(allocation: &Allocation) -> Result<(), ValidationError> {
    let byte_len =
        u32::try_from(allocation.bytes.len()).map_err(|_| ValidationError::ArithmeticOverflow)?;
    if byte_len == 0 {
        if allocation.validity.is_empty() {
            return Ok(());
        }
        return Err(ValidationError::ValidityCoverageOverlap(allocation.id));
    }
    if allocation.validity.is_empty() {
        return Err(ValidationError::ValidityCoverageGap(allocation.id));
    }

    let mut expected_offset = 0_u32;
    let mut previous: Option<ValidityRegion> = None;
    for region in &allocation.validity {
        if let Some(prior) = previous
            && prior >= *region
        {
            return Err(ValidationError::NonCanonicalValidityOrder(allocation.id));
        }
        previous = Some(*region);
        if region.len == 0 {
            return Err(ValidationError::InvalidValidityWidth(allocation.id));
        }
        if region.offset > expected_offset {
            return Err(ValidationError::ValidityCoverageGap(allocation.id));
        }
        if region.offset < expected_offset {
            return Err(ValidationError::ValidityCoverageOverlap(allocation.id));
        }
        let end = region
            .offset
            .checked_add(region.len)
            .ok_or(ValidationError::ArithmeticOverflow)?;
        if end > byte_len {
            return Err(ValidationError::ValidityCoverageOverlap(allocation.id));
        }
        let start =
            usize::try_from(region.offset).map_err(|_| ValidationError::ArithmeticOverflow)?;
        let end_index = usize::try_from(end).map_err(|_| ValidationError::ArithmeticOverflow)?;
        let bytes = &allocation.bytes[start..end_index];
        match region.class {
            ValidityClass::Bytes => {}
            ValidityClass::PaddingZero => {
                if bytes.iter().any(|byte| *byte != 0) {
                    return Err(ValidationError::InvalidPadding(allocation.id));
                }
            }
            ValidityClass::Bool => {
                if region.len != 1 {
                    return Err(ValidationError::InvalidValidityWidth(allocation.id));
                }
                if bytes[0] > 1 {
                    return Err(ValidationError::InvalidBitPattern(allocation.id));
                }
            }
            ValidityClass::NonZero => {
                if bytes.iter().all(|byte| *byte == 0) {
                    return Err(ValidationError::InvalidBitPattern(allocation.id));
                }
            }
            ValidityClass::Pointer => {
                if !matches!(region.len, 4 | 8) {
                    return Err(ValidationError::InvalidValidityWidth(allocation.id));
                }
                if bytes.iter().any(|byte| *byte != 0) {
                    return Err(ValidationError::IntegerForgedPointer(allocation.id));
                }
            }
        }
        expected_offset = end;
    }
    if expected_offset != byte_len {
        return Err(ValidationError::ValidityCoverageGap(allocation.id));
    }
    Ok(())
}

fn validate_relocation_order(allocation: &Allocation) -> Result<(), ValidationError> {
    let mut previous: Option<Relocation> = None;
    let mut previous_end = 0_u32;
    for relocation in &allocation.relocations {
        if let Some(prior) = previous {
            if prior >= *relocation {
                return Err(ValidationError::NonCanonicalRelocationOrder(allocation.id));
            }
            if relocation.source_offset < previous_end {
                return Err(ValidationError::RelocationOverlap(allocation.id));
            }
        }
        previous_end = relocation
            .source_offset
            .checked_add(u32::from(relocation.width))
            .ok_or(ValidationError::ArithmeticOverflow)?;
        previous = Some(*relocation);
    }
    Ok(())
}

fn validate_relocation_source(
    allocation: &Allocation,
    relocation: &Relocation,
    pointer_regions: &mut BTreeSet<(u32, u8)>,
) -> Result<(), ValidationError> {
    if !matches!(relocation.width, 4 | 8) {
        return Err(ValidationError::InvalidRelocationWidth(allocation.id));
    }
    if !relocation
        .source_offset
        .is_multiple_of(u32::from(relocation.width))
    {
        return Err(ValidationError::UnalignedRelocation(allocation.id));
    }
    let end = relocation
        .source_offset
        .checked_add(u32::from(relocation.width))
        .ok_or(ValidationError::ArithmeticOverflow)?;
    let byte_len =
        u32::try_from(allocation.bytes.len()).map_err(|_| ValidationError::ArithmeticOverflow)?;
    if end > byte_len {
        return Err(ValidationError::RelocationOutOfBounds(allocation.id));
    }
    if !pointer_regions.remove(&(relocation.source_offset, relocation.width)) {
        return Err(ValidationError::RelocationWithoutPointerRegion(
            allocation.id,
        ));
    }
    let start = usize::try_from(relocation.source_offset)
        .map_err(|_| ValidationError::ArithmeticOverflow)?;
    let end = usize::try_from(end).map_err(|_| ValidationError::ArithmeticOverflow)?;
    if allocation.bytes[start..end].iter().any(|byte| *byte != 0) {
        return Err(ValidationError::IntegerForgedPointer(allocation.id));
    }
    Ok(())
}

fn validate_target_addend(
    source: AllocationId,
    relocation: &Relocation,
    target: &Allocation,
) -> Result<(), ValidationError> {
    let target_len =
        i128::try_from(target.bytes.len()).map_err(|_| ValidationError::ArithmeticOverflow)?;
    let addend = i128::from(relocation.addend);
    if addend < 0 || addend >= target_len {
        return Err(ValidationError::TargetAddendOutOfBounds {
            source,
            target: target.id,
        });
    }
    Ok(())
}

fn validate_capability(
    source: AllocationId,
    relocation: &Relocation,
    target: &Allocation,
) -> Result<(), ValidationError> {
    let expected = if target.mutability == Mutability::Mutable {
        (ProvenancePolicy::Unique, CapabilityPolicy::ReadWrite)
    } else if target.address_space == AddressSpace::Global {
        (ProvenancePolicy::Unique, CapabilityPolicy::ReadOnly)
    } else {
        (ProvenancePolicy::SharedReadOnly, CapabilityPolicy::ReadOnly)
    };
    if (relocation.provenance, relocation.capability) != expected {
        return Err(ValidationError::CapabilityMismatch {
            source,
            target: target.id,
        });
    }
    Ok(())
}

fn validate_acyclic_depth(
    edges: &[Vec<usize>],
    indegree: &[u32],
    max_depth: u32,
) -> Result<(), ValidationError> {
    let mut remaining_indegree = indegree.to_vec();
    let mut depth = vec![0_u32; edges.len()];
    let mut ready = VecDeque::new();
    for (index, degree) in remaining_indegree.iter().enumerate() {
        if *degree == 0 {
            ready.push_back(index);
        }
    }
    let mut visited = 0_usize;
    while let Some(source) = ready.pop_front() {
        visited = visited
            .checked_add(1)
            .ok_or(ValidationError::ArithmeticOverflow)?;
        for target in &edges[source] {
            let candidate_depth = depth[source]
                .checked_add(1)
                .ok_or(ValidationError::ArithmeticOverflow)?;
            if candidate_depth > max_depth {
                return Err(ValidationError::ResourceLimit {
                    resource: Resource::RelocationDepth,
                    observed: u64::from(candidate_depth),
                    limit: u64::from(max_depth),
                });
            }
            depth[*target] = depth[*target].max(candidate_depth);
            remaining_indegree[*target] = remaining_indegree[*target]
                .checked_sub(1)
                .ok_or(ValidationError::ArithmeticOverflow)?;
            if remaining_indegree[*target] == 0 {
                ready.push_back(*target);
            }
        }
    }
    if visited != edges.len() {
        return Err(ValidationError::UnsupportedRelocationCycle);
    }
    Ok(())
}

fn encoded_len(graph: &DeviceConstantGraphV2) -> Result<u64, ValidationError> {
    let mut length = HEADER_BYTES;
    for allocation in &graph.allocations {
        length = length
            .checked_add(ALLOCATION_HEADER_BYTES)
            .and_then(|value| value.checked_add(usize_to_u64(allocation.bytes.len()).ok()?))
            .and_then(|value| {
                value.checked_add(
                    usize_to_u64(allocation.validity.len())
                        .ok()?
                        .checked_mul(VALIDITY_REGION_BYTES)?,
                )
            })
            .and_then(|value| {
                value.checked_add(
                    usize_to_u64(allocation.relocations.len())
                        .ok()?
                        .checked_mul(RELOCATION_BYTES)?,
                )
            })
            .ok_or(ValidationError::EncodingSizeOverflow)?;
    }
    Ok(length)
}

fn check_limit(resource: Resource, observed: u64, limit: u64) -> Result<(), ValidationError> {
    if observed > limit {
        return Err(ValidationError::ResourceLimit {
            resource,
            observed,
            limit,
        });
    }
    Ok(())
}

fn check_decode_limit(resource: Resource, observed: u64, limit: u64) -> Result<(), DecodeError> {
    if observed > limit {
        return Err(DecodeError::ResourceLimit {
            resource,
            observed,
            limit,
        });
    }
    Ok(())
}

fn checked_decode_total(
    resource: Resource,
    current: u64,
    increment: u64,
    limit: u64,
) -> Result<u64, DecodeError> {
    let total = current
        .checked_add(increment)
        .ok_or(DecodeError::LengthOverflow)?;
    check_decode_limit(resource, total, limit)?;
    Ok(total)
}

fn usize_to_u64(value: usize) -> Result<u64, ValidationError> {
    u64::try_from(value).map_err(|_| ValidationError::ArithmeticOverflow)
}

fn usize_to_u64_decode(value: usize) -> Result<u64, DecodeError> {
    u64::try_from(value).map_err(|_| DecodeError::LengthOverflow)
}

fn put_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_i64(output: &mut Vec<u8>, value: i64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn decode_kind(tag: u8) -> Result<AllocationKind, DecodeError> {
    match tag {
        0 => Ok(AllocationKind::Constant),
        1 => Ok(AllocationKind::Static),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_mutability(tag: u8) -> Result<Mutability, DecodeError> {
    match tag {
        0 => Ok(Mutability::ReadOnly),
        1 => Ok(Mutability::Mutable),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_address_space(tag: u8) -> Result<AddressSpace, DecodeError> {
    match tag {
        0 => Ok(AddressSpace::Constant),
        1 => Ok(AddressSpace::Global),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_validity(tag: u8) -> Result<ValidityClass, DecodeError> {
    match tag {
        0 => Ok(ValidityClass::Bytes),
        1 => Ok(ValidityClass::PaddingZero),
        2 => Ok(ValidityClass::Bool),
        3 => Ok(ValidityClass::NonZero),
        4 => Ok(ValidityClass::Pointer),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_provenance(tag: u8) -> Result<ProvenancePolicy, DecodeError> {
    match tag {
        0 => Ok(ProvenancePolicy::SharedReadOnly),
        1 => Ok(ProvenancePolicy::Unique),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_capability(tag: u8) -> Result<CapabilityPolicy, DecodeError> {
    match tag {
        0 => Ok(CapabilityPolicy::ReadOnly),
        1 => Ok(CapabilityPolicy::ReadWrite),
        _ => Err(DecodeError::UnknownTag),
    }
}

struct Cursor<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .position
            .checked_add(len)
            .ok_or(DecodeError::LengthOverflow)?;
        let value = self
            .input
            .get(self.position..end)
            .ok_or(DecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn read_u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }

    fn read_u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    fn read_u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    fn read_i64(&mut self) -> Result<i64, DecodeError> {
        Ok(i64::from_le_bytes(self.read_array()?))
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        self.take(N)?.try_into().map_err(|_| DecodeError::Truncated)
    }

    fn is_finished(&self) -> bool {
        self.position == self.input.len()
    }
}
