//! Explicit scalar semantics for one masked tile load, not launch authority.

use crate::{ExecutionOperationErrorV15, ExecutionRoleV15};
use std::fmt;

/// The order is observable when a program treats fragment components differently.
/// There is intentionally no default and no target-independent choice of order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionTileLayoutV1 {
    /// Component `element` belongs to `lane * elements + element`.
    Blocked,
    /// Component `element` belongs to `element * lanes + lane`.
    Striped,
}

/// Validated geometry and an explicit layout. This value does not authenticate
/// source provenance, launch geometry, memory safety, or a native implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionTileScheduleV1 {
    layout: ExecutionTileLayoutV1,
    lanes: u16,
    elements: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionTileScheduleErrorV1 {
    Geometry(ExecutionOperationErrorV15),
    Coordinate { lane: u16, element: u16 },
}

impl fmt::Display for ExecutionTileScheduleErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Geometry(error) => error.fmt(formatter),
            Self::Coordinate { lane, element } => {
                write!(
                    formatter,
                    "tile coordinate ({lane}, {element}) is outside its geometry"
                )
            }
        }
    }
}

impl std::error::Error for ExecutionTileScheduleErrorV1 {}

impl ExecutionTileScheduleV1 {
    pub fn new(
        layout: ExecutionTileLayoutV1,
        lanes: u16,
        elements: u16,
    ) -> Result<Self, ExecutionTileScheduleErrorV1> {
        ExecutionRoleV15::MaskedTileU32 { lanes, elements }
            .validate()
            .map_err(ExecutionTileScheduleErrorV1::Geometry)?;
        Ok(Self {
            layout,
            lanes,
            elements,
        })
    }

    pub const fn layout(self) -> ExecutionTileLayoutV1 {
        self.layout
    }

    pub const fn lanes(self) -> u16 {
        self.lanes
    }

    pub const fn elements(self) -> u16 {
        self.elements
    }

    /// Computes the offset without allocating, reading memory, or wrapping.
    pub fn offset(self, lane: u16, element: u16) -> Result<u64, ExecutionTileScheduleErrorV1> {
        if lane >= self.lanes || element >= self.elements {
            return Err(ExecutionTileScheduleErrorV1::Coordinate { lane, element });
        }
        // Validated u16 geometry makes both expressions exact in u64.
        Ok(match self.layout {
            ExecutionTileLayoutV1::Blocked => {
                u64::from(lane) * u64::from(self.elements) + u64::from(element)
            }
            ExecutionTileLayoutV1::Striped => {
                u64::from(element) * u64::from(self.lanes) + u64::from(lane)
            }
        })
    }

    /// `None` means inactive: addition overflow and out-of-bounds both suppress
    /// the read. Inactive components have value zero and mask false.
    pub fn address(
        self,
        lane: u16,
        element: u16,
        base: u64,
        length: u64,
    ) -> Result<Option<u64>, ExecutionTileScheduleErrorV1> {
        Ok(base
            .checked_add(self.offset(lane, element)?)
            .filter(|index| *index < length))
    }

    /// Executes one reference component. The callback is invoked exactly once
    /// for an active component and never for an inactive or invalid component.
    /// Callers must execute components at the original tile-load effect site,
    /// even when the resulting fragment is later discarded.
    pub fn load_u32(
        self,
        lane: u16,
        element: u16,
        base: u64,
        length: u64,
        mut read: impl FnMut(u64) -> u32,
    ) -> Result<(u32, bool), ExecutionTileScheduleErrorV1> {
        Ok(match self.address(lane, element, base, length)? {
            Some(index) => (read(index), true),
            None => (0, false),
        })
    }
}

#[cfg(test)]
#[path = "execution_tile_schedule_v1_tests.rs"]
mod tests;
