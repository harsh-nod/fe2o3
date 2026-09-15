//! Live ordered observation/result ports, not an initial-memory theorem.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Observation {
    producer: Ptr<Operation>,
    result: Value,
    view: Value,
    index: Value,
    guard: Value,
    fallback: Value,
    preceding: Option<Ptr<Operation>>,
}

/// Context-local Value IDs alone are not owner evidence: different arenas may
/// allocate identical IDs. Only the checked live sequence creates these ports.
#[derive(Clone, Copy)]
pub(in super::super) struct ReadResultPort<'a> {
    graph: &'a LiveEvents,
    ordinal: usize,
    producer: Ptr<Operation>,
    result: Value,
}

impl ReadResultPort<'_> {
    pub(in super::super) const fn value(&self) -> Value {
        self.result
    }
}

impl Observation {
    pub(crate) const fn producer(self) -> Ptr<Operation> {
        self.producer
    }
    pub(crate) const fn result(self) -> Value {
        self.result
    }
    pub(crate) const fn view(self) -> Value {
        self.view
    }
    pub(crate) const fn index(self) -> Value {
        self.index
    }
    pub(crate) const fn guard(self) -> Value {
        self.guard
    }
    pub(crate) const fn fallback(self) -> Value {
        self.fallback
    }
    /// Previous static observation, not a memory-content version. When a guard
    /// is false that event performs no read, and its scalar result is fallback.
    pub(crate) const fn preceding(self) -> Option<Ptr<Operation>> {
        self.preceding
    }
}

/// This borrowed sequence cannot escape the before/after live-graph replay.
/// Labels or scalar Values copied out of it confer no independent authority.
pub(crate) struct LiveReadSequence<'a> {
    graph: &'a LiveEvents,
    observations: &'a [Observation; 4],
}

impl LiveReadSequence<'_> {
    pub(crate) fn context(&self) -> &Context {
        &self.graph.context
    }
    pub(crate) fn function(&self) -> &FuncOp {
        &self.graph.function
    }
    pub(in super::super) fn result_ports(
        &self,
        charge: Charge<'_>,
    ) -> Result<[ReadResultPort<'_>; 4]> {
        formula::charge(
            charge,
            std::mem::size_of::<[ReadResultPort<'_>; 4]>().div_ceil(std::mem::size_of::<usize>())
                + 4,
        )?;
        Ok(std::array::from_fn(|ordinal| {
            let event = self.observations[ordinal];
            ReadResultPort {
                graph: self.graph,
                ordinal,
                producer: event.producer,
                result: event.result,
            }
        }))
    }
    pub(crate) fn observation(&self, ordinal: usize) -> Option<Observation> {
        self.observations.get(ordinal).copied()
    }

    /// Checks ports already belonging to this live function. This does NOT
    /// authenticate a different root function, an allocation, or an input map.
    pub(in super::super) fn check_result_ports(
        &self,
        results: &[ReadResultPort<'_>],
        charge: Charge<'_>,
    ) -> Result<()> {
        formula::charge(charge, 1)?;
        if results.len() != self.observations.len() {
            return Err(Error::Roster);
        }
        for (ordinal, (candidate, event)) in results.iter().zip(self.observations).enumerate() {
            formula::charge(charge, 5)?;
            if !std::ptr::eq(candidate.graph, self.graph)
                || candidate.ordinal != ordinal
                || candidate.producer != event.producer
                || candidate.result != event.result
                || candidate.result.defining_op() != Some(event.producer)
            {
                return Err(Error::Changed);
            }
        }
        Ok(())
    }
}

impl LiveEvents {
    pub(in super::super) fn with_live_read_sequence<T>(
        &self,
        expected: &Formula,
        charge: Charge<'_>,
        inspect: impl FnOnce(LiveReadSequence<'_>, Charge<'_>) -> Result<T>,
    ) -> Result<T> {
        self.verify(expected, charge)?;
        // Fixed local storage, not a new work owner or claimed process cap.
        formula::charge(
            charge,
            std::mem::size_of::<[Option<Observation>; 4]>().div_ceil(std::mem::size_of::<usize>())
                + 4,
        )?;
        let mut observations = [None; 4];
        let mut preceding = None;
        let mut count = 0;
        // Enumerate the actual function, never a caller's used-result list.
        // Thus unused/coincident reads remain distinct volatile observations.
        for pointer in self
            .function
            .get_entry_block(&self.context)
            .deref(&self.context)
            .iter(&self.context)
        {
            formula::charge(charge, 1)?;
            let op = Operation::get_op_dyn(pointer, &self.context);
            let Some(read) = op.downcast_ref::<SemanticTypedReadOp>() else {
                continue;
            };
            if count == observations.len() {
                return Err(Error::Roster);
            }
            formula::charge(charge, 16)?;
            let (guard, fallback) = read.guarded(&self.context).ok_or(Error::Changed)?;
            let indices = read.indices(&self.context).ok_or(Error::Changed)?;
            let [index] = indices.as_slice() else {
                return Err(Error::Changed);
            };
            let event = Observation {
                producer: pointer,
                result: read.result(&self.context),
                view: read.view(&self.context),
                index: *index,
                guard,
                fallback,
                preceding,
            };
            if event.result.defining_op() != Some(pointer)
                || event.view != self.view
                || read.volatility(&self.context) != Some(SemanticReadVolatilityAttr::Volatile)
                || read.ordering(&self.context) != Some(SemanticReadOrderingAttr::Unordered)
                || read.memory_space(&self.context) != Some(MemorySpaceAttr::Global)
            {
                return Err(Error::Changed);
            }
            observations[count] = Some(event);
            preceding = Some(pointer);
            count += 1;
        }
        if count != 4 {
            return Err(Error::Roster);
        }
        let [Some(a), Some(b), Some(c), Some(d)] = observations else {
            return Err(Error::Roster);
        };
        let observations = [a, b, c, d];
        let result = inspect(
            LiveReadSequence {
                graph: self,
                observations: &observations,
            },
            charge,
        );
        // Read-only Context APIs can still replace operands. Reject mutation
        // even when the inspector itself also returned an error.
        self.verify(expected, charge)?;
        result
    }
}

#[cfg(test)]
#[path = "read_observation_tests.rs"]
mod tests;
