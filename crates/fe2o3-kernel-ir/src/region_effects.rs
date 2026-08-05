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
        if self.access_width == 0 {
            return Err(RegionValidationError::ZeroAccessWidth);
        }
        if !self.alignment.is_power_of_two() {
            return Err(RegionValidationError::InvalidAlignment {
                alignment: self.alignment,
            });
        }

        self.region.validate(invocations)?;

        if let Some((constant, coefficient)) = self.region.byte_offset.affine_parts()
            && (constant % self.alignment != 0 || coefficient % self.alignment != 0)
        {
            return Err(RegionValidationError::MisalignedAccess {
                alignment: self.alignment,
            });
        }

        if let Some((byte_length, coefficient)) = self.region.byte_length.affine_parts()
            && coefficient == 0
            && byte_length < self.access_width
        {
            return Err(RegionValidationError::AccessExceedsRegion {
                access_width: self.access_width,
                byte_length,
            });
        }

        Ok(())
    }
}

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
    MisalignedAccess {
        alignment: u64,
    },
    AccessExceedsRegion {
        access_width: u64,
        byte_length: u64,
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
            Self::MisalignedAccess { alignment } => write!(
                formatter,
                "affine byte offsets are not aligned to {alignment} for every invocation"
            ),
            Self::AccessExceedsRegion {
                access_width,
                byte_length,
            } => write!(
                formatter,
                "access width {access_width} exceeds region byte length {byte_length}"
            ),
        }
    }
}

impl Error for RegionValidationError {}
