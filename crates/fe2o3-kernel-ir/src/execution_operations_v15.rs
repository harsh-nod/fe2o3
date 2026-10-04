//! Closed execution roles and operations. SSA definitions, not payload IDs, identify acquisitions.

use std::fmt;

use crate::{MAX_VALUE_ARGUMENTS_V1, ValueId};

pub const MAX_EXECUTION_LANES_V15: u16 = 256;
pub const MAX_EXECUTION_ELEMENTS_V15: u16 = 125;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionRoleV15 {
    Context,
    Workgroup,
    MaskedTileU32 { lanes: u16, elements: u16 },
    LaneFragmentU32 { lanes: u16, elements: u16 },
}

impl ExecutionRoleV15 {
    pub fn validate(self) -> Result<(), ExecutionOperationErrorV15> {
        match self {
            Self::Context | Self::Workgroup => Ok(()),
            Self::MaskedTileU32 { lanes, elements } | Self::LaneFragmentU32 { lanes, elements } => {
                validate_geometry(lanes, elements)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionOperationV15 {
    ContextIssue,
    WorkgroupDerive {
        context: ValueId,
    },
    ScopeEnd {
        workgroup: ValueId,
        discarded: Vec<ValueId>,
    },
    MaskedTileLoadU32 {
        workgroup: ValueId,
        input: ValueId,
        base: ValueId,
        lanes: u16,
        elements: u16,
    },
    TileIntoFragmentU32 {
        tile: ValueId,
        lanes: u16,
        elements: u16,
    },
    FragmentIntoPartsU32 {
        fragment: ValueId,
        lanes: u16,
        elements: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionOperationErrorV15 {
    InvalidGeometry { lanes: u16, elements: u16 },
    DiscardLimitExceeded { actual: usize, max: usize },
    NonCanonicalDiscardOrder,
}

impl fmt::Display for ExecutionOperationErrorV15 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGeometry { lanes, elements } => write!(
                formatter,
                "execution geometry {lanes} x {elements} is outside 1..=256 x 1..=125"
            ),
            Self::DiscardLimitExceeded { actual, max } => {
                write!(formatter, "execution discard count {actual} exceeds {max}")
            }
            Self::NonCanonicalDiscardOrder => {
                formatter.write_str("execution discard values must be strictly increasing")
            }
        }
    }
}

impl std::error::Error for ExecutionOperationErrorV15 {}

fn validate_geometry(lanes: u16, elements: u16) -> Result<(), ExecutionOperationErrorV15> {
    if (1..=MAX_EXECUTION_LANES_V15).contains(&lanes)
        && (1..=MAX_EXECUTION_ELEMENTS_V15).contains(&elements)
    {
        Ok(())
    } else {
        Err(ExecutionOperationErrorV15::InvalidGeometry { lanes, elements })
    }
}

impl ExecutionOperationV15 {
    /// Checks the untrusted raw payload without allocating or projecting a descriptor.
    pub fn validate_payload(&self) -> Result<(), ExecutionOperationErrorV15> {
        match self {
            Self::ContextIssue | Self::WorkgroupDerive { .. } => Ok(()),
            Self::ScopeEnd { discarded, .. } => {
                let max = MAX_VALUE_ARGUMENTS_V1 - 1;
                if discarded.len() > max {
                    return Err(ExecutionOperationErrorV15::DiscardLimitExceeded {
                        actual: discarded.len(),
                        max,
                    });
                }
                if discarded.windows(2).any(|pair| pair[0] >= pair[1]) {
                    return Err(ExecutionOperationErrorV15::NonCanonicalDiscardOrder);
                }
                Ok(())
            }
            Self::MaskedTileLoadU32 {
                lanes, elements, ..
            }
            | Self::TileIntoFragmentU32 {
                lanes, elements, ..
            }
            | Self::FragmentIntoPartsU32 {
                lanes, elements, ..
            } => validate_geometry(*lanes, *elements),
        }
    }

    /// Visits the exact wire operand roster without allocating.
    pub fn try_visit_operands_v1<E>(
        &self,
        mut visitor: impl FnMut(ValueId) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::ContextIssue => {}
            Self::WorkgroupDerive { context } => visitor(*context)?,
            Self::ScopeEnd {
                workgroup,
                discarded,
            } => {
                visitor(*workgroup)?;
                for value in discarded {
                    visitor(*value)?;
                }
            }
            Self::MaskedTileLoadU32 {
                workgroup,
                input,
                base,
                ..
            } => {
                visitor(*workgroup)?;
                visitor(*input)?;
                visitor(*base)?;
            }
            Self::TileIntoFragmentU32 { tile, .. } => visitor(*tile)?,
            Self::FragmentIntoPartsU32 { fragment, .. } => visitor(*fragment)?,
        }
        Ok(())
    }
}
