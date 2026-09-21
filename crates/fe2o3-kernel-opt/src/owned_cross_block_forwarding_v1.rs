//! One owning cross-block private Store-to-Load pass, without policy authority.
use crate::private_cell_promotion_resources_v1 as resources;
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingErrorV1 as PairError,
    CanonicalKirCrossBlockForwardingLimitsV1 as Limits,
    CanonicalKirCrossBlockForwardingOriginV1 as Row,
    CanonicalKirCrossBlockForwardingStorageV1 as PairStorage,
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirMemorySsaErrorV1 as MemoryError, CanonicalKirMemorySsaInputSourceV1 as Input,
    CanonicalKirMemorySsaNodeIdV1 as NodeId, CanonicalKirMemorySsaNodeV1 as Node,
    CanonicalKirMemorySsaV1 as Memory, CheckedCanonicalKirCrossBlockForwardingV1 as Pair,
    check_canonical_kir_cross_block_forwarding_v1,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, CanonicalKernelIrReplayAdmissionErrorV12 as AdmissionError,
    CanonicalKernelIrReplayStorageV12 as OutputStorage,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirControlFlowScopeErrorV1 as FlowError,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirOperationCoordinateV1 as Site,
    KirLocalMemoryEffectRefV1 as Effect, MemoryAccess, OperationKind as Kind, ScalarType, Type,
    UnaryOp, ValueId, VerifiedCanonicalKernelIrIdentityV12 as Identity,
    VerifiedCanonicalKernelIrModuleV12 as Owner, with_canonical_kir_control_flow_v1,
};
use std::{fmt, mem::size_of};

/// A failed required owning phase discards its private candidate.
#[derive(Debug)]
pub enum OwnedCrossBlockForwardingErrorV1 {
    /// Cumulative work, allocation, storage or arithmetic refusal.
    Resource(Resource),
    /// Exact input inventory failed.
    Inventory(InventoryError),
    /// Complete input memory-version construction failed.
    Memory(MemoryError),
    /// Actual owner-bound dominance failed.
    ControlFlow(FlowError),
    /// Fresh actual output admission failed.
    Admission(AdmissionError),
    /// Independent actual input/output relation failed.
    Pair(PairError),
    /// A dense index, bounded queue or exact recipe invariant failed.
    Recipe(&'static str),
    /// The supplied replay input is another complete canonical subject.
    ForeignInput,
    /// Scoped partial candidate construction unwound.
    Panicked,
}
type Error = OwnedCrossBlockForwardingErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl From<Resource> for Error {
    fn from(v: Resource) -> Self {
        Self::Resource(v)
    }
}
impl From<InventoryError> for Error {
    fn from(v: InventoryError) -> Self {
        Self::Inventory(v)
    }
}
impl From<MemoryError> for Error {
    fn from(v: MemoryError) -> Self {
        Self::Memory(v)
    }
}
impl From<FlowError> for Error {
    fn from(v: FlowError) -> Self {
        Self::ControlFlow(v)
    }
}
impl From<AdmissionError> for Error {
    fn from(v: AdmissionError) -> Self {
        Self::Admission(v)
    }
}
impl From<PairError> for Error {
    fn from(v: PairError) -> Self {
        Self::Pair(v)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "owning cross-block private forwarding: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Actual freshly admitted output, complete lineage and fixed analysis limits.
/// The input identity is not source custody; an enclosing source continuation
/// must retain its actual source owner and replay the entire chain.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::OwnedCrossBlockForwardingV1;
/// fn duplicate(v: &OwnedCrossBlockForwardingV1) -> OwnedCrossBlockForwardingV1 { v.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_kernel_opt::OwnedCrossBlockForwardingV1;
/// fn detach(v: OwnedCrossBlockForwardingV1) { let _ = v.output; }
/// ```
pub struct OwnedCrossBlockForwardingV1 {
    output: Owner,
    output_storage: OutputStorage,
    input_identity: Identity,
    origins: Vec<Row>,
    limits: Limits,
    retained: usize,
}
impl OwnedCrossBlockForwardingV1 {
    /// Actual immutable final graph.
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    /// Complete input canonical identity, not a replay owner.
    pub const fn input_identity(&self) -> &Identity {
        &self.input_identity
    }
    /// Complete original-order operation correspondence.
    pub fn origins(&self) -> &[Row] {
        &self.origins
    }
    /// Exact analysis settings used again by replay.
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    /// Unreserved output/header/actual-origin-capacity owning addition.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// This component grants no source, native, artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Requires the owning addition live and checks both actual endpoints anew.
    pub fn replay_against<'a>(
        &'a self,
        input: &'a Owner,
        budget: &mut Budget<'_>,
    ) -> Result<(Pair<'a>, PairStorage)> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| {
            meter.work(
                size_of::<Identity>()
                    .checked_add(3)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if self.retained != retained(self.output_storage, &self.origins)? {
                return Err(Resource::Accounting.into());
            }
            if input.canonical().identity() != &self.input_identity {
                return Err(Error::ForeignInput);
            }
            meter.derive(|b| {
                Ok(check_canonical_kir_cross_block_forwarding_v1(
                    input,
                    &self.output,
                    &self.origins,
                    self.limits,
                    b,
                )?)
            })
        })
    }
}

/// Replaces eligible cross-block private integer Loads at their original sites.
/// Every other operation, Store, allocation, ValueId and CFG edge stays exact.
/// Phi consensus includes all syntactic incoming occurrences; unresolved cycles
/// become Unknown. Other Defs are terminal clobbers. Non-total scalar intervals
/// and each prior Load are conservative cuts, not a claim of instruction motion.
///
/// Added selection is O(B+O+D+U+M+I) work and scratch, beyond existing inventory,
/// MemorySSA, CFG, copy/admission and independent checking costs. A fixed FIFO
/// processes each phi's at most two lattice changes without predecessor rescans.
/// All buffers are prepaid before exact-floor CFG scopes. Success returns an
/// unreserved owning receipt; scoped failure/panic drops partial data before
/// valid-ledger cleanup and never refunds work, peak or first-denial history.
pub fn prepare_owned_cross_block_forwarding_v1(
    input: &Owner,
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<OwnedCrossBlockForwardingV1> {
    resources::scoped(budget, |meter| {
        meter.reserve(header()?)?;
        let (inventory, storage) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
        meter.reserve(storage.retained_storage())?;
        let (memory, storage) =
            meter.derive(|b| Ok(Memory::derive(&inventory, limits.memory, b)?))?;
        meter.reserve(storage.retained_storage())?;
        let origins = plan(&inventory, &memory, limits, meter)?;
        let (mut candidate, storage) =
            meter.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
        meter.reserve(storage.retained_storage())?;
        for row in &origins {
            meter.work(5)?;
            let Some(store) = row.store else {
                continue;
            };
            let store_at = operation_index(&inventory, store)?;
            let Kind::Store { value, .. } = inventory.operations()[store_at].operation.kind else {
                return Err(Error::Recipe("selected initializing Store"));
            };
            let body = candidate
                .functions
                .get_mut(row.output.block.function.0 as usize)
                .and_then(|f| f.body.as_mut())
                .ok_or(Error::Recipe("candidate function"))?;
            let operation = body
                .blocks
                .get_mut(row.output.block.block as usize)
                .and_then(|b| b.operations.get_mut(row.output.operation as usize))
                .ok_or(Error::Recipe("candidate operation"))?;
            operation.kind = Kind::Binary {
                op: BinaryOp::BitOr,
                lhs: value,
                rhs: value,
            };
        }
        let (output, output_storage) = meter.derive(|b| {
            Ok(Owner::from_module_ref_with_verification_budget_v12(
                &candidate, b,
            )?)
        })?;
        meter.reserve(output_storage.retained_storage())?;
        let pair_storage = {
            let (_pair, storage) = meter.derive(|b| {
                Ok(check_canonical_kir_cross_block_forwarding_v1(
                    input, &output, &origins, limits, b,
                )?)
            })?;
            meter.reserve(storage.retained_storage())?;
            storage
        };
        meter.release(pair_storage.retained_storage())?;
        let retained = retained(output_storage, &origins)?;
        // Candidate and all scratch reservations remain conservatively live
        // until their actual backing drops before the outer scoped cleanup.
        meter.work(1)?;
        Ok(OwnedCrossBlockForwardingV1 {
            output,
            output_storage,
            input_identity: *input.canonical().identity(),
            origins,
            limits,
            retained,
        })
    })
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Pending,
    Exact(usize),
    Unknown,
}
impl State {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Pending, value) | (value, Self::Pending) => value,
            (Self::Exact(a), Self::Exact(b)) if a == b => self,
            _ => Self::Unknown,
        }
    }
}
struct Queue {
    ring: Vec<usize>,
    present: Vec<bool>,
    head: usize,
    tail: usize,
    length: usize,
}
impl Queue {
    fn new(count: usize, meter: &mut Meter<'_, '_>) -> Result<Self> {
        Ok(Self {
            ring: filled(count, 0, meter)?,
            present: filled(count, false, meter)?,
            head: 0,
            tail: 0,
            length: 0,
        })
    }
    fn push(&mut self, node: usize, meter: &mut Meter<'_, '_>) -> Result<()> {
        meter.work(4)?;
        let present = self
            .present
            .get_mut(node)
            .ok_or(Error::Recipe("queue node bound"))?;
        if *present {
            return Ok(());
        }
        if self.length == self.ring.len() {
            return Err(Error::Recipe("fixed queue capacity"));
        }
        self.ring[self.tail] = node;
        self.tail = if self.tail + 1 == self.ring.len() {
            0
        } else {
            self.tail + 1
        };
        self.length += 1;
        *present = true;
        Ok(())
    }
    fn pop(&mut self, meter: &mut Meter<'_, '_>) -> Result<Option<usize>> {
        meter.work(3)?;
        if self.length == 0 {
            return Ok(None);
        }
        let node = self.ring[self.head];
        self.head = if self.head + 1 == self.ring.len() {
            0
        } else {
            self.head + 1
        };
        self.length -= 1;
        self.present[node] = false;
        Ok(Some(node))
    }
}
#[derive(Clone, Copy)]
struct Access {
    pointer: ValueId,
    definition: usize,
    access: MemoryAccess,
    stored: Option<(ValueId, usize)>,
}
#[derive(Clone, Copy)]
struct Candidate {
    store: usize,
    value_definition: usize,
}

fn plan(
    a: &Inventory<'_>,
    memory: &Memory<'_, '_>,
    limits: Limits,
    meter: &mut Meter<'_, '_>,
) -> Result<Vec<Row>> {
    meter.work(13)?;
    meter.reserve(plan_headers()?)?;
    let n = memory.node_count();
    let bound = a
        .edges()
        .len()
        .checked_add(a.functions().len())
        .ok_or(Resource::Arithmetic)?;
    let mut state = filled(n, State::Unknown, meter)?;
    let mut phis = filled(a.blocks().len(), None::<NodeId>, meter)?;
    let mut versions = filled(a.operations().len(), None::<NodeId>, meter)?;
    let mut local_cut = filled(a.operations().len(), false, meter)?;
    let mut exit_cut = filled(a.blocks().len(), false, meter)?;
    let (mut dependencies, _) = meter.table::<(usize, usize)>(bound)?;
    let mut counts = filled(n.checked_add(1).ok_or(Resource::Arithmetic)?, 0usize, meter)?;
    let mut cursors = filled(n, 0usize, meter)?;
    let mut reverse = filled(bound, usize::MAX, meter)?;
    let mut changes = filled(n, 0u8, meter)?;
    let mut queue = Queue::new(n, meter)?;
    let slots = slots(a, meter)?;
    let mut candidates = filled(a.operations().len(), None::<Candidate>, meter)?;
    let (mut origins, _) = meter.table::<Row>(a.operations().len())?;
    for row in a.operations() {
        meter.push(
            &mut origins,
            Row {
                input: row.coordinate,
                output: row.coordinate,
                store: None,
            },
        )?;
    }

    for (block_at, block) in a.blocks().iter().enumerate() {
        meter.work(4)?;
        let phi = meter.derive(|b| Ok(memory.block_entry(block.coordinate, b)?))?;
        if !matches!(meter.derive(|b| Ok(memory.node(phi,b)?.clone()))?, Node::Phi { block: owner, .. } if owner == block.coordinate)
        {
            return Err(Error::Recipe("exact block phi"));
        }
        phis[block_at] = Some(phi);
        state[phi.index()] = State::Pending;
        let mut cut = false;
        for at in block.operations.clone() {
            meter.work(4)?;
            local_cut[at] = cut;
            let node = meter.derive(|b| Ok(memory.operation(a.operations()[at].coordinate, b)?))?;
            if let Some(node) = node {
                match meter.derive(|b| Ok(memory.node(node, b)?.clone()))? {
                    Node::Def { operation, .. } if operation == a.operations()[at].coordinate => {
                        state[node.index()] = State::Exact(at);
                        cut = false;
                        continue;
                    }
                    Node::Use {
                        operation,
                        incoming,
                    } if operation == a.operations()[at].coordinate => {
                        versions[at] = Some(incoming);
                    }
                    _ => return Err(Error::Recipe("exact operation memory node")),
                }
            }
            if !transparent(a, at, meter)? {
                cut = true;
            }
        }
        exit_cut[block_at] = cut;
    }
    for (at, block) in a.blocks().iter().enumerate() {
        meter.work(2)?;
        let parent = phis[at].ok_or(Error::Recipe("block phi roster"))?;
        let inputs = meter.derive(|b| Ok(memory.phi_inputs(parent, b)?))?;
        for incoming in inputs {
            meter.work(5)?;
            let cut = match incoming.source() {
                Input::Entry(function) if function == block.coordinate.function => true,
                Input::Edge(edge) if edge.source.function == block.coordinate.function => {
                    exit_cut[block_index(a, edge.source)?]
                }
                _ => return Err(Error::Recipe("phi input function")),
            };
            if cut {
                state[parent.index()] = State::Unknown;
                continue;
            }
            let child = incoming.state();
            let valid = match meter.derive(|b| Ok(memory.node(child, b)?.clone()))? {
                Node::Phi { block: child, .. } => child.function == block.coordinate.function,
                Node::Def { operation, .. } => {
                    operation.block.function == block.coordinate.function
                }
                _ => false,
            };
            if !valid {
                return Err(Error::Recipe(
                    "phi dependency is a same-function Def or phi",
                ));
            }
            meter.push(&mut dependencies, (child.index(), parent.index()))?;
            counts[child.index() + 1] = counts[child.index() + 1]
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?;
        }
    }
    for i in 0..n {
        meter.work(3)?;
        counts[i + 1] = counts[i + 1]
            .checked_add(counts[i])
            .ok_or(Resource::Arithmetic)?;
        cursors[i] = counts[i];
    }
    if counts[n] != dependencies.len() {
        return Err(Error::Recipe("dependency cardinality"));
    }
    for (child, parent) in dependencies {
        meter.work(3)?;
        let at = cursors[child];
        if at >= counts[child + 1] || at >= reverse.len() {
            return Err(Error::Recipe("dependency range"));
        }
        reverse[at] = parent;
        cursors[child] = at.checked_add(1).ok_or(Resource::Arithmetic)?;
    }
    for (at, value) in state.iter().enumerate() {
        meter.work(1)?;
        if *value != State::Pending {
            queue.push(at, meter)?;
        }
    }
    settle(
        &mut state,
        &counts,
        &reverse,
        &mut changes,
        &mut queue,
        meter,
    )?;
    for (at, value) in state.iter_mut().enumerate() {
        meter.work(2)?;
        if *value == State::Pending {
            *value = State::Unknown;
            changes[at] = changes[at]
                .checked_add(1)
                .filter(|n| *n <= 2)
                .ok_or(Error::Recipe("finite phi height"))?;
            queue.push(at, meter)?;
        }
    }
    settle(
        &mut state,
        &counts,
        &reverse,
        &mut changes,
        &mut queue,
        meter,
    )?;
    for at in 0..a.operations().len() {
        meter.work(3)?;
        if local_cut[at] {
            continue;
        }
        let Some(node) = versions[at] else {
            continue;
        };
        let State::Exact(store_at) = state[node.index()] else {
            continue;
        };
        let Some(load) = access(a, at, &slots, meter)?.filter(|v| v.stored.is_none()) else {
            continue;
        };
        let Some(store) = access(a, store_at, &slots, meter)?.filter(|v| v.stored.is_some()) else {
            continue;
        };
        let (_, definition) = store.stored.ok_or(Error::Recipe("Store value"))?;
        if a.operations()[store_at].coordinate.block == a.operations()[at].coordinate.block
            || a.operations()[store_at].coordinate.block.function
                != a.operations()[at].coordinate.block.function
            || load.pointer != store.pointer
            || load.definition != store.definition
            || load.access != store.access
            || a.definitions()[definition].ty != &a.operations()[at].operation.results[0].ty
        {
            continue;
        }
        candidates[at] = Some(Candidate {
            store: store_at,
            value_definition: definition,
        });
    }
    // Only prepaid buffers are mutated inside exact-floor CFG callbacks.
    for function in a.functions() {
        meter.work(1)?;
        if function.function.body.is_none() {
            continue;
        }
        meter.derive(|budget| {
            with_canonical_kir_control_flow_v1(
                a.owner(),
                function.coordinate,
                limits.control_flow,
                budget,
                |flow, budget| {
                    for at in function.operations.clone() {
                        budget.charge_work(3)?;
                        let Some(candidate) = candidates[at] else {
                            continue;
                        };
                        let load = a.operations()[at].coordinate;
                        let store = a.operations()[candidate.store].coordinate;
                        if !flow.is_reachable(load.block, budget)?
                            || !flow.dominates(store.block, load.block, budget)?
                        {
                            continue;
                        }
                        let available = match a.definitions()[candidate.value_definition].coordinate
                        {
                            Definition::FunctionArgument { function: f, .. } => {
                                f == function.coordinate
                            }
                            Definition::BlockArgument { block, .. } => {
                                flow.dominates(block, load.block, budget)?
                            }
                            Definition::Result { operation, .. } => {
                                (operation.block != load.block
                                    || operation.operation < load.operation)
                                    && flow.dominates(operation.block, load.block, budget)?
                            }
                        };
                        if available {
                            origins[at].store = Some(store);
                        }
                    }
                    Ok(())
                },
            )
        })?;
    }
    Ok(origins)
}
fn plan_headers() -> Result<usize> {
    // Twelve temporary vector handles and the complete fixed FIFO. The retained
    // origins handle is already paid once by the owning continuation header.
    [
        size_of::<Vec<State>>(),
        size_of::<Vec<Option<NodeId>>>(),
        size_of::<Vec<Option<NodeId>>>(),
        size_of::<Vec<bool>>(),
        size_of::<Vec<bool>>(),
        size_of::<Vec<(usize, usize)>>(),
        size_of::<Vec<usize>>(),
        size_of::<Vec<usize>>(),
        size_of::<Vec<usize>>(),
        size_of::<Vec<u8>>(),
        size_of::<Vec<u32>>(),
        size_of::<Vec<Option<Candidate>>>(),
        size_of::<Queue>(),
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        total
            .checked_add(bytes)
            .ok_or_else(|| Resource::Arithmetic.into())
    })
}
fn settle(
    state: &mut [State],
    offsets: &[usize],
    parents: &[usize],
    changes: &mut [u8],
    queue: &mut Queue,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    while let Some(child) = queue.pop(meter)? {
        for parent in &parents[offsets[child]..offsets[child + 1]] {
            meter.work(5)?;
            let parent = *parent;
            if parent >= state.len() {
                return Err(Error::Recipe("reverse dependency parent"));
            }
            let value = state[parent].join(state[child]);
            if value != state[parent] {
                changes[parent] = changes[parent]
                    .checked_add(1)
                    .filter(|v| *v <= 2)
                    .ok_or(Error::Recipe("finite phi height"))?;
                state[parent] = value;
                queue.push(parent, meter)?;
            }
        }
    }
    Ok(())
}

fn filled<T: Clone>(count: usize, value: T, meter: &mut Meter<'_, '_>) -> Result<Vec<T>> {
    let (mut rows, _) = meter.table(count)?;
    meter.work(count)?;
    rows.resize(count, value);
    Ok(rows)
}
fn integer(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Scalar(
            ScalarType::I8
                | ScalarType::I16
                | ScalarType::I32
                | ScalarType::I64
                | ScalarType::U8
                | ScalarType::U16
                | ScalarType::U32
                | ScalarType::U64
        )
    )
}
fn scalar(ty: &Type) -> bool {
    *ty == Type::BOOL || integer(ty)
}
fn slots(a: &Inventory<'_>, meter: &mut Meter<'_, '_>) -> Result<Vec<u32>> {
    let mut alignments = filled(a.definitions().len(), 0u32, meter)?;
    for row in a.operations() {
        meter.work(7)?;
        let Kind::Alloca {
            element: Type::Scalar(element),
            count: None,
            address_space: AddressSpace::Private,
            alignment,
        } = &row.operation.kind
        else {
            continue;
        };
        if !integer(&Type::Scalar(*element))
            || !alignment.is_power_of_two()
            || *alignment
                < u32::from(
                    element
                        .bit_width()
                        .ok_or(Error::Recipe("integer cell width"))?
                        / 8,
                )
            || row.results.len() != 1
            || row.effects.len() != 1
            || !row.compiler_ordering().is_empty()
            || !matches!(
                a.effects()[row.effects.start].effect,
                Effect::Allocate(AddressSpace::Private)
            )
        {
            continue;
        }
        let Type::Pointer(pointer) = a.definitions()[row.results.start].ty else {
            continue;
        };
        if pointer.address_space == AddressSpace::Private
            && pointer.access == AccessMode::ReadWrite
            && *pointer.pointee == Type::Scalar(*element)
        {
            alignments[row.results.start] = *alignment;
        }
    }
    for row in a.operations() {
        meter.work(1)?;
        for operand in &a.uses()[row.operands.clone()] {
            meter.work(4)?;
            if alignments[operand.definition] == 0 {
                continue;
            }
            let permitted = match row.operation.kind {
                Kind::Load { pointer, access } => {
                    pointer == operand.value
                        && !access.volatile
                        && access.address_space == AddressSpace::Private
                }
                Kind::Store {
                    pointer,
                    value,
                    access,
                } => {
                    pointer == operand.value
                        && value != operand.value
                        && !access.volatile
                        && access.address_space == AddressSpace::Private
                }
                _ => false,
            };
            if !permitted {
                alignments[operand.definition] = 0;
            }
        }
    }
    for block in a.blocks() {
        meter.work(1)?;
        for operand in &a.uses()[block.terminator_uses.clone()] {
            meter.work(2)?;
            alignments[operand.definition] = 0;
        }
    }
    Ok(alignments)
}
fn access(
    a: &Inventory<'_>,
    at: usize,
    slots: &[u32],
    meter: &mut Meter<'_, '_>,
) -> Result<Option<Access>> {
    meter.work(10)?;
    let row = a
        .operations()
        .get(at)
        .ok_or(Error::Recipe("memory operation"))?;
    let (pointer, access, value) = match row.operation.kind {
        Kind::Load { pointer, access }
            if row.operands.len() == 1
                && row.results.len() == 1
                && integer(&row.operation.results[0].ty) =>
        {
            (pointer, access, None)
        }
        Kind::Store {
            pointer,
            value,
            access,
        } if row.operands.len() == 2 && row.results.is_empty() => (pointer, access, Some(value)),
        _ => return Ok(None),
    };
    if access.address_space != AddressSpace::Private
        || access.volatile
        || !access.alignment.is_power_of_two()
        || row.effects.len() != 1
        || !row.compiler_ordering().is_empty()
    {
        return Ok(None);
    }
    let uses = &a.uses()[row.operands.clone()];
    if uses[0].value != pointer {
        return Err(Error::Recipe("exact pointer operand"));
    }
    let definition = uses[0].definition;
    if slots[definition] == 0 || access.alignment > slots[definition] {
        return Ok(None);
    }
    let stored = if let Some(value) = value {
        if uses[1].value != value {
            return Err(Error::Recipe("exact stored operand"));
        }
        if !integer(a.definitions()[uses[1].definition].ty)
            || !matches!(
                a.effects()[row.effects.start].effect,
                Effect::Write(AddressSpace::Private)
            )
        {
            return Ok(None);
        }
        Some((value, uses[1].definition))
    } else {
        if !matches!(
            a.effects()[row.effects.start].effect,
            Effect::Read(AddressSpace::Private)
        ) {
            return Ok(None);
        }
        None
    };
    Ok(Some(Access {
        pointer,
        definition,
        access,
        stored,
    }))
}
fn transparent(a: &Inventory<'_>, at: usize, meter: &mut Meter<'_, '_>) -> Result<bool> {
    meter.work(5)?;
    let row = &a.operations()[at];
    if row.results.len() != 1
        || !scalar(&row.operation.results[0].ty)
        || !row.effects.is_empty()
        || !row.compiler_ordering().is_empty()
    {
        return Ok(false);
    }
    let allowed = match &row.operation.kind {
        Kind::Constant(value) => scalar(&value.ty()),
        Kind::Unary {
            op: UnaryOp::Not, ..
        }
        | Kind::Binary {
            op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
            ..
        }
        | Kind::Compare { .. }
        | Kind::Select { .. } => true,
        _ => false,
    };
    if !allowed {
        return Ok(false);
    }
    for operand in &a.uses()[row.operands.clone()] {
        meter.work(2)?;
        if !scalar(a.definitions()[operand.definition].ty) {
            return Ok(false);
        }
    }
    Ok(true)
}
fn block_index(a: &Inventory<'_>, block: Block) -> Result<usize> {
    let function = a
        .functions()
        .get(block.function.0 as usize)
        .ok_or(Error::Recipe("function coordinate"))?;
    function
        .blocks
        .start
        .checked_add(block.block as usize)
        .filter(|at| *at < function.blocks.end && a.blocks()[*at].coordinate == block)
        .ok_or(Error::Recipe("block coordinate"))
}
fn operation_index(a: &Inventory<'_>, site: Site) -> Result<usize> {
    let block = &a.blocks()[block_index(a, site.block)?];
    block
        .operations
        .start
        .checked_add(site.operation as usize)
        .filter(|at| *at < block.operations.end && a.operations()[*at].coordinate == site)
        .ok_or(Error::Recipe("operation coordinate"))
}
fn header() -> Result<usize> {
    size_of::<OwnedCrossBlockForwardingV1>()
        .checked_sub(size_of::<Owner>())
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn retained(output: OutputStorage, rows: &Vec<Row>) -> Result<usize> {
    header()?
        .checked_add(output.retained_storage())
        .and_then(|n| n.checked_add(rows.capacity().checked_mul(size_of::<Row>())?))
        .ok_or_else(|| Resource::Arithmetic.into())
}

#[cfg(test)]
#[path = "owned_cross_block_forwarding_v1_tests.rs"]
mod tests;
