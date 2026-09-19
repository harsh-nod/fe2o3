// Included in occurrence capture: historical State/layout and charges stay intact.
struct RowAllocation<'a, 'w> {
    budget: Option<&'a mut Budget<'w>>,
}
impl<'a, 'w> RowAllocation<'a, 'w> {
    fn legacy() -> Self {
        Self { budget: None }
    }
    fn metered(budget: &'a mut Budget<'w>) -> Result<Self> {
        budget.reserve_storage(size_of::<KirNeutralOccurrenceRowsV1>())?;
        Ok(Self {
            budget: Some(budget),
        })
    }
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>> {
        match self.budget.as_deref_mut() {
            None => vector(count),
            Some(budget) => {
                crate::commutative_cse_owner_v1::resources::vector(count, budget).map_err(E::from)
            }
        }
    }
    fn descendants(&mut self, count: usize) -> Result<Vec<Descendant>> {
        if self.budget.is_some() {
            self.vector(count)
        } else {
            Ok(Vec::new())
        }
    }
    fn work(&mut self, count: usize) -> Result<()> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.charge_work(count)?;
        }
        Ok(())
    }
    fn release(&mut self, bytes: usize) -> Result<()> {
        self.budget
            .as_deref_mut()
            .ok_or(E::Coverage)?
            .release_storage(bytes)?;
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct CommutativeCapture(Arc<CommutativeShared>);
struct CommutativeShared {
    inner: Mutex<CommutativeState>,
    poisoned: std::sync::atomic::AtomicBool,
}
struct CommutativeState {
    state: State,
    next: Vec<usize>,
    inputs: Vec<Option<usize>>,
    finished: bool,
    #[cfg(test)]
    fault: u8,
    #[cfg(test)]
    erased: bool,
    #[cfg(test)]
    actual_mutation_seen: bool,
}
const NO_VALUE: usize = usize::MAX;
#[cfg(test)]
std::thread_local! { static ACTUAL_COMMUTATIVE_MUTATION: Cell<bool> = const { Cell::new(false) }; }

impl CommutativeCapture {
    pub(crate) fn new(
        ctx: &Context,
        root: Ptr<Operation>,
        source: &Module,
        roster: &LiveRosterV12,
        limits: Limits,
        roster_work: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        // State/Arc retain the inherited opaque logical capture envelope. New
        // lineage vectors are separately prepaid at their actual capacities.
        let state = State::new(ctx, root, source, roster, limits, roster_work)?;
        let mut allocation = RowAllocation {
            budget: Some(budget),
        };
        let mut next = allocation.vector(state.values.rows.len())?;
        allocation.work(state.values.rows.len())?;
        next.resize(state.values.rows.len(), NO_VALUE);
        let mut inputs = allocation.vector(state.inputs.len())?;
        allocation.work(state.inputs.len())?;
        let mut cursor = 0;
        for &input in &state.inputs {
            let declared = match input {
                Definition::FunctionArgument { function, argument } => {
                    let f = state
                        .functions
                        .get(function.0 as usize)
                        .ok_or(E::Coverage)?;
                    if f.live.is_none() {
                        if argument as usize >= f.parameters {
                            return Err(E::Coverage);
                        }
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if declared {
                inputs.push(None);
                continue;
            }
            if state.values.rows.get(cursor).and_then(|row| row.input) != Some(input) {
                return Err(E::Coverage);
            }
            inputs.push(Some(cursor));
            cursor += 1;
        }
        if cursor != state.values.rows.len() {
            return Err(E::Coverage);
        }
        Ok(Self(Arc::new(CommutativeShared {
            inner: Mutex::new(CommutativeState {
                state,
                next,
                inputs,
                finished: false,
                #[cfg(test)]
                fault: 0,
                #[cfg(test)]
                erased: false,
                #[cfg(test)]
                actual_mutation_seen: false,
            }),
            poisoned: std::sync::atomic::AtomicBool::new(false),
        })))
    }
    fn apply(&self, run: impl FnOnce(&mut CommutativeState) -> Result<()>) -> bool {
        use std::sync::atomic::Ordering;
        if self.0.poisoned.load(Ordering::Relaxed) {
            return false;
        }
        let Ok(mut state) = self.0.inner.try_lock() else {
            self.0.poisoned.store(true, Ordering::Relaxed);
            return false;
        };
        if state.state.failure.is_some() {
            return false;
        }
        if let Err(error) = run(&mut state) {
            state.state.failure = Some(error);
            return false;
        }
        true
    }
    pub(crate) fn failure(&self) -> Option<E> {
        if self.0.poisoned.load(std::sync::atomic::Ordering::Relaxed) {
            return Some(E::Lifecycle);
        }
        self.0
            .inner
            .try_lock()
            .map_or(Some(E::Lifecycle), |s| s.state.failure.clone())
    }
    pub(crate) fn begin(&self, epoch: OperationGraphEpochV1) -> bool {
        self.apply(|s| {
            if s.finished || s.state.current.is_some() || s.state.completed != 0 {
                return Err(E::Passes);
            }
            s.state.current = Some((0, epoch.sequence()));
            Ok(())
        })
    }
    pub(crate) fn end(&self, ctx: &Context, epoch: OperationGraphEpochV1) -> bool {
        self.apply(|s| {
            let (pass, before) = s.state.current.ok_or(E::Passes)?;
            if pass != 0
                || s.state.completed != 0
                || s.finished
                || epoch.sequence() < before
                || epoch.sequence() - before > 1
            {
                return Err(E::Passes);
            }
            s.state.census(ctx, 2)?;
            s.state.current = None;
            s.state.completed = 1;
            s.state.epoch = Some(epoch.sequence());
            Ok(())
        })
    }
    pub(crate) fn with_roster_meter<T>(
        &self,
        run: impl FnOnce(&mut dyn FnMut(usize) -> Result<()>) -> Result<T>,
    ) -> Result<T> {
        if let Some(error) = self.failure() {
            return Err(error);
        }
        let mut s = self.0.inner.try_lock().map_err(|_| E::Lifecycle)?;
        let result = run(&mut |work| s.state.step(work));
        if let Err(error) = &result {
            s.state.failure = Some(error.clone());
        }
        result
    }
    pub(crate) fn observer(&self) -> Box<dyn RewriteObserver> {
        Box::new(self.clone())
    }
    pub(crate) fn lineage_storage(&self) -> Result<usize> {
        let state = self.0.inner.try_lock().map_err(|_| E::Lifecycle)?;
        state
            .next
            .capacity()
            .checked_mul(size_of::<usize>())
            .and_then(|n| {
                n.checked_add(
                    state
                        .inputs
                        .capacity()
                        .checked_mul(size_of::<Option<usize>>())?,
                )
            })
            .ok_or(E::Arithmetic)
    }
    pub(crate) fn finish(
        &self,
        ctx: &Context,
        roster: &LiveRosterV12,
        output: &Module,
        budget: &mut Budget<'_>,
    ) -> Result<KirNeutralOccurrenceRowsV1> {
        if let Some(error) = self.failure() {
            return Err(error);
        }
        let mut s = self.0.inner.try_lock().map_err(|_| E::Lifecycle)?;
        let result = (|| {
            if s.finished
                || s.state.completed != 1
                || s.state.current.is_some()
                || s.state.functions.len() != output.functions.len()
            {
                return Err(E::Passes);
            }
            s.finished = true;
            s.state.census(ctx, 3)?;
            let mut allocation = RowAllocation::metered(budget)?;
            let CommutativeState {
                state,
                next,
                inputs,
                ..
            } = &mut *s;
            assemble_occurrence_rows(
                state,
                ctx,
                roster,
                output,
                &mut allocation,
                |state, rows, allocation| {
                    commutative_definitions(state, next, inputs, roster, rows, allocation)
                },
            )
        })();
        if let Err(error) = &result {
            s.state.failure = Some(error.clone());
        }
        result
    }
    #[cfg(test)]
    pub(crate) fn set_fault(&self, fault: u8) {
        self.0.inner.lock().unwrap().fault = if fault <= 2 { fault } else { 0 };
        ACTUAL_COMMUTATIVE_MUTATION.set(false);
    }
    #[cfg(test)]
    pub(crate) fn actual_mutation_seen(&self) -> bool {
        self.0
            .inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .actual_mutation_seen
    }
    #[cfg(test)]
    pub(crate) fn last_actual_mutation_seen() -> bool {
        ACTUAL_COMMUTATIVE_MUTATION.get()
    }
    #[cfg(test)]
    pub(crate) fn tamper_lineage(&self, fault: u8) -> Result<()> {
        let mut s = self.0.inner.lock().unwrap();
        let next_len = s.next.len();
        s.state.step(next_len)?;
        let from = s
            .next
            .iter()
            .position(|target| *target != NO_VALUE)
            .ok_or(E::Coverage)?;
        match fault {
            3 => s.next[from] = NO_VALUE,
            4 => s.next[from] = from,
            5 => s.next[from] = s.next.len(),
            6 => s.finished = true,
            7 => {
                let values_len = s.state.values.rows.len();
                s.state.step(values_len)?;
                s.next[from] = s
                    .state
                    .values
                    .rows
                    .iter()
                    .position(|row| {
                        row.alive && matches!(row.input, Some(Definition::FunctionArgument { .. }))
                    })
                    .ok_or(E::Coverage)?;
            }
            _ => return Err(E::Coverage),
        }
        Ok(())
    }
}
impl RewriteObserver for CommutativeCapture {
    fn observe(&mut self, ctx: &Context, event: RewriteEvent) {
        self.apply(|s| {
            if s.state.current.is_none() || s.finished {
                return Err(E::Passes);
            }
            #[cfg(test)]
            if s.erased && s.fault != 0 {
                // This later event occurs after the first physical erase. A real
                // full census must agree with the retired row before the fault.
                s.state.census(ctx, 90)?;
                if !s.state.operations.rows.iter().any(|row| !row.alive) {
                    return Err(E::Coverage);
                }
                s.actual_mutation_seen = true;
                ACTUAL_COMMUTATIVE_MUTATION.set(true);
                if s.fault == 2 {
                    panic!("commutative observer after actual mutation");
                }
                return Err(E::Relation);
            }
            match event {
                RewriteEvent::ValueReplaced { old, new } => {
                    let from = s.state.value(old)?;
                    let to = s.state.value(new)?;
                    s.state.step(2)?;
                    if from == to || s.next[from] != NO_VALUE {
                        return Err(E::Relation);
                    }
                    s.next[from] = to;
                }
                RewriteEvent::OperationErased(raw) => {
                    let id = s.state.op(raw)?;
                    let row = &s.state.operations.rows[id];
                    if row.results.len() != 1 || s.next[row.results[0]] == NO_VALUE {
                        return Err(E::Coverage);
                    }
                    #[cfg(test)]
                    {
                        s.erased = true;
                    }
                }
                _ => return Err(E::UnsupportedMutation),
            }
            s.state.observe(ctx, event)
        });
    }
    fn observes_occurrences(&self) -> bool {
        true
    }
    fn observe_occurrence(&mut self, _: &Context, _: RewriteOccurrenceEvent) {
        self.apply(|_| Err(E::UnsupportedMutation));
    }
}

fn commutative_definitions(
    state: &mut State,
    next: &[usize],
    inputs: &[Option<usize>],
    roster: &LiveRosterV12,
    rows: &mut KirNeutralOccurrenceRowsV1,
    allocation: &mut RowAllocation<'_, '_>,
) -> Result<()> {
    let count = state.values.rows.len();
    if next.len() != count || inputs.len() != state.inputs.len() {
        return Err(E::Coverage);
    }
    let mut terminal = allocation.vector(count)?;
    let mut resolved = allocation.vector(count)?;
    let mut status = allocation.vector(count)?;
    let mut stack = allocation.vector(count)?;
    allocation.work(count.checked_mul(3).ok_or(E::Arithmetic)?)?;
    terminal.resize(count, None::<Definition>);
    resolved.resize(count, NO_VALUE);
    status.resize(count, 0u8);
    allocation.work(roster.len())?;
    for &(key, endpoint) in roster {
        if let LiveKeyV12::Value(value) = key {
            let id = state.value(value)?;
            if terminal[id].replace(definition(endpoint)?).is_some() {
                return Err(E::Coverage);
            }
        }
    }
    for source in 0..count {
        allocation.work(1)?;
        let mut cursor = source;
        while status[cursor] != 2 {
            allocation.work(1)?;
            if status[cursor] == 1 {
                return Err(E::Relation);
            }
            status[cursor] = 1;
            if stack.len() == count {
                return Err(E::Limit);
            }
            stack.push(cursor);
            if next[cursor] == NO_VALUE {
                if terminal[cursor].is_none() || !state.values.rows[cursor].alive {
                    return Err(E::Coverage);
                }
                resolved[cursor] = cursor;
                status[cursor] = 2;
                break;
            }
            if state.values.rows[cursor].alive || terminal[cursor].is_some() {
                return Err(E::Coverage);
            }
            cursor = next[cursor];
            if cursor >= count {
                return Err(E::Coverage);
            }
        }
        let target = resolved[cursor];
        while let Some(id) = stack.pop() {
            allocation.work(1)?;
            resolved[id] = target;
            status[id] = 2;
        }
    }
    allocation.work(state.inputs.len())?;
    for (ordinal, &input) in state.inputs.iter().enumerate() {
        let (output, retained) = match inputs[ordinal] {
            None => (input, true),
            Some(source) => {
                let target = resolved[source];
                (
                    terminal.get(target).copied().flatten().ok_or(E::Coverage)?,
                    source == target,
                )
            }
        };
        rows.definitions.push(DefinitionRow {
            input,
            outputs: Range {
                start: index(rows.definition_outputs.len())?,
                len: 1,
            },
        });
        rows.definition_outputs.push(Descendant {
            output,
            kind: if retained {
                DescendantKind::Retained
            } else {
                DescendantKind::Substituted
            },
        });
    }
    let scratch = terminal
        .capacity()
        .checked_mul(size_of::<Option<Definition>>())
        .and_then(|n| n.checked_add(resolved.capacity().checked_mul(size_of::<usize>())?))
        .and_then(|n| n.checked_add(status.capacity().checked_mul(size_of::<u8>())?))
        .and_then(|n| n.checked_add(stack.capacity().checked_mul(size_of::<usize>())?))
        .ok_or(E::Arithmetic)?;
    drop(terminal);
    drop(resolved);
    drop(status);
    drop(stack);
    allocation.release(scratch)?;
    Ok(())
}
