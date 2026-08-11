use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use fe2o3_kernel_ir::BlockId;

use crate::control_flow::ControlFlowBudget;
use crate::{
    ControlFlowAnalysis, ControlFlowDiagnostic, ControlFlowResource, ControlFlowResourceUsage,
};

/// Stable frontend identity for one promotable source variable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SsaVariable(pub u32);

/// Definition and liveness facts used for pruned block-parameter placement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsaVariablePlacement {
    pub variable: SsaVariable,
    pub definition_blocks: BTreeSet<BlockId>,
    pub live_in_blocks: BTreeSet<BlockId>,
}

/// Deterministic pruned placement, indexed in both useful directions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsaPlacement {
    by_block: BTreeMap<BlockId, BTreeSet<SsaVariable>>,
    by_variable: BTreeMap<SsaVariable, BTreeSet<BlockId>>,
    resource_usage: ControlFlowResourceUsage,
}

impl SsaPlacement {
    pub fn variables_at(&self, block: BlockId) -> Option<&BTreeSet<SsaVariable>> {
        self.by_block.get(&block)
    }

    pub fn blocks_for(&self, variable: SsaVariable) -> Option<&BTreeSet<BlockId>> {
        self.by_variable.get(&variable)
    }

    pub fn by_block(&self) -> &BTreeMap<BlockId, BTreeSet<SsaVariable>> {
        &self.by_block
    }

    pub const fn resource_usage(&self) -> ControlFlowResourceUsage {
        self.resource_usage
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SsaPlacementDiagnostic {
    ResourceLimitExceeded {
        resource: ControlFlowResource,
        required: usize,
        limit: usize,
        storage_items: usize,
        work_units: usize,
    },
    DuplicateVariable {
        variable: SsaVariable,
    },
    MissingDefinition {
        variable: SsaVariable,
    },
    UnknownDefinitionBlock {
        variable: SsaVariable,
        block: BlockId,
    },
    UnreachableDefinitionBlock {
        variable: SsaVariable,
        block: BlockId,
    },
    UnknownLiveInBlock {
        variable: SsaVariable,
        block: BlockId,
    },
    UnreachableLiveInBlock {
        variable: SsaVariable,
        block: BlockId,
    },
}

impl fmt::Display for SsaPlacementDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimitExceeded {
                resource,
                required,
                limit,
                storage_items,
                work_units,
            } => write!(
                formatter,
                "SSA placement {} require {required} items, exceeding the deterministic limit {limit}; aggregate storage {storage_items}, aggregate work {work_units}",
                resource.description()
            ),
            Self::DuplicateVariable { variable } => {
                write!(formatter, "duplicate SSA variable v{}", variable.0)
            }
            Self::MissingDefinition { variable } => {
                write!(
                    formatter,
                    "SSA variable v{} has no definition block",
                    variable.0
                )
            }
            Self::UnknownDefinitionBlock { variable, block } => write!(
                formatter,
                "SSA variable v{} has unknown definition block {block}",
                variable.0
            ),
            Self::UnreachableDefinitionBlock { variable, block } => write!(
                formatter,
                "SSA variable v{} has unreachable definition block {block}",
                variable.0
            ),
            Self::UnknownLiveInBlock { variable, block } => write!(
                formatter,
                "SSA variable v{} has unknown live-in block {block}",
                variable.0
            ),
            Self::UnreachableLiveInBlock { variable, block } => write!(
                formatter,
                "SSA variable v{} has unreachable live-in block {block}",
                variable.0
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsaPlacementErrors {
    diagnostics: Vec<SsaPlacementDiagnostic>,
}

impl SsaPlacementErrors {
    pub fn diagnostics(&self) -> &[SsaPlacementDiagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for SsaPlacementErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "SSA placement failed with {} diagnostic(s)",
            self.diagnostics.len()
        )?;
        for diagnostic in &self.diagnostics {
            writeln!(formatter, "  {diagnostic}")?;
        }
        Ok(())
    }
}

impl Error for SsaPlacementErrors {}

/// Places pruned SSA block parameters using iterated dominance frontiers.
///
/// Definition and liveness facts remain caller-supplied analysis inputs. This
/// function validates their graph identities but does not grant type,
/// initialization, or frontend-causality authority.
pub fn place_pruned_ssa_parameters(
    control_flow: &ControlFlowAnalysis,
    variables: &[SsaVariablePlacement],
) -> Result<SsaPlacement, SsaPlacementErrors> {
    let mut budget = ControlFlowBudget::from_usage(control_flow.resource_usage());
    let validation_work = variables.iter().fold(variables.len(), |work, placement| {
        work.checked_add(placement.definition_blocks.len())
            .and_then(|work| work.checked_add(placement.live_in_blocks.len()))
            .unwrap_or(usize::MAX)
    });
    charge(&mut budget, BudgetCharge::Work(validation_work))?;
    charge(&mut budget, BudgetCharge::Storage(variables.len()))?;
    let mut diagnostics = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for placement in variables {
        if !seen.insert(placement.variable) {
            diagnostics.insert(SsaPlacementDiagnostic::DuplicateVariable {
                variable: placement.variable,
            });
        }
        if placement.definition_blocks.is_empty() {
            diagnostics.insert(SsaPlacementDiagnostic::MissingDefinition {
                variable: placement.variable,
            });
        }
        validate_blocks(
            control_flow,
            placement.variable,
            &placement.definition_blocks,
            true,
            &mut diagnostics,
        );
        validate_blocks(
            control_flow,
            placement.variable,
            &placement.live_in_blocks,
            false,
            &mut diagnostics,
        );
    }
    if !diagnostics.is_empty() {
        return Err(SsaPlacementErrors {
            diagnostics: diagnostics.into_iter().collect(),
        });
    }

    charge(
        &mut budget,
        BudgetCharge::Storage(control_flow.blocks().len()),
    )?;
    let mut by_block = control_flow
        .blocks()
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut by_variable = BTreeMap::new();
    charge(&mut budget, BudgetCharge::Storage(variables.len()))?;
    let mut ordered = variables.iter().collect::<Vec<_>>();
    charge(&mut budget, BudgetCharge::Work(sort_work(ordered.len())))?;
    ordered.sort_by_key(|placement| placement.variable);
    for placement in ordered {
        charge(&mut budget, BudgetCharge::Work(1))?;
        let frontiers = control_flow
            .iterated_dominance_frontier_with_budget(&placement.definition_blocks, &mut budget)
            .map_err(resource_errors)?
            .expect("validated definitions are reachable");
        let mut blocks = BTreeSet::new();
        for block in frontiers {
            charge(&mut budget, BudgetCharge::Work(1))?;
            if placement.live_in_blocks.contains(&block) {
                charge(&mut budget, BudgetCharge::SsaOutput(1))?;
                blocks.insert(block);
            }
        }
        for block in &blocks {
            charge(&mut budget, BudgetCharge::Work(1))?;
            charge(&mut budget, BudgetCharge::Storage(1))?;
            by_block
                .get_mut(block)
                .expect("validated placement block belongs to the function")
                .insert(placement.variable);
        }
        charge(&mut budget, BudgetCharge::Storage(1))?;
        by_variable.insert(placement.variable, blocks);
    }

    Ok(SsaPlacement {
        by_block,
        by_variable,
        resource_usage: budget.usage(),
    })
}

enum BudgetCharge {
    Storage(usize),
    SsaOutput(usize),
    Work(usize),
}

fn charge(budget: &mut ControlFlowBudget, charge: BudgetCharge) -> Result<(), SsaPlacementErrors> {
    let result = match charge {
        BudgetCharge::Storage(items) => budget.storage(items),
        BudgetCharge::SsaOutput(items) => budget.ssa_output(items),
        BudgetCharge::Work(items) => budget.work(items),
    };
    result.map_err(resource_errors)
}

fn resource_errors(diagnostic: ControlFlowDiagnostic) -> SsaPlacementErrors {
    let ControlFlowDiagnostic::ResourceLimitExceeded {
        resource,
        required,
        limit,
        storage_items,
        work_units,
    } = diagnostic
    else {
        unreachable!("SSA placement budget produced a non-resource diagnostic");
    };
    SsaPlacementErrors {
        diagnostics: vec![SsaPlacementDiagnostic::ResourceLimitExceeded {
            resource,
            required,
            limit,
            storage_items,
            work_units,
        }],
    }
}

fn sort_work(items: usize) -> usize {
    let levels = if items <= 1 {
        1
    } else {
        usize::BITS as usize - (items - 1).leading_zeros() as usize
    };
    items
        .checked_mul(levels)
        .and_then(|work| work.checked_mul(2))
        .unwrap_or(usize::MAX)
}

fn validate_blocks(
    control_flow: &ControlFlowAnalysis,
    variable: SsaVariable,
    blocks: &BTreeSet<BlockId>,
    definitions: bool,
    diagnostics: &mut BTreeSet<SsaPlacementDiagnostic>,
) {
    for block in blocks {
        let diagnostic = if !control_flow.blocks().contains(block) {
            if definitions {
                SsaPlacementDiagnostic::UnknownDefinitionBlock {
                    variable,
                    block: *block,
                }
            } else {
                SsaPlacementDiagnostic::UnknownLiveInBlock {
                    variable,
                    block: *block,
                }
            }
        } else if !control_flow.is_reachable(*block) {
            if definitions {
                SsaPlacementDiagnostic::UnreachableDefinitionBlock {
                    variable,
                    block: *block,
                }
            } else {
                SsaPlacementDiagnostic::UnreachableLiveInBlock {
                    variable,
                    block: *block,
                }
            }
        } else {
            continue;
        };
        diagnostics.insert(diagnostic);
    }
}
