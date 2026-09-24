//! Source initialization and entry custody for the closed multi-entry recurrence.
use super::*;
#[path = "single_entry_initializer_v1.rs"]
pub(super) mod single;
#[cfg(test)]
#[path = "multi_entry_induction_v1_tests.rs"]
mod tests;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};

type Result<T> = std::result::Result<T, ProductionRankedProjectionErrorV1>;

impl ProjectedInductionPreheaderControlV1 {
    pub(super) fn clone_single_v1(&self) -> Result<Self> {
        match self {
            Self::Direct => Ok(Self::Direct),
            Self::Optional {
                discriminant,
                explicit_value,
                explicit_target,
                otherwise,
            } => Ok(Self::Optional {
                discriminant: discriminant.clone(),
                explicit_value: *explicit_value,
                explicit_target: *explicit_target,
                otherwise: *otherwise,
            }),
            Self::Multiple(_) => Err(reject(
                "multi-entry induction cannot be copied as a historical single-entry proof",
            )),
            Self::DistantDirect(_) => Err(reject(
                "distant induction cannot discard its initialization proof",
            )),
        }
    }
}

fn resource(error: Resource) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
    )
}

fn reject(reason: &'static str) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Incomplete(reason)
}

fn sum(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| resource(Resource::Arithmetic))
}

fn bytes<T>(count: usize) -> Result<usize> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| resource(Resource::Arithmetic))
}

#[derive(Default)]
pub(super) struct Scope {
    ledger: Option<(usize, Ledger, usize)>,
    retained: usize,
}

pub(super) struct Context<'s, 'f> {
    pub(super) scope: &'s mut Scope,
    pub(super) facts: &'f mut dyn ProjectedAssertionFactsV1,
}

impl Scope {
    fn check(&mut self, facts: &mut dyn ProjectedAssertionFactsV1) -> Result<()> {
        let (address, ledger) = facts.helper_value_ledger_v1()?;
        let storage = facts.scalar_private_storage_v1()?;
        if let Some((expected_address, expected, floor)) = self.ledger {
            if address != expected_address
                || ledger != expected
                || storage < sum(floor, self.retained)?
            {
                return Err(resource(Resource::Accounting));
            }
        } else {
            self.ledger = Some((address, ledger, storage));
            let header = std::mem::size_of::<Scope>();
            facts.reserve_scalar_private_storage_v1(header)?;
            self.retained = header;
        }
        Ok(())
    }

    fn reserve(&mut self, facts: &mut dyn ProjectedAssertionFactsV1, amount: usize) -> Result<()> {
        self.check(facts)?;
        let retained = sum(self.retained, amount)?;
        facts.reserve_scalar_private_storage_v1(amount)?;
        self.retained = retained;
        Ok(())
    }

    fn release(&mut self, facts: &mut dyn ProjectedAssertionFactsV1, amount: usize) -> Result<()> {
        self.check(facts)?;
        let retained = self
            .retained
            .checked_sub(amount)
            .ok_or_else(|| resource(Resource::Accounting))?;
        facts.release_scalar_private_storage_v1(amount)?;
        self.retained = retained;
        Ok(())
    }
}

impl Context<'_, '_> {
    fn charge(&mut self, work: usize) -> Result<()> {
        self.scope.check(self.facts)?;
        self.facts.charge_private_array_work(work)
    }

    fn reserve(&mut self, amount: usize) -> Result<()> {
        self.scope.reserve(self.facts, amount)
    }

    fn backing<T>(&mut self, count: usize) -> Result<Vec<T>> {
        self.reserve(bytes::<T>(count)?)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| resource(Resource::Allocation))?;
        let excess = values
            .capacity()
            .checked_sub(count)
            .ok_or_else(|| resource(Resource::Accounting))?;
        self.reserve(bytes::<T>(excess)?)?;
        Ok(values)
    }
}

pub(super) fn with_scope<F: ProjectedAssertionFactsV1>(
    facts: &mut F,
    action: impl FnOnce(&mut Scope, &mut F) -> Result<ProductionRankedRootProgramV1>,
) -> Result<ProductionRankedRootProgramV1> {
    let mut scope = Scope::default();
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| action(&mut scope, facts)));
    if scope.ledger.is_some() {
        scope.check(facts)?;
        let retained = scope.retained;
        scope.release(facts, retained)?;
    }
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[derive(Debug, Eq, PartialEq)]
#[cfg_attr(test, derive(Clone))]
pub(super) struct Entries {
    // Same-call custody only: the actual immutable source outlives this
    // private row, which is dropped inside with_scope before returning a root.
    source_address: usize,
    function: SemanticFunctionIdentityV1,
    pub(super) initialization: ScalarAssignmentSiteV1,
    pub(super) header: usize,
    induction: SemanticLocalIdV1,
    ranked_initial: Option<ProductionRankedValueV1>,
    pub(super) rows: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Entry {
    pub(super) block: usize,
    successor: usize,
    role: SemanticEdgeRoleV1,
    target: usize,
}

impl Entries {
    pub(super) fn contains(&self, block: usize) -> bool {
        self.rows
            .binary_search_by_key(&block, |row| row.block)
            .is_ok()
    }
}

fn moved(operand: &SemanticOperandV1, local: SemanticLocalIdV1) -> bool {
    matches!(operand, SemanticOperandV1::Move(place) if place.local() == local)
}

fn value_kills(
    value: &SemanticRvalueKindV1,
    local: SemanticLocalIdV1,
    context: &mut Context<'_, '_>,
) -> Result<bool> {
    context.charge(3)?;
    if matches!(value,
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. } if place.local() == local)
    {
        return Ok(true);
    }
    let mut found = false;
    value.try_visit_operands(|operand| -> Result<()> {
        context.charge(2)?;
        found |= moved(operand, local);
        Ok(())
    })?;
    Ok(found)
}

fn statement_kills(
    kind: &SemanticStatementKindV1,
    local: SemanticLocalIdV1,
    context: &mut Context<'_, '_>,
) -> Result<bool> {
    context.charge(5)?;
    let mut destination = false;
    visit_statement_definition_places(kind, &mut |place| destination |= place.local() == local);
    if destination {
        return Ok(true);
    }
    Ok(match kind {
        SemanticStatementKindV1::Assign(value) => {
            value_kills(value.value().kind(), local, context)?
        }
        SemanticStatementKindV1::Store(value) => moved(value.value(), local),
        SemanticStatementKindV1::AtomicRmw(value) => moved(value.value(), local),
        SemanticStatementKindV1::AtomicCompareExchange(value) => {
            moved(value.expected(), local) || moved(value.replacement(), local)
        }
        SemanticStatementKindV1::StorageLive(value)
        | SemanticStatementKindV1::StorageDead(value) => *value == local,
        SemanticStatementKindV1::Assume(value) => moved(value, local),
        SemanticStatementKindV1::SetDiscriminant { .. }
        | SemanticStatementKindV1::Deinitialize(_)
        | SemanticStatementKindV1::Nop => false,
    })
}

fn terminator_kills(
    kind: &SemanticTerminatorKindV1,
    local: SemanticLocalIdV1,
    context: &mut Context<'_, '_>,
) -> Result<bool> {
    context.charge(4)?;
    Ok(match kind {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => moved(discriminant, local),
        SemanticTerminatorKindV1::Call(call) => {
            let mut killed = call
                .destination()
                .is_some_and(|destination| destination.place().local() == local);
            for operand in call.arguments() {
                context.charge(2)?;
                killed |= moved(operand, local);
            }
            killed
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            let mut killed = false;
            for operand in call.arguments() {
                context.charge(2)?;
                killed |= moved(operand, local);
            }
            killed
        }
        SemanticTerminatorKindV1::Drop { place, .. } => place.local() == local,
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            let message_kills = match message {
                SemanticAssertMessageV1::BoundsCheck {
                    length: left,
                    index: right,
                }
                | SemanticAssertMessageV1::Overflow { left, right, .. }
                | SemanticAssertMessageV1::MisalignedPointerDereference {
                    required_alignment: left,
                    found_alignment: right,
                } => moved(left, local) || moved(right, local),
                SemanticAssertMessageV1::DivisionByZero(value)
                | SemanticAssertMessageV1::RemainderByZero(value) => moved(value, local),
                SemanticAssertMessageV1::NullPointerDereference
                | SemanticAssertMessageV1::ResumedAfterReturn
                | SemanticAssertMessageV1::ResumedAfterPanic => false,
            };
            moved(condition, local) || message_kills
        }
        SemanticTerminatorKindV1::FalseEdge { .. } => {
            return Err(reject(
                "multi-entry induction retains an unnormalized false edge",
            ));
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => false,
    })
}

struct Scratch {
    seen: Vec<u8>,
    region: Vec<u8>,
    pending: Vec<usize>,
}

impl Scratch {
    fn new(blocks: usize, context: &mut Context<'_, '_>) -> Result<Self> {
        context.reserve(std::mem::size_of::<Scratch>())?;
        let mut seen = context.backing(blocks)?;
        let mut region = context.backing(blocks)?;
        let pending = context.backing(blocks)?;
        context.charge(
            bytes::<u8>(blocks)?
                .checked_mul(2)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        seen.resize(blocks, 0);
        region.resize(blocks, 0);
        Ok(Self {
            seen,
            region,
            pending,
        })
    }

    fn retained(&self) -> Result<usize> {
        sum(
            std::mem::size_of::<Self>(),
            sum(
                sum(self.seen.capacity(), self.region.capacity())?,
                bytes::<usize>(self.pending.capacity())?,
            )?,
        )
    }

    fn reset(&mut self, context: &mut Context<'_, '_>) -> Result<()> {
        context.charge(sum(self.seen.len(), self.pending.len())?)?;
        self.seen.fill(0);
        self.pending.clear();
        Ok(())
    }

    fn seed(&mut self, block: usize, context: &mut Context<'_, '_>) -> Result<()> {
        context.charge(3)?;
        let seen = self
            .seen
            .get_mut(block)
            .ok_or_else(|| reject("multi-entry traversal left the source block table"))?;
        if *seen == 0 {
            if self.pending.len() == self.pending.capacity() {
                return Err(resource(Resource::Accounting));
            }
            *seen = 1;
            self.pending.push(block);
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn initialization(
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    outside: &[bool],
    header: usize,
    latch: usize,
    induction: SemanticLocalIdV1,
    entries: &[usize],
    scratch: &mut Scratch,
    context: &mut Context<'_, '_>,
) -> Result<ScalarAssignmentSiteV1> {
    let mut initializer = None;
    for (block_index, block) in function.blocks().iter().enumerate() {
        context.charge(2)?;
        for (statement_index, statement) in block.statements().iter().enumerate() {
            context.charge(5)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow { place, .. } | SemanticRvalueKindV1::AddressOf { place, .. } if place.local() == induction)
            {
                return Err(reject(
                    "multi-entry induction has an escaped initialization binding",
                ));
            }
            if assignment.destination().local() != induction || block_index == latch {
                continue;
            }
            if !assignment.destination().projections().is_empty()
                || !outside.get(block_index).copied().unwrap_or(false)
                || !matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(_))
                || initializer
                    .replace(ScalarAssignmentSiteV1 {
                        block: block_index,
                        statement: statement_index,
                    })
                    .is_some()
            {
                return Err(reject(
                    "multi-entry induction does not have one outside initializer",
                ));
            }
        }
    }
    let initializer =
        initializer.ok_or_else(|| reject("multi-entry induction has no outside initializer"))?;
    scratch.reset(context)?;
    if graph.entry != initializer.block {
        scratch.seed(graph.entry, context)?;
    }
    while !scratch.pending.is_empty() {
        context.charge(3)?;
        let block = scratch
            .pending
            .pop()
            .ok_or_else(|| resource(Resource::Accounting))?;
        for &target in &graph.successors[block] {
            context.charge(2)?;
            if target != initializer.block {
                scratch.seed(target, context)?;
            }
        }
    }
    context.charge(sum(entries.len(), 1)?)?;
    if scratch.seen[header] != 0 || entries.iter().any(|entry| scratch.seen[*entry] != 0) {
        return Err(reject(
            "multi-entry initializer does not dominate every entry occurrence",
        ));
    }
    scratch.reset(context)?;
    scratch.seed(initializer.block, context)?;
    while !scratch.pending.is_empty() {
        context.charge(3)?;
        let block = scratch
            .pending
            .pop()
            .ok_or_else(|| resource(Resource::Accounting))?;
        for &target in &graph.successors[block] {
            context.charge(3)?;
            if target != header && outside[target] {
                scratch.seed(target, context)?;
            }
        }
    }
    context.charge(scratch.region.len())?;
    scratch.region.copy_from_slice(&scratch.seen);
    scratch.reset(context)?;
    scratch.seed(header, context)?;
    while !scratch.pending.is_empty() {
        context.charge(3)?;
        let block = scratch
            .pending
            .pop()
            .ok_or_else(|| resource(Resource::Accounting))?;
        for &source in &graph.predecessors[block] {
            context.charge(3)?;
            if outside[source] {
                scratch.seed(source, context)?;
            }
        }
    }
    for (block_index, block) in function.blocks().iter().enumerate() {
        context.charge(3)?;
        if scratch.region[block_index] == 0 || scratch.seen[block_index] == 0 {
            continue;
        }
        for (statement_index, statement) in block.statements().iter().enumerate() {
            context.charge(2)?;
            if block_index == initializer.block && statement_index <= initializer.statement {
                continue;
            }
            if statement_kills(statement.kind(), induction, context)? {
                return Err(reject(
                    "multi-entry induction initialization is killed before an entry",
                ));
            }
        }
        if terminator_kills(block.terminator().kind(), induction, context)? {
            return Err(reject(
                "multi-entry induction initialization is killed at an entry endpoint",
            ));
        }
    }
    Ok(initializer)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    outside: &[bool],
    header: usize,
    latch: usize,
    induction: SemanticLocalIdV1,
    entries: &[usize],
    context: &mut Context<'_, '_>,
) -> Result<Box<Entries>> {
    context.charge(8)?;
    if entries.len() < 2 || header == graph.entry || outside.len() != graph.successors.len() {
        return Err(reject(
            "multi-entry induction requires distinct outside entries and a non-entry header",
        ));
    }
    let mut previous = None;
    for &block in entries {
        context.charge(8)?;
        if previous.is_some_and(|previous| previous >= block)
            || graph.reachable.get(block).copied() != Some(true)
            || outside.get(block).copied() != Some(true)
            || graph.successors[block].as_slice() != [header]
            || !matches!(function.blocks()[block].terminator().kind(), SemanticTerminatorKindV1::Goto(edge)
                if edge.role() == SemanticEdgeRoleV1::Goto && edge.target().index() as usize == header)
        {
            return Err(reject(
                "multi-entry induction requires ordered exact outside Goto occurrences",
            ));
        }
        previous = Some(block);
    }
    context.reserve(std::mem::size_of::<Entries>())?;
    let mut rows = context.backing(entries.len())?;
    let mut scratch = Scratch::new(function.blocks().len(), context)?;
    let initialization = initialization(
        function,
        graph,
        outside,
        header,
        latch,
        induction,
        entries,
        &mut scratch,
        context,
    )?;
    require_reinitialization(
        graph,
        header,
        initialization.block,
        entries,
        &mut scratch,
        context,
    )?;
    let scratch_storage = scratch.retained()?;
    drop(scratch);
    context.scope.release(context.facts, scratch_storage)?;
    for &block in entries {
        context.charge(5)?;
        rows.push(Entry {
            block,
            successor: 0,
            role: SemanticEdgeRoleV1::Goto,
            target: header,
        });
    }
    context.charge(64)?;
    Ok(Box::new(Entries {
        source_address: function as *const _ as usize,
        function: function.identity(),
        initialization,
        header,
        induction,
        ranked_initial: None,
        rows,
    }))
}

fn require_reinitialization(
    graph: &ProjectedLoopCfgV1,
    header: usize,
    initializer: usize,
    entries: &[usize],
    scratch: &mut Scratch,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    // A later entry must rerun the assignment, not reuse a prior latch value.
    scratch.reset(context)?;
    scratch.seed(header, context)?;
    while !scratch.pending.is_empty() {
        context.charge(3)?;
        let block = scratch
            .pending
            .pop()
            .ok_or_else(|| resource(Resource::Accounting))?;
        for &target in &graph.successors[block] {
            context.charge(2)?;
            if target != initializer {
                scratch.seed(target, context)?;
            }
        }
    }
    context.charge(entries.len())?;
    if entries.iter().any(|entry| scratch.seen[*entry] != 0) {
        return Err(reject("induction initializer is bypassed on loop re-entry"));
    }
    Ok(())
}

pub(super) fn bind_initial(
    entries: &mut Entries,
    initial: ProductionRankedValueV1,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    context.charge(3)?;
    if entries.ranked_initial.replace(initial).is_some() {
        return Err(reject("multi-entry initialization was bound twice"));
    }
    Ok(())
}

pub(super) fn prepay_header_entry_searches(
    predecessors: usize,
    entries: usize,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    let lookup = sum(4, usize::BITS as usize - entries.leading_zeros() as usize)?;
    context.charge(
        predecessors
            .checked_mul(lookup)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    )
}

pub(super) fn check_roles(
    inductions: &[ProjectedUniformInductionV1],
    context: &mut Context<'_, '_>,
) -> Result<()> {
    for (ordinal, induction) in inductions.iter().enumerate() {
        context.charge(1)?;
        let ProjectedInductionPreheaderControlV1::Multiple(entries) = &induction.preheader_control
        else {
            continue;
        };
        for entry in &entries.rows {
            context.charge(1)?;
            for (other_ordinal, other) in inductions.iter().enumerate() {
                context.charge(5)?;
                if [other.initializer_block, other.header, other.latch].contains(&entry.block) {
                    return Err(reject(
                        "multi-entry induction has ambiguous entry, initializer, header or latch ownership",
                    ));
                }
                if other_ordinal == ordinal {
                    continue;
                }
                if let ProjectedInductionPreheaderControlV1::Multiple(other) =
                    &other.preheader_control
                {
                    context.charge(sum(
                        2,
                        usize::BITS as usize - other.rows.len().leading_zeros() as usize,
                    )?)?;
                    if other.contains(entry.block) {
                        return Err(reject("multi-entry inductions share an entry occurrence"));
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn before_emission(
    function: &SemanticFunctionDeclV1,
    inductions: &[ProjectedUniformInductionV1],
    context: &mut Context<'_, '_>,
) -> Result<()> {
    check_roles(inductions, context)?;
    for induction in inductions {
        context.charge(1)?;
        let ProjectedInductionPreheaderControlV1::Multiple(entries) = &induction.preheader_control
        else {
            continue;
        };
        context.charge(50)?;
        if entries.source_address != function as *const _ as usize
            || entries.function != function.identity()
            || entries.header != induction.header
            || entries.induction != induction.source_progress.induction
            || entries.initialization.block != induction.initializer_block
            || entries.ranked_initial != Some(induction.initial)
            || !matches!(
                induction.source_progress.update,
                ProjectedSourceInductionUpdateV1::Ordinary
            )
        {
            return Err(reject(
                "multi-entry ranked initialization lost its source association",
            ));
        }
        let mut ordinal = 0;
        for (block, source) in function.blocks().iter().enumerate() {
            context.charge(sum(
                3,
                usize::BITS as usize - induction.loop_blocks.len().leading_zeros() as usize,
            )?)?;
            if induction.contains_block(block) {
                continue;
            }
            let mut successor = 0;
            source
                .terminator()
                .kind()
                .try_for_each_edge(|edge| -> Result<()> {
                    context.charge(4)?;
                    if edge.target().index() as usize == induction.header {
                        let expected = entries.rows.get(ordinal).ok_or_else(|| {
                            reject("multi-entry ranked source has an unlisted outside occurrence")
                        })?;
                        if *expected
                            != (Entry {
                                block,
                                successor,
                                role: edge.role(),
                                target: induction.header,
                            })
                        {
                            return Err(reject(
                                "multi-entry ranked source occurrence changed before emission",
                            ));
                        }
                        ordinal = sum(ordinal, 1)?;
                    }
                    successor = sum(successor, 1)?;
                    Ok(())
                })?;
        }
        if ordinal != entries.rows.len() {
            return Err(reject(
                "multi-entry ranked source omitted an outside occurrence",
            ));
        }
        let lookup = sum(
            4,
            usize::BITS as usize - entries.rows.len().leading_zeros() as usize,
        )?;
        context.charge(
            function
                .blocks()
                .len()
                .checked_mul(lookup)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
    }
    Ok(())
}

pub(super) fn replay(
    entries: &Entries,
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    header: usize,
    latch: usize,
    induction: SemanticLocalIdV1,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    context.charge(40)?;
    if entries.source_address != function as *const _ as usize
        || entries.function != function.identity()
        || entries.header != header
        || entries.induction != induction
    {
        return Err(reject(
            "multi-entry proof is attached to a different source recurrence",
        ));
    }
    let retained = context.scope.retained;
    let result = (|| {
        let count = function.blocks().len();
        let mut scratch = Scratch::new(count, context)?;
        if graph.entry != header {
            scratch.seed(graph.entry, context)?;
        }
        while !scratch.pending.is_empty() {
            context.charge(3)?;
            let block = scratch
                .pending
                .pop()
                .ok_or_else(|| resource(Resource::Accounting))?;
            for &target in &graph.successors[block] {
                context.charge(2)?;
                if target != header {
                    scratch.seed(target, context)?;
                }
            }
        }
        context.reserve(sum(
            std::mem::size_of::<Vec<bool>>(),
            std::mem::size_of::<Vec<usize>>(),
        )?)?;
        let mut outside = context.backing(count)?;
        context.charge(count)?;
        outside.extend(scratch.seen.iter().map(|value| *value != 0));
        let predecessors = graph
            .predecessors
            .get(header)
            .ok_or_else(|| reject("multi-entry header left the source graph"))?;
        let mut sources = context.backing(predecessors.len())?;
        for &source in predecessors {
            context.charge(3)?;
            if graph.reachable[source] && outside[source] {
                sources.push(source);
            }
        }
        let actual = build(
            function, graph, &outside, header, latch, induction, &sources, context,
        )?;
        context.charge(12)?;
        if actual.initialization != entries.initialization
            || actual.rows.len() != entries.rows.len()
        {
            return Err(reject(
                "multi-entry initializer or entry count changed before ranked emission",
            ));
        }
        for (actual, expected) in actual.rows.iter().zip(&entries.rows) {
            context.charge(5)?;
            if actual != expected {
                return Err(reject(
                    "multi-entry source occurrence order or identity changed",
                ));
            }
        }
        Ok(())
    })();
    let temporary = context
        .scope
        .retained
        .checked_sub(retained)
        .ok_or_else(|| resource(Resource::Accounting))?;
    context.scope.release(context.facts, temporary)?;
    result
}
