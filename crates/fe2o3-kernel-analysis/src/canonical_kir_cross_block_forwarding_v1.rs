//! Independent at-site private Load replacement across complete memory phis.
use crate::{
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirMemorySsaErrorV1 as MemoryError, CanonicalKirMemorySsaInputSourceV1 as Input,
    CanonicalKirMemorySsaLimitsV1 as MemoryLimits, CanonicalKirMemorySsaNodeIdV1 as NodeId,
    CanonicalKirMemorySsaNodeV1 as Node, CanonicalKirMemorySsaV1 as Memory,
    canonical_kir_private_cell_pair_resources_v1 as resources,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirControlFlowScopeErrorV1 as FlowError,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirOperationCoordinateV1 as Site,
    ControlFlowLimits, KirLocalMemoryEffectRefV1 as Effect, MemoryAccess, Module,
    OperationKind as Kind, ScalarType, Type, UnaryOp, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner, with_canonical_kir_control_flow_v1,
};
use std::{fmt, mem::size_of};

/// Exact bounded analysis settings retained by an owning continuation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CanonicalKirCrossBlockForwardingLimitsV1 {
    /// Existing complete MemorySSA census limits.
    pub memory: MemoryLimits,
    /// Existing owner-bound CFG construction limits.
    pub control_flow: ControlFlowLimits,
}
type Limits = CanonicalKirCrossBlockForwardingLimitsV1;

/// One input-ordered operation, at the identical output coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirCrossBlockForwardingOriginV1 {
    /// Exact original operation coordinate.
    pub input: Site,
    /// Exact unchanged coordinate in the actual output.
    pub output: Site,
    /// An exact retained initializing Store, or None for an unchanged operation.
    pub store: Option<Site>,
}
type Row = CanonicalKirCrossBlockForwardingOriginV1;

/// Failed pair checks never grant a no-op or a source admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirCrossBlockForwardingErrorV1 {
    /// Cumulative work, storage, allocation or arithmetic refusal.
    Resource(Resource),
    /// An actual endpoint inventory failed.
    Inventory(InventoryError),
    /// Complete input memory version construction or a query failed.
    Memory(MemoryError),
    /// Actual owner-bound dominance could not be checked.
    ControlFlow(FlowError),
    /// An endpoint, lineage, exact cell or all-path condition disagrees.
    Mismatch(&'static str),
    /// A scoped partial check unwound.
    Panicked,
}
type Error = CanonicalKirCrossBlockForwardingErrorV1;
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
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cross-block private forwarding: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Unreserved borrowed witness header; endpoint and row storage remains external.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirCrossBlockForwardingStorageV1(usize);
impl CanonicalKirCrossBlockForwardingStorageV1 {
    /// Reserve while retaining the returned witness.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Checked actual pair and complete lineage, without source or native authority.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV1 as Pair;
/// fn duplicate<'a>(p: &Pair<'a>) -> Pair<'a> { p.clone() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::check_canonical_kir_cross_block_forwarding_v1;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn detach(a: Owner, b: Owner, budget: &mut Budget<'_>) {
///     let (pair, _) = check_canonical_kir_cross_block_forwarding_v1(&a, &b, &[], Default::default(), budget).unwrap();
///     drop(a);
///     let _ = pair.input();
/// }
/// ```
pub struct CheckedCanonicalKirCrossBlockForwardingV1<'a> {
    input: &'a Owner,
    output: &'a Owner,
    origins: &'a [Row],
}
impl<'a> CheckedCanonicalKirCrossBlockForwardingV1<'a> {
    /// Actual connected input owner.
    pub const fn input(&self) -> &'a Owner {
        self.input
    }
    /// Actual independently checked output owner.
    pub const fn output(&self) -> &'a Owner {
        self.output
    }
    /// Complete original-order correspondence, including every retained operation.
    pub const fn origins(&self) -> &'a [Row] {
        self.origins
    }
    /// This relation alone grants no source, artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Independently checks same-site fixed-integer Load copies from one dominating
/// direct private Store. Every syntactic memory-phi input is retained; each
/// visited phi must have a real matching Store terminal in its dependency closure.
/// Entry, other Defs, escaping pointers and conservative non-total interval cuts
/// refuse. It never calls the producer or accepts producer analysis arrays.
///
/// Added work is O(B+O+D+U+M+I+R+wire), with O(D+O+M+I) actual-capacity scratch;
/// existing inventory, MemorySSA and CFG costs remain separately bounded. All
/// buffers are prepaid outside the CFG scopes. Failure/unwind drops scratch
/// before restoring the valid ledger's entry floor, preserving denial history.
pub fn check_canonical_kir_cross_block_forwarding_v1<'a>(
    input: &'a Owner,
    output: &'a Owner,
    origins: &'a [Row],
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<(
    CheckedCanonicalKirCrossBlockForwardingV1<'a>,
    CanonicalKirCrossBlockForwardingStorageV1,
)> {
    resources::scoped(budget, |meter| check(input, output, origins, limits, meter))
}

#[derive(Clone, Copy)]
struct Access {
    pointer: ValueId,
    definition: usize,
    access: MemoryAccess,
    stored: Option<(ValueId, usize)>,
}

fn check<'a>(
    input: &'a Owner,
    output: &'a Owner,
    rows: &'a [Row],
    limits: Limits,
    meter: &mut Meter<'_, '_>,
) -> Result<(
    CheckedCanonicalKirCrossBlockForwardingV1<'a>,
    CanonicalKirCrossBlockForwardingStorageV1,
)> {
    let header = size_of::<CheckedCanonicalKirCrossBlockForwardingV1<'_>>();
    meter.reserve(header)?;
    meter.work(13)?;
    meter.reserve(scratch_headers()?)?;
    let (a, size) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
    meter.reserve(size.retained_storage())?;
    let (b, size) = meter.derive(|b| Ok(Inventory::derive(output, b)?))?;
    meter.reserve(size.retained_storage())?;
    let (memory, size) = meter.derive(|b| Ok(Memory::derive(&a, limits.memory, b)?))?;
    meter.reserve(size.retained_storage())?;
    meter.work(
        input
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(output.canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    headers(input.module(), output.module())?;
    if rows.len() != a.operations().len() || rows.len() != b.operations().len() {
        return Err(Error::Mismatch("complete operation cardinality"));
    }
    let slots = slots(&a, meter)?;
    let (local_cut, exit_cut) = cuts(&a, &memory, meter)?;
    let m = memory.node_count();
    let incidence = a
        .edges()
        .len()
        .checked_add(a.functions().len())
        .ok_or(Resource::Arithmetic)?;
    let mut required = filled(m, None::<usize>, meter)?;
    let mut phi = filled(m, false, meter)?;
    let mut grounded = filled(m, false, meter)?;
    let (mut queue, _) = meter.table::<NodeId>(m)?;
    let (mut reverse, _) = meter.table::<(usize, usize)>(incidence)?;
    let (mut grounding, _) = meter.table::<usize>(m)?;
    let mut counts = filled(m.checked_add(1).ok_or(Resource::Arithmetic)?, 0usize, meter)?;
    let mut cursor = filled(m, 0usize, meter)?;
    let mut parents = filled(incidence, usize::MAX, meter)?;
    let mut stored_definitions = filled(rows.len(), None::<usize>, meter)?;

    for (at, row) in rows.iter().enumerate() {
        meter.work(8)?;
        let old = &a.operations()[at];
        let new = &b.operations()[at];
        if row.input != old.coordinate
            || row.output != old.coordinate
            || new.coordinate != old.coordinate
        {
            return Err(Error::Mismatch("complete unchanged coordinates"));
        }
        let Some(store_site) = row.store else {
            if old.operation != new.operation {
                return Err(Error::Mismatch("retained operation"));
            }
            continue;
        };
        let store_at = operation_index(&a, store_site)?;
        let store = access(&a, store_at, &slots, meter)?
            .filter(|a| a.stored.is_some())
            .ok_or(Error::Mismatch("exact ordinary direct-slot Store"))?;
        let load = access(&a, at, &slots, meter)?
            .filter(|a| a.stored.is_none())
            .ok_or(Error::Mismatch("exact ordinary direct-slot Load"))?;
        let (value, definition) = store.stored.ok_or(Error::Mismatch("stored value"))?;
        if store_site.block.function != row.input.block.function
            || store_site.block == row.input.block
            || store.pointer != load.pointer
            || store.definition != load.definition
            || store.access != load.access
            || local_cut[at]
            || a.definitions()[definition].ty != &old.operation.results[0].ty
            || old.operation.results != new.operation.results
            || new.operation.kind
                != (Kind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: value,
                    rhs: value,
                })
        {
            return Err(Error::Mismatch("exact cross-block typed Load copy"));
        }
        stored_definitions[at] = Some(definition);
        let node = meter
            .derive(|b| Ok(memory.operation(row.input, b)?))?
            .ok_or(Error::Mismatch("Load MemorySSA Use"))?;
        let incoming = match meter.derive(|b| Ok(memory.node(node, b)?.clone()))? {
            Node::Use {
                operation,
                incoming,
            } if operation == row.input => incoming,
            _ => return Err(Error::Mismatch("exact Load version")),
        };
        label(incoming, store_at, &mut required, &mut queue, meter)?;
    }

    // Independent closure, then reverse grounding; no producer lattice is used.
    let mut next = 0usize;
    while next < queue.len() {
        meter.work(4)?;
        let id = queue[next];
        next += 1;
        let index = id.index();
        let store = required[index].ok_or(Error::Mismatch("required terminal"))?;
        match meter.derive(|b| Ok(memory.node(id, b)?.clone()))? {
            Node::Def { operation, .. } if operation == a.operations()[store].coordinate => {}
            Node::Phi { block, .. } => {
                phi[index] = true;
                if block.function != a.operations()[store].coordinate.block.function {
                    return Err(Error::Mismatch("memory phi function"));
                }
                let inputs = meter.derive(|b| Ok(memory.phi_inputs(id, b)?))?;
                for incoming in inputs {
                    meter.work(6)?;
                    let Input::Edge(edge) = incoming.source() else {
                        return Err(Error::Mismatch("live-on-entry contribution"));
                    };
                    if edge.source.function != block.function
                        || exit_cut[block_index(&a, edge.source)?]
                    {
                        return Err(Error::Mismatch("complete predecessor interval"));
                    }
                    let child = incoming.state();
                    match meter.derive(|b| Ok(memory.node(child, b)?.clone()))? {
                        Node::Def { operation, .. }
                            if operation == a.operations()[store].coordinate =>
                        {
                            if !grounded[index] {
                                grounded[index] = true;
                                meter.push(&mut grounding, index)?;
                            }
                        }
                        Node::Phi {
                            block: child_block, ..
                        } if child_block.function == block.function => {
                            label(child, store, &mut required, &mut queue, meter)?;
                            meter.push(&mut reverse, (child.index(), index))?;
                            counts[child.index() + 1] = counts[child.index() + 1]
                                .checked_add(1)
                                .ok_or(Resource::Arithmetic)?;
                        }
                        _ => {
                            return Err(Error::Mismatch(
                                "every phi input is the same Store or phi",
                            ));
                        }
                    }
                }
            }
            _ => {
                return Err(Error::Mismatch(
                    "exact Store terminal, never search past a Def",
                ));
            }
        }
    }
    for i in 0..m {
        meter.work(3)?;
        counts[i + 1] = counts[i + 1]
            .checked_add(counts[i])
            .ok_or(Resource::Arithmetic)?;
        cursor[i] = counts[i];
    }
    if counts[m] != reverse.len() {
        return Err(Error::Mismatch("reverse incidence count"));
    }
    for (child, parent) in reverse {
        meter.work(3)?;
        let at = cursor[child];
        if at >= counts[child + 1] || at >= parents.len() {
            return Err(Error::Mismatch("reverse incidence bound"));
        }
        parents[at] = parent;
        cursor[child] = at.checked_add(1).ok_or(Resource::Arithmetic)?;
    }
    next = 0;
    while next < grounding.len() {
        meter.work(2)?;
        let child = grounding[next];
        next += 1;
        for parent in &parents[counts[child]..counts[child + 1]] {
            meter.work(4)?;
            let parent = *parent;
            if parent >= m || required[parent] != required[child] {
                return Err(Error::Mismatch("grounding label"));
            }
            if !grounded[parent] {
                grounded[parent] = true;
                meter.push(&mut grounding, parent)?;
            }
        }
    }
    for i in 0..m {
        meter.work(2)?;
        if phi[i] && !grounded[i] {
            return Err(Error::Mismatch("each phi needs an actual Store path"));
        }
    }

    // Every buffer above remains prepaid in the exact CFG entry floor.
    for function in a.functions() {
        meter.work(1)?;
        if function.function.body.is_none() {
            continue;
        }
        meter.derive(|budget| {
            with_canonical_kir_control_flow_v1(
                input,
                function.coordinate,
                limits.control_flow,
                budget,
                |flow, budget| {
                    for at in function.operations.clone() {
                        budget.charge_work(3)?;
                        let Some(store) = rows[at].store else {
                            continue;
                        };
                        let load = rows[at].input;
                        if !flow.is_reachable(load.block, budget)?
                            || !flow.dominates(store.block, load.block, budget)?
                        {
                            return Err(Error::Mismatch("reachable dominating initializing Store"));
                        }
                        let definition = stored_definitions[at]
                            .ok_or(Error::Mismatch("replacement definition"))?;
                        let block = match a.definitions()[definition].coordinate {
                            Definition::FunctionArgument { function: f, .. }
                                if f == function.coordinate =>
                            {
                                continue;
                            }
                            Definition::BlockArgument { block, .. } => block,
                            Definition::Result { operation, .. } => {
                                if operation.block == load.block
                                    && operation.operation >= load.operation
                                {
                                    return Err(Error::Mismatch(
                                        "same-block replacement availability",
                                    ));
                                }
                                operation.block
                            }
                            _ => return Err(Error::Mismatch("replacement function")),
                        };
                        if !flow.dominates(block, load.block, budget)? {
                            return Err(Error::Mismatch("replacement dominance"));
                        }
                    }
                    Ok(())
                },
            )
        })?;
    }
    meter.work(1)?;
    Ok((
        CheckedCanonicalKirCrossBlockForwardingV1 {
            input,
            output,
            origins: rows,
        },
        CanonicalKirCrossBlockForwardingStorageV1(header),
    ))
}

fn scratch_headers() -> Result<usize> {
    // slots; local/exit cuts; required/phi/grounded; closure/reverse/grounding;
    // CSR counts/cursor/parents; stored-definition table. Backing is paid below.
    [
        size_of::<Vec<u32>>(),
        size_of::<Vec<bool>>(),
        size_of::<Vec<bool>>(),
        size_of::<Vec<Option<usize>>>(),
        size_of::<Vec<bool>>(),
        size_of::<Vec<bool>>(),
        size_of::<Vec<NodeId>>(),
        size_of::<Vec<(usize, usize)>>(),
        size_of::<Vec<usize>>(),
        size_of::<Vec<usize>>(),
        size_of::<Vec<usize>>(),
        size_of::<Vec<usize>>(),
        size_of::<Vec<Option<usize>>>(),
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        total
            .checked_add(bytes)
            .ok_or_else(|| Resource::Arithmetic.into())
    })
}

fn filled<T: Clone>(len: usize, value: T, meter: &mut Meter<'_, '_>) -> Result<Vec<T>> {
    let (mut rows, _) = meter.table(len)?;
    meter.work(len)?;
    rows.resize(len, value);
    Ok(rows)
}
fn label(
    id: NodeId,
    store: usize,
    labels: &mut [Option<usize>],
    queue: &mut Vec<NodeId>,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    meter.work(3)?;
    let slot = labels
        .get_mut(id.index())
        .ok_or(Error::Mismatch("memory node bound"))?;
    match *slot {
        Some(previous) if previous != store => {
            Err(Error::Mismatch("conflicting exact Store labels"))
        }
        Some(_) => Ok(()),
        None => {
            *slot = Some(store);
            meter.push(queue, id)
        }
    }
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
    let mut alignment = filled(a.definitions().len(), 0u32, meter)?;
    for row in a.operations() {
        meter.work(7)?;
        let Kind::Alloca {
            element: Type::Scalar(element),
            count: None,
            address_space: AddressSpace::Private,
            alignment: align,
        } = &row.operation.kind
        else {
            continue;
        };
        if !integer(&Type::Scalar(*element))
            || !align.is_power_of_two()
            || *align
                < u32::from(
                    element
                        .bit_width()
                        .ok_or(Error::Mismatch("fixed cell width"))?
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
            alignment[row.results.start] = *align;
        }
    }
    for row in a.operations() {
        meter.work(1)?;
        for operand in &a.uses()[row.operands.clone()] {
            meter.work(4)?;
            if alignment[operand.definition] == 0 {
                continue;
            }
            let allowed = match row.operation.kind {
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
            if !allowed {
                alignment[operand.definition] = 0;
            }
        }
    }
    for block in a.blocks() {
        meter.work(1)?;
        for operand in &a.uses()[block.terminator_uses.clone()] {
            meter.work(2)?;
            alignment[operand.definition] = 0;
        }
    }
    Ok(alignment)
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
        .ok_or(Error::Mismatch("memory operation coordinate"))?;
    let (pointer, memory, value) = match row.operation.kind {
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
    if memory.address_space != AddressSpace::Private
        || memory.volatile
        || !memory.alignment.is_power_of_two()
        || row.effects.len() != 1
        || !row.compiler_ordering().is_empty()
    {
        return Ok(None);
    }
    let uses = &a.uses()[row.operands.clone()];
    if uses[0].value != pointer {
        return Err(Error::Mismatch("exact memory pointer operand"));
    }
    let definition = uses[0].definition;
    if slots[definition] == 0 || memory.alignment > slots[definition] {
        return Ok(None);
    }
    let stored = if let Some(value) = value {
        if uses[1].value != value {
            return Err(Error::Mismatch("exact Store value operand"));
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
        access: memory,
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
fn cuts(
    a: &Inventory<'_>,
    memory: &Memory<'_, '_>,
    meter: &mut Meter<'_, '_>,
) -> Result<(Vec<bool>, Vec<bool>)> {
    let mut local = filled(a.operations().len(), false, meter)?;
    let mut exits = filled(a.blocks().len(), false, meter)?;
    for (block_index, block) in a.blocks().iter().enumerate() {
        let mut cut = false;
        for at in block.operations.clone() {
            meter.work(3)?;
            local[at] = cut;
            let id = meter.derive(|b| Ok(memory.operation(a.operations()[at].coordinate, b)?))?;
            let definition = if let Some(id) = id {
                matches!(
                    meter.derive(|b| Ok(memory.node(id, b)?.clone()))?,
                    Node::Def { .. }
                )
            } else {
                false
            };
            if definition {
                cut = false;
            } else if !transparent(a, at, meter)? {
                cut = true;
            }
        }
        exits[block_index] = cut;
    }
    Ok((local, exits))
}
fn block_index(a: &Inventory<'_>, block: Block) -> Result<usize> {
    let function = a
        .functions()
        .get(block.function.0 as usize)
        .ok_or(Error::Mismatch("function coordinate"))?;
    function
        .blocks
        .start
        .checked_add(block.block as usize)
        .filter(|at| *at < function.blocks.end && a.blocks()[*at].coordinate == block)
        .ok_or(Error::Mismatch("block coordinate"))
}
fn operation_index(a: &Inventory<'_>, site: Site) -> Result<usize> {
    let block = &a.blocks()[block_index(a, site.block)?];
    block
        .operations
        .start
        .checked_add(site.operation as usize)
        .filter(|at| *at < block.operations.end && a.operations()[*at].coordinate == site)
        .ok_or(Error::Mismatch("operation coordinate"))
}
fn headers(a: &Module, b: &Module) -> Result<()> {
    let Module {
        id,
        functions,
        kernels,
        required_capabilities,
    } = a;
    if id != &b.id
        || kernels != &b.kernels
        || required_capabilities != &b.required_capabilities
        || functions.len() != b.functions.len()
    {
        return Err(Error::Mismatch("module payload"));
    }
    for (a, b) in functions.iter().zip(&b.functions) {
        let fe2o3_kernel_ir::Function {
            id,
            signature,
            role,
            body,
            required_capabilities,
        } = a;
        if id != &b.id
            || signature != &b.signature
            || role != &b.role
            || required_capabilities != &b.required_capabilities
        {
            return Err(Error::Mismatch("function payload"));
        }
        match (body, &b.body) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                if a.parameters != b.parameters || a.blocks.len() != b.blocks.len() {
                    return Err(Error::Mismatch("body payload"));
                }
                for (a, b) in a.blocks.iter().zip(&b.blocks) {
                    if a.id != b.id
                        || a.parameters != b.parameters
                        || a.terminator != b.terminator
                        || a.operations.len() != b.operations.len()
                    {
                        return Err(Error::Mismatch("exact CFG and block cardinality"));
                    }
                }
            }
            _ => return Err(Error::Mismatch("declaration/body")),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "canonical_kir_cross_block_forwarding_v1_tests.rs"]
mod tests;
