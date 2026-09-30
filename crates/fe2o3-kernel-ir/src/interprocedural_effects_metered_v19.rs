//! Byte-metered effects on the exact borrowed V18 owner. This does not discharge
//! memory accesses, prove source equivalence, or grant publication authority.

use super::{
    AssemblyTypeBudgetV1, MAX_INTERPROCEDURAL_ASSEMBLY_TYPE_WORK_V1,
    MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1, MAX_INTERPROCEDURAL_EFFECT_FUNCTIONS_V1,
    is_closed_u32_assembly_with_types_v30,
};
use crate::{
    AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, CompilerOrderingEffectSummaryV12, Function,
    FunctionId, FunctionOperationLocation, KirAddressSpacesRefV1,
    KirLocalMemoryEffectRefV1 as Effect, MemoryOrdering, OperationKind, SynchronizationScope, Type,
    ValueId, VerifiedCanonicalKernelIrModuleV18 as Owner,
    verification_typed_storage_v2::{allocate_vector_v2, prior_denial_v2, vector_bytes_v2},
};
use std::{
    cell::Cell,
    cmp::Ordering,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// A refusal of the original ledger or exact immutable owner/callable custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalEffectErrorV19 {
    /// The original shared ledger or allocator denied the next operation.
    Resource(Resource),
    /// A same-byte but different owner or function was supplied.
    ForeignOwner,
    /// The consumer explicitly rejected its result.
    Consumer,
    /// A callback or rejected captured value unwound.
    Panicked,
}
impl From<Resource> for CanonicalEffectErrorV19 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for CanonicalEffectErrorV19 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "canonical metered effects V19: {self:?}")
    }
}
impl std::error::Error for CanonicalEffectErrorV19 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::ForeignOwner | Self::Consumer | Self::Panicked => None,
        }
    }
}
type Error = CanonicalEffectErrorV19;
type Result<T> = std::result::Result<T, Error>;

/// The unchanged effect policy's incomplete reason, borrowing original names.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum CanonicalEffectReasonV19<'owner> {
    /// A callee is a declaration, not a known complete intrinsic.
    Declaration(&'owner FunctionId),
    /// This original function was already on the current DFS stack.
    Recursive(&'owner FunctionId),
    /// An assembly operation is outside the existing closed U32 effects policy.
    Assembly(&'owner FunctionId, FunctionOperationLocation),
    /// The unchanged function-count policy refused this module.
    FunctionLimit(usize),
    /// The unchanged cumulative call-edge policy refused the next call.
    CallLimit(usize),
    /// The unchanged assembly type-work policy refused this table.
    AssemblyTypeLimit(usize),
}

// Address-space sets are finite in the existing IR. Their exact five-bit key
// avoids copying the owner's BTreeSet payload into every transitive summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Key {
    Allocate(AddressSpace),
    Read(AddressSpace),
    Write(AddressSpace),
    VolatileRead(AddressSpace),
    VolatileWrite(AddressSpace),
    Atomic(AddressSpace, SynchronizationScope, MemoryOrdering),
    Synchronize(SynchronizationScope, SynchronizationScope, u8),
    Fence(SynchronizationScope, MemoryOrdering, u8),
}
#[derive(Clone, Copy)]
struct Row<'owner> {
    key: Key,
    effect: Effect<'owner>,
}

fn space_bit(space: AddressSpace) -> u8 {
    1 << match space {
        AddressSpace::Private => 0,
        AddressSpace::Workgroup => 1,
        AddressSpace::Global => 2,
        AddressSpace::Constant => 3,
        AddressSpace::Generic => 4,
    }
}
fn spaces(spaces: KirAddressSpacesRefV1<'_>, budget: &mut Budget<'_>) -> Result<u8> {
    match spaces {
        KirAddressSpacesRefV1::Singleton(space) => {
            budget.charge_work(1)?;
            Ok(space_bit(space))
        }
        KirAddressSpacesRefV1::Borrowed(spaces) => {
            budget.charge_work(spaces.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
            Ok(spaces
                .iter()
                .fold(0, |mask, &space| mask | space_bit(space)))
        }
    }
}
fn row<'owner>(effect: Effect<'owner>, budget: &mut Budget<'_>) -> Result<Row<'owner>> {
    budget.charge_work(1)?;
    let key = match effect {
        Effect::Allocate(s) => Key::Allocate(s),
        Effect::Read(s) => Key::Read(s),
        Effect::Write(s) => Key::Write(s),
        Effect::VolatileRead(s) => Key::VolatileRead(s),
        Effect::VolatileWrite(s) => Key::VolatileWrite(s),
        Effect::Atomic {
            address_space,
            scope,
            ordering,
        } => Key::Atomic(address_space, scope, ordering),
        Effect::Synchronize {
            execution_scope,
            memory_scope,
            address_spaces,
        } => Key::Synchronize(
            execution_scope,
            memory_scope,
            spaces(address_spaces, budget)?,
        ),
        Effect::Fence {
            memory_scope,
            ordering,
            address_spaces,
        } => Key::Fence(memory_scope, ordering, spaces(address_spaces, budget)?),
    };
    Ok(Row { key, effect })
}

struct Decision<'owner> {
    effects: Vec<Row<'owner>>,
    reasons: Vec<CanonicalEffectReasonV19<'owner>>,
    ordering: CompilerOrderingEffectSummaryV12,
    state: u8,
}
impl Decision<'_> {
    fn new() -> Self {
        Self {
            effects: Vec::new(),
            reasons: Vec::new(),
            ordering: CompilerOrderingEffectSummaryV12::empty(),
            state: 0,
        }
    }
}

/// A borrowed exact-owner summary. Incompleteness is never treated as purity.
/// Returned effect views borrow the original verified IR, not cloned payloads.
pub struct CanonicalEffectDecisionV19<'scope, 'owner> {
    decision: &'scope Decision<'owner>,
    function: &'owner Function,
}
impl<'scope, 'owner> CanonicalEffectDecisionV19<'scope, 'owner> {
    /// The actual function in the immutable owner that this summary describes.
    pub fn original_function(&self) -> &'owner Function {
        self.function
    }
    /// Whether the existing transitive effect policy completed this function.
    pub fn is_complete(&self) -> bool {
        self.decision.reasons.is_empty()
    }
    /// True only for a complete summary without memory or compiler-order effects.
    pub fn is_complete_and_pure(&self) -> bool {
        self.is_complete() && self.decision.effects.is_empty() && self.decision.ordering.is_empty()
    }
    /// All retained local/transitive physical effects, once per exact effect.
    pub fn effects(&self) -> impl Iterator<Item = Effect<'owner>> + '_ {
        self.decision.effects.iter().map(|row| row.effect)
    }
    /// Original incomplete reasons, deduplicated without cloning source names.
    pub fn incomplete_reasons(&self) -> &[CanonicalEffectReasonV19<'owner>] {
        &self.decision.reasons
    }
    /// Complete or partial compiler-order effects, preserving all three families.
    pub fn compiler_ordering(&self) -> CompilerOrderingEffectSummaryV12 {
        self.decision.ordering
    }
}

/// Paid summaries tied to one immutable owner and one live shared ledger slot.
/// No constructor accepts a detached effect report, bytes hash, or success flag.
pub struct CanonicalEffectScopeV19<'owner> {
    owner: &'owner Owner,
    by_name: Vec<usize>,
    decisions: Vec<Decision<'owner>>,
    slot: usize,
    ledger: Ledger,
    floor: usize,
    failure: Cell<Option<Error>>,
}
impl<'owner> CanonicalEffectScopeV19<'owner> {
    fn retain<T>(&self, value: Result<T>) -> Result<T> {
        if let Err(error) = value.as_ref() {
            if self.failure.get().is_none() {
                self.failure.set(Some(*error));
            }
        }
        value
    }
    fn check(&self, budget: &Budget<'_>) -> Result<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
        {
            // An unrelated account cannot mutate this scope's denial state.
            return Err(Resource::Accounting.into());
        }
        if let Some(error) = self.failure.get() {
            return Err(error);
        }
        self.retain(prior_denial_v2(budget).map_err(Into::into))?;
        if budget.storage() < self.floor {
            return self.retain(Err(Resource::Accounting.into()));
        }
        Ok(())
    }
    /// Queries an actual function reference in this exact owner. Equal names or
    /// equal bytes from a different module are not sufficient custody.
    pub fn function(
        &self,
        owner: &Owner,
        function: &Function,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalEffectDecisionV19<'_, 'owner>> {
        let decision = self.function_named(owner, &function.id, budget)?;
        self.retain(budget.charge_work(1).map_err(Into::into))?;
        if !std::ptr::eq(decision.function, function) {
            return self.retain(Err(Error::ForeignOwner));
        }
        Ok(decision)
    }
    /// Performs a paid name lookup in the exact owner and returns the actual
    /// matched function. The name alone is not proof of a caller's call edge;
    /// that join remains the original operation consumer's responsibility.
    pub fn function_named(
        &self,
        owner: &Owner,
        name: &FunctionId,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalEffectDecisionV19<'_, 'owner>> {
        self.check(budget)?;
        self.retain(budget.charge_work(1).map_err(Into::into))?;
        if !std::ptr::eq(self.owner, owner) {
            return self.retain(Err(Error::ForeignOwner));
        }
        let ordinal = self.retain(find_function(self.owner, &self.by_name, name, budget))?;
        Ok(CanonicalEffectDecisionV19 {
            decision: &self.decisions[ordinal],
            function: &self.owner.module().functions[ordinal],
        })
    }
}

fn drain<T>(value: T) -> bool {
    let mut outcome = catch_unwind(AssertUnwindSafe(|| drop(value)));
    let panicked = outcome.is_err();
    while let Err(payload) = outcome {
        outcome = catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
    panicked
}
fn selected<T>(result: Result<T>, error: Error) -> Result<T> {
    drain(result);
    Err(error)
}

/// Builds exact-owner interprocedural effects on the caller's cumulative work
/// and live typed-byte ledger. The V1 API/decision policy remains unchanged.
/// Captures, result envelopes, DFS frames, indexes and coexisting Vec capacities
/// are paid here; the original owner and consumer-owned output remain caller-paid.
/// A swallowed original-account query/resource denial is sticky; a foreign
/// budget is rejected without poisoning the original account. Scratch drops
/// before the original floor is restored; foreign slots/ledgers are never refunded.
pub fn with_canonical_effects_v19<'owner, 'work, T, F>(
    owner: &'owner Owner,
    budget: &mut Budget<'work>,
    consume: F,
) -> Result<T>
where
    F: for<'scope> FnOnce(&'scope CanonicalEffectScopeV19<'owner>, &mut Budget<'work>) -> Result<T>,
{
    let floor = budget.storage();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let mut pending = Some(consume);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        prior_denial_v2(budget)?;
        budget.charge_work(4)?;
        let headers = size_of::<CanonicalEffectScopeV19<'_>>()
            .checked_add(size_of::<Builder<'_>>())
            .and_then(|n| n.checked_add(size_of::<Result<CanonicalEffectDecisionV19<'_, '_>>>()))
            .and_then(|n| n.checked_add(size_of::<Option<F>>()))
            .and_then(|n| {
                n.checked_add(size_of::<std::thread::Result<Result<T>>>().checked_mul(2)?)
            })
            .and_then(|n| n.checked_add(size_of::<std::thread::Result<()>>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(headers)?;
        let (by_name, decisions) = Builder::build(owner, budget)?;
        let scope = CanonicalEffectScopeV19 {
            owner,
            by_name,
            decisions,
            slot,
            ledger,
            floor: budget.storage(),
            failure: Cell::new(None),
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            pending.take().expect("one effect consumer")(&scope, budget)
        }));
        let result = match result {
            Ok(result) => result,
            Err(payload) => {
                drain(payload);
                Err(Error::Panicked)
            }
        };
        let _ = scope.check(budget);
        let error = scope
            .failure
            .get()
            .or_else(|| result.as_ref().err().copied())
            .or_else(|| {
                (budget.storage() != scope.floor).then_some(Error::Resource(Resource::Accounting))
            });
        let result = match error {
            Some(error) => selected(result, error),
            None => result,
        };
        drop(scope);
        result
    }));
    let mut result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            let first = (ledger == budget.work_ledger_identity_v1())
                .then(|| prior_denial_v2(budget).err())
                .flatten();
            drain(payload);
            Err(first.map(Error::Resource).unwrap_or(Error::Panicked))
        }
    };
    if drain(pending) && result.is_ok() {
        result = selected(result, Error::Panicked);
    }
    if slot != std::ptr::from_ref(&*budget) as usize || ledger != budget.work_ledger_identity_v1() {
        return if result.is_err() {
            result
        } else {
            selected(result, Resource::Accounting.into())
        };
    }
    if result.is_ok() {
        if let Err(error) = prior_denial_v2(budget) {
            result = selected(result, error.into());
        }
    }
    let refund = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|amount| budget.release_storage(amount));
    if result.is_ok() {
        if let Err(error) = refund {
            result = selected(result, error.into());
        }
    }
    result
}

fn find_function(
    owner: &Owner,
    index: &[usize],
    name: &FunctionId,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    let mut low = 0;
    let mut high = index.len();
    while low < high {
        let middle = low + (high - low) / 2;
        let ordinal = index[middle];
        let candidate = &owner.module().functions[ordinal].id;
        budget.charge_work(
            candidate
                .as_str()
                .len()
                .min(name.as_str().len())
                .checked_add(2)
                .ok_or(Resource::Arithmetic)?,
        )?;
        match candidate.cmp(name) {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Ok(ordinal),
        }
    }
    Err(Error::ForeignOwner)
}

// Replacement buffers coexist until copying is finished; allocator excess is
// charged by allocate_vector_v2. Every owned element here is Copy/no-drop.
fn grow<T: Copy>(values: &mut Vec<T>, budget: &mut Budget<'_>) -> Result<()> {
    if values.len() < values.capacity() {
        return Ok(());
    }
    budget.charge_work(values.len())?;
    let requested = values
        .len()
        .checked_mul(2)
        .ok_or(Resource::Arithmetic)?
        .max(1);
    let mut replacement = allocate_vector_v2(requested, budget)?;
    replacement.extend_from_slice(values);
    let old = std::mem::replace(values, replacement);
    let old_bytes = vector_bytes_v2(&old)?;
    let old_allocated = old.capacity() != 0;
    drop(old);
    if old_allocated {
        budget.release_storage(old_bytes)?;
    }
    Ok(())
}
fn insert<T: Copy>(
    values: &mut Vec<T>,
    value: T,
    comparison_work: usize,
    compare: impl Fn(&T, &T) -> Ordering,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let mut low = 0;
    let mut high = values.len();
    while low < high {
        budget.charge_work(comparison_work)?;
        let middle = low + (high - low) / 2;
        match compare(&values[middle], &value) {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Ok(()),
        }
    }
    budget.charge_work(
        values
            .len()
            .checked_sub(low)
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    grow(values, budget)?;
    values.insert(low, value);
    Ok(())
}

fn merge_sorted<T: Copy>(
    into: &mut Vec<T>,
    from: &[T],
    comparison_work: usize,
    compare: impl Fn(&T, &T) -> Ordering,
    budget: &mut Budget<'_>,
) -> Result<()> {
    if from.is_empty() {
        return Ok(());
    }
    let capacity = into
        .len()
        .checked_add(from.len())
        .ok_or(Resource::Arithmetic)?;
    let mut result = allocate_vector_v2(capacity, budget)?;
    let (mut left, mut right) = (0, 0);
    while left < into.len() || right < from.len() {
        budget.charge_work(comparison_work.checked_add(1).ok_or(Resource::Arithmetic)?)?;
        if right == from.len()
            || (left < into.len() && compare(&into[left], &from[right]) == Ordering::Less)
        {
            result.push(into[left]);
            left += 1;
        } else if left == into.len() || compare(&into[left], &from[right]) == Ordering::Greater {
            result.push(from[right]);
            right += 1;
        } else {
            result.push(into[left]);
            left += 1;
            right += 1;
        }
    }
    let old = std::mem::replace(into, result);
    let allocated = old.capacity() != 0;
    let bytes = vector_bytes_v2(&old)?;
    drop(old);
    if allocated {
        budget.release_storage(bytes)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Frame {
    function: usize,
    block: usize,
    operation: usize,
    child: Option<usize>,
}
struct Builder<'owner> {
    owner: &'owner Owner,
    by_name: Vec<usize>,
    decisions: Vec<Decision<'owner>>,
    frames: Vec<Frame>,
    call_edges: usize,
    name_work: usize,
    assembly: AssemblyTypeBudgetV1,
}
impl<'owner> Builder<'owner> {
    fn build(
        owner: &'owner Owner,
        budget: &mut Budget<'_>,
    ) -> Result<(Vec<usize>, Vec<Decision<'owner>>)> {
        let functions = &owner.module().functions;
        budget.charge_work(functions.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
        let mut by_name = allocate_vector_v2(functions.len(), budget)?;
        let mut decisions = allocate_vector_v2(functions.len(), budget)?;
        let mut names = 0;
        for (ordinal, function) in functions.iter().enumerate() {
            budget.charge_work(
                function
                    .id
                    .as_str()
                    .len()
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            names = names.max(function.id.as_str().len());
            by_name.push(ordinal);
            decisions.push(Decision::new());
        }
        crate::verification_bounded_sort_by_v1(
            &mut by_name,
            names.checked_add(2).ok_or(Resource::Arithmetic)?,
            budget,
            |&a, &b| functions[a].id.cmp(&functions[b].id),
        )?;
        let frames = allocate_vector_v2(
            functions.len().min(MAX_INTERPROCEDURAL_EFFECT_FUNCTIONS_V1),
            budget,
        )?;
        let mut this = Self {
            owner,
            by_name,
            decisions,
            frames,
            call_edges: 0,
            name_work: names.checked_add(16).ok_or(Resource::Arithmetic)?,
            assembly: AssemblyTypeBudgetV1 {
                used: 0,
                limit: MAX_INTERPROCEDURAL_ASSEMBLY_TYPE_WORK_V1,
            },
        };
        for ordinal in 0..functions.len() {
            budget.charge_work(1)?;
            if functions.len() > MAX_INTERPROCEDURAL_EFFECT_FUNCTIONS_V1 {
                this.reason(
                    ordinal,
                    CanonicalEffectReasonV19::FunctionLimit(functions.len()),
                    budget,
                )?;
                this.decisions[ordinal].state = 2;
            } else if this.decisions[ordinal].state == 0 {
                this.enter(ordinal, budget)?;
                this.walk(budget)?;
            }
        }
        let frame_bytes = vector_bytes_v2(&this.frames)?;
        drop(this.frames);
        budget.release_storage(frame_bytes)?;
        Ok((this.by_name, this.decisions))
    }
    fn reason(
        &mut self,
        ordinal: usize,
        reason: CanonicalEffectReasonV19<'owner>,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        insert(
            &mut self.decisions[ordinal].reasons,
            reason,
            self.name_work,
            Ord::cmp,
            budget,
        )
    }
    fn effect(
        &mut self,
        ordinal: usize,
        effect: Row<'owner>,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        insert(
            &mut self.decisions[ordinal].effects,
            effect,
            8,
            |a, b| a.key.cmp(&b.key),
            budget,
        )
    }
    fn enter(&mut self, ordinal: usize, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(2)?;
        self.decisions[ordinal].state = 1;
        let function = &self.owner.module().functions[ordinal];
        if function.body.is_none() {
            self.reason(
                ordinal,
                CanonicalEffectReasonV19::Declaration(&function.id),
                budget,
            )?;
            self.decisions[ordinal].state = 2;
            return Ok(());
        }
        self.assembly(ordinal, budget)?;
        if self.frames.len() == self.frames.capacity() {
            return Err(Resource::Accounting.into());
        }
        self.frames.push(Frame {
            function: ordinal,
            block: 0,
            operation: 0,
            child: None,
        });
        Ok(())
    }
    fn merge(&mut self, into: usize, from: usize, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        if into == from {
            return Err(Resource::Accounting.into());
        }
        let (destination, source) = if into < from {
            let (lower, higher) = self.decisions.split_at_mut(from);
            (&mut lower[into], &higher[0])
        } else {
            let (lower, higher) = self.decisions.split_at_mut(into);
            (&mut higher[0], &lower[from])
        };
        destination.ordering = destination.ordering.union(source.ordering);
        merge_sorted(
            &mut destination.effects,
            &source.effects,
            16,
            |a, b| a.key.cmp(&b.key),
            budget,
        )?;
        merge_sorted(
            &mut destination.reasons,
            &source.reasons,
            self.name_work.checked_mul(2).ok_or(Resource::Arithmetic)?,
            Ord::cmp,
            budget,
        )?;
        Ok(())
    }
    fn walk(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        while let Some(frame) = self.frames.last().copied() {
            budget.charge_work(1)?;
            let function = &self.owner.module().functions[frame.function];
            let body = function.body.as_ref().expect("active definition");
            if let Some(child) = frame.child {
                self.merge(frame.function, child, budget)?;
                self.frames.last_mut().unwrap().child = None;
                continue;
            }
            let Some(block) = body.blocks.get(frame.block) else {
                self.decisions[frame.function].state = 2;
                self.frames.pop();
                continue;
            };
            let Some(operation) = block.operations.get(frame.operation) else {
                let top = self.frames.last_mut().unwrap();
                top.block += 1;
                top.operation = 0;
                continue;
            };
            self.frames.last_mut().unwrap().operation += 1;
            self.decisions[frame.function].ordering = self.decisions[frame.function]
                .ordering
                .union(operation.compiler_ordering_effects_v12());
            if let OperationKind::Call { callee, .. } = &operation.kind {
                if !operation.has_complete_effect_summary_with_budget_v1(budget)? {
                    self.call_edges = self
                        .call_edges
                        .saturating_add(1)
                        .min(MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1 + 1);
                    if self.call_edges > MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1 {
                        self.reason(
                            frame.function,
                            CanonicalEffectReasonV19::CallLimit(self.call_edges),
                            budget,
                        )?;
                        continue;
                    }
                    let child = find_function(self.owner, &self.by_name, callee, budget)?;
                    match self.decisions[child].state {
                        0 => {
                            self.frames.last_mut().unwrap().child = Some(child);
                            self.enter(child, budget)?;
                        }
                        1 => self.reason(
                            frame.function,
                            CanonicalEffectReasonV19::Recursive(
                                &self.owner.module().functions[child].id,
                            ),
                            budget,
                        )?,
                        _ => self.merge(frame.function, child, budget)?,
                    }
                    continue;
                }
            }
            if let OperationKind::InlineAssembly(assembly) = &operation.kind {
                budget.charge_work(assembly.declared_effects.len())?;
            }
            operation.try_visit_local_memory_effects_v1(|effect| {
                let row = row(effect, budget)?;
                self.effect(frame.function, row, budget)
            })?;
        }
        Ok(())
    }
    fn assembly(&mut self, ordinal: usize, budget: &mut Budget<'_>) -> Result<()> {
        let function = &self.owner.module().functions[ordinal];
        let body = function.body.as_ref().expect("definition");
        let mut found = false;
        for block in &body.blocks {
            budget.charge_work(1)?;
            for operation in &block.operations {
                budget.charge_work(1)?;
                if matches!(operation.kind, OperationKind::InlineAssembly(_)) {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        if !found {
            return Ok(());
        }
        let types = self.assembly_types(ordinal, budget)?;
        for block in &body.blocks {
            budget.charge_work(1)?;
            for (index, operation) in block.operations.iter().enumerate() {
                budget.charge_work(1)?;
                let OperationKind::InlineAssembly(assembly) = &operation.kind else {
                    continue;
                };
                let complete = if let Some(types) = &types {
                    let log =
                        usize::BITS as usize - types.len().max(1).leading_zeros() as usize + 1;
                    budget.charge_work(
                        assembly
                            .mnemonic
                            .len()
                            .checked_add(32)
                            .and_then(|n| n.checked_add(log.checked_mul(2)?))
                            .ok_or(Resource::Arithmetic)?,
                    )?;
                    is_closed_u32_assembly_with_types_v30(operation, |value| {
                        types
                            .binary_search_by_key(&value, |(id, _)| *id)
                            .ok()
                            .and_then(|i| types[i].1.as_scalar())
                    })
                } else {
                    false
                };
                if !complete {
                    budget.charge_work(1)?;
                    let reasons = &mut self.decisions[ordinal].reasons;
                    grow(reasons, budget)?;
                    reasons.push(CanonicalEffectReasonV19::Assembly(
                        &function.id,
                        FunctionOperationLocation::new(block.id, index),
                    ));
                }
            }
        }
        crate::verification_bounded_sort_by_v1(
            &mut self.decisions[ordinal].reasons,
            self.name_work,
            budget,
            Ord::cmp,
        )?;
        if let Some(types) = types {
            let bytes = vector_bytes_v2(&types)?;
            drop(types);
            budget.release_storage(bytes)?;
        }
        Ok(())
    }
    fn assembly_types(
        &mut self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<Vec<(ValueId, &'owner Type)>>> {
        let function = &self.owner.module().functions[ordinal];
        let body = function.body.as_ref().expect("definition");
        let census = (|| {
            budget.charge_work(2).map_err(|_| ())?;
            self.assembly
                .charge(body.parameters.len())
                .map_err(|_| ())?;
            self.assembly.charge(body.blocks.len()).map_err(|_| ())?;
            for block in &body.blocks {
                budget.charge_work(2).map_err(|_| ())?;
                self.assembly
                    .charge(block.parameters.len())
                    .map_err(|_| ())?;
                self.assembly
                    .charge(block.operations.len())
                    .map_err(|_| ())?;
                for operation in &block.operations {
                    budget.charge_work(1).map_err(|_| ())?;
                    self.assembly
                        .charge(operation.results.len())
                        .map_err(|_| ())?;
                }
            }
            Ok::<_, ()>(())
        })();
        if census.is_err() {
            prior_denial_v2(budget)?;
            self.reason(
                ordinal,
                CanonicalEffectReasonV19::AssemblyTypeLimit(self.assembly.used),
                budget,
            )?;
            return Ok(None);
        }
        let mut count = body.parameters.len();
        budget.charge_work(1)?;
        for block in &body.blocks {
            budget.charge_work(1)?;
            count = count
                .checked_add(block.parameters.len())
                .ok_or(Resource::Arithmetic)?;
            for operation in &block.operations {
                budget.charge_work(1)?;
                count = count
                    .checked_add(operation.results.len())
                    .ok_or(Resource::Arithmetic)?;
            }
        }
        let mut types = allocate_vector_v2(count, budget)?;
        budget.charge_work(count)?;
        types.extend(
            body.parameters
                .iter()
                .copied()
                .zip(&function.signature.parameters),
        );
        for block in &body.blocks {
            budget.charge_work(1)?;
            types.extend(block.parameters.iter().map(|value| (value.id, &value.ty)));
            for operation in &block.operations {
                budget.charge_work(1)?;
                types.extend(operation.results.iter().map(|value| (value.id, &value.ty)));
            }
        }
        crate::verification_bounded_sort_by_v1(&mut types, 1, budget, |a, b| a.0.cmp(&b.0))?;
        Ok(Some(types))
    }
}

#[cfg(test)]
#[path = "interprocedural_effects_metered_v19_tests.rs"]
mod tests;
