use std::error::Error;
use std::fmt;

use crate::{AddressSpace, AtomicKind, MemoryOrdering, ScalarType, SynchronizationScope};

/// A frontend-assigned identity for one logical allocation.
///
/// The value is symbolic: it identifies allocation provenance within an
/// analysis input and is never interpreted as a device address.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AllocationId(u32);

impl AllocationId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Allocation provenance retained by a memory region.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AllocationIdentity {
    Known(AllocationId),
    Unknown,
}

impl From<AllocationId> for AllocationIdentity {
    fn from(value: AllocationId) -> Self {
        Self::Known(value)
    }
}

/// An expression in bytes over a one-dimensional invocation index.
///
/// Affine expressions are non-negative and have the form
/// `constant + invocation_coefficient * invocation_index`. `Unbounded`
/// explicitly records that the frontend could not retain a checked form.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ByteExpression {
    Affine {
        constant: u64,
        invocation_coefficient: u64,
    },
    Unbounded,
}

impl ByteExpression {
    pub const fn constant(value: u64) -> Self {
        Self::Affine {
            constant: value,
            invocation_coefficient: 0,
        }
    }

    pub const fn invocation_affine(constant: u64, invocation_coefficient: u64) -> Self {
        Self::Affine {
            constant,
            invocation_coefficient,
        }
    }

    pub fn checked_evaluate(self, invocation_index: u64) -> Result<u64, RegionValidationError> {
        match self {
            Self::Affine {
                constant,
                invocation_coefficient,
            } => invocation_coefficient
                .checked_mul(invocation_index)
                .and_then(|offset| constant.checked_add(offset))
                .ok_or(RegionValidationError::ExpressionOverflow {
                    expression: self,
                    invocation_index,
                }),
            Self::Unbounded => Err(RegionValidationError::UnboundedExpression),
        }
    }

    const fn affine_parts(self) -> Option<(u64, u64)> {
        match self {
            Self::Affine {
                constant,
                invocation_coefficient,
            } => Some((constant, invocation_coefficient)),
            Self::Unbounded => None,
        }
    }

    const fn is_unbounded(self) -> bool {
        matches!(self, Self::Unbounded)
    }
}

/// A non-empty half-open range of one-dimensional invocation indices.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InvocationRange1d {
    start: u64,
    end_exclusive: u64,
}

impl InvocationRange1d {
    pub fn new(start: u64, end_exclusive: u64) -> Result<Self, RegionValidationError> {
        if start >= end_exclusive {
            return Err(RegionValidationError::EmptyInvocationRange {
                start,
                end_exclusive,
            });
        }
        Ok(Self {
            start,
            end_exclusive,
        })
    }

    pub fn from_count(count: u64) -> Result<Self, RegionValidationError> {
        Self::new(0, count)
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end_exclusive(self) -> u64 {
        self.end_exclusive
    }

    pub const fn last(self) -> u64 {
        self.end_exclusive - 1
    }

    pub const fn contains(self, invocation_index: u64) -> bool {
        self.start <= invocation_index && invocation_index < self.end_exclusive
    }

    const fn is_singleton(self) -> bool {
        self.end_exclusive - self.start == 1
    }

    fn intersection(self, other: Self) -> Option<Self> {
        let start = self.start.max(other.start);
        let end_exclusive = self.end_exclusive.min(other.end_exclusive);
        (start < end_exclusive).then_some(Self {
            start,
            end_exclusive,
        })
    }
}

/// A frontend-assigned synchronization epoch.
///
/// Epoch equality is semantic; numeric ordering is kept only for stable
/// sorting and does not by itself establish a synchronization relation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SynchronizationEpoch(u32);

impl SynchronizationEpoch {
    pub const INITIAL: Self = Self(0);

    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

/// A byte region accessed by every invocation described by an analysis input.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryRegion {
    pub allocation: AllocationIdentity,
    pub address_space: AddressSpace,
    pub byte_offset: ByteExpression,
    pub byte_length: ByteExpression,
}

impl MemoryRegion {
    pub const fn new(
        allocation: AllocationIdentity,
        address_space: AddressSpace,
        byte_offset: ByteExpression,
        byte_length: ByteExpression,
    ) -> Self {
        Self {
            allocation,
            address_space,
            byte_offset,
            byte_length,
        }
    }

    pub fn validate(&self, invocations: InvocationRange1d) -> Result<(), RegionValidationError> {
        for invocation_index in [invocations.start(), invocations.last()] {
            let byte_offset = self.byte_offset.checked_evaluate(invocation_index)?;
            let byte_length = self.byte_length.checked_evaluate(invocation_index)?;
            if byte_length == 0 {
                return Err(RegionValidationError::ZeroByteLength { invocation_index });
            }
            byte_offset.checked_add(byte_length).ok_or(
                RegionValidationError::RegionEndOverflow {
                    byte_offset,
                    byte_length,
                    invocation_index,
                },
            )?;
        }
        Ok(())
    }

    const fn has_unbounded_expression(&self) -> bool {
        self.byte_offset.is_unbounded() || self.byte_length.is_unbounded()
    }
}

/// Atomic semantics required for two overlapping atomic accesses to match.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AtomicEffect {
    pub kind: AtomicKind,
    pub value_type: ScalarType,
    pub scope: SynchronizationScope,
    pub ordering: MemoryOrdering,
}

/// The executable permission exercised by an effect.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RegionEffectKind {
    Read,
    Write,
    Atomic(AtomicEffect),
}

/// A target-neutral per-invocation memory effect.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegionEffect {
    pub kind: RegionEffectKind,
    pub region: MemoryRegion,
    pub access_width: u64,
    pub alignment: u64,
    pub epoch: SynchronizationEpoch,
}

impl RegionEffect {
    pub const fn new(
        kind: RegionEffectKind,
        region: MemoryRegion,
        access_width: u64,
        alignment: u64,
        epoch: SynchronizationEpoch,
    ) -> Self {
        Self {
            kind,
            region,
            access_width,
            alignment,
            epoch,
        }
    }

    pub fn validate(&self, invocations: InvocationRange1d) -> Result<(), RegionValidationError> {
        self.validate_access_metadata()?;
        self.region.validate(invocations)?;
        for invocation_index in [invocations.start(), invocations.last()] {
            let byte_length = self.region.byte_length.checked_evaluate(invocation_index)?;
            if byte_length < self.access_width {
                return Err(RegionValidationError::AccessExceedsRegion {
                    access_width: self.access_width,
                    byte_length,
                    invocation_index,
                });
            }
        }
        Ok(())
    }

    fn validate_access_metadata(&self) -> Result<(), RegionValidationError> {
        if self.access_width == 0 {
            return Err(RegionValidationError::ZeroAccessWidth);
        }
        if !self.alignment.is_power_of_two() {
            return Err(RegionValidationError::InvalidAlignment {
                alignment: self.alignment,
            });
        }
        if let RegionEffectKind::Atomic(atomic) = self.kind {
            let Some(value_width_bits) = atomic.value_type.bit_width() else {
                return Err(RegionValidationError::AtomicValueWidthUnknown {
                    value_type: atomic.value_type,
                });
            };
            if value_width_bits % 8 != 0 || self.access_width != u64::from(value_width_bits / 8) {
                return Err(RegionValidationError::AtomicWidthMismatch {
                    value_type: atomic.value_type,
                    value_width_bits,
                    access_width: self.access_width,
                });
            }
        }

        if let Some((constant, coefficient)) = self.region.byte_offset.affine_parts()
            && (constant % self.alignment != 0 || coefficient % self.alignment != 0)
        {
            return Err(RegionValidationError::MisalignedAccess {
                alignment: self.alignment,
            });
        }

        Ok(())
    }
}

/// Which dynamic invocation pairs are considered by overlap analysis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InvocationPairing {
    /// Pair only accesses performed by the same invocation index.
    SameInvocation,
    /// Pair only accesses performed by different invocation indices.
    DistinctInvocations,
    /// Pair all indices, including the same index.
    AnyInvocations,
}

/// A conservative result for two symbolic regions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RegionOverlap {
    Disjoint,
    MayOverlap,
    Indeterminate(RegionIndeterminateReason),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RegionIndeterminateReason {
    UnknownAllocation,
    UnboundedByteExpression,
}

/// Why a pair of effects cannot race within the modeled epoch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NoConflictReason {
    DisjointRegions,
    SharedReads,
    CompatibleAtomics,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConflictReason {
    OverlappingNonAtomicWrite,
    AtomicNonAtomicOverlap,
    IncompatibleAtomicOverlap,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConflictIndeterminateReason {
    Region(RegionIndeterminateReason),
    /// Different epoch numbers do not prove that an appropriately scoped
    /// synchronization edge orders the effects.
    EpochOrderingNotEstablished {
        left: SynchronizationEpoch,
        right: SynchronizationEpoch,
    },
    /// This first same-device model does not retain workgroup or subgroup
    /// membership, so narrower atomic scopes cannot cover its whole domain.
    AtomicScopeCoverageNotEstablished {
        scope: SynchronizationScope,
    },
}

/// A race-oriented classification for a pair of per-invocation effects.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EffectConflict {
    NoConflict(NoConflictReason),
    Conflict(ConflictReason),
    Indeterminate(ConflictIndeterminateReason),
}

/// Computes whether any selected invocation pair may access overlapping bytes.
///
/// `Disjoint` is returned only for a proof handled by this bounded model.
/// Unsupported affine relationships conservatively return `MayOverlap`;
/// missing provenance or bounds return `Indeterminate`.
pub fn analyze_region_overlap(
    left: &MemoryRegion,
    left_invocations: InvocationRange1d,
    right: &MemoryRegion,
    right_invocations: InvocationRange1d,
    pairing: InvocationPairing,
) -> Result<RegionOverlap, RegionAnalysisError> {
    validate_bounded_region(left, left_invocations).map_err(RegionAnalysisError::LeftRegion)?;
    validate_bounded_region(right, right_invocations).map_err(RegionAnalysisError::RightRegion)?;

    if matches!(left.allocation, AllocationIdentity::Unknown)
        || matches!(right.allocation, AllocationIdentity::Unknown)
    {
        return Ok(RegionOverlap::Indeterminate(
            RegionIndeterminateReason::UnknownAllocation,
        ));
    }
    if left.has_unbounded_expression() || right.has_unbounded_expression() {
        return Ok(RegionOverlap::Indeterminate(
            RegionIndeterminateReason::UnboundedByteExpression,
        ));
    }

    let (AllocationIdentity::Known(left_allocation), AllocationIdentity::Known(right_allocation)) =
        (left.allocation, right.allocation)
    else {
        unreachable!("unknown allocations returned above")
    };
    if left_allocation != right_allocation {
        return Ok(RegionOverlap::Disjoint);
    }
    if left.address_space != right.address_space {
        return Err(RegionAnalysisError::AllocationAddressSpaceMismatch {
            allocation: left_allocation,
            left: left.address_space,
            right: right.address_space,
        });
    }

    let Some((left_domain, right_domain)) =
        paired_domains(left_invocations, right_invocations, pairing)
    else {
        return Ok(RegionOverlap::Disjoint);
    };

    if left_domain.is_singleton() && right_domain.is_singleton() {
        return exact_overlap(left, left_domain.start(), right, right_domain.start())
            .map_err(RegionAnalysisError::Arithmetic);
    }

    if pairing == InvocationPairing::DistinctInvocations && proves_affine_partition(left, right) {
        return Ok(RegionOverlap::Disjoint);
    }

    let left_envelope =
        region_envelope(left, left_domain).map_err(RegionAnalysisError::Arithmetic)?;
    let right_envelope =
        region_envelope(right, right_domain).map_err(RegionAnalysisError::Arithmetic)?;
    if left_envelope.is_disjoint(right_envelope) {
        Ok(RegionOverlap::Disjoint)
    } else {
        Ok(RegionOverlap::MayOverlap)
    }
}

/// Classifies a pair of effects without granting safe-launch authority.
///
/// Different epoch numbers remain indeterminate until a later analysis binds
/// them to a barrier with compatible participants, scope, and memory spaces.
pub fn analyze_effect_conflict(
    left: &RegionEffect,
    left_invocations: InvocationRange1d,
    right: &RegionEffect,
    right_invocations: InvocationRange1d,
    pairing: InvocationPairing,
) -> Result<EffectConflict, RegionAnalysisError> {
    left.validate(left_invocations)
        .map_err(RegionAnalysisError::LeftRegion)?;
    right
        .validate(right_invocations)
        .map_err(RegionAnalysisError::RightRegion)?;

    match analyze_region_overlap(
        &left.region,
        left_invocations,
        &right.region,
        right_invocations,
        pairing,
    )? {
        RegionOverlap::Disjoint => Ok(EffectConflict::NoConflict(
            NoConflictReason::DisjointRegions,
        )),
        RegionOverlap::Indeterminate(reason) => Ok(EffectConflict::Indeterminate(
            ConflictIndeterminateReason::Region(reason),
        )),
        RegionOverlap::MayOverlap => classify_overlapping_effects(left, right),
    }
}

fn classify_overlapping_effects(
    left: &RegionEffect,
    right: &RegionEffect,
) -> Result<EffectConflict, RegionAnalysisError> {
    match (left.kind, right.kind) {
        (RegionEffectKind::Read, RegionEffectKind::Read) => {
            return Ok(EffectConflict::NoConflict(NoConflictReason::SharedReads));
        }
        (RegionEffectKind::Atomic(left_atomic), RegionEffectKind::Atomic(right_atomic))
            if left_atomic == right_atomic
                && left.access_width == right.access_width
                && left.alignment == right.alignment
                && same_constant_atomic_object(&left.region, &right.region) =>
        {
            if !matches!(
                left_atomic.scope,
                SynchronizationScope::Device | SynchronizationScope::System
            ) {
                return Ok(EffectConflict::Indeterminate(
                    ConflictIndeterminateReason::AtomicScopeCoverageNotEstablished {
                        scope: left_atomic.scope,
                    },
                ));
            }
            return Ok(EffectConflict::NoConflict(
                NoConflictReason::CompatibleAtomics,
            ));
        }
        _ => {}
    }

    if left.epoch != right.epoch {
        return Ok(EffectConflict::Indeterminate(
            ConflictIndeterminateReason::EpochOrderingNotEstablished {
                left: left.epoch,
                right: right.epoch,
            },
        ));
    }

    let reason = match (left.kind, right.kind) {
        (RegionEffectKind::Atomic(_), RegionEffectKind::Atomic(_)) => {
            ConflictReason::IncompatibleAtomicOverlap
        }
        (RegionEffectKind::Atomic(_), _) | (_, RegionEffectKind::Atomic(_)) => {
            ConflictReason::AtomicNonAtomicOverlap
        }
        _ => ConflictReason::OverlappingNonAtomicWrite,
    };
    Ok(EffectConflict::Conflict(reason))
}

fn same_constant_atomic_object(left: &MemoryRegion, right: &MemoryRegion) -> bool {
    if left != right || matches!(left.allocation, AllocationIdentity::Unknown) {
        return false;
    }
    matches!(left.byte_offset.affine_parts(), Some((_, 0)))
        && matches!(left.byte_length.affine_parts(), Some((_, 0)))
}

fn validate_bounded_region(
    region: &MemoryRegion,
    invocations: InvocationRange1d,
) -> Result<(), RegionValidationError> {
    if region.has_unbounded_expression() {
        Ok(())
    } else {
        region.validate(invocations)
    }
}

fn paired_domains(
    left: InvocationRange1d,
    right: InvocationRange1d,
    pairing: InvocationPairing,
) -> Option<(InvocationRange1d, InvocationRange1d)> {
    match pairing {
        InvocationPairing::AnyInvocations => Some((left, right)),
        InvocationPairing::SameInvocation => {
            left.intersection(right).map(|domain| (domain, domain))
        }
        InvocationPairing::DistinctInvocations => {
            if left.is_singleton() && right.is_singleton() && left.start() == right.start() {
                None
            } else {
                Some((left, right))
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ByteInterval {
    start: u64,
    end_exclusive: u64,
}

impl ByteInterval {
    const fn is_disjoint(self, other: Self) -> bool {
        self.end_exclusive <= other.start || other.end_exclusive <= self.start
    }
}

fn byte_interval(
    region: &MemoryRegion,
    invocation_index: u64,
) -> Result<ByteInterval, RegionValidationError> {
    let start = region.byte_offset.checked_evaluate(invocation_index)?;
    let byte_length = region.byte_length.checked_evaluate(invocation_index)?;
    let end_exclusive =
        start
            .checked_add(byte_length)
            .ok_or(RegionValidationError::RegionEndOverflow {
                byte_offset: start,
                byte_length,
                invocation_index,
            })?;
    Ok(ByteInterval {
        start,
        end_exclusive,
    })
}

fn exact_overlap(
    left: &MemoryRegion,
    left_invocation: u64,
    right: &MemoryRegion,
    right_invocation: u64,
) -> Result<RegionOverlap, RegionValidationError> {
    let left = byte_interval(left, left_invocation)?;
    let right = byte_interval(right, right_invocation)?;
    Ok(if left.is_disjoint(right) {
        RegionOverlap::Disjoint
    } else {
        RegionOverlap::MayOverlap
    })
}

fn region_envelope(
    region: &MemoryRegion,
    invocations: InvocationRange1d,
) -> Result<ByteInterval, RegionValidationError> {
    let first = byte_interval(region, invocations.start())?;
    let last = byte_interval(region, invocations.last())?;
    Ok(ByteInterval {
        start: first.start,
        end_exclusive: last.end_exclusive,
    })
}

fn proves_affine_partition(left: &MemoryRegion, right: &MemoryRegion) -> bool {
    if left.byte_offset != right.byte_offset || left.byte_length != right.byte_length {
        return false;
    }
    let Some((_, stride)) = left.byte_offset.affine_parts() else {
        return false;
    };
    let Some((byte_length, length_coefficient)) = left.byte_length.affine_parts() else {
        return false;
    };
    length_coefficient == 0 && byte_length != 0 && stride >= byte_length
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RegionAnalysisError {
    LeftRegion(RegionValidationError),
    RightRegion(RegionValidationError),
    Arithmetic(RegionValidationError),
    AllocationAddressSpaceMismatch {
        allocation: AllocationId,
        left: AddressSpace,
        right: AddressSpace,
    },
}

impl fmt::Display for RegionAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeftRegion(error) => write!(formatter, "invalid left region: {error}"),
            Self::RightRegion(error) => write!(formatter, "invalid right region: {error}"),
            Self::Arithmetic(error) => write!(formatter, "region arithmetic failed: {error}"),
            Self::AllocationAddressSpaceMismatch {
                allocation,
                left,
                right,
            } => write!(
                formatter,
                "allocation {} has inconsistent address spaces {left:?} and {right:?}",
                allocation.value()
            ),
        }
    }
}

impl Error for RegionAnalysisError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RegionValidationError {
    EmptyInvocationRange {
        start: u64,
        end_exclusive: u64,
    },
    UnboundedExpression,
    ExpressionOverflow {
        expression: ByteExpression,
        invocation_index: u64,
    },
    ZeroByteLength {
        invocation_index: u64,
    },
    RegionEndOverflow {
        byte_offset: u64,
        byte_length: u64,
        invocation_index: u64,
    },
    ZeroAccessWidth,
    InvalidAlignment {
        alignment: u64,
    },
    AtomicValueWidthUnknown {
        value_type: ScalarType,
    },
    AtomicWidthMismatch {
        value_type: ScalarType,
        value_width_bits: u16,
        access_width: u64,
    },
    MisalignedAccess {
        alignment: u64,
    },
    AccessExceedsRegion {
        access_width: u64,
        byte_length: u64,
        invocation_index: u64,
    },
}

impl fmt::Display for RegionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInvocationRange {
                start,
                end_exclusive,
            } => write!(
                formatter,
                "invocation range [{start}, {end_exclusive}) must be non-empty"
            ),
            Self::UnboundedExpression => formatter.write_str("byte expression is unbounded"),
            Self::ExpressionOverflow {
                expression,
                invocation_index,
            } => write!(
                formatter,
                "byte expression {expression:?} overflows at invocation {invocation_index}"
            ),
            Self::ZeroByteLength { invocation_index } => {
                write!(
                    formatter,
                    "byte length is zero at invocation {invocation_index}"
                )
            }
            Self::RegionEndOverflow {
                byte_offset,
                byte_length,
                invocation_index,
            } => write!(
                formatter,
                "region end {byte_offset} + {byte_length} overflows at invocation {invocation_index}"
            ),
            Self::ZeroAccessWidth => formatter.write_str("access width must be non-zero"),
            Self::InvalidAlignment { alignment } => write!(
                formatter,
                "access alignment {alignment} must be a non-zero power of two"
            ),
            Self::AtomicValueWidthUnknown { value_type } => write!(
                formatter,
                "atomic value type {value_type:?} has no target-neutral byte width"
            ),
            Self::AtomicWidthMismatch {
                value_type,
                value_width_bits,
                access_width,
            } => write!(
                formatter,
                "atomic value type {value_type:?} is {value_width_bits} bits but access width is {access_width} bytes"
            ),
            Self::MisalignedAccess { alignment } => write!(
                formatter,
                "affine byte offsets are not aligned to {alignment} for every invocation"
            ),
            Self::AccessExceedsRegion {
                access_width,
                byte_length,
                invocation_index,
            } => write!(
                formatter,
                "access width {access_width} exceeds region byte length {byte_length} at invocation {invocation_index}"
            ),
        }
    }
}

impl Error for RegionValidationError {}
