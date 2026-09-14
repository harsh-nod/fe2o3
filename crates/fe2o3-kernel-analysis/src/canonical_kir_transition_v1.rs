//! Independent, owner-bound checking of the fixed scalar/CFG occurrence relation.
//! This establishes only the named local rewrite rules, never general semantic
//! equivalence, formal verification, compiler execution, or runtime authority.

use crate::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirDefinitionDescendantKindV1 as DescendantKind,
    CanonicalKirOperationOriginV1 as Origin, CanonicalKirTransitionCandidateV1 as Candidate,
    ScalarType,
};
use std::{error::Error as StdError, fmt, mem::size_of};

mod catalog_transport;
mod control;
mod control_index;
mod index;
mod payload;
mod structure;
mod values;

pub use catalog_transport::*;
pub use control_index::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirTransitionErrorV1 {
    Resource(Resource),
    Arithmetic,
    InvalidCoordinate,
    IncompleteRows,
    /// An explicit checked rule failed; there is no legacy-map fallback.
    Rule(&'static str),
}
type Error = CanonicalKirTransitionErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::Arithmetic => formatter.write_str("canonical transition arithmetic overflow"),
            Self::InvalidCoordinate => {
                formatter.write_str("canonical transition coordinate is absent")
            }
            Self::IncompleteRows => {
                formatter.write_str("canonical transition occurrence coverage is incomplete")
            }
            Self::Rule(rule) => write!(formatter, "canonical transition rule rejected: {rule}"),
        }
    }
}
impl StdError for Error {}

/// The result borrows the actual checked inventories and immutable candidate
/// rows. It cannot outlive either graph or authorize adoption of another owner.
#[derive(Debug)]
pub struct CheckedCanonicalKirTransitionV1<'a, 'input, 'output, 'rows> {
    input: &'a Inventory<'input>,
    output: &'a Inventory<'output>,
    rows: Candidate<'rows>,
}
impl<'a, 'input, 'output, 'rows> CheckedCanonicalKirTransitionV1<'a, 'input, 'output, 'rows> {
    pub const fn input(&self) -> &'a Inventory<'input> {
        self.input
    }
    pub const fn output(&self) -> &'a Inventory<'output> {
        self.output
    }
    pub const fn rows(&self) -> Candidate<'rows> {
        self.rows
    }
    /// Closed rule set, not an equivalence or formal-proof statement.
    pub const fn checked_rules(&self) -> &'static [&'static str] {
        &[
            "exact declarations and signatures",
            "typed scalar substitutions",
            "complete final operand occurrences",
            "selected branches and merge chains",
            "ordered edge-argument transport",
            "complete surviving ordered operations",
        ]
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Logical checked-view payload only. Graphs, inventories, and row owners are
/// borrowed and must already be reserved by the caller. Reserve this transfer
/// before the next controlled allocation; drop the checked view before release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirTransitionStorageV1 {
    retained: usize,
}
impl CanonicalKirTransitionStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Checks the two actual graph subjects without cloning or decoding either
/// module. Every coordinate lookup, variable payload comparison, allocation,
/// fixed-point visit and CFG queue step is charged before execution.
///
/// Scope exit drops all transient facts before restoring the incoming storage
/// floor on success or failure. Accepted work, peak, and first-failure history
/// are never reset. Success transfers only the small borrowed checked view.
pub fn check_canonical_kir_transition_v1<'a, 'input, 'output, 'rows>(
    input: &'a Inventory<'input>,
    output: &'a Inventory<'output>,
    rows: Candidate<'rows>,
    budget: &mut Budget<'_>,
) -> Result<(
    CheckedCanonicalKirTransitionV1<'a, 'input, 'output, 'rows>,
    CanonicalKirTransitionStorageV1,
)> {
    let floor = budget.storage();
    let retained = size_of::<CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>>();
    let result = (|| {
        budget.charge_work(1)?;
        budget.reserve_storage(retained)?;
        let mut state = State::new(input, output, rows, budget)?;
        state.check_structure(budget)?;
        state.solve_values(budget)?;
        state.check_control(budget)?;
        state.check_values_and_uses(budget)?;
        state.check_order(budget)?;
        Ok(CheckedCanonicalKirTransitionV1 {
            input,
            output,
            rows,
        })
    })();
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(release)?;
    result.map(|checked| (checked, CanonicalKirTransitionStorageV1 { retained }))
}

const NONE: usize = usize::MAX;
const INTERNAL: usize = usize::MAX - 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Literal {
    ty: ScalarType,
    bits: u128,
}

struct State<'a, 'input, 'output, 'rows> {
    input: &'a Inventory<'input>,
    output: &'a Inventory<'output>,
    rows: Candidate<'rows>,
    function_input: Vec<usize>,
    function_output: Vec<usize>,
    block_output: Vec<usize>,
    block_position: Vec<usize>,
    block_head: Vec<usize>,
    block_tail: Vec<usize>,
    operation_output: Vec<usize>,
    operation_input: Vec<usize>,
    anchors: Vec<usize>,
    retained_anchors: Vec<u8>,
    parents: Vec<usize>,
    literals: Vec<Option<Literal>>,
    incoming_head: Vec<usize>,
    incoming_next: Vec<usize>,
    edge_target: Vec<usize>,
    edge_output: Vec<usize>,
    reachable: Vec<u8>,
    pending: Vec<usize>,
}

fn allocate<T: Copy>(count: usize, value: T, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    let bytes = count.checked_mul(size_of::<T>()).ok_or(Error::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    budget.charge_work(count)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    values.resize(count, value);
    Ok(values)
}

impl<'a, 'input, 'output, 'rows> State<'a, 'input, 'output, 'rows> {
    fn new(
        input: &'a Inventory<'input>,
        output: &'a Inventory<'output>,
        rows: Candidate<'rows>,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(1)?;
        if rows.functions.len() != output.functions().len()
            || input.functions().len() != output.functions().len()
            || rows.blocks.len() != output.blocks().len()
            || rows.operations.len() != output.operations().len()
            || rows.definitions.len() != input.definitions().len()
            || rows.uses.len() != output.uses().len()
            || rows.edges.len() != output.edges().len()
            || rows.edge_arguments.len() != output.edge_arguments().len()
            || rows.segments.len() > input.blocks().len()
        {
            return Err(Error::IncompleteRows);
        }
        budget.reserve_storage(size_of::<Self>())?;
        let mut state = Self {
            input,
            output,
            rows,
            function_input: allocate(output.functions().len(), NONE, budget)?,
            function_output: allocate(input.functions().len(), NONE, budget)?,
            block_output: allocate(input.blocks().len(), NONE, budget)?,
            block_position: allocate(input.blocks().len(), NONE, budget)?,
            block_head: allocate(output.blocks().len(), NONE, budget)?,
            block_tail: allocate(output.blocks().len(), NONE, budget)?,
            operation_output: allocate(input.operations().len(), NONE, budget)?,
            operation_input: allocate(output.operations().len(), NONE, budget)?,
            anchors: allocate(output.definitions().len(), NONE, budget)?,
            retained_anchors: allocate(output.definitions().len(), 0, budget)?,
            parents: allocate(input.definitions().len(), NONE, budget)?,
            literals: allocate(input.definitions().len(), None, budget)?,
            incoming_head: allocate(input.blocks().len(), NONE, budget)?,
            incoming_next: allocate(input.edges().len(), NONE, budget)?,
            edge_target: allocate(input.edges().len(), NONE, budget)?,
            edge_output: allocate(input.edges().len(), NONE, budget)?,
            reachable: allocate(input.blocks().len(), 0, budget)?,
            pending: allocate(input.blocks().len(), NONE, budget)?,
        };
        for (index, parent) in state.parents.iter_mut().enumerate() {
            budget.charge_work(1)?;
            *parent = index;
        }
        for (index, edge) in input.edges().iter().enumerate() {
            budget.charge_work(1)?;
            let target = index::block(input, edge.target, budget)?;
            state.edge_target[index] = target;
            state.incoming_next[index] = state.incoming_head[target];
            state.incoming_head[target] = index;
        }
        Ok(state)
    }

    fn root(&self, mut definition: usize, budget: &mut Budget<'_>) -> Result<usize> {
        loop {
            budget.charge_work(1)?;
            let next = *self
                .parents
                .get(definition)
                .ok_or(Error::InvalidCoordinate)?;
            if next == definition {
                return Ok(definition);
            }
            if next >= definition {
                return Err(Error::Rule("substitution forest ordering"));
            }
            definition = next;
        }
    }

    fn literal(&self, definition: usize, budget: &mut Budget<'_>) -> Result<Option<Literal>> {
        Ok(self.literals[self.root(definition, budget)?])
    }

    fn equal(&self, a: usize, b: usize, budget: &mut Budget<'_>) -> Result<bool> {
        let a = self.root(a, budget)?;
        let b = self.root(b, budget)?;
        Ok(a == b || self.literals[a].is_some() && self.literals[a] == self.literals[b])
    }

    fn union(&mut self, a: usize, b: usize, budget: &mut Budget<'_>) -> Result<bool> {
        let a = self.root(a, budget)?;
        let b = self.root(b, budget)?;
        if a == b {
            return Ok(false);
        }
        if !payload::ty(
            self.input.definitions()[a].ty,
            self.input.definitions()[b].ty,
            budget,
        )? {
            return Err(Error::Rule("substitution type"));
        }
        if self.literals[a].is_some()
            && self.literals[b].is_some()
            && self.literals[a] != self.literals[b]
        {
            return Err(Error::Rule("conflicting scalar facts"));
        }
        budget.charge_work(1)?;
        let (low, high) = if a < b { (a, b) } else { (b, a) };
        self.parents[high] = low;
        self.literals[low] = self.literals[low].or(self.literals[high]);
        Ok(true)
    }

    fn publish_literal(
        &mut self,
        definition: usize,
        value: Literal,
        budget: &mut Budget<'_>,
    ) -> Result<bool> {
        let root = self.root(definition, budget)?;
        if let Some(old) = self.literals[root] {
            if old != value {
                return Err(Error::Rule("conflicting constant derivation"));
            }
            return Ok(false);
        }
        budget.charge_work(1)?;
        self.literals[root] = Some(value);
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
