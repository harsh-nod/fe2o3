//! Private paid initialization proofs shared by single and multiple loop entries.
use super::*;
pub(super) use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
#[cfg(test)]
#[path = "induction_initialization_v1_tests.rs"]
mod tests;
type Result<T> = std::result::Result<T, ProductionRankedProjectionErrorV1>;
pub(super) fn resource(error: Resource) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
    )
}

pub(super) fn reject(reason: &'static str) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Incomplete(reason)
}

pub(super) fn sum(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| resource(Resource::Arithmetic))
}

pub(super) fn bytes<T>(count: usize) -> Result<usize> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| resource(Resource::Arithmetic))
}

#[derive(Default)]
pub(super) struct Scope {
    ledger: Option<(usize, Ledger, usize)>,
    pub(super) retained: usize,
    singles: Vec<Single>,
    registered: usize,
}

pub(super) struct Context<'s, 'f> {
    pub(super) scope: &'s mut Scope,
    pub(super) facts: &'f mut dyn ProjectedAssertionFactsV1,
}

impl Scope {
    pub(super) fn check(&mut self, facts: &mut dyn ProjectedAssertionFactsV1) -> Result<()> {
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

    pub(super) fn reserve(
        &mut self,
        facts: &mut dyn ProjectedAssertionFactsV1,
        amount: usize,
    ) -> Result<()> {
        self.check(facts)?;
        let retained = sum(self.retained, amount)?;
        facts.reserve_scalar_private_storage_v1(amount)?;
        self.retained = retained;
        Ok(())
    }

    pub(super) fn release(
        &mut self,
        facts: &mut dyn ProjectedAssertionFactsV1,
        amount: usize,
    ) -> Result<()> {
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
    pub(super) fn charge(&mut self, work: usize) -> Result<()> {
        self.scope.check(self.facts)?;
        self.facts.charge_private_array_work(work)
    }

    pub(super) fn reserve(&mut self, amount: usize) -> Result<()> {
        self.scope.reserve(self.facts, amount)
    }

    pub(super) fn backing<T>(&mut self, count: usize) -> Result<Vec<T>> {
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
    // The registry owns backing, unlike the historical header-only scope.
    // Destroy it while its complete reservation remains live, including unwind.
    drop(std::mem::take(&mut scope.singles));
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

pub(super) struct Scratch {
    pub(super) seen: Vec<u8>,
    pub(super) region: Vec<u8>,
    pub(super) pending: Vec<usize>,
}

impl Scratch {
    pub(super) fn new(blocks: usize, context: &mut Context<'_, '_>) -> Result<Self> {
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

    pub(super) fn retained(&self) -> Result<usize> {
        sum(
            std::mem::size_of::<Self>(),
            sum(
                sum(self.seen.capacity(), self.region.capacity())?,
                bytes::<usize>(self.pending.capacity())?,
            )?,
        )
    }

    pub(super) fn reset(&mut self, context: &mut Context<'_, '_>) -> Result<()> {
        context.charge(sum(self.seen.len(), self.pending.len())?)?;
        self.seen.fill(0);
        self.pending.clear();
        Ok(())
    }

    pub(super) fn seed(&mut self, block: usize, context: &mut Context<'_, '_>) -> Result<()> {
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
pub(super) fn initialization(
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PendingSingle {
    source_address: usize,
    function: SemanticFunctionIdentityV1,
    initialization: ScalarAssignmentSiteV1,
    header: usize,
    preheader: usize,
    successor: usize,
    role: SemanticEdgeRoleV1,
    latch: usize,
    induction: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Single {
    source: PendingSingle,
    initial: ProductionRankedValueV1,
}

fn pending_binding_storage() -> usize {
    std::mem::size_of::<(&SemanticOperandV1, PendingSingle)>()
}

fn single_error(error: ProductionRankedProjectionErrorV1) -> ProductionRankedProjectionErrorV1 {
    let ProductionRankedProjectionErrorV1::Incomplete(reason) = error else {
        return error;
    };
    reject(match reason {
        "multi-entry induction has an escaped initialization binding" => {
            "separated induction has an escaped initialization binding"
        }
        "multi-entry induction does not have one outside initializer" => {
            "separated induction does not have one outside initializer"
        }
        "multi-entry induction has no outside initializer" => {
            "separated induction has no outside initializer"
        }
        "multi-entry initializer does not dominate every entry occurrence" => {
            "separated initializer does not dominate its entry occurrence"
        }
        "multi-entry induction initialization is killed before an entry" => {
            "separated induction initialization is killed before its entry"
        }
        "multi-entry induction initialization is killed at an entry endpoint" => {
            "separated induction initialization is killed at its entry endpoint"
        }
        "multi-entry induction retains an unnormalized false edge" => {
            "separated induction retains an unnormalized false edge"
        }
        "multi-entry traversal left the source block table" => {
            "separated traversal left the source block table"
        }
        other => other,
    })
}

fn temporary<T>(
    context: &mut Context<'_, '_>,
    action: impl FnOnce(&mut Context<'_, '_>) -> Result<T>,
) -> Result<T> {
    context.scope.check(context.facts)?;
    let retained = context.scope.retained;
    // This helper returns only fixed-size proof coordinates or unit. All owned
    // scratch in action is destroyed before any of its reservation is released.
    let result = action(context);
    let amount = context
        .scope
        .retained
        .checked_sub(retained)
        .ok_or_else(|| resource(Resource::Accounting))?;
    context.scope.release(context.facts, amount)?;
    result
}

fn typed_operand<'a>(
    function: &'a SemanticFunctionDeclV1,
    site: ScalarAssignmentSiteV1,
    induction: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    context: &mut Context<'_, '_>,
) -> Result<&'a SemanticOperandV1> {
    context.charge(8)?;
    let statement = function
        .blocks()
        .get(site.block)
        .and_then(|block| block.statements().get(site.statement))
        .ok_or_else(|| reject("separated initialization has a stale definition site"))?;
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Err(reject("separated initialization is not an assignment"));
    };
    let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
        return Err(reject("separated initialization is not an exact Use"));
    };
    if assignment.destination().local() != induction
        || !assignment.destination().projections().is_empty()
        || assignment.destination().ty() != ty
        || assignment.value().result_type() != ty
        || operand.ty() != ty
        || function
            .locals()
            .get(induction.index() as usize)
            .map(|local| local.ty())
            != Some(ty)
    {
        return Err(reject(
            "separated initialization changed its exact scalar binding",
        ));
    }
    Ok(operand)
}

fn exact_entry(
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    outside: &[bool],
    header: usize,
    preheader: usize,
    context: &mut Context<'_, '_>,
) -> Result<(usize, SemanticEdgeRoleV1)> {
    let mut found = None;
    for (block, source) in function.blocks().iter().enumerate() {
        context.charge(3)?;
        if !graph.reachable[block] || !outside[block] {
            continue;
        }
        let mut successor = 0;
        source
            .terminator()
            .kind()
            .try_for_each_edge(|edge| -> Result<()> {
                context.charge(4)?;
                if edge.target().index() as usize == header {
                    if block != preheader || found.replace((successor, edge.role())).is_some() {
                        return Err(reject(
                            "separated initialization requires one exact outside entry occurrence",
                        ));
                    }
                }
                successor = sum(successor, 1)?;
                Ok(())
            })?;
    }
    found.ok_or_else(|| reject("separated initialization lost its outside entry occurrence"))
}

#[allow(clippy::too_many_arguments)]
fn prove(
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    header: usize,
    preheader: usize,
    latch: usize,
    loop_blocks: &[usize],
    induction: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    context: &mut Context<'_, '_>,
) -> Result<PendingSingle> {
    temporary(context, |context| {
        context.charge(12)?;
        let count = function.blocks().len();
        if count == 0
            || header >= count
            || preheader >= count
            || latch >= count
            || graph.successors.len() != count
            || graph.predecessors.len() != count
            || graph.reachable.len() != count
            || graph.entry == header
            || !graph.reachable[header]
            || !graph.reachable[preheader]
            || !graph.reachable[latch]
        {
            return Err(reject("separated initialization has stale CFG coordinates"));
        }
        let mut definitions = 0usize;
        for block in function.blocks() {
            context.charge(2)?;
            for statement in block.statements() {
                context.charge(5)?;
                visit_statement_definition_places(statement.kind(), &mut |place| {
                    if place.local() == induction {
                        definitions = definitions.saturating_add(1);
                    }
                });
            }
            context.charge(2)?;
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && call
                    .destination()
                    .is_some_and(|destination| destination.place().local() == induction)
            {
                definitions = definitions.saturating_add(1);
            }
        }
        if definitions != 2 {
            return Err(reject(
                "separated initialization requires exactly its initializer and latch definitions",
            ));
        }
        let mut scratch = Scratch::new(count, context)?;
        scratch.seed(graph.entry, context)?;
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
        context.reserve(std::mem::size_of::<Vec<bool>>())?;
        let mut outside = context.backing(count)?;
        context.charge(count)?;
        outside.extend(scratch.seen.iter().map(|value| *value != 0));
        let (successor, role) = exact_entry(function, graph, &outside, header, preheader, context)?;
        let initialization = initialization(
            function,
            graph,
            &outside,
            header,
            latch,
            induction,
            &[preheader],
            &mut scratch,
            context,
        )
        .map_err(single_error)?;
        if initialization.block == preheader {
            return Err(reject(
                "separated initialization cannot replace the historical immediate path",
            ));
        }
        typed_operand(function, initialization, induction, ty, context)?;
        scratch.reset(context)?;
        scratch.seed(latch, context)?;
        while !scratch.pending.is_empty() {
            context.charge(3)?;
            let block = scratch
                .pending
                .pop()
                .ok_or_else(|| resource(Resource::Accounting))?;
            if block == header {
                continue;
            }
            for &source in &graph.predecessors[block] {
                context.charge(2)?;
                scratch.seed(source, context)?;
            }
        }
        context.charge(3)?;
        if scratch.seen[preheader] != 0 || graph.successors[latch].as_slice() != [header] {
            return Err(reject(
                "separated initialization lost its unique natural-loop boundary",
            ));
        }
        let mut ordinal = 0usize;
        for (block, &present) in scratch.seen.iter().enumerate() {
            context.charge(3)?;
            if present != 0 {
                if loop_blocks.get(ordinal) != Some(&block) {
                    return Err(reject(
                        "separated initialization changed its exact loop membership",
                    ));
                }
                ordinal = sum(ordinal, 1)?;
            }
        }
        if ordinal != loop_blocks.len() {
            return Err(reject("separated initialization has extra loop members"));
        }
        context.charge(64)?;
        Ok(PendingSingle {
            source_address: function as *const _ as usize,
            function: function.identity(),
            initialization,
            header,
            preheader,
            successor,
            role,
            latch,
            induction,
            ty,
        })
    })
}

pub(super) fn find_single<'a>(
    function: &'a SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    topology: &ProjectedNaturalLoopTopologyV1,
    header: usize,
    induction: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    context: &mut Context<'_, '_>,
) -> Result<(&'a SemanticOperandV1, PendingSingle)> {
    if matches!(
        topology.preheader_control,
        ProjectedInductionPreheaderControlV1::Multiple(_)
    ) {
        return Err(reject(
            "a separated single initializer cannot replace a multi-entry proof",
        ));
    }
    context.reserve(pending_binding_storage())?;
    let proof = prove(
        function,
        graph,
        header,
        topology.initializer_block,
        topology.latch,
        &topology.loop_blocks,
        induction,
        ty,
        context,
    )?;
    let operand = typed_operand(function, proof.initialization, induction, ty, context)?;
    Ok((operand, proof))
}

pub(super) fn bind_single(
    proof: PendingSingle,
    initial: ProductionRankedValueV1,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    context.charge(8)?;
    if context.scope.singles.len() != context.scope.registered
        || context
            .scope
            .singles
            .last()
            .is_some_and(|row| row.source.header >= proof.header)
    {
        return Err(reject(
            "separated initialization registry lost its ordered roster",
        ));
    }
    if context.scope.singles.len() == context.scope.singles.capacity() {
        let count = sum(
            context.scope.singles.capacity(),
            context.scope.singles.capacity().max(1),
        )?;
        context.reserve(std::mem::size_of::<Vec<Single>>())?;
        let mut replacement = context.backing(count)?;
        context.charge(context.scope.singles.len())?;
        let old = std::mem::take(&mut context.scope.singles);
        let previous = bytes::<Single>(old.capacity())?;
        replacement.extend(old);
        context.scope.singles = replacement;
        context.scope.release(
            context.facts,
            sum(previous, std::mem::size_of::<Vec<Single>>())?,
        )?;
    }
    context.scope.singles.push(Single {
        source: proof,
        initial,
    });
    context.scope.registered = sum(context.scope.registered, 1)?;
    context
        .scope
        .release(context.facts, pending_binding_storage())
}

pub(super) fn replay_roster(
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    inductions: &[ProjectedUniformInductionV1],
    context: &mut Context<'_, '_>,
) -> Result<()> {
    if inductions.is_empty() && context.scope.registered == 0 && context.scope.singles.is_empty() {
        return Ok(());
    }
    temporary(context, |context| {
        let mut matched = 0usize;
        context.charge(2)?;
        if context.scope.registered != context.scope.singles.len() {
            return Err(reject(
                "separated initialization registry omitted a registered proof",
            ));
        }
        context.charge(
            context
                .scope
                .singles
                .len()
                .checked_mul(2)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        for pair in context.scope.singles.windows(2) {
            if pair[0].source.header >= pair[1].source.header {
                return Err(reject(
                    "separated initialization registry is not strictly ordered",
                ));
            }
        }
        context.reserve(std::mem::size_of::<Vec<u8>>())?;
        let count = context.scope.registered;
        let mut visited = context.backing::<u8>(count)?;
        context.charge(count)?;
        visited.resize(count, 0);
        for induction in inductions {
            context.charge(4)?;
            if matches!(
                induction.preheader_control,
                ProjectedInductionPreheaderControlV1::Multiple(_)
            ) {
                continue;
            }
            let local = induction.source_progress.induction;
            let block = function
                .blocks()
                .get(induction.initializer_block)
                .ok_or_else(|| reject("separated initialization preheader left its source"))?;
            let mut immediate = false;
            for statement in block.statements() {
                context.charge(3)?;
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                    && assignment.destination().local() == local
                    && assignment.destination().projections().is_empty()
                {
                    immediate = true;
                }
            }
            context.charge(sum(
                2,
                usize::BITS as usize - context.scope.singles.len().leading_zeros() as usize,
            )?)?;
            let index = context
                .scope
                .singles
                .binary_search_by_key(&induction.header, |row| row.source.header);
            if immediate {
                if index.is_ok() {
                    return Err(reject(
                        "separated proof attached to a historical immediate initializer",
                    ));
                }
                continue;
            }
            let index = index
                .map_err(|_| reject("separated initialization is missing its registered proof"))?;
            context.charge(2)?;
            if visited[index] != 0 {
                return Err(reject("separated initialization candidate occurs twice"));
            }
            visited[index] = 1;
            let copies = sum(
                std::mem::size_of::<Single>(),
                std::mem::size_of::<PendingSingle>(),
            )?;
            context.reserve(copies)?;
            let row = context.scope.singles[index];
            context.charge(64)?;
            if row.source.source_address != function as *const _ as usize
                || row.source.function != function.identity()
                || row.source.induction != local
                || row.source.ty != induction.source_progress.induction_type
                || row.source.preheader != induction.initializer_block
                || row.source.latch != induction.latch
                || row.initial != induction.initial
            {
                return Err(reject(
                    "separated initialization lost its exact source or ranked binding",
                ));
            }
            let actual = prove(
                function,
                graph,
                induction.header,
                induction.initializer_block,
                induction.latch,
                &induction.loop_blocks,
                local,
                induction.source_progress.induction_type,
                context,
            )?;
            context.charge(64)?;
            if actual != row.source {
                return Err(reject(
                    "separated initialization changed before ranked emission",
                ));
            }
            context.scope.release(context.facts, copies)?;
            matched = sum(matched, 1)?;
        }
        if matched != context.scope.registered {
            return Err(reject(
                "separated initialization registry has an extra or duplicate candidate",
            ));
        }
        Ok(())
    })
}

fn require_historical(
    function: &SemanticFunctionDeclV1,
    inductions: &[ProjectedUniformInductionV1],
    mut charge: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    for induction in inductions {
        charge(4)?;
        if matches!(
            induction.preheader_control,
            ProjectedInductionPreheaderControlV1::Multiple(_)
        ) {
            continue;
        }
        let block = function
            .blocks()
            .get(induction.initializer_block)
            .ok_or_else(|| reject("separated initialization preheader left its source"))?;
        charge(
            block
                .statements()
                .len()
                .checked_mul(3)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        if !block.statements().iter().any(|statement| matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment) if assignment.destination().local() == induction.source_progress.induction && assignment.destination().projections().is_empty())) {
            return Err(reject("separated initialization is missing its registered proof"));
        }
    }
    Ok(())
}

pub(super) fn without_scope(
    function: &SemanticFunctionDeclV1,
    inductions: &[ProjectedUniformInductionV1],
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> Result<()> {
    require_historical(function, inductions, |work| {
        facts.charge_private_array_work(work)
    })
}

pub(super) fn without_context(
    function: &SemanticFunctionDeclV1,
    inductions: &[ProjectedUniformInductionV1],
) -> Result<()> {
    let mut work = 0;
    require_historical(function, inductions, |amount| {
        project_loop_graph_charge_v1(&mut work, amount)
    })
}

pub(super) fn before_emission(
    function: &SemanticFunctionDeclV1,
    inductions: &[ProjectedUniformInductionV1],
    context: &mut Context<'_, '_>,
) -> Result<()> {
    if inductions.is_empty() && context.scope.registered == 0 && context.scope.singles.is_empty() {
        return Ok(());
    }
    if context.scope.registered == 0 && context.scope.singles.is_empty() {
        // Historical immediate/multiple entries need no additional CFG owner.
        // A missing entire registry still cannot admit a separated candidate.
        return require_historical(function, inductions, |work| context.charge(work));
    }
    temporary(context, |context| {
        let graph = projected_loop_cfg_graph_paid_v1(function, context)?;
        replay_roster(function, &graph, inductions, context)
    })
}
