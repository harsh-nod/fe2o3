use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};

#[cfg(test)]
#[path = "production_origin_worklist_v1_tests.rs"]
mod tests;

const NO_LINK: usize = usize::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OriginStateV1<T> {
    Pending,
    Exact(T),
    Unknown,
}

impl<T: Copy + Eq> OriginStateV1<T> {
    fn join(self, incoming: Self) -> Self {
        match (self, incoming) {
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Pending, other) | (other, Self::Pending) => other,
            (Self::Exact(left), Self::Exact(right)) if left == right => self,
            (Self::Exact(_), Self::Exact(_)) => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OriginWorkErrorV1 {
    Resource(Resource),
    Shape,
}

impl From<Resource> for OriginWorkErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

type Result<T> = std::result::Result<T, OriginWorkErrorV1>;

struct Link {
    target: usize,
    next: usize,
}

/// Bounded exact-origin transport, not may-alias analysis or source authority.
/// Labels and all syntactic dependencies are authenticated by the caller.
/// Requested payload reservations, including failure cleanup, stay caller-owned.
pub(super) struct OriginWorkV1<T> {
    origins: Vec<OriginStateV1<T>>,
    heads: Vec<usize>,
    links: Vec<Link>,
    queue: Vec<usize>,
    queued: Vec<bool>,
    read: usize,
    write: usize,
    pending: usize,
    seeded: usize,
    edges: usize,
    ledger: Ledger,
    minimum_storage: usize,
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

impl<T: Copy + Eq> OriginWorkV1<T> {
    pub(super) fn new(nodes: usize, edges: usize, budget: &mut Budget<'_>) -> Result<Self> {
        let mut origins = reserved(nodes, budget)?;
        let mut heads = reserved(nodes, budget)?;
        let links = reserved(edges, budget)?;
        let mut queue = reserved(nodes, budget)?;
        let mut queued = reserved(nodes, budget)?;
        budget.charge_work(nodes.checked_mul(4).ok_or(Resource::Arithmetic)?)?;
        origins.resize(nodes, OriginStateV1::Unknown);
        heads.resize(nodes, NO_LINK);
        queue.resize(nodes, 0);
        queued.resize(nodes, false);
        Ok(Self {
            origins,
            heads,
            links,
            queue,
            queued,
            read: 0,
            write: 0,
            pending: 0,
            seeded: 0,
            edges,
            ledger: budget.work_ledger_identity_v1(),
            minimum_storage: budget.storage(),
        })
    }

    fn check_ledger(&self, budget: &Budget<'_>) -> Result<()> {
        if self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.minimum_storage
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }

    pub(super) fn seed_next(
        &mut self,
        value: OriginStateV1<T>,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.check_ledger(budget)?;
        budget.charge_work(1)?;
        let slot = self
            .origins
            .get_mut(self.seeded)
            .ok_or(OriginWorkErrorV1::Shape)?;
        *slot = value;
        self.seeded += 1;
        Ok(())
    }

    pub(super) fn add_link(
        &mut self,
        source: usize,
        target: usize,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.check_ledger(budget)?;
        budget.charge_work(1)?;
        if self.seeded != self.origins.len()
            || source >= self.origins.len()
            || target >= self.origins.len()
            || self.links.len() >= self.edges
        {
            return Err(OriginWorkErrorV1::Shape);
        }
        let link = self.links.len();
        self.links.push(Link {
            target,
            next: self.heads[source],
        });
        self.heads[source] = link;
        Ok(())
    }

    pub(super) fn solve(mut self, budget: &mut Budget<'_>) -> Result<Vec<OriginStateV1<T>>> {
        self.check_ledger(budget)?;
        if self.seeded != self.origins.len() || self.links.len() != self.edges {
            return Err(OriginWorkErrorV1::Shape);
        }
        for index in 0..self.origins.len() {
            budget.charge_work(1)?;
            if self.origins[index] != OriginStateV1::Pending {
                self.enqueue(index, budget)?;
            }
        }
        self.propagate(budget)?;
        // An ungrounded incoming cycle cannot disappear from a partly grounded phi.
        for index in 0..self.origins.len() {
            budget.charge_work(1)?;
            if self.origins[index] == OriginStateV1::Pending {
                self.origins[index] = OriginStateV1::Unknown;
                self.enqueue(index, budget)?;
            }
        }
        self.propagate(budget)?;
        Ok(self.origins)
    }

    fn enqueue(&mut self, definition: usize, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        let marked = self
            .queued
            .get_mut(definition)
            .ok_or(OriginWorkErrorV1::Shape)?;
        if *marked {
            return Ok(());
        }
        if self.pending == self.queue.len() {
            return Err(OriginWorkErrorV1::Shape);
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
                let row = self.links.get(link).ok_or(OriginWorkErrorV1::Shape)?;
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
