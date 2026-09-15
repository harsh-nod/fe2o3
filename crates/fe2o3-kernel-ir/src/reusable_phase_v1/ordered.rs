//! Linear custody over the actual KIR function and its existing indexed CFG.
//! Facts below are verifier-local SSA ancestry, never source or borrow authority.
use super::*;
use crate::{BlockId, Function, IndexedControlFlow, Operation, OperationKind, Type,
    ExecutionCapabilityOperationV1 as Exec, ExecutionCapabilityRoleV1 as Role,
    Terminator, MAX_MODULE_BYTES_V1};
use ReusablePhaseOperationV1 as Phase;
#[path = "ordered/memory.rs"]
mod memory;
#[path = "ordered/read_region.rs"]
mod read_region;
#[path = "ordered/physical.rs"]
pub(super) mod physical;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReusablePhaseCheckLimitsV1 { pub work: usize, pub temporary_bytes: usize }
impl ReusablePhaseCheckLimitsV1 {
    pub const DEFAULT: Self = Self { work: 1_048_576, temporary_bytes: MAX_MODULE_BYTES_V1 };
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReusablePhaseCheckUsageV1 { pub work: usize, pub peak_temporary_bytes: usize }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReusablePhaseCheckErrorV1 {
    WorkLimit, StorageLimit, Overflow, Allocation, LocalContract, DuplicateValue,
    MissingProducer, NonDominatingUse, DuplicateConsumption, ConsumedValue,
    WrongOwner, WrongPhase, WrongStorage, WrongCompletion, WrongOccurrence,
    MissingEnd, Escape, UnsupportedPhaseConsumer, UnsupportedCycle,
}
type Error = ReusablePhaseCheckErrorV1;
type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Point { block: BlockId, operation: usize }
#[derive(Clone, Copy, Debug)]
struct Fact {
    owner: ValueId,
    allocation: Option<ValueId>,
    begin: Option<Point>,
    bind: Option<Point>,
    barrier: Option<Point>,
}
struct Value<'a> { id: ValueId, ty: &'a Type, at: Point, producer: usize, fact: Option<Fact>, consumed: Option<Point> }
struct Node<'a> { at: Point, op: &'a Operation }
struct Budget { limits: ReusablePhaseCheckLimitsV1, usage: ReusablePhaseCheckUsageV1, held: usize }
impl Budget {
    fn work(&mut self, n: usize) -> Result<()> {
        self.usage.work = self.usage.work.checked_add(n).ok_or(Error::Overflow)?;
        if self.usage.work > self.limits.work { return Err(Error::WorkLimit); }
        Ok(())
    }
    fn hold(&mut self, bytes: usize) -> Result<()> {
        self.held = self.held.checked_add(bytes).ok_or(Error::Overflow)?;
        if self.held > self.limits.temporary_bytes { return Err(Error::StorageLimit); }
        self.usage.peak_temporary_bytes = self.usage.peak_temporary_bytes.max(self.held);
        Ok(())
    }
    fn reserve<T>(&mut self, n: usize) -> Result<Vec<T>> {
        let requested = n.checked_mul(std::mem::size_of::<T>()).ok_or(Error::Overflow)?;
        self.hold(requested)?;
        let mut v = Vec::new();
        v.try_reserve_exact(n).map_err(|_| Error::Allocation)?;
        let actual = v.capacity().checked_mul(std::mem::size_of::<T>()).ok_or(Error::Overflow)?;
        self.hold(actual.checked_sub(requested).ok_or(Error::Overflow)?)?;
        Ok(v)
    }
    fn release_vec<T>(&mut self, v: Vec<T>) {
        self.held -= v.capacity() * std::mem::size_of::<T>();
        drop(v);
    }
}
fn index(values: &[Value<'_>], id: ValueId, b: &mut Budget) -> Result<usize> {
    b.work(1 + values.len().max(1).ilog2() as usize)?;
    values.binary_search_by_key(&id, |v| v.id).map_err(|_| Error::MissingProducer)
}
fn fact(values: &[Value<'_>], id: ValueId, b: &mut Budget) -> Result<Fact> {
    values[index(values, id, b)?].fact.ok_or(Error::MissingProducer)
}
fn operation<'a>(nodes: &'a [Node<'a>], at: Point, b: &mut Budget) -> Result<&'a Operation> {
    b.work(1 + nodes.len().max(1).ilog2() as usize)?;
    nodes.binary_search_by_key(&at, |n| n.at).map(|i| nodes[i].op).map_err(|_| Error::MissingProducer)
}
fn before(cfg: &IndexedControlFlow, a: Point, z: Point, b: &mut Budget) -> Result<bool> {
    b.work(1 + cfg.block_count().max(1).ilog2() as usize)?;
    Ok(if a.block == z.block { a.operation < z.operation } else { cfg.dominates(a.block, z.block) })
}
fn same_phase(a: Fact, z: Fact) -> Result<()> {
    if a.begin.is_none() || a.begin != z.begin || a.owner != z.owner { Err(Error::WrongPhase) } else { Ok(()) }
}
fn consume(values: &mut [Value<'_>], id: ValueId, at: Point, b: &mut Budget) -> Result<()> {
    let i = index(values, id, b)?;
    if values[i].consumed.replace(at).is_some() { return Err(Error::DuplicateConsumption); }
    Ok(())
}
fn put(values: &mut [Value<'_>], id: ValueId, f: Fact, b: &mut Budget) -> Result<()> {
    let i = index(values, id, b)?;
    if values[i].fact.replace(f).is_some() { return Err(Error::DuplicateValue); }
    Ok(())
}
fn phase<'a>(nodes: &'a [Node<'a>], f: Fact, b: &mut Budget) -> Result<&'a ReusablePhaseOpV1> {
    let OperationKind::ReusablePhase(p) = &operation(nodes, f.begin.ok_or(Error::WrongPhase)?, b)?.kind else {
        return Err(Error::WrongPhase);
    };
    if !matches!(p.operation, Phase::Begin { .. }) { return Err(Error::WrongPhase); }
    Ok(p)
}
fn recipe(p: &ReusablePhaseOpV1) -> Result<PhaseDefinedCallV1> {
    match p.source { PhaseOperationSourceV1::Defined(d) => Ok(d), _ => Err(Error::WrongOccurrence) }
}
fn source_matches_begin(p: &ReusablePhaseOpV1, begin: &ReusablePhaseOpV1) -> Result<()> {
    let Phase::Begin { wrapper, invoke, source_protocol, .. } = &begin.operation else { return Err(Error::WrongPhase); };
    let issue = recipe(begin)?;
    let expected = PhaseKeyV1::for_begin(issue.call).ok_or(Error::WrongOccurrence)?;
    let valid = match &p.source {
        PhaseOperationSourceV1::Defined(d) => d.call.same_expansion(issue.call)
            && d.call.source.occurrence.is_some_and(|o| o.caller_instance() == invoke.call.callee_instance),
        PhaseOperationSourceV1::ClosureReturn { phase, closure, source_protocol: protocol, .. } =>
            *phase == expected && closure == invoke && protocol == source_protocol,
        PhaseOperationSourceV1::WrapperDrop { phase, drop_call, source_protocol: protocol, .. } =>
            *phase == expected && drop_call.same_expansion(issue.call) && protocol == source_protocol
                && drop_call.source().occurrence.is_some_and(|o| o.caller_instance() == wrapper.call.callee_instance),
        PhaseOperationSourceV1::LeaseEnd { phase, bind, source_event, source_protocol: protocol } =>
            *phase == expected && bind.same_expansion(issue.call) && protocol == source_protocol
                && source_event.instance == invoke.call.callee_instance,
        PhaseOperationSourceV1::WrapperEnd { phase, wrapper_normal_target, source_protocol: protocol } =>
            *phase == expected && *wrapper_normal_target == wrapper.call.expanded_normal_target && protocol == source_protocol,
    };
    if valid { Ok(()) } else { Err(Error::WrongOccurrence) }
}

/// Run after ordinary local KIR/CFG verification. No graph is rebuilt and no
/// phase type on a function/block parameter can serve as an issuer.
pub fn verify_reusable_phase_function_v1(function: &Function, cfg: &IndexedControlFlow,
    limits: ReusablePhaseCheckLimitsV1) -> Result<ReusablePhaseCheckUsageV1> {
    run(function, cfg, limits, |_, _, _| Ok(())).map(|(usage, ())| usage)
}

fn run<'a, T>(function: &'a Function, cfg: &IndexedControlFlow,
    limits: ReusablePhaseCheckLimitsV1,
    finish: impl FnOnce(&[Node<'a>], &[Value<'a>], &mut Budget) -> Result<T>,
) -> Result<(ReusablePhaseCheckUsageV1, T)> {
    let mut b = Budget { limits: ReusablePhaseCheckLimitsV1 {
        work: limits.work.min(ReusablePhaseCheckLimitsV1::DEFAULT.work),
        temporary_bytes: limits.temporary_bytes.min(ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes),
    }, usage: ReusablePhaseCheckUsageV1::default(), held: 0 };
    b.work(1)?;
    let Some(body) = &function.body else {
        let output = finish(&[], &[], &mut b)?;
        return Ok((b.usage, output));
    };
    let mut count = 0usize;
    let mut nodes_count = 0usize;
    let mut present = false;
    for block in &body.blocks {
        b.work(1 + block.parameters.len())?;
        if block.parameters.iter().any(|v| special(&v.ty)) { return Err(Error::Escape); }
        for op in &block.operations {
            b.work(1 + op.results.len())?;
            present |= matches!(op.kind, OperationKind::ReusablePhase(_));
            count = count.checked_add(op.results.len()).ok_or(Error::Overflow)?;
            nodes_count = nodes_count.checked_add(1).ok_or(Error::Overflow)?;
        }
    }
    b.work(function.signature.parameters.len() + function.signature.results.len())?;
    if function.signature.parameters.iter().chain(&function.signature.results).any(special) { return Err(Error::Escape); }
    if !present {
        if body.blocks.iter().flat_map(|v| &v.operations).flat_map(|v| &v.results).any(|v| special(&v.ty)) {
            return Err(Error::MissingProducer);
        }
        let output = finish(&[], &[], &mut b)?;
        return Ok((b.usage, output));
    }
    let mut values = b.reserve::<Value<'_>>(count)?;
    let mut nodes = b.reserve::<Node<'_>>(nodes_count)?;
    for block in &body.blocks {
        for (i, op) in block.operations.iter().enumerate() {
            b.work(1)?;
            let at = Point { block: block.id, operation: i };
            nodes.push(Node { at, op });
        }
    }
    sort_charge(nodes.len(), &mut b)?;
    nodes.sort_unstable_by_key(|n| n.at);
    for (producer,n) in nodes.iter().enumerate() {
        for v in &n.op.results {
            b.work(1)?;
            values.push(Value {id:v.id,ty:&v.ty,at:n.at,producer,fact:None,consumed:None});
        }
    }
    sort_charge(values.len(), &mut b)?;
    values.sort_unstable_by_key(|v|v.id);
    for pair in values.windows(2) { b.work(1)?; if pair[0].id==pair[1].id {return Err(Error::DuplicateValue);} }
    let order = schedule(&nodes, &values, cfg, &mut b)?;
    for i in order.iter().copied() { b.work(1)?; step(&nodes[i], &nodes, &mut values, &mut b)?; }
    b.release_vec(order);
    audit(function, cfg, &nodes, &values, &mut b)?;
    let output = finish(&nodes, &values, &mut b)?;
    b.release_vec(nodes);
    b.release_vec(values);
    Ok((b.usage, output))
}

fn special(t: &Type) -> bool {
    match t {
        Type::Pointer(p) => special(&p.pointee), Type::Slice(s) => special(&s.element),
        _ => matches!(t, Type::ReusablePhaseToken(_) | Type::ExecutionCapability(crate::ExecutionCapabilityTypeV1 {
            role: Role::ReusableWorkgroup | Role::ReusablePhaseCompletion, .. })),
    }
}

fn sort_charge(n:usize,b:&mut Budget)->Result<()> {
    b.work(n.checked_mul(1+n.max(1).ilog2() as usize).and_then(|n|n.checked_mul(4)).ok_or(Error::Overflow)?)
}
fn dependencies(op:&Operation)->&[ValueId] {
    match &op.kind {
        OperationKind::ReusablePhase(p)=>&p.operands,
        OperationKind::ExecutionCapability(e) if matches!(e.operation,
            Exec::LdsInitializeByInvocation {..} | Exec::LdsReadPublished {..})
            && e.operands.len() == 3 => &e.operands[..2],
        OperationKind::ExecutionCapability(e) if matches!(e.operation,Exec::LdsAllocate {..}
            |Exec::LdsAllocateBorrowed {..}|Exec::ReusableLdsConversion(_)|Exec::WorkgroupBarrier {..}
            |Exec::LdsPublish {..})=>&e.operands,
        _=>&[],
    }
}
fn schedule(nodes:&[Node<'_>],values:&[Value<'_>],cfg:&IndexedControlFlow,b:&mut Budget)->Result<Vec<usize>> {
    let n=nodes.len();
    let mut degrees=b.reserve::<usize>(n)?;degrees.resize(n,0);
    let mut offsets=b.reserve::<usize>(n.checked_add(1).ok_or(Error::Overflow)?)?;offsets.resize(n+1,0);
    for (to,node) in nodes.iter().enumerate() {
        b.work(1)?;
        if dependencies(node.op).len()>crate::MAX_EXECUTION_CAPABILITY_OPERANDS_V1 {return Err(Error::LocalContract);}
        for id in dependencies(node.op) {
            b.work(1)?;
            let v=&values[index(values,*id,b)?];
            if !before(cfg,v.at,node.at,b)? {return Err(Error::NonDominatingUse);}
            degrees[to]=degrees[to].checked_add(1).ok_or(Error::Overflow)?;
            offsets[v.producer+1]=offsets[v.producer+1].checked_add(1).ok_or(Error::Overflow)?;
        }
    }
    for i in 1..offsets.len() {b.work(1)?;offsets[i]=offsets[i].checked_add(offsets[i-1]).ok_or(Error::Overflow)?;}
    let mut edges=b.reserve::<usize>(offsets[n])?;edges.resize(offsets[n],0);
    let mut used=b.reserve::<usize>(n)?;used.resize(n,0);
    for (to,node) in nodes.iter().enumerate() {
        b.work(1)?;
        for id in dependencies(node.op) {
            b.work(1)?;let from=values[index(values,*id,b)?].producer;
            let slot=offsets[from].checked_add(used[from]).ok_or(Error::Overflow)?;
            if slot>=offsets[from+1] {return Err(Error::MissingProducer);}
            edges[slot]=to;used[from]+=1;
        }
    }
    for i in 0..n {b.work(1)?;if used[i]!=offsets[i+1]-offsets[i] {return Err(Error::MissingProducer);}}
    b.release_vec(used);
    let mut ready=b.reserve::<usize>(n)?;
    for (node,degree) in degrees.iter().enumerate() {b.work(1)?;if *degree==0 {ready.push(node);}}
    let mut cursor=0;
    while cursor<ready.len() {
        b.work(1)?;let from=ready[cursor];cursor+=1;
        for target in &edges[offsets[from]..offsets[from+1]] {
            b.work(1)?;
            degrees[*target]=degrees[*target].checked_sub(1).ok_or(Error::MissingProducer)?;
            if degrees[*target]==0 {
                if ready.len()>=n || ready.len()==ready.capacity() {return Err(Error::StorageLimit);}
                ready.push(*target);
            }
        }
    }
    if ready.len()!=n {return Err(Error::UnsupportedCycle);}
    b.release_vec(edges);b.release_vec(offsets);b.release_vec(degrees);
    Ok(ready)
}

fn step(n: &Node<'_>, nodes: &[Node<'_>], values: &mut [Value<'_>], b: &mut Budget) -> Result<()> {
    let OperationKind::ReusablePhase(p) = &n.op.kind else {
        let OperationKind::ExecutionCapability(e) = &n.op.kind else { return Ok(()); };
        let base = |owner| Fact { owner, allocation: None, begin: None, bind: None, barrier: None };
        match &e.operation {
            Exec::WorkgroupDerive { .. } => {
                if !e.is_complete() || n.op.results.len() != 1 { return Err(Error::LocalContract); }
                put(values, n.op.results[0].id, base(n.op.results[0].id), b)?;
            }
            Exec::LdsAllocate { .. } | Exec::LdsAllocateBorrowed { .. } => {
                if !e.is_complete() || e.operands.len() != 1 || n.op.results.len() != 1 { return Err(Error::LocalContract); }
                let f = fact(values, e.operands[0], b)?;
                if f.begin.is_some() { return Err(Error::UnsupportedPhaseConsumer); }
                put(values, n.op.results[0].id, Fact { allocation: Some(n.op.results[0].id), ..f }, b)?;
            }
            Exec::ReusableLdsConversion(_) => {
                if !e.is_complete() || e.operands.len() != 1 || n.op.results.len() != 1 { return Err(Error::LocalContract); }
                let f = fact(values, e.operands[0], b)?;
                if f.allocation.is_none() || f.begin.is_some() { return Err(Error::WrongStorage); }
                consume(values, e.operands[0], n.at, b)?;
                put(values, n.op.results[0].id, f, b)?;
            }
            Exec::LdsInitializeByInvocation { .. } | Exec::LdsPublish { .. }
            | Exec::LdsReadPublished { .. } => memory::step(n, e, nodes, values, b)?,
            Exec::WorkgroupBarrier { .. } => {
                if !e.is_complete() || e.operands.len() != 1 || n.op.results.len() != 1 { return Err(Error::LocalContract); }
                let f = fact(values, e.operands[0], b)?;
                if f.begin.is_none() { put(values, n.op.results[0].id, f, b)?; return Ok(()); }
                if f.barrier.is_some() { return Err(Error::WrongCompletion); }
                consume(values, e.operands[0], n.at, b)?;
                put(values, n.op.results[0].id, Fact { barrier: Some(n.at), ..f }, b)?;
            }
            _ => {}
        }
        return Ok(());
    };
    if p.operands.len() > crate::MAX_EXECUTION_CAPABILITY_OPERANDS_V1 { return Err(Error::LocalContract); }
    let mut input = [None; crate::MAX_EXECUTION_CAPABILITY_OPERANDS_V1];
    for (slot, id) in p.operands.iter().enumerate() { input[slot] = Some(values[index(values, *id, b)?].ty); }
    let mut types = b.reserve::<&Type>(p.operands.len())?;
    for t in &input[..p.operands.len()] { types.push(t.ok_or(Error::MissingProducer)?); }
    // The transition allocates at most16 result types, each with one bounded root string.
    let temporary = crate::MAX_EXECUTION_CAPABILITY_RESULTS_V1
        .checked_mul(std::mem::size_of::<Type>() + 256).ok_or(Error::Overflow)?;
    b.hold(temporary)?;
    let expected = p.checked_result_types(&types).ok_or(Error::LocalContract)?;
    let actual = expected.capacity().checked_mul(std::mem::size_of::<Type>()).ok_or(Error::Overflow)?
        .checked_add(expected.iter().map(|t| match t {
            Type::ExecutionCapability(c) => c.provenance.root.retained_capacity_bytes(),
            Type::ReusablePhaseToken(t) => t.provenance.root.retained_capacity_bytes(), _ => 0,
        }).sum::<usize>()).ok_or(Error::Overflow)?;
    let extra = actual.saturating_sub(temporary);
    b.hold(extra)?;
    b.work(expected.len())?;
    if !n.op.results.iter().map(|r| &r.ty).eq(expected.iter()) { return Err(Error::LocalContract); }
    drop(expected);
    b.held -= temporary + extra;
    b.release_vec(types);
    let f = fact(values, p.operands[0], b)?;
    let ids = &p.operands;
    let out = &n.op.results;
    match &p.operation {
        Phase::OwnerConvert { .. } => {
            if f.begin.is_some() || f.allocation.is_some() { return Err(Error::WrongOwner); }
            consume(values, ids[0], n.at, b)?;
            put(values, out[0].id, f, b)?;
        }
        Phase::Begin { .. } => {
            if f.begin.is_some() || f.allocation.is_some() { return Err(Error::WrongOwner); }
            let call = recipe(p)?.call;
            for prior in nodes {
                b.work(1)?;
                if prior.at == n.at { continue; }
                if let OperationKind::ReusablePhase(other) = &prior.op.kind {
                    if matches!(other.operation, Phase::Begin { .. }) && recipe(other)?.call == call { return Err(Error::WrongOccurrence); }
                }
            }
            consume(values, ids[0], n.at, b)?;
            for r in out { put(values, r.id, Fact { begin: Some(n.at), ..f }, b)?; }
        }
        Phase::Bind { .. } => {
            source_matches_begin(p, phase(nodes, f, b)?)?;
            same_phase(f, fact(values, ids[1], b)?)?;
            let s = fact(values, ids[2], b)?;
            if s.begin.is_some() || s.owner != f.owner || s.allocation.is_none() { return Err(Error::WrongStorage); }
            consume(values, ids[0], n.at, b)?;
            consume(values, ids[2], n.at, b)?;
            put(values, out[0].id, f, b)?;
            for r in &out[1..] { put(values, r.id, Fact { allocation: s.allocation, bind: Some(n.at), ..f }, b)?; }
        }
        Phase::Seal { workgroup_before_barrier, workgroup_after_barrier, barrier_call, .. } => {
            source_matches_begin(p, phase(nodes, f, b)?)?;
            let z = fact(values, ids[1], b)?;
            same_phase(f, z)?;
            let barrier_at = z.barrier.ok_or(Error::WrongCompletion)?;
            let OperationKind::ExecutionCapability(barrier) = &operation(nodes, barrier_at, b)?.kind else {
                return Err(Error::WrongCompletion);
            };
            let Exec::WorkgroupBarrier { input_workgroup, output_workgroup, semantics } = barrier.operation else { return Err(Error::WrongCompletion); };
            if barrier.source != barrier_call.source || input_workgroup != *workgroup_before_barrier
                || barrier_call.source.occurrence.is_none_or(|o| o.expanded_block() != barrier_at.block.0)
                || barrier_call.expanded_normal_target != n.at.block.0
                || output_workgroup != *workgroup_after_barrier
                || semantics.scope != crate::ExecutionMemoryScopeV1::Workgroup
                || semantics.ordering != crate::ExecutionMemoryOrderingV1::AcquireRelease
                || semantics.spaces != crate::ExecutionMemorySpacesV1::Workgroup { return Err(Error::WrongCompletion); }
            consume(values, ids[0], n.at, b)?;
            consume(values, ids[1], n.at, b)?;
            for r in out { put(values, r.id, z, b)?; }
        }
        Phase::RelayClosure { .. } => {
            source_matches_begin(p, phase(nodes, f, b)?)?;
            let producer = operation(nodes, values[index(values, ids[0], b)?].at, b)?;
            if !matches!(producer.kind, OperationKind::ReusablePhase(ReusablePhaseOpV1 { operation: Phase::Seal { .. }, .. })) {
                return Err(Error::WrongCompletion);
            }
            consume(values, ids[0], n.at, b)?;
            put(values, out[0].id, f, b)?;
        }
        Phase::RelayDrop { .. } => {
            source_matches_begin(p, phase(nodes, f, b)?)?;
            let c = fact(values, ids[1], b)?;
            same_phase(f, c)?;
            let producer = operation(nodes, values[index(values, ids[1], b)?].at, b)?;
            if f.barrier.is_none() || f.barrier != c.barrier
                || !matches!(producer.kind, OperationKind::ReusablePhase(ReusablePhaseOpV1 { operation: Phase::RelayClosure { .. }, .. })) {
                return Err(Error::WrongCompletion);
            }
            for id in ids { consume(values, *id, n.at, b)?; }
            for r in out { put(values, r.id, f, b)?; }
        }
        Phase::CloseStorage { .. } => {
            source_matches_begin(p, phase(nodes, f, b)?)?;
            let l = fact(values, ids[1], b)?;
            same_phase(f, l)?;
            if f.bind.is_none() || f.bind != l.bind || f.allocation != l.allocation { return Err(Error::WrongStorage); }
            let PhaseOperationSourceV1::LeaseEnd { bind, .. } = p.source else { return Err(Error::WrongOccurrence); };
            let OperationKind::ReusablePhase(original) = &operation(nodes, f.bind.ok_or(Error::WrongStorage)?, b)?.kind else {
                return Err(Error::WrongStorage);
            };
            if recipe(original)?.call != bind { return Err(Error::WrongOccurrence); }
            for id in ids { consume(values, *id, n.at, b)?; }
            put(values, out[0].id, f, b)?;
        }
        Phase::End { storage_count } => {
            source_matches_begin(p, phase(nodes, f, b)?)?;
            let c = fact(values, ids[1], b)?;
            same_phase(f, c)?;
            if f.barrier.is_none() || f.barrier != c.barrier { return Err(Error::WrongCompletion); }
            let mut count = 0usize;
            // Follow the actual linear cursor backwards to enumerate every Bind;
            // no side table of active loans can create a missing End operand.
            let mut cursor = ids[0];
            loop {
                b.work(1)?;
                let producer = operation(nodes, values[index(values, cursor, b)?].at, b)?;
                let OperationKind::ReusablePhase(producer) = &producer.kind else { return Err(Error::WrongPhase); };
                match producer.operation {
                    Phase::Begin { .. } => break,
                    Phase::Bind { .. } => {
                        count = count.checked_add(1).ok_or(Error::Overflow)?;
                        let slot = usize::from(*storage_count).checked_sub(count).ok_or(Error::WrongStorage)?;
                        let closed = fact(values, ids[slot + 2], b)?;
                        same_phase(f, closed)?;
                        let producer_point = values[index(values, cursor, b)?].at;
                        if closed.bind != Some(producer_point) { return Err(Error::WrongStorage); }
                        cursor = producer.operands[0];
                    }
                    Phase::RelayDrop { .. } | Phase::Seal { .. } => cursor = producer.operands[0],
                    _ => return Err(Error::WrongPhase),
                }
            }
            if count != usize::from(*storage_count) { return Err(Error::WrongStorage); }
            for id in ids { consume(values, *id, n.at, b)?; }
            put(values, out[0].id, Fact { begin: None, bind: None, barrier: None, allocation: None, ..f }, b)?;
            for (id, result) in ids[2..].iter().zip(&out[1..]) {
                let s = fact(values, *id, b)?;
                put(values, result.id, Fact { begin: None, bind: None, barrier: None, ..s }, b)?;
            }
        }
    }
    Ok(())
}

fn audit(function: &Function, cfg: &IndexedControlFlow, nodes: &[Node<'_>], values: &[Value<'_>], b: &mut Budget) -> Result<()> {
    let mut consuming = b.reserve::<bool>(nodes.len())?;
    b.work(nodes.len())?;
    consuming.resize(nodes.len(), false);
    for v in values {
        b.work(1)?;
        if (special(v.ty) || v.fact.is_some_and(|f| f.begin.is_some())) && v.consumed.is_none()
            && !matches!(v.ty, Type::ExecutionCapability(c) if c.role == Role::ReusableWorkgroup) {
            return Err(Error::MissingEnd);
        }
        if matches!(v.ty, Type::ReusablePhaseToken(_)) && v.fact.is_none() { return Err(Error::MissingProducer); }
        if let Some(at) = v.consumed {
            b.work(1 + nodes.len().max(1).ilog2() as usize)?;
            let i = nodes.binary_search_by_key(&at, |n| n.at).map_err(|_| Error::MissingProducer)?;
            consuming[i] = true;
        }
    }
    for (node_index, n) in nodes.iter().enumerate() {
        b.work(1)?;
        let mut upper = 32usize;
        match &n.op.kind {
            OperationKind::Call { arguments, .. } => upper = upper.max(arguments.len()),
            OperationKind::InlineAssembly(a) => upper = upper.max(a.operands.len().checked_mul(2).ok_or(Error::Overflow)?),
            _ => {}
        }
        let held = upper.checked_mul(std::mem::size_of::<ValueId>()).ok_or(Error::Overflow)?;
        b.hold(held)?;
        let operands = n.op.operands();
        let actual = operands.capacity().checked_mul(std::mem::size_of::<ValueId>()).ok_or(Error::Overflow)?;
        b.hold(actual.saturating_sub(held))?;
        for id in &operands {
            b.work(1)?;
            let Some(v) = lookup_optional(values, *id, b)? else { continue; };
            if let Some(consumed) = v.consumed {
                if consumed != n.at && !before(cfg, n.at, consumed, b)? {
                    if !memory::is_shared_read(&n.op.kind)
                        || !before(cfg, v.at, n.at, b)?
                        || read_region::reaches(cfg, consumed, n.at, b)? {
                        return Err(Error::ConsumedValue);
                    }
                }
            }
            if v.fact.is_some_and(|f| f.begin.is_some()) || special(v.ty) {
                match &n.op.kind {
                    OperationKind::ReusablePhase(_) => {}
                    OperationKind::ExecutionCapability(e) if matches!(e.operation,
                        Exec::WorkgroupBarrier { .. } | Exec::LdsInitializeByInvocation { .. }
                        | Exec::LdsPublish { .. } | Exec::LdsReadPublished { .. }) => {}
                    _ => return Err(Error::UnsupportedPhaseConsumer),
                }
            }
        }
        drop(operands);
        b.held -= held.max(actual);
        // A shared-read loop preserves the same available SSA values. A loop
        // that repeats any consumption still needs the separate generation meet.
        if consuming[node_index] {
            no_generation_cycle(cfg, n.at.block, b)?;
        }
        if let OperationKind::ReusablePhase(p) = &n.op.kind {
            if matches!(p.operation, Phase::Begin { .. }) {
                no_generation_cycle(cfg, n.at.block, b)?;
            }
            if matches!(p.operation, Phase::End { .. }) {
                let f = fact(values, p.operands[0], b)?;
                read_region::closes_every_exit(cfg, f.begin.ok_or(Error::WrongPhase)?, n.at, b)?;
            }
        }
    }
    b.release_vec(consuming);
    let body = function.body.as_ref().ok_or(Error::MissingProducer)?;
    for block in &body.blocks {
        let mut check = |id| -> Result<()> {
            b.work(1)?;
            if let Some(v) = lookup_optional(values, id, b)? {
                if special(v.ty) || v.fact.is_some_and(|f| f.begin.is_some()) || v.consumed.is_some() { return Err(Error::Escape); }
            }
            Ok(())
        };
        match block.terminator.as_ref().ok_or(Error::LocalContract)? {
            Terminator::Branch { arguments, .. } | Terminator::Return { values: arguments } =>
                for v in arguments { check(*v)?; },
            Terminator::ConditionalBranch { condition, then_arguments, else_arguments, .. } => {
                check(*condition)?; for v in then_arguments.iter().chain(else_arguments) { check(*v)?; }
            }
            Terminator::Switch { selector, cases, default_arguments, .. } => {
                check(*selector)?; for c in cases { for v in &c.arguments { check(*v)?; } }
                for v in default_arguments { check(*v)?; }
            }
            Terminator::IntegerSwitch { selector, cases, default_arguments, .. } => {
                check(*selector)?; for c in cases { for v in &c.arguments { check(*v)?; } }
                for v in default_arguments { check(*v)?; }
            }
            Terminator::Unreachable => {}
        }
    }
    Ok(())
}

fn no_generation_cycle(cfg: &IndexedControlFlow, begin: BlockId, b: &mut Budget) -> Result<()> {
    let mut seen = b.reserve::<bool>(cfg.block_count())?;
    b.work(cfg.block_count())?;
    seen.resize(cfg.block_count(), false);
    let mut queue = b.reserve::<BlockId>(cfg.block_count())?;
    queue.push(begin);
    seen[cfg.block_position(begin).ok_or(Error::MissingProducer)?] = true;
    let mut next = 0;
    while let Some(block) = queue.get(next).copied() {
        next += 1;
        for edge in cfg.outgoing_edges(block).ok_or(Error::MissingProducer)? {
            b.work(1)?;
            let target = cfg.edge_target(edge).ok_or(Error::MissingProducer)?;
            if target == begin { return Err(Error::UnsupportedCycle); }
            let index = cfg.block_position(target).ok_or(Error::MissingProducer)?;
            if !seen[index] {
                seen[index] = true;
                if queue.len() == queue.capacity() { return Err(Error::StorageLimit); }
                queue.push(target);
            }
        }
    }
    b.release_vec(queue);
    b.release_vec(seen);
    Ok(())
}
fn lookup_optional<'a, 'v>(values: &'a [Value<'v>], id: ValueId, b: &mut Budget) -> Result<Option<&'a Value<'v>>> {
    b.work(1+values.len().max(1).ilog2() as usize)?;
    Ok(values.binary_search_by_key(&id,|v|v.id).ok().map(|i|&values[i]))
}
