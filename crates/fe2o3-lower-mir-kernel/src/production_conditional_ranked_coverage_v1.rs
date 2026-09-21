//! Bounded structural coverage of a borrowed, constructor-checked ranked recipe.
//! This is not source/ranked translation replay or a production proof receipt.

use super::{ProductionConditionalRankedOutputErrorV1, ProductionConditionalRankedOutputV1};
#[cfg(test)]
use dialect_kernel::AccessKindAttr;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
};
use fe2o3_pliron::{
    ProductionGpuWriteSiteV2, ProductionRankedKernelV1, ProductionRankedValueV1 as Value,
};
#[cfg(test)]
use fe2o3_pliron::{
    ProductionRankedOperationV1 as Op, ProductionRankedTerminatorV1 as Term,
    ProductionSemanticExpressionV2 as Expression,
};
use std::fmt;

/// Refusal of a descriptive structural query, not a runtime predicate failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionConditionalRankedCoverageErrorV1 {
    /// The inherited caller ledger cannot admit the next traversal operation.
    Resource(ResourceError),
    /// The existing canonical/source/output/extent join refused the candidate.
    Output(ProductionConditionalRankedOutputErrorV1),
    /// An operation is outside the total, effect-closed fragment.
    UnsupportedOperation {
        /// Ranked block ordinal, not a canonical Kernel IR BlockId.
        block: u32,
        /// Exact operation ordinal in that ranked block.
        operation: u32,
    },
    /// The terminator or block arguments are outside the closed fragment.
    UnsupportedTerminator {
        /// Ranked block ordinal.
        block: u32,
    },
    /// A condition is neither the selected bounds predicate nor two literals.
    UnresolvedCondition {
        /// Ranked block containing the condition.
        block: u32,
    },
    /// The selected bounds case reaches an explicit trap.
    AbnormalExit {
        /// Ranked block containing the trap.
        block: u32,
        /// Assumed truth value of the exact global-X < output-length predicate.
        predicate: bool,
    },
    /// The selected bounds case does not perform its required number of writes.
    WriteCount {
        /// Ranked block where the count fails.
        block: u32,
        /// True requires one selected write; false requires zero.
        predicate: bool,
    },
    /// A deterministic bounds case visits more blocks than the recipe contains.
    Cycle {
        /// Truth value of the selected bounds predicate on the cyclic path.
        predicate: bool,
    },
    /// A retained coordinate does not identify the required recipe occurrence.
    Coordinate,
}

impl From<ResourceError> for ProductionConditionalRankedCoverageErrorV1 {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl From<ProductionConditionalRankedOutputErrorV1> for ProductionConditionalRankedCoverageErrorV1 {
    fn from(error: ProductionConditionalRankedOutputErrorV1) -> Self {
        match error {
            ProductionConditionalRankedOutputErrorV1::Resource(error) => Self::Resource(error),
            error => Self::Output(error),
        }
    }
}

impl fmt::Display for ProductionConditionalRankedCoverageErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(out),
            Self::Output(error) => error.fmt(out),
            _ => write!(out, "conditional ranked structural coverage: {self:?}"),
        }
    }
}

impl std::error::Error for ProductionConditionalRankedCoverageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Output(error) => Some(error),
            _ => None,
        }
    }
}

/// A borrowed conditional CFG relation, with all external premises retained.
///
/// For the exact ranked global-X index and rederived output-length operand,
/// the true case performs exactly the selected write and the false case none;
/// both reach normal returns. No other executable effects or possibly trapping
/// computations are admitted. This statement is conditional on the joined
/// output's unchanged address-arithmetic and pointer-validity premises.
///
/// This does NOT authenticate source/ranked translation, reference coordinates,
/// stored-value equivalence, ownership, runtime length/launch values, or N <= G.
/// In particular source correspondence replay reconstructs only the canonical
/// graph, not this ranked relation. No clean-report typestate, proof/counter
/// credit, artifact custody, or launch permission is produced.
pub struct ProductionConditionalRankedCoverageV1<'a> {
    output: ProductionConditionalRankedOutputV1<'a>,
    true_exit_block: u32,
    false_exit_block: u32,
}

impl fmt::Debug for ProductionConditionalRankedCoverageV1<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("ProductionConditionalRankedCoverageV1")
            .field("output", &self.output)
            .field("true_exit_block", &self.true_exit_block)
            .field("false_exit_block", &self.false_exit_block)
            .finish()
    }
}

impl<'a> ProductionConditionalRankedCoverageV1<'a> {
    /// Exact borrowed join, including rederived extent and unchanged AddressDomain.
    pub const fn output(&self) -> &ProductionConditionalRankedOutputV1<'a> {
        &self.output
    }

    /// Normal ranked return reached when the selected bounds predicate is true.
    pub const fn true_exit_block(&self) -> u32 {
        self.true_exit_block
    }

    /// Normal ranked return reached when the selected bounds predicate is false.
    pub const fn false_exit_block(&self) -> u32 {
        self.false_exit_block
    }
}

type Result<T> = std::result::Result<T, ProductionConditionalRankedCoverageErrorV1>;
use ProductionConditionalRankedCoverageErrorV1 as Error;

pub(super) fn check_ranked_coverage_v1<'a>(
    output: ProductionConditionalRankedOutputV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<ProductionConditionalRankedCoverageV1<'a>> {
    // This reuses the original owner floor and the complete extent-use check,
    // even when the caller has already run that query on the same borrow.
    let output = output.rederive_output_extent_v1(budget)?;
    let extent = match output.dynamic_extent() {
        super::ProductionConditionalRankedExtentV1::CanonicalOutputLength { operand, .. } => {
            operand
        }
        super::ProductionConditionalRankedExtentV1::Unbound(_) => return Err(Error::Coordinate),
    };
    let [false_exit_block, true_exit_block] = check_paths(
        output.candidate().kernel(),
        output.ranked_index(),
        extent,
        output.gpu_write_site(),
        budget,
    )?;
    Ok(ProductionConditionalRankedCoverageV1 {
        output,
        true_exit_block,
        false_exit_block,
    })
}

fn check_paths(
    kernel: &ProductionRankedKernelV1,
    index: Value,
    extent: Value,
    write: ProductionGpuWriteSiteV2,
    budget: &mut Budget<'_>,
) -> Result<[u32; 2]> {
    use fe2o3_pliron::ProductionRankedRecipeCoverageErrorV1 as E;
    fe2o3_pliron::check_ranked_recipe_paths_v1(kernel, index, extent, write, budget).map_err(
        |error| match error {
            E::Resource(error) => Error::Resource(error),
            E::Coordinate => Error::Coordinate,
            E::UnsupportedOperation { block, operation } => {
                Error::UnsupportedOperation { block, operation }
            }
            E::UnsupportedTerminator { block } => Error::UnsupportedTerminator { block },
            E::UnresolvedCondition { block } => Error::UnresolvedCondition { block },
            E::AbnormalExit { block, predicate } => Error::AbnormalExit { block, predicate },
            E::WriteCount { block, predicate } => Error::WriteCount { block, predicate },
            E::Cycle { predicate } => Error::Cycle { predicate },
        },
    )
}

#[cfg(test)]
#[path = "production_conditional_ranked_coverage_v1_tests.rs"]
mod tests;
