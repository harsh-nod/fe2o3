use pliron::derive::pliron_attr;

use crate::{DistributionAttr, TileError, check_rank};

/// An explicit rank-one logical distribution, independent of physical hardware.
///
/// This selects a bijection between logical elements and lane/component pairs;
/// it does not establish memory ownership, collective participation, or memory validity.
#[pliron_attr(name = "tile.distribution_order", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DistributionOrderAttr {
    /// Each lane holds consecutive logical elements: `lane * elements + component`.
    Blocked,
    /// Successive lanes hold consecutive elements: `component * lanes + lane`.
    Striped,
}

impl DistributionAttr {
    /// Maps one bounded lane/component pair to its rank-one logical index.
    ///
    /// Geometry is checked even for attributes obtained from parsing. No
    /// element enumeration, padding, physical offset, or implicit order is used.
    pub fn logical_index(
        &self,
        order: DistributionOrderAttr,
        lane: u32,
        component: u32,
    ) -> Result<u32, TileError> {
        self.verify_rank_one()?;
        if lane >= self.lanes() {
            return Err(TileError::LaneOutOfBounds {
                lane,
                lanes: self.lanes(),
            });
        }
        if component >= self.elements_per_lane() {
            return Err(TileError::ComponentOutOfBounds {
                component,
                elements_per_lane: self.elements_per_lane(),
            });
        }
        let (major, stride, minor) = match order {
            DistributionOrderAttr::Blocked => (lane, self.elements_per_lane(), component),
            DistributionOrderAttr::Striped => (component, self.lanes(), lane),
        };
        major
            .checked_mul(stride)
            .and_then(|base| base.checked_add(minor))
            .ok_or(TileError::DistributionOverflow)
    }

    /// Returns the unique `(lane, component)` for a rank-one logical index.
    pub fn fragment_coordinate(
        &self,
        order: DistributionOrderAttr,
        logical_index: u32,
    ) -> Result<(u32, u32), TileError> {
        self.verify_rank_one()?;
        let total_elements = self.total_elements()?;
        if logical_index >= total_elements {
            return Err(TileError::LogicalIndexOutOfBounds {
                logical_index,
                total_elements,
            });
        }
        Ok(match order {
            DistributionOrderAttr::Blocked => (
                logical_index / self.elements_per_lane(),
                logical_index % self.elements_per_lane(),
            ),
            DistributionOrderAttr::Striped => {
                (logical_index % self.lanes(), logical_index / self.lanes())
            }
        })
    }

    pub(crate) fn verify_rank_one(&self) -> Result<(), TileError> {
        check_rank(self.rank())?;
        self.total_elements()?;
        if self.rank() != 1 {
            return Err(TileError::MappingRequiresRankOne(self.rank()));
        }
        Ok(())
    }
}
