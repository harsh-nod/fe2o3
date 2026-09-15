//! Share immutable CFG entries without copying their retained payload. Every
//! payload and handle owns a reservation in the same analysis storage ledger.
use super::*;

#[path = "unchanged_join_v1.rs"]
mod unchanged_join_v1;

#[cfg(test)]
#[path = "retained_flow_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "publication_eq74_tests.rs"]
mod publication_eq74_tests;

// Logical bookkeeping words: Rc's two counts, plus the payload Storage's
// ledger handle and node count. The Flow's existing nodes are charged separately.
const PAYLOAD_OVERHEAD: usize = 4;
// One Rc handle and its separate two-word Storage reservation. CFG capacity
// remains separately charged, so this also conservatively charges entry slots.
const HANDLE_NODES: usize = 3;

struct Payload {
    flow: Flow,
    storage: Storage,
}

pub(in super::super) struct RetainedFlow {
    payload: Rc<Payload>,
    handle: Storage,
}

impl PartialEq for RetainedFlow {
    fn eq(&self, other: &Self) -> bool {
        self.flow() == other.flow()
    }
}
impl Eq for RetainedFlow {}

impl RetainedFlow {
    pub(in super::super) fn new(flow: Flow, budget: &mut Budget) -> Result<Self> {
        budget.charge(1)?;
        let nodes = flow.nodes().checked_add(PAYLOAD_OVERHEAD).ok_or(())?;
        // Both reservations precede allocating the shared owner. Any failure
        // drops already-reserved storage and the unretained working Flow.
        let storage = budget.reserve(nodes)?;
        let handle = budget.reserve(HANDLE_NODES)?;
        Ok(Self {
            payload: Rc::new(Payload { flow, storage }),
            handle,
        })
    }

    fn same_budget(&self, budget: &Budget) -> Result<()> {
        (Rc::ptr_eq(&self.payload.storage.live, &budget.storage)
            && Rc::ptr_eq(&self.handle.live, &budget.storage))
        .then_some(())
        .ok_or(())
    }

    pub(in super::super) fn flow(&self) -> &Flow {
        &self.payload.flow
    }

    pub(in super::super) fn share(&self, budget: &mut Budget) -> Result<Self> {
        self.same_budget(budget)?;
        budget.charge(1)?;
        let handle = budget.reserve(HANDLE_NODES)?;
        Ok(Self {
            payload: Rc::clone(&self.payload),
            handle,
        })
    }

    pub(in super::super) fn working(&self, budget: &mut Budget) -> Result<Flow> {
        self.same_budget(budget)?;
        // The retained source is reserved; the old clone preflight counts the
        // independent mutable copy and its capacity-sensitive size-cache work.
        self.flow().clone_retained(budget)
    }

    // Equality for queue publication only. The caller must still perform the
    // original normalized join before asking whether its output changed.
    pub(in super::super) fn publication_eq(&self, other: &Self, budget: &mut Budget) -> Result<bool> {
        self.same_budget(budget)?;
        other.same_budget(budget)?;
        budget.charge(1)?;
        if Rc::ptr_eq(&self.payload, &other.payload) {
            return Ok(true);
        }
        budget.charge(self.flow().nodes().checked_add(other.flow().nodes()).ok_or(())?)?;
        Ok(self == other)
    }

    pub(in super::super) fn join(&self, other: &Self, budget: &mut Budget) -> Result<Self> {
        self.same_budget(budget)?;
        other.same_budget(budget)?;
        budget.charge(1)?;
        let unchanged = if Rc::ptr_eq(&self.payload, &other.payload) {
            // Immutable identity proves equality only, never normalization.
            unchanged_join_v1::normalized(self.flow(), budget)?
        } else {
            unchanged_join_v1::can_reuse(self.flow(), other.flow(), budget)?
        };
        if unchanged {
            return self.share(budget);
        }
        // Equality without the checked dead/escape invariant is insufficient.
        // All other inputs retain the original meet and scratch preflight.
        let joined = self.flow().join(other.flow(), budget)?;
        Self::new(joined, budget)
    }

    pub(in super::super) fn into_working(self, budget: &mut Budget) -> Result<Flow> {
        self.same_budget(budget)?;
        budget.charge(1)?;
        let Self { payload, handle } = self;
        let result = match Rc::try_unwrap(payload) {
            Ok(Payload { flow, storage }) => {
                // No other retained entry can observe this Flow. Transfer it
                // back to the existing unreserved working-flow discipline.
                drop(storage);
                Ok(flow)
            }
            Err(payload) => payload.flow.clone_retained(budget),
        };
        drop(handle);
        result
    }
}
