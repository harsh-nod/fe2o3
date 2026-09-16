use fe2o3_kernel_analysis::{CanonicalKirInventoryErrorV1 as Error, CanonicalKirInventoryV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirFunctionCoordinateV1 as Function,
    ValueId, VerifiedCanonicalKernelIrModuleV12,
};

type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
#[path = "production_value_origin_v1_tests.rs"]
mod tests;

const NO_LINK: usize = usize::MAX;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Origin {
    Pending,
    Exact(Definition),
    Unknown,
}

impl Origin {
    fn join(self, incoming: Self) -> Self {
        match (self, incoming) {
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Pending, other) | (other, Self::Pending) => other,
            (Self::Exact(left), Self::Exact(right)) if left == right => self,
            (Self::Exact(_), Self::Exact(_)) => Self::Unknown,
        }
    }
}

struct Link {
    target: usize,
    next: usize,
}

struct Work {
    origins: Vec<Origin>,
    heads: Vec<usize>,
    links: Vec<Link>,
    queue: Vec<usize>,
    queued: Vec<bool>,
    read: usize,
    write: usize,
    pending: usize,
}

fn reserved<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    budget.charge_work(1)?;
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    Ok(values)
}

impl Work {
    fn new(definitions: usize, edges: usize, budget: &mut Budget<'_>) -> Result<Self> {
        let mut origins = reserved(definitions, budget)?;
        let mut heads = reserved(definitions, budget)?;
        let links = reserved(edges, budget)?;
        let mut queue = reserved(definitions, budget)?;
        let mut queued = reserved(definitions, budget)?;
        budget.charge_work(definitions.checked_mul(4).ok_or(Resource::Arithmetic)?)?;
        origins.resize(definitions, Origin::Unknown);
        heads.resize(definitions, NO_LINK);
        queue.resize(definitions, 0);
        queued.resize(definitions, false);
        Ok(Self {
            origins,
            heads,
            links,
            queue,
            queued,
            read: 0,
            write: 0,
            pending: 0,
        })
    }

    fn enqueue(&mut self, definition: usize, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        let marked = self
            .queued
            .get_mut(definition)
            .ok_or(Error::InconsistentOwner)?;
        if *marked {
            return Ok(());
        }
        if self.pending == self.queue.len() {
            return Err(Error::InconsistentOwner);
        }
        *marked = true;
        self.queue[self.write] = definition;
        self.write = if self.write + 1 == self.queue.len() {
            0
        } else {
            self.write + 1
        };
        self.pending += 1;
        Ok(())
    }

    fn propagate(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        while self.pending != 0 {
            budget.charge_work(1)?;
            let source = self.queue[self.read];
            self.read = if self.read + 1 == self.queue.len() {
                0
            } else {
                self.read + 1
            };
            self.pending -= 1;
            self.queued[source] = false;
            let incoming = self.origins[source];
            let mut link = self.heads[source];
            while link != NO_LINK {
                budget.charge_work(1)?;
                let row = self.links.get(link).ok_or(Error::InconsistentOwner)?;
                let (target, next) = (row.target, row.next);
                let merged = self.origins[target].join(incoming);
                if merged != self.origins[target] {
                    self.origins[target] = merged;
                    self.enqueue(target, budget)?;
                }
                link = next;
            }
        }
        Ok(())
    }
}

/// Resolves whole-value block transport, never casts, aliasing or bounds.
/// Every syntactic incoming edge contributes, including unreachable edges.
/// Requested scratch payload is O(function definitions + edge arguments).
/// Ordinary Result returns restore the ledger floor; no unwind/RSS bound is claimed.
pub(super) fn resolve_whole_value_origin_v1(
    inventory: &CanonicalKirInventoryV1<'_>,
    expected_owner: &VerifiedCanonicalKernelIrModuleV12,
    function: Function,
    value: ValueId,
    budget: &mut Budget<'_>,
) -> Result<Option<Definition>> {
    let floor = budget.storage();
    let result = resolve_inner(inventory, expected_owner, function, value, budget);
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(release)?;
    result
}

fn resolve_inner(
    inventory: &CanonicalKirInventoryV1<'_>,
    expected_owner: &VerifiedCanonicalKernelIrModuleV12,
    function: Function,
    value: ValueId,
    budget: &mut Budget<'_>,
) -> Result<Option<Definition>> {
    budget.charge_work(3)?;
    if !inventory.belongs_to(expected_owner) {
        return Err(Error::InconsistentOwner);
    }
    let function_row = inventory
        .functions()
        .get(function.0 as usize)
        .filter(|row| row.coordinate == function && row.function.body.is_some())
        .ok_or(Error::InconsistentOwner)?;
    let requested = inventory
        .definition_for_value(function, value, budget)?
        .ok_or(Error::InconsistentOwner)?;
    let range = function_row.definitions.clone();
    let definitions = inventory
        .definitions()
        .get(range.clone())
        .ok_or(Error::InconsistentOwner)?;
    let edges = inventory
        .edge_arguments()
        .get(function_row.edge_arguments.clone())
        .ok_or(Error::InconsistentOwner)?;
    let mut work = Work::new(definitions.len(), edges.len(), budget)?;
    let mut selected = None;
    for (index, definition) in definitions.iter().enumerate() {
        budget.charge_work(1)?;
        let (actual_function, origin) = match definition.coordinate {
            Definition::FunctionArgument { function, .. } => {
                (function, Origin::Exact(definition.coordinate))
            }
            Definition::BlockArgument { block, .. } => (
                block.function,
                if block.block == 0 {
                    Origin::Unknown
                } else {
                    Origin::Pending
                },
            ),
            Definition::Result { operation, .. } => (
                operation.block.function,
                Origin::Exact(definition.coordinate),
            ),
        };
        if actual_function != function {
            return Err(Error::InconsistentOwner);
        }
        if definition.coordinate == requested.coordinate {
            selected = Some(index);
        }
        work.origins[index] = origin;
    }
    for edge in edges {
        budget.charge_work(1)?;
        if !range.contains(&edge.incoming_definition) || !range.contains(&edge.target_definition) {
            return Err(Error::InconsistentOwner);
        }
        let source = edge.incoming_definition - range.start;
        let target = edge.target_definition - range.start;
        if !matches!(
            definitions[target].coordinate,
            Definition::BlockArgument { block, .. } if block.function == function
        ) || work.links.len() == edges.len()
        {
            return Err(Error::InconsistentOwner);
        }
        let link = work.links.len();
        work.links.push(Link {
            target,
            next: work.heads[source],
        });
        work.heads[source] = link;
    }
    for index in 0..work.origins.len() {
        budget.charge_work(1)?;
        if work.origins[index] != Origin::Pending {
            work.enqueue(index, budget)?;
        }
    }
    work.propagate(budget)?;
    // An ungrounded incoming cycle cannot disappear from a partly grounded phi.
    for index in 0..work.origins.len() {
        budget.charge_work(1)?;
        if work.origins[index] == Origin::Pending {
            work.origins[index] = Origin::Unknown;
            work.enqueue(index, budget)?;
        }
    }
    work.propagate(budget)?;
    let selected = selected.ok_or(Error::InconsistentOwner)?;
    budget.charge_work(1)?;
    Ok(match work.origins[selected] {
        Origin::Exact(definition) => Some(definition),
        Origin::Unknown => None,
        Origin::Pending => return Err(Error::InconsistentOwner),
    })
}
