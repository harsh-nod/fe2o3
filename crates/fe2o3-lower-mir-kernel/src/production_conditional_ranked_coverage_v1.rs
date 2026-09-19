//! Bounded structural coverage of a borrowed, constructor-checked ranked recipe.
//! This is not source/ranked translation replay or a production proof receipt.

use super::{ProductionConditionalRankedOutputErrorV1, ProductionConditionalRankedOutputV1};
use dialect_kernel::AccessKindAttr;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
};
use fe2o3_pliron::{
    ProductionGpuWriteSiteV2, ProductionRankedKernelV1, ProductionRankedOperationV1 as Op,
    ProductionRankedTerminatorV1 as Term, ProductionRankedValueV1 as Value,
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

fn ordinal(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| ResourceError::Arithmetic.into())
}

fn check_paths(
    kernel: &ProductionRankedKernelV1,
    index: Value,
    extent: Value,
    write: ProductionGpuWriteSiteV2,
    budget: &mut Budget<'_>,
) -> Result<[u32; 2]> {
    // The immutable constructor already checked unique/dense definitions,
    // operand scope/types and every edge's target/arguments. No second graph or
    // definition map is needed. Reject unsupported operations even if dead.
    let mut write_seen = false;
    for (block_id, block) in kernel.blocks().iter().enumerate() {
        budget.charge_work(4)?;
        let block_id = ordinal(block_id)?;
        if block.index_argument_count() != 0 {
            return Err(Error::UnsupportedTerminator { block: block_id });
        }
        for (operation_id, operation) in block.operations().iter().enumerate() {
            budget.charge_work(4)?;
            let operation_id = ordinal(operation_id)?;
            match operation {
                Op::ExecutionLayout { .. }
                | Op::InvocationIndex { .. }
                | Op::View { .. }
                | Op::ViewInSpace { .. }
                | Op::IndexConstant { .. }
                | Op::IndexUnknown { .. }
                | Op::SemanticConstant { .. }
                | Op::SemanticSymbol { .. }
                | Op::OwnershipContract { .. }
                | Op::RequestEffectRefinement { .. }
                | Op::RequireEffectRefinement { .. } => {}
                Op::SemanticExpression {
                    expression: Expression::Constant { .. } | Expression::Symbol { .. },
                    ..
                } => {}
                Op::Access {
                    kind: AccessKindAttr::Write,
                    ..
                }
                | Op::ValueAccess {
                    kind: AccessKindAttr::Write,
                    ..
                } if write == ProductionGpuWriteSiteV2::new(block_id, operation_id) => {
                    write_seen = true;
                }
                _ => {
                    return Err(Error::UnsupportedOperation {
                        block: block_id,
                        operation: operation_id,
                    });
                }
            }
        }
        // Validate all conditions, including those in unreachable blocks. Only
        // an exactly evaluated edge may make a trap or cycle infeasible.
        successor(kernel, block_id, index, extent, false, budget)?;
    }
    if !write_seen {
        return Err(Error::Coordinate);
    }
    Ok([
        walk(kernel, index, extent, write, false, budget)?,
        walk(kernel, index, extent, write, true, budget)?,
    ])
}

fn walk(
    kernel: &ProductionRankedKernelV1,
    index: Value,
    extent: Value,
    write: ProductionGpuWriteSiteV2,
    predicate: bool,
    budget: &mut Budget<'_>,
) -> Result<u32> {
    let mut block_id = 0;
    let mut wrote = false;
    // In this fragment each fixed predicate case has exactly one successor.
    // More than B visits therefore implies a repeated block, without needing
    // a visited bitmap, allocation, capacity estimate or scratch rollback.
    for _ in 0..kernel.blocks().len() {
        budget.charge_work(4)?;
        if block_id == write.block() {
            if wrote || !predicate {
                return Err(Error::WriteCount {
                    block: block_id,
                    predicate,
                });
            }
            wrote = true;
        }
        match successor(kernel, block_id, index, extent, predicate, budget)? {
            Edge::Block(target) => block_id = target,
            Edge::Return if wrote == predicate => return Ok(block_id),
            Edge::Return => {
                return Err(Error::WriteCount {
                    block: block_id,
                    predicate,
                });
            }
            Edge::Trap => {
                return Err(Error::AbnormalExit {
                    block: block_id,
                    predicate,
                });
            }
        }
    }
    Err(Error::Cycle { predicate })
}

enum Edge {
    Block(u32),
    Return,
    Trap,
}

fn successor(
    kernel: &ProductionRankedKernelV1,
    block_id: u32,
    index: Value,
    extent: Value,
    predicate: bool,
    budget: &mut Budget<'_>,
) -> Result<Edge> {
    budget.charge_work(6)?;
    let block = kernel
        .blocks()
        .get(block_id as usize)
        .ok_or(Error::Coordinate)?;
    let (condition, true_block, false_block) = match block.terminator() {
        Term::Branch { target } => return Ok(Edge::Block(*target)),
        Term::Return => return Ok(Edge::Return),
        Term::Trap => return Ok(Edge::Trap),
        Term::IndexLessThan {
            lhs,
            rhs,
            true_block,
            false_block,
        } if *lhs == index && *rhs == extent => (predicate, *true_block, *false_block),
        Term::IndexLessThan {
            lhs,
            rhs,
            true_block,
            false_block,
        }
        | Term::IndexEqual {
            lhs,
            rhs,
            true_block,
            false_block,
        } => {
            let lhs = literal(kernel, *lhs, budget)?;
            let rhs = literal(kernel, *rhs, budget)?;
            let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
                return Err(Error::UnresolvedCondition { block: block_id });
            };
            let condition = if matches!(block.terminator(), Term::IndexEqual { .. }) {
                lhs == rhs
            } else {
                lhs < rhs
            };
            (condition, *true_block, *false_block)
        }
        _ => return Err(Error::UnsupportedTerminator { block: block_id }),
    };
    Ok(Edge::Block(if condition {
        true_block
    } else {
        false_block
    }))
}

fn literal(
    kernel: &ProductionRankedKernelV1,
    value: Value,
    budget: &mut Budget<'_>,
) -> Result<Option<u64>> {
    budget.charge_work(1)?;
    let Value::Local(id) = value else {
        return Ok(None);
    };
    for block in kernel.blocks() {
        budget.charge_work(1)?;
        for operation in block.operations() {
            budget.charge_work(2)?;
            if let Op::IndexConstant { result, value } = operation
                && *result == id
            {
                return Ok(Some(*value));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "production_conditional_ranked_coverage_v1_tests.rs"]
mod tests;
