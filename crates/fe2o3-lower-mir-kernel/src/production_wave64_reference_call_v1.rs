//! Borrowed source-SSA relation for the Wave64 arithmetic receiver and context.
//! This issues no EXEC, launch, convergence, target or artifact authority.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{SsaResolvedEventV1, SsaValueV1};
use fe2o3_pliron::{
    ProductionSemanticSsaEntryOriginV1 as EntryOrigin,
    ProductionSemanticSsaEventRoleV1 as EventRole,
    ProductionSemanticSsaFunctionOccurrencesV1 as Occurrences,
    ProductionSemanticSsaOccurrenceSiteV1 as Site,
    ProductionSemanticSsaOperandRoleV1 as OperandRole,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Debug)]
pub(super) enum ReferenceErrorV1 {
    Resource(Resource),
    Unavailable(&'static str),
    CallbackPanicked,
}
impl From<Resource> for ReferenceErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
type ResultV1<T> = Result<T, ReferenceErrorV1>;
fn refuse<T>(why: &'static str) -> ResultV1<T> {
    Err(ReferenceErrorV1::Unavailable(why))
}
fn unique<T>(slot: &mut Option<T>, value: T) -> ResultV1<()> {
    if slot.replace(value).is_some() {
        refuse("ambiguous original SSA occurrence")
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Context,
    LaneCarrier,
    LaneValue,
    Snapshot { lane_field: u32 },
    Zst,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceOrigin {
    RootIssuedSsa {
        function: SemanticFunctionIdV1,
        value: SsaValueV1,
    },
    RootRetainedCall {
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        local: SemanticLocalIdV1,
        context_type: SemanticTypeIdV1,
    },
    ZeroState,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Meaning {
    kind: Kind,
    ty: SemanticTypeIdV1,
    referent: Option<SemanticTypeIdV1>,
    origin: SourceOrigin,
}

/// Copyable coordinates are inspection data, never detached source authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ArgumentV1 {
    Snapshot {
        reference_type: SemanticTypeIdV1,
        snapshot_type: SemanticTypeIdV1,
        lane_field: u32,
    },
    Rank {
        scalar_type: SemanticTypeIdV1,
    },
    Context {
        reference_type: SemanticTypeIdV1,
        context_type: SemanticTypeIdV1,
    },
}
pub(super) struct CheckedReferenceCallV1<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    helper: SemanticFunctionIdV1,
    arguments: [ArgumentV1; 2],
    meanings: [Meaning; 2],
}
impl CheckedReferenceCallV1<'_> {
    pub(super) fn owner(&self) -> &ProductionSemanticSsaOwnerV1 {
        self.owner
    }
    pub(super) fn caller(&self) -> SemanticFunctionIdV1 {
        self.caller
    }
    pub(super) fn block(&self) -> SemanticBlockIdV1 {
        self.block
    }
    pub(super) fn helper(&self) -> SemanticFunctionIdV1 {
        self.helper
    }
    pub(super) fn argument(&self, index: usize) -> Option<ArgumentV1> {
        self.arguments.get(index).copied()
    }
    pub(super) fn helper_declaration(&self) -> &SemanticFunctionDeclV1 {
        &self.owner.source_semantic().functions()[self.helper.index() as usize]
    }
    pub(super) fn source_call(&self) -> &SemanticDirectCallV1 {
        call_at(self.owner, self.caller, self.block).expect("checked immutable source call")
    }
    // Exact origins remain borrowed through this relation, not recreated from types.
    pub(super) fn same_source_origins(&self, other: &Self) -> bool {
        std::ptr::eq(self.owner, other.owner) && self.meanings == other.meanings
    }
}

fn call_at(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
) -> Option<&SemanticDirectCallV1> {
    match owner
        .source_semantic()
        .functions()
        .get(function.index() as usize)?
        .blocks()
        .get(block.index() as usize)?
        .terminator()
        .kind()
    {
        SemanticTerminatorKindV1::Call(call) => Some(call),
        _ => None,
    }
}
fn rows(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
) -> ResultV1<Occurrences<'_>> {
    owner
        .occurrences_v1()
        .and_then(|capture| capture.function(function))
        .ok_or(ReferenceErrorV1::Unavailable(
            "original source SSA occurrences required",
        ))
}
fn site(block: SemanticBlockIdV1) -> Site {
    Site::Terminator {
        block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
    }
}
fn base_use(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    site: Site,
    role: OperandRole,
    budget: &mut Budget<'_>,
) -> ResultV1<SsaValueV1> {
    let mut found = None;
    for event in rows(owner, function)?.events() {
        budget.charge_work(1)?;
        if event.site() == site && event.operand() == role && event.role() == EventRole::BaseUse {
            let Some(SsaResolvedEventV1::Use { value, .. }) = event.resolved() else {
                return refuse("original operand has no SSA use");
            };
            if !event.is_reachable() || !event.is_promoted() {
                return refuse("original operand is unreachable or retained");
            }
            unique(&mut found, value)?;
        }
    }
    found.ok_or(ReferenceErrorV1::Unavailable(
        "original operand SSA occurrence absent",
    ))
}
fn result_value(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    budget: &mut Budget<'_>,
) -> ResultV1<Option<SsaValueV1>> {
    let call =
        call_at(owner, function, block).ok_or(ReferenceErrorV1::Unavailable("producer call"))?;
    let destination = call
        .destination()
        .ok_or(ReferenceErrorV1::Unavailable("producer destination"))?;
    if !destination.place().projections().is_empty()
        || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
    {
        return refuse("projected or exceptional producer destination");
    }
    let local = destination.place().local().index();
    let occurrences = rows(owner, function)?;
    let mut found = None;
    let mut retained = false;
    for edge in occurrences.edge_definitions() {
        budget.charge_work(1)?;
        if edge.edge().source().get() == block.index() && edge.variable().get() == local {
            if edge.edge().ordinal() != 0 || edge.ordinal() != 0 || !edge.is_reachable() {
                return refuse("unavailable original producer edge");
            }
            if edge.is_promoted() {
                if retained {
                    return refuse("mixed source producer storage classes");
                }
                unique(
                    &mut found,
                    edge.value().ok_or(ReferenceErrorV1::Unavailable(
                        "promoted producer lacks SSA value",
                    ))?,
                )?;
            } else {
                // An unrelated retained issuer is not an SSA candidate. Its
                // original call/local ancestry is checked separately at Borrow.
                if edge.value().is_some() || retained || found.is_some() {
                    return refuse("inconsistent retained original producer edge");
                }
                retained = true;
            }
        }
    }
    for event in occurrences.events() {
        budget.charge_work(1)?;
        if event.site() == site(block)
            && event.role() == EventRole::DestinationDefine
            && let Some(SsaResolvedEventV1::Define { variable, value }) = event.resolved()
            && variable.get() == local
        {
            if !event.is_reachable() || !event.is_promoted() {
                return refuse("unavailable producer definition");
            }
            unique(&mut found, value)?;
        }
    }
    Ok(found)
}

struct Resolver<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    visits: usize,
}
impl<'a> Resolver<'a> {
    fn declaration(&self, ty: SemanticTypeIdV1) -> ResultV1<&'a SemanticTypeDeclV1> {
        self.owner
            .source_semantic()
            .types()
            .get(ty.index() as usize)
            .ok_or(ReferenceErrorV1::Unavailable("source type coordinate"))
    }
    fn is_u32(&self, ty: SemanticTypeIdV1) -> bool {
        self.declaration(ty).is_ok_and(|decl| {
            decl.rust_type_kind() == SemanticRustTypeKindV1::Ordinary
                && matches!(
                    decl.shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32
                    })
                )
        })
    }
    fn is_f32(&self, ty: SemanticTypeIdV1) -> bool {
        self.declaration(ty).is_ok_and(|decl| {
            decl.rust_type_kind() == SemanticRustTypeKindV1::Ordinary
                && matches!(
                    decl.shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 })
                )
        })
    }
    fn zst(&self, ty: SemanticTypeIdV1) -> bool {
        self.declaration(ty)
            .is_ok_and(|decl| decl.layout().size_bytes() == Some(0))
    }

    fn exact_lane_field(&self, ty: SemanticTypeIdV1, field: u32) -> ResultV1<bool> {
        let SemanticTypeShapeV1::Aggregate(fields) = self.declaration(ty)?.shape() else {
            return refuse("current lane carrier is not its original aggregate");
        };
        if fields.fields().len() > 8 {
            return refuse("current lane field census");
        }
        let mut lane = None;
        for (index, ty) in fields.fields().iter().copied().enumerate() {
            if self.zst(ty) {
                continue;
            }
            if !self.is_u32(ty) {
                return refuse("current lane has non-u32 runtime state");
            }
            unique(&mut lane, index as u32)?;
        }
        Ok(lane == Some(field))
    }
    // Address-taken arithmetic snapshots are not promoted SSA. Authenticate
    // their actual unique source initializer; do not invent an SSA producer.
    // A consuming call reads the reference holder too. Ending storage after
    // Borrow alone is insufficient: every original later referent/holder read
    // must be disconnected from this kill, including paths around backedges.
    fn require_snapshot_dead_suffix(
        &self,
        function: SemanticFunctionIdV1,
        local: SemanticLocalIdV1,
        dead_block: usize,
        dead_statement: usize,
        budget: &mut Budget<'_>,
    ) -> ResultV1<()> {
        let declaration = &self.owner.source_semantic().functions()[function.index() as usize];
        if declaration.blocks().len() > 128 || dead_block >= declaration.blocks().len() {
            return refuse("retained lifetime CFG coordinate");
        }
        let mut holders = [u32::MAX; 128];
        let mut holder_count = 0;
        let mut closure = [None; 128];
        for (bi, block) in declaration.blocks().iter().enumerate() {
            for (si, statement) in block.statements().iter().enumerate() {
                budget.charge_work(1)?;
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else { continue };
                if matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared, place
                } if place.local() == local && place.projections().is_empty()) {
                    if !assignment.destination().projections().is_empty() || holder_count == holders.len() {
                        return refuse("retained lifetime shared-holder census");
                    }
                    holders[holder_count] = assignment.destination().local().index();
                    closure[holder_count] = Some((function, assignment.destination().local().index(),
                        Some((SemanticBlockIdV1::from_index(bi as u32), si as u32))));
                    holder_count += 1;
                }
            }
        }
        // Inspect direct root holders and their exact original formal0 only.
        // Formal0 may read scalar fields; further whole-reference forwarding
        // fails closed. No checked_call recursion or new authority is issued.
        let mut closure_len = holder_count;
        let mut next = 0;
        while next < closure_len {
            budget.charge_work(4)?;
            let (holder_function, holder, borrow) = closure[next]
                .ok_or(ReferenceErrorV1::Unavailable("retained lifetime holder task"))?;
            next += 1;
            match super::require_unwritten_wave64_local_v1(self.owner, holder_function, holder, borrow, budget) {
                Ok(()) => {}
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) => return Err(error.into()),
                Err(error) => {
                    let kind = if let ProductionSemanticKirErrorV1::Unsupported {
                        function, block: Some(block), statement: Some(statement), ..
                    } = &error {
                        self.owner.source_semantic().functions().get(*function as usize)
                            .and_then(|f| f.blocks().get(*block as usize))
                            .and_then(|b| b.statements().get(*statement as usize))
                            .map(|s| match s.kind() {
                                SemanticStatementKindV1::StorageLive(_) => "StorageLive",
                                SemanticStatementKindV1::StorageDead(_) => "StorageDead",
                                SemanticStatementKindV1::Assign(_) => "Assign",
                                _ => "other",
                            }).unwrap_or("absent")
                    } else { "no statement coordinate" };
                    eprintln!("Wave64 snapshot holder {} in function {}: {:?}; source kind {}",
                        holder, holder_function.index(), error, kind);
                    return refuse("retained snapshot holder lifetime or source write around storage end");
                }
            }
            let body = &self.owner.source_semantic().functions()[holder_function.index() as usize];
            let reference_type = body.locals().get(holder as usize)
                .ok_or(ReferenceErrorV1::Unavailable("retained lifetime holder local"))?.ty();
            let SemanticTypeShapeV1::Pointer(reference) = self.declaration(reference_type)?.shape() else {
                return refuse("retained lifetime holder is not a source reference");
            };
            if reference.kind() != SemanticPointerKindV1::Reference
                || reference.mutability() != SemanticMutabilityV1::Immutable
                || reference.address_space() != 0 || reference.pointer_width_bits() != 64
                || reference.metadata() != SemanticPointerMetadataV1::None
                || reference.pointee() != declaration.locals()[local.index() as usize].ty()
            {
                return refuse("retained lifetime holder reference shape changed");
            }
            for event in rows(self.owner, holder_function)?.events() {
                budget.charge_work(1)?;
                if !matches!(event.event(), fe2o3_mir_model::SsaEventV1::Use(variable) if variable.get() == holder) {
                    continue;
                }
                let allowed = match (event.site(), event.operand()) {
                    (Site::Terminator { block }, OperandRole::CallArgument(0)) => {
                        if borrow.is_none() {
                            return refuse("retained snapshot formal whole-reference forwarding unavailable");
                        }
                        let call = call_at(self.owner, holder_function, SemanticBlockIdV1::from_index(block.get()))
                            .ok_or(ReferenceErrorV1::Unavailable("retained lifetime holder source call"))?;
                        let Some(SemanticCallableDeclV1::Defined { function: callee }) = self.owner.source_semantic()
                            .callables().get(call.callee().index() as usize) else {
                            return refuse("retained snapshot holder foreign call escape around storage end");
                        };
                        let callee_body = &self.owner.source_semantic().functions()[callee.index() as usize];
                        let abi = callee_body.abi();
                        if callee_body.role() != SemanticFunctionRoleV1::InternalHelper
                            || callee_body.export().is_some() || abi.source_input_types().len() != 3
                            || abi.source_input_types()[0] != reference_type
                            || abi.source_argument_ownership()[0] != SemanticSourceArgumentOwnershipV1::SharedBorrow
                            || !self.is_f32(abi.source_output_type())
                            || !matches!(call.arguments().first(), Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place))
                                if place.local().index() == holder && place.projections().is_empty() && place.ty() == reference_type)
                        {
                            return refuse("retained snapshot holder is not an exact shared source formal");
                        }
                        let mut formal = None;
                        for (index, row) in callee_body.locals().iter().enumerate() {
                            budget.charge_work(1)?;
                            if row.role() == SemanticLocalRoleV1::Argument(0) {
                                if row.ty() != reference_type { return refuse("retained lifetime formal type changed"); }
                                unique(&mut formal, index as u32)?;
                            }
                        }
                        let formal = formal.ok_or(ReferenceErrorV1::Unavailable("retained lifetime formal absent"))?;
                        budget.charge_work(closure_len)?;
                        if !closure[..closure_len].iter().flatten().any(|(function, local, _)| function == callee && *local == formal) {
                            if closure_len == closure.len() { return refuse("retained lifetime formal census"); }
                            closure[closure_len] = Some((*callee, formal, None));
                            closure_len += 1;
                        }
                        true
                    }
                    (Site::Statement { block, statement }, OperandRole::RvalueOperand(ordinal)) => {
                        let mut allowed = false;
                        let original = body.blocks().get(block.get() as usize)
                            .and_then(|block| block.statements().get(statement as usize));
                        if let Some(SemanticStatementKindV1::Assign(assignment)) = original.map(SemanticStatementV1::kind) {
                            let mut index = 0u32;
                            assignment.value().kind().try_visit_operands(|operand| {
                                budget.charge_work(1)?;
                                if index == ordinal {
                                    if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = operand {
                                        allowed = place.local().index() == holder
                                            && super::closed_wave64_scalar_projection_v1(self.owner, place);
                                    }
                                }
                                index += 1;
                                Ok::<(), ReferenceErrorV1>(())
                            })?;
                        }
                        allowed
                    }
                    _ => false,
                };
                if !allowed {
                    return refuse("retained snapshot holder alias or escape around storage end");
                }
            }
        }
        let occurrences = rows(self.owner, function)?;
        // Seed successors only. Reaching dead_block again means earlier
        // statements can execute again after the storage end.
        let mut reachable = [false; 128];
        for edge in occurrences.successors() {
            budget.charge_work(1)?;
            let from = edge.id().source().get() as usize;
            let to = edge.edge().target().index() as usize;
            if from >= declaration.blocks().len() || to >= declaration.blocks().len() {
                return refuse("retained lifetime source edge coordinate");
            }
            if from == dead_block { reachable[to] = true; }
        }
        for _ in 0..128 {
            let mut changed = false;
            for edge in occurrences.successors() {
                budget.charge_work(1)?;
                let from = edge.id().source().get() as usize;
                let to = edge.edge().target().index() as usize;
                if reachable[from] && !reachable[to] {
                    reachable[to] = true;
                    changed = true;
                }
            }
            if !changed { break; }
        }
        for event in occurrences.events() {
            budget.charge_work(1)?;
            let fe2o3_mir_model::SsaEventV1::Use(variable) = event.event() else { continue };
            budget.charge_work(holder_count)?;
            let holder = holders[..holder_count].contains(&variable.get());
            if variable.get() != local.index() && !holder {
                continue;
            }
            let (block, after_in_same_block) = match event.site() {
                Site::Statement { block, statement } =>
                    (block.get() as usize, statement as usize > dead_statement),
                Site::Terminator { block } => (block.get() as usize, true),
            };
            if block >= declaration.blocks().len() {
                return refuse("retained lifetime source use coordinate");
            }
            // Count unpromoted and unreachable original events too. Raw/alias
            // escapes remain independently refused by the closed-holder guard.
            if reachable[block] || (block == dead_block && after_in_same_block) {
                return refuse("retained snapshot source use can follow lifetime end");
            }
        }
        Ok(())
    }

    fn retained_snapshot(
        &mut self,
        function: SemanticFunctionIdV1,
        use_site: Site,
        place: &SemanticPlaceV1,
        depth: usize,
        budget: &mut Budget<'_>,
    ) -> ResultV1<Meaning> {
        self.charge(depth, budget)?;
        if function != self.root || !place.projections().is_empty() {
            return refuse("retained snapshot requires the exact root local");
        }
        let declaration = &self.owner.source_semantic().functions()[function.index() as usize];
        let (use_block, use_statement) = match use_site {
            Site::Statement { block, statement } => (block.get() as usize, statement as usize),
            _ => return refuse("retained referent is not an original shared borrow statement"),
        };
        if declaration.blocks().len() > 128 {
            return refuse("retained constructor CFG cap");
        }
        let local = place.local();
        let mut initializer = None;
        for (bi, block) in declaration.blocks().iter().enumerate() {
            for (si, statement) in block.statements().iter().enumerate() {
                budget.charge_work(1)?;
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        if assignment.destination().local() == local {
                            if !assignment.destination().projections().is_empty()
                                || !matches!(
                                    assignment.value().kind(),
                                    SemanticRvalueKindV1::Aggregate(_)
                                )
                            {
                                return refuse("retained snapshot has a nonconstructor write");
                            }
                            unique(&mut initializer, (bi, si, assignment.value()))?;
                        }
                        match assignment.value().kind() {
                            SemanticRvalueKindV1::AddressOf { place, .. }
                                if place.local() == local =>
                            {
                                return refuse("retained snapshot has raw address exposure");
                            }
                            SemanticRvalueKindV1::Borrow { kind, place }
                                if place.local() == local =>
                            {
                                if *kind != SemanticBorrowKindV1::Shared
                                    || !place.projections().is_empty()
                                {
                                    return refuse(
                                        "retained snapshot has mutable/projected exposure",
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    SemanticStatementKindV1::StorageDead(dead) if *dead == local => {
                        self.require_snapshot_dead_suffix(function, local, bi, si, budget)?;
                    }
                    SemanticStatementKindV1::Store(store)
                        if store.destination().local() == local =>
                    {
                        return refuse("retained snapshot source store");
                    }
                    SemanticStatementKindV1::Deinitialize(place) if place.local() == local => {
                        return refuse("retained snapshot deinitialization");
                    }
                    SemanticStatementKindV1::SetDiscriminant { place, .. }
                        if place.local() == local =>
                    {
                        return refuse("retained snapshot discriminant write");
                    }
                    SemanticStatementKindV1::AtomicRmw(atomic)
                        if atomic.destination().local() == local
                            || atomic.address().local() == local =>
                    {
                        return refuse("retained snapshot atomic write");
                    }
                    SemanticStatementKindV1::AtomicCompareExchange(atomic)
                        if atomic.destination().local() == local
                            || atomic.address().local() == local =>
                    {
                        return refuse("retained snapshot atomic compare exchange");
                    }
                    _ => {}
                }
            }
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && call
                    .destination()
                    .is_some_and(|d| d.place().local() == local)
            {
                return refuse("retained snapshot is overwritten by a call");
            }
        }
        let (constructor_block, constructor_statement, value) = initializer.ok_or(
            ReferenceErrorV1::Unavailable("retained source constructor absent"),
        )?;
        // StorageLive starts a fresh allocation lifetime. Only initial entry
        // lives or same-block lives before this unique constructor may precede
        // the already-required constructor domination of every borrow.
        for (bi, block) in declaration.blocks().iter().enumerate() {
            for (si, statement) in block.statements().iter().enumerate() {
                budget.charge_work(1)?;
                if matches!(statement.kind(), SemanticStatementKindV1::StorageLive(live) if *live == local)
                    && !((bi == constructor_block && si < constructor_statement)
                        || (bi == declaration.entry().index() as usize && bi != constructor_block))
                {
                    return refuse("retained snapshot allocation lifetime restarted");
                }
            }
        }
        if constructor_block == use_block {
            if constructor_statement >= use_statement {
                return refuse("constructor does not precede shared borrow");
            }
        } else {
            // A reachability search with the constructor block removed proves
            // dominance; the source successor inventory is the owner's capture.
            let mut reachable = [false; 128];
            let entry = declaration.entry().index() as usize;
            if entry != constructor_block {
                reachable[entry] = true;
            }
            for _ in 0..128 {
                let mut changed = false;
                for edge in rows(self.owner, function)?.successors() {
                    budget.charge_work(1)?;
                    let from = edge.id().source().get() as usize;
                    let to = edge.edge().target().index() as usize;
                    if from >= 128 || to >= 128 {
                        return refuse("retained source CFG coordinate");
                    }
                    if reachable[from] && to != constructor_block && !reachable[to] {
                        reachable[to] = true;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            if reachable[use_block] {
                return refuse("constructor does not dominate shared borrow");
            }
        }
        let meaning = self.rvalue(
            function,
            Site::Statement {
                block: fe2o3_mir_model::SsaBlockIdV1::new(constructor_block as u32),
                statement: constructor_statement as u32,
            },
            value.result_type(),
            value.kind(),
            depth + 1,
            budget,
        )?;
        if !matches!(meaning.kind, Kind::Snapshot { .. }) || meaning.ty != place.ty() {
            return refuse("retained referent is not the original arithmetic snapshot");
        }
        Ok(meaning)
    }

    // A borrowed zero-state context remains address-taken source storage. Its
    // exact root intrinsic destination is an issuer, never an invented SSA key.
    fn retained_context(
        &mut self,
        function: SemanticFunctionIdV1,
        use_site: Site,
        place: &SemanticPlaceV1,
        depth: usize,
        budget: &mut Budget<'_>,
    ) -> ResultV1<Meaning> {
        self.charge(depth, budget)?;
        if function != self.root || !place.projections().is_empty() || !self.zst(place.ty()) {
            return refuse("retained context requires exact zero-state root local");
        }
        let declaration = &self.owner.source_semantic().functions()[function.index() as usize];
        let Site::Statement {
            block: use_block, ..
        } = use_site
        else {
            return refuse("retained context must be original shared borrow statement");
        };
        if declaration.blocks().len() > 128 {
            return refuse("retained context CFG cap");
        }
        let local = place.local();
        let mut issuer = None;
        for (bi, block) in declaration.blocks().iter().enumerate() {
            for statement in block.statements() {
                budget.charge_work(1)?;
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        if assignment.destination().local() == local {
                            return refuse("retained context has a source assignment");
                        }
                        match assignment.value().kind() {
                            SemanticRvalueKindV1::AddressOf { place, .. }
                                if place.local() == local =>
                            {
                                return refuse("retained context has raw address exposure");
                            }
                            SemanticRvalueKindV1::Borrow { kind, place }
                                if place.local() == local =>
                            {
                                if *kind != SemanticBorrowKindV1::Shared
                                    || !place.projections().is_empty()
                                {
                                    return refuse(
                                        "retained context has mutable/projected exposure",
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    SemanticStatementKindV1::StorageDead(dead) if *dead == local => {
                        return refuse("retained context lifetime ends in source");
                    }
                    SemanticStatementKindV1::Store(store)
                        if store.destination().local() == local =>
                    {
                        return refuse("retained context source store");
                    }
                    SemanticStatementKindV1::Deinitialize(place) if place.local() == local => {
                        return refuse("retained context deinitialization");
                    }
                    SemanticStatementKindV1::SetDiscriminant { place, .. }
                        if place.local() == local =>
                    {
                        return refuse("retained context discriminant write");
                    }
                    SemanticStatementKindV1::AtomicRmw(atomic)
                        if atomic.destination().local() == local
                            || atomic.address().local() == local =>
                    {
                        return refuse("retained context atomic write");
                    }
                    SemanticStatementKindV1::AtomicCompareExchange(atomic)
                        if atomic.destination().local() == local
                            || atomic.address().local() == local =>
                    {
                        return refuse("retained context atomic compare exchange");
                    }
                    _ => {}
                }
            }
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let Some(destination) = call.destination().filter(|d| d.place().local() == local)
            else {
                continue;
            };
            if !destination.place().projections().is_empty()
                || destination.place().ty() != place.ty()
                || !call.arguments().is_empty()
                || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
                || !matches!(self.owner.source_semantic().callables().get(call.callee().index() as usize),
                    Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent { context }, ..
                    }) if *context == place.ty())
            {
                return refuse("retained context has nonissuer call write");
            }
            let producer = SemanticBlockIdV1::from_index(bi as u32);
            // The captured source edge must explicitly be reachable, retained,
            // valueless and the exact normal return destination of this call.
            let mut original_edge = None;
            for edge in rows(self.owner, function)?.edge_definitions() {
                budget.charge_work(1)?;
                if edge.edge().source().get() == producer.index()
                    && edge.variable().get() == local.index()
                {
                    if edge.edge().ordinal() != 0
                        || edge.ordinal() != 0
                        || !edge.is_reachable()
                        || edge.is_promoted()
                        || edge.value().is_some()
                    {
                        return refuse("retained context original call edge mismatch");
                    }
                    unique(&mut original_edge, ())?;
                }
            }
            if original_edge.is_none() {
                return refuse("retained context original edge absent");
            }
            unique(&mut issuer, producer)?;
        }
        let producer = issuer.ok_or(ReferenceErrorV1::Unavailable(
            "retained context exact issuer absent",
        ))?;
        // In the issuer block the call terminator follows every statement.
        // Entry lives also precede any path to this dominating call. All other
        // lifetime resets fail closed; none inherit the previous origin.
        for (bi, block) in declaration.blocks().iter().enumerate() {
            for statement in block.statements() {
                budget.charge_work(1)?;
                if matches!(statement.kind(), SemanticStatementKindV1::StorageLive(live) if *live == local)
                    && bi != producer.index() as usize
                    && bi != declaration.entry().index() as usize
                {
                    return refuse("retained context allocation lifetime restarted");
                }
            }
        }
        if producer.index() == use_block.get() {
            return refuse("context call cannot precede statement in its own terminator block");
        }
        // Removing the actual call block must disconnect the borrow from entry.
        let mut reachable = [false; 128];
        let entry = declaration.entry().index() as usize;
        if entry != producer.index() as usize {
            reachable[entry] = true;
        }
        for _ in 0..128 {
            let mut changed = false;
            for edge in rows(self.owner, function)?.successors() {
                budget.charge_work(1)?;
                let from = edge.id().source().get() as usize;
                let to = edge.edge().target().index() as usize;
                if from >= 128 || to >= 128 {
                    return refuse("retained context CFG coordinate");
                }
                if reachable[from] && to != producer.index() as usize && !reachable[to] {
                    reachable[to] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        if reachable[use_block.get() as usize] {
            return refuse("context issuer does not dominate shared borrow");
        }
        Ok(Meaning {
            kind: Kind::Context,
            ty: place.ty(),
            referent: None,
            origin: SourceOrigin::RootRetainedCall {
                function,
                block: producer,
                local,
                context_type: place.ty(),
            },
        })
    }
    fn charge(&mut self, depth: usize, budget: &mut Budget<'_>) -> ResultV1<()> {
        budget.charge_work(8)?;
        if depth >= 64 || self.visits >= 4096 {
            return refuse("reference ancestry depth/work cap");
        }
        self.visits += 1;
        Ok(())
    }
    fn shared(&self, mut meaning: Meaning, reference: SemanticTypeIdV1) -> ResultV1<Meaning> {
        let SemanticTypeShapeV1::Pointer(pointer) = self.declaration(reference)?.shape() else {
            return refuse("shared source borrow has no reference type");
        };
        if meaning.referent.is_some()
            || pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.mutability() != SemanticMutabilityV1::Immutable
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || pointer.address_space() != 0
            || pointer.pointer_width_bits() != 64
            || pointer.pointee() != meaning.ty
            || !matches!(
                meaning.kind,
                Kind::Context | Kind::Snapshot { .. } | Kind::LaneCarrier
            )
        {
            return refuse("borrow is not exact immutable reference to original referent");
        }
        meaning.referent = Some(meaning.ty);
        meaning.ty = reference;
        Ok(meaning)
    }
    fn project(&self, mut meaning: Meaning, place: &SemanticPlaceV1) -> ResultV1<Meaning> {
        for projection in place.projections() {
            match projection.kind() {
                SemanticProjectionKindV1::Dereference => {
                    let referent = meaning
                        .referent
                        .take()
                        .ok_or(ReferenceErrorV1::Unavailable(
                            "dereference lacks original shared borrow",
                        ))?;
                    if referent != projection.result_type() {
                        return refuse("dereference source type changed");
                    }
                    meaning.ty = referent;
                }
                SemanticProjectionKindV1::Field(field) if meaning.referent.is_none() => {
                    let SemanticTypeShapeV1::Aggregate(fields) =
                        self.declaration(meaning.ty)?.shape()
                    else {
                        return refuse("field source is not original aggregate");
                    };
                    let field_ty = *fields
                        .fields()
                        .get(field as usize)
                        .ok_or(ReferenceErrorV1::Unavailable("source field out of range"))?;
                    if field_ty != projection.result_type() {
                        return refuse("source field type changed");
                    }
                    meaning.kind = match meaning.kind {
                        Kind::Snapshot { lane_field }
                            if field == lane_field && self.is_u32(field_ty) =>
                        {
                            Kind::LaneValue
                        }
                        Kind::LaneCarrier if self.exact_lane_field(meaning.ty, field)? => {
                            Kind::LaneValue
                        }
                        _ if self.zst(field_ty) => Kind::Zst,
                        _ => return refuse("field does not carry the original physical lane"),
                    };
                    meaning.ty = field_ty;
                }
                _ => return refuse("raw, mutable, indexed or cast reference projection"),
            }
        }
        if meaning.ty != place.ty() {
            return refuse("final source projection type changed");
        }
        Ok(meaning)
    }
    fn operand(
        &mut self,
        function: SemanticFunctionIdV1,
        site: Site,
        role: OperandRole,
        operand: &SemanticOperandV1,
        depth: usize,
        budget: &mut Budget<'_>,
    ) -> ResultV1<Meaning> {
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                let value = base_use(self.owner, function, site, role, budget)?;
                let meaning = self.resolve(function, value, depth + 1, budget)?;
                self.project(meaning, place)
            }
            SemanticOperandV1::Constant(_) if self.zst(operand.ty()) => Ok(Meaning {
                kind: Kind::Zst,
                ty: operand.ty(),
                referent: None,
                origin: SourceOrigin::ZeroState,
            }),
            _ => refuse("constant cannot manufacture lane or context ancestry"),
        }
    }
    fn resolve(
        &mut self,
        function: SemanticFunctionIdV1,
        value: SsaValueV1,
        depth: usize,
        budget: &mut Budget<'_>,
    ) -> ResultV1<Meaning> {
        self.charge(depth, budget)?;
        let declaration = self
            .owner
            .source_semantic()
            .functions()
            .get(function.index() as usize)
            .ok_or(ReferenceErrorV1::Unavailable("source function coordinate"))?;
        // Only actual compiler operations at the original root seed authority.
        if function == self.root {
            for (index, block) in declaration.blocks().iter().enumerate() {
                budget.charge_work(1)?;
                let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                    continue;
                };
                let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) = self
                    .owner
                    .source_semantic()
                    .callables()
                    .get(call.callee().index() as usize)
                else {
                    continue;
                };
                let (kind, ty) = match operation {
                    SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
                        lane,
                        wave_width: 64,
                    } => (Kind::LaneCarrier, *lane),
                    SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent { context } => {
                        (Kind::Context, *context)
                    }
                    _ => continue,
                };
                let block_id = SemanticBlockIdV1::from_index(index as u32);
                if result_value(self.owner, function, block_id, budget)? == Some(value) {
                    if !call.arguments().is_empty()
                        || call.destination().is_none_or(|d| d.place().ty() != ty)
                    {
                        return refuse("compiler producer source type/arity changed");
                    }
                    return Ok(Meaning {
                        kind,
                        ty,
                        referent: None,
                        origin: SourceOrigin::RootIssuedSsa { function, value },
                    });
                }
            }
        }
        let occurrences = rows(self.owner, function)?;
        // Formals are not issuers. Resolve EVERY original incoming call back to
        // the actual root producer; foreign origins and recursion fail closed.
        for entry in occurrences.entry_definitions() {
            budget.charge_work(1)?;
            if entry.value() != Some(value) {
                continue;
            }
            let EntryOrigin::Argument(argument) = entry.origin() else {
                return refuse("non-positional reference formal");
            };
            if function == self.root {
                return refuse("kernel argument cannot issue context/lane");
            }
            let mut inherited = None;
            for (fi, caller) in self.owner.source_semantic().functions().iter().enumerate() {
                for (bi, block) in caller.blocks().iter().enumerate() {
                    budget.charge_work(1)?;
                    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                        continue;
                    };
                    if !matches!(self.owner.source_semantic().callables().get(call.callee().index() as usize),
                        Some(SemanticCallableDeclV1::Defined { function: callee }) if *callee == function)
                    {
                        continue;
                    }
                    let operand = call.arguments().get(argument as usize).ok_or(
                        ReferenceErrorV1::Unavailable("actual caller argument absent"),
                    )?;
                    let meaning = self.operand(
                        SemanticFunctionIdV1::from_index(fi as u32),
                        site(SemanticBlockIdV1::from_index(bi as u32)),
                        OperandRole::CallArgument(argument),
                        operand,
                        depth + 1,
                        budget,
                    )?;
                    if declaration
                        .abi()
                        .source_input_types()
                        .get(argument as usize)
                        != Some(&meaning.ty)
                    {
                        return refuse("formal/caller source type mismatch");
                    }
                    match inherited {
                        Some(previous) if previous != meaning => {
                            return refuse("distinct source origins join at formal");
                        }
                        None => inherited = Some(meaning),
                        _ => {}
                    }
                }
            }
            return inherited.ok_or(ReferenceErrorV1::Unavailable(
                "formal has no original caller",
            ));
        }
        match value {
            SsaValueV1::BlockArgument { block, variable } => {
                let plan = self
                    .owner
                    .plan_for_function(function)
                    .ok_or(ReferenceErrorV1::Unavailable("actual SSA plan"))?;
                let mut inherited = None;
                for successor in occurrences.successors() {
                    budget.charge_work(1)?;
                    if successor.edge().target().index() != block.get() {
                        continue;
                    }
                    let arguments = plan
                        .plan()
                        .edge_arguments(successor.id())
                        .ok_or(ReferenceErrorV1::Unavailable("actual SSA edge arguments"))?;
                    for argument in arguments {
                        budget.charge_work(1)?;
                        if argument.variable() != variable {
                            continue;
                        }
                        if argument.value() == value {
                            continue;
                        }
                        let meaning =
                            self.resolve(function, argument.value(), depth + 1, budget)?;
                        match inherited {
                            Some(previous) if previous != meaning => {
                                return refuse("distinct source origins join at block");
                            }
                            None => inherited = Some(meaning),
                            _ => {}
                        }
                    }
                }
                inherited.ok_or(ReferenceErrorV1::Unavailable(
                    "SSA block argument has no origin",
                ))
            }
            SsaValueV1::Definition(_) => {
                let mut definition = None;
                for event in occurrences.events() {
                    budget.charge_work(1)?;
                    if matches!(event.resolved(), Some(SsaResolvedEventV1::Define { value: actual, .. }) if actual == value)
                    {
                        if !event.is_reachable() || !event.is_promoted() {
                            return refuse("unavailable source definition");
                        }
                        unique(&mut definition, event.site())?;
                    }
                }
                let Some(site @ Site::Statement { block, statement }) = definition else {
                    return refuse("non-issuer source call result is not reference ancestry");
                };
                let statement = declaration
                    .blocks()
                    .get(block.get() as usize)
                    .and_then(|b| b.statements().get(statement as usize))
                    .ok_or(ReferenceErrorV1::Unavailable("source statement coordinate"))?;
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    return refuse("source definition is not assignment");
                };
                if !assignment.destination().projections().is_empty() {
                    return refuse("projected source definition");
                }
                self.rvalue(
                    function,
                    site,
                    assignment.value().result_type(),
                    assignment.value().kind(),
                    depth + 1,
                    budget,
                )
            }
        }
    }
    fn rvalue(
        &mut self,
        function: SemanticFunctionIdV1,
        site: Site,
        ty: SemanticTypeIdV1,
        rvalue: &SemanticRvalueKindV1,
        depth: usize,
        budget: &mut Budget<'_>,
    ) -> ResultV1<Meaning> {
        self.charge(depth, budget)?;
        let meaning = match rvalue {
            SemanticRvalueKindV1::Use(operand) => self.operand(
                function,
                site,
                OperandRole::RvalueOperand(0),
                operand,
                depth + 1,
                budget,
            )?,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } => {
                // Elision is not storage classification: a normal retained
                // BaseUse remains in the captured syntax with no SSA resolution.
                let occurrences = rows(self.owner, function)?;
                let mut retained = occurrences.elisions().contains(&site);
                if !retained {
                    let mut original_place = None;
                    for event in occurrences.events() {
                        budget.charge_work(1)?;
                        if event.site() != site
                            || event.operand() != OperandRole::RvaluePlace
                            || event.role() != EventRole::BaseUse
                        {
                            continue;
                        }
                        if !matches!(event.event(), fe2o3_mir_model::SsaEventV1::Use(variable)
                            if variable.get() == place.local().index())
                            || !event.is_reachable()
                        {
                            return refuse("original shared borrow place occurrence mismatch");
                        }
                        if event.is_promoted() {
                            if !matches!(event.resolved(), Some(SsaResolvedEventV1::Use { variable, .. })
                                if variable.get() == place.local().index())
                            {
                                return refuse(
                                    "promoted shared borrow place lacks original SSA use",
                                );
                            }
                            unique(&mut original_place, false)?;
                        } else {
                            if event.resolved().is_some()
                                || function != self.root
                                || !place.projections().is_empty()
                            {
                                return refuse(
                                    "retained shared borrow lacks exact root-local source storage",
                                );
                            }
                            unique(&mut original_place, true)?;
                        }
                    }
                    retained = original_place.ok_or(ReferenceErrorV1::Unavailable(
                        "original shared borrow place occurrence absent",
                    ))?;
                }
                let meaning = if retained {
                    if self.zst(place.ty()) {
                        self.retained_context(function, site, place, depth + 1, budget)?
                    } else {
                        self.retained_snapshot(function, site, place, depth + 1, budget)?
                    }
                } else {
                    let value =
                        base_use(self.owner, function, site, OperandRole::RvaluePlace, budget)?;
                    let meaning = self.resolve(function, value, depth + 1, budget)?;
                    self.project(meaning, place)?
                };
                self.shared(meaning, ty)?
            }

            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand,
            } => {
                let mut meaning = self.operand(
                    function,
                    site,
                    OperandRole::RvalueOperand(0),
                    operand,
                    depth + 1,
                    budget,
                )?;
                if meaning.kind != Kind::LaneValue
                    || meaning.referent.is_some()
                    || !matches!(
                        self.declaration(meaning.ty)?.shape(),
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            signed: false,
                            bits: 32 | 64
                        })
                    )
                    || !matches!(
                        self.declaration(ty)?.shape(),
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            signed: false,
                            bits: 32 | 64
                        })
                    )
                {
                    return refuse("integer cast is not identity transport of original lane");
                }
                meaning.ty = ty;
                meaning
            }
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Remainder,
                left,
                right,
            } => {
                let meaning = self.operand(
                    function,
                    site,
                    OperandRole::RvalueOperand(0),
                    left,
                    depth + 1,
                    budget,
                )?;
                if meaning.kind != Kind::LaneValue
                    || meaning.referent.is_some()
                    || meaning.ty != ty
                    || exact_unsigned_semantic_constant_v1(
                        self.owner.source_semantic().types(),
                        right,
                        ty,
                    ) != Some(64)
                {
                    return refuse("rank remainder is not exact identity modulo Wave64");
                }
                meaning
            }
            SemanticRvalueKindV1::Aggregate(aggregate)
                if matches!(aggregate.kind(), SemanticAggregateKindV1::Aggregate) =>
            {
                let SemanticTypeShapeV1::Aggregate(fields) = self.declaration(ty)?.shape() else {
                    return refuse("source snapshot constructor is not aggregate");
                };
                if aggregate.operands().len() != fields.fields().len()
                    || aggregate.operands().len() > 8
                {
                    return refuse("source snapshot field census changed");
                }
                let mut lane = None;
                for (index, operand) in aggregate.operands().iter().enumerate() {
                    budget.charge_work(1)?;
                    let expected = fields.fields()[index];
                    if operand.ty() != expected {
                        return refuse("source snapshot field type mismatch");
                    }
                    let meaning = self.operand(
                        function,
                        site,
                        OperandRole::RvalueOperand(index as u32),
                        operand,
                        depth + 1,
                        budget,
                    )?;
                    if meaning.kind == Kind::LaneValue
                        && meaning.referent.is_none()
                        && self.is_u32(expected)
                    {
                        unique(&mut lane, (index as u32, meaning))?;
                    } else if meaning.kind != Kind::Zst || !self.zst(expected) {
                        return refuse(
                            "snapshot contains state without original current-lane ancestry",
                        );
                    }
                }
                let (lane_field, mut lane) = lane.ok_or(ReferenceErrorV1::Unavailable(
                    "snapshot has no issued lane component",
                ))?;
                lane.kind = Kind::Snapshot { lane_field };
                lane.ty = ty;
                lane
            }
            _ => return refuse("computed, mutable, raw or reconstructed reference ancestry"),
        };
        if meaning.ty != ty {
            return refuse("source assignment type mismatch");
        }
        Ok(meaning)
    }
}

fn derive<'a>(
    owner: &'a ProductionSemanticSsaOwnerV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    budget: &mut Budget<'_>,
) -> ResultV1<CheckedReferenceCallV1<'a>> {
    let source = owner.source_semantic();
    if source.roots().len() != 1 || source.functions().len() > 32 {
        return refuse("one actual root and bounded helper roster required");
    }
    let root = source.roots()[0];
    let root_decl = &source.functions()[root.index() as usize];
    if root_decl.role() != SemanticFunctionRoleV1::KernelRoot || root_decl.kernel_entry().is_none()
    {
        return refuse("original kernel root required");
    }
    let call = call_at(owner, caller, block)
        .ok_or(ReferenceErrorV1::Unavailable("original Defined call"))?;
    let Some(SemanticCallableDeclV1::Defined { function: helper }) =
        source.callables().get(call.callee().index() as usize)
    else {
        return refuse("reference transport requires actual Defined call");
    };
    let helper_decl = &source.functions()[helper.index() as usize];
    let abi = helper_decl.abi();
    if helper_decl.role() != SemanticFunctionRoleV1::InternalHelper
        || helper_decl.export().is_some()
        || call.arguments().len() != 3
        || abi.source_input_types().len() != 3
        || !call.variadic_argument_abis().is_empty()
        || abi.can_unwind()
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
        || call.destination().is_none_or(|d| {
            !d.place().projections().is_empty() || d.place().ty() != abi.source_output_type()
        })
    {
        return refuse("closed Wave64 helper source signature unavailable");
    }
    let mut resolver = Resolver {
        owner,
        root,
        visits: 0,
    };
    if !resolver.is_f32(abi.source_output_type())
        || !resolver.is_f32(call.arguments()[2].ty())
        || abi.source_input_types()[2] != call.arguments()[2].ty()
    {
        return refuse("closed Wave64 helper scalar is not f32");
    }
    let mut meanings = [None; 2];
    let mut arguments = [None; 2];
    for index in 0..2 {
        let operand = &call.arguments()[index];
        if operand.ty() != abi.source_input_types()[index] {
            return refuse("actual helper source argument type changed");
        }
        let meaning = resolver.operand(
            caller,
            site(block),
            OperandRole::CallArgument(index as u32),
            operand,
            0,
            budget,
        )?;
        arguments[index] = Some(match (index, meaning.kind, meaning.referent) {
            (0, Kind::Snapshot { lane_field }, Some(snapshot_type)) => {
                if abi.source_argument_ownership()[index]
                    != SemanticSourceArgumentOwnershipV1::SharedBorrow
                {
                    return refuse("snapshot receiver is not original shared borrow");
                }
                ArgumentV1::Snapshot {
                    reference_type: meaning.ty,
                    snapshot_type,
                    lane_field,
                }
            }
            (0, Kind::LaneValue, None) if resolver.is_u32(meaning.ty) => {
                if abi.source_argument_ownership()[index]
                    != SemanticSourceArgumentOwnershipV1::ByValue
                {
                    return refuse("rank is not original by-value scalar");
                }
                ArgumentV1::Rank {
                    scalar_type: meaning.ty,
                }
            }
            (1, Kind::Context, Some(context_type)) => {
                if !resolver.zst(context_type) {
                    return refuse("issued context is not zero-state");
                }
                if abi.source_argument_ownership()[index]
                    != SemanticSourceArgumentOwnershipV1::SharedBorrow
                {
                    return refuse("context is not original shared borrow");
                }
                ArgumentV1::Context {
                    reference_type: meaning.ty,
                    context_type,
                }
            }
            _ => return refuse("helper reference role substitution"),
        });
        meanings[index] = Some(meaning);
    }
    if abi.source_argument_ownership()[2] != SemanticSourceArgumentOwnershipV1::ByValue {
        return refuse("f32 helper value is not by value");
    }
    Ok(CheckedReferenceCallV1 {
        owner,
        caller,
        block,
        helper: *helper,
        arguments: arguments.map(|v| v.expect("checked two original roles")),
        meanings: meanings.map(|v| v.expect("checked two original origins")),
    })
}

/// Every actual incoming source call agrees on one closed helper transport.
pub(super) struct CheckedReferenceHelperV1<'a> {
    representative: CheckedReferenceCallV1<'a>,
    incoming_count: usize,
}
impl<'a> CheckedReferenceHelperV1<'a> {
    pub(super) fn representative(&self) -> &CheckedReferenceCallV1<'a> {
        &self.representative
    }
    pub(super) fn incoming_count(&self) -> usize {
        self.incoming_count
    }
}
fn derive_helper<'a>(
    owner: &'a ProductionSemanticSsaOwnerV1,
    helper: SemanticFunctionIdV1,
    budget: &mut Budget<'_>,
) -> ResultV1<CheckedReferenceHelperV1<'a>> {
    let source = owner.source_semantic();
    if source.functions().len() > 32
        || source
            .functions()
            .get(helper.index() as usize)
            .is_none_or(|f| f.role() != SemanticFunctionRoleV1::InternalHelper)
    {
        return refuse("closed reference helper coordinate/role");
    }
    let mut representative: Option<CheckedReferenceCallV1<'a>> = None;
    let mut incoming_count = 0usize;
    for (fi, function) in source.functions().iter().enumerate() {
        for (bi, block) in function.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            if !matches!(source.callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::Defined { function }) if *function == helper)
            {
                continue;
            }
            // Unknown, retained, unreachable or foreign incoming calls are never skipped.
            let relation = derive(
                owner,
                SemanticFunctionIdV1::from_index(fi as u32),
                SemanticBlockIdV1::from_index(bi as u32),
                budget,
            )?;
            if let Some(previous) = &representative {
                if previous.arguments != relation.arguments
                    || !previous.same_source_origins(&relation)
                {
                    return refuse("helper incoming source origins or nominal roles disagree");
                }
            } else {
                representative = Some(relation);
            }
            incoming_count = incoming_count
                .checked_add(1)
                .ok_or(ReferenceErrorV1::Unavailable(
                    "helper incoming census overflow",
                ))?;
        }
    }
    Ok(CheckedReferenceHelperV1 {
        representative: representative.ok_or(ReferenceErrorV1::Unavailable(
            "helper has no original incoming call",
        ))?,
        incoming_count,
    })
}
/// The same callback must cover helper planning, emission and original call/formal replay.
pub(super) fn with_checked_reference_helper_v1<R: Copy + 'static>(
    owner: &ProductionSemanticSsaOwnerV1,
    helper: SemanticFunctionIdV1,
    budget: &mut Budget<'_>,
    inspect: impl FnOnce(&CheckedReferenceHelperV1<'_>, &mut Budget<'_>) -> ResultV1<R>,
) -> ResultV1<R> {
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    let receipt = owner
        .occurrence_storage()
        .ok_or(ReferenceErrorV1::Unavailable(
            "captured source occurrences required",
        ))?;
    if budget.storage() < receipt.retained_storage() {
        return Err(Resource::Accounting.into());
    }
    let identity = budget.work_ledger_identity_v1();
    let slot = budget as *mut _ as usize;
    budget.reserve_storage(SCRATCH_V1)?;
    let floor = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let relation = derive_helper(owner, helper, budget)?;
        inspect(&relation, budget)
    }));
    if slot != budget as *mut _ as usize
        || identity != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        return Err(Resource::Accounting.into());
    }
    let denied = budget.failed_work().is_some() || budget.failed_storage().is_some();
    budget.release_storage(SCRATCH_V1)?;
    match outcome {
        Ok(Ok(_)) if denied => Err(Resource::Accounting.into()),
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(ReferenceErrorV1::CallbackPanicked)
        }
    }
}

const SCRATCH_V1: usize = 64 * 4096;
/// Inspects the same immutable owner and original source occurrences. No view or
/// relation can escape; a consuming planner must run inside this callback.
pub(super) fn with_checked_reference_call_v1<R: Copy + 'static>(
    owner: &ProductionSemanticSsaOwnerV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    budget: &mut Budget<'_>,
    inspect: impl FnOnce(&CheckedReferenceCallV1<'_>, &mut Budget<'_>) -> ResultV1<R>,
) -> ResultV1<R> {
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    let receipt = owner
        .occurrence_storage()
        .ok_or(ReferenceErrorV1::Unavailable(
            "captured source occurrences required",
        ))?;
    if budget.storage() < receipt.retained_storage() {
        return Err(Resource::Accounting.into());
    }
    let identity = budget.work_ledger_identity_v1();
    let slot = budget as *mut _ as usize;
    budget.reserve_storage(SCRATCH_V1)?;
    let floor = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let relation = derive(owner, caller, block, budget)?;
        inspect(&relation, budget)
    }));
    if slot != budget as *mut _ as usize
        || identity != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        return Err(Resource::Accounting.into());
    }
    let denied = budget.failed_work().is_some() || budget.failed_storage().is_some();
    budget.release_storage(SCRATCH_V1)?;
    match outcome {
        Ok(Ok(_)) if denied => Err(Resource::Accounting.into()),
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(ReferenceErrorV1::CallbackPanicked)
        }
    }
}

/// Exact root borrow/initializer/issuer coordinates, usable only while borrowing
/// the same source owner. This is arithmetic payload transport, never EXEC.
pub(super) struct CheckedReferenceBorrowV1<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    statement: u32,
    source_rvalue: &'a SemanticRvalueV1,
    reference_type: SemanticTypeIdV1,
    snapshot_type: SemanticTypeIdV1,
    lane_field: u32,
    lane_producer: (SemanticFunctionIdV1, SsaValueV1),
    constructor: (SemanticBlockIdV1, u32),
}
impl CheckedReferenceBorrowV1<'_> {
    pub(super) fn owner(&self) -> &ProductionSemanticSsaOwnerV1 {
        self.owner
    }
    pub(super) fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub(super) fn block(&self) -> SemanticBlockIdV1 {
        self.block
    }
    pub(super) fn statement(&self) -> u32 {
        self.statement
    }
    pub(super) fn source_rvalue(&self) -> &SemanticRvalueV1 {
        self.source_rvalue
    }
    pub(super) fn reference_type(&self) -> SemanticTypeIdV1 {
        self.reference_type
    }
    pub(super) fn snapshot_type(&self) -> SemanticTypeIdV1 {
        self.snapshot_type
    }
    pub(super) fn lane_field(&self) -> u32 {
        self.lane_field
    }
    pub(super) fn lane_producer(&self) -> (SemanticFunctionIdV1, SsaValueV1) {
        self.lane_producer
    }
    pub(super) fn constructor(&self) -> (SemanticBlockIdV1, u32) {
        self.constructor
    }
    pub(super) fn initializer_rvalue(&self) -> &SemanticRvalueV1 {
        let statement = &self.owner.source_semantic().functions()[self.function.index() as usize]
            .blocks()[self.constructor.0.index() as usize]
            .statements()[self.constructor.1 as usize];
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            unreachable!("immutable checked original initializer")
        };
        assignment.value()
    }
}
impl CheckedReferenceCallV1<'_> {
    pub(super) fn lane_producer(&self) -> (SemanticFunctionIdV1, SsaValueV1) {
        match self.meanings[0].origin {
            SourceOrigin::RootIssuedSsa { function, value } => (function, value),
            _ => unreachable!("checked arithmetic snapshot/rank retains actual root SSA issuer"),
        }
    }
    pub(super) fn context_producer(&self) -> Option<(SemanticFunctionIdV1, SsaValueV1)> {
        match self.meanings[1].origin {
            SourceOrigin::RootIssuedSsa { function, value } => Some((function, value)),
            _ => None,
        }
    }
    pub(super) fn retained_context_producer(
        &self,
    ) -> Option<(
        SemanticFunctionIdV1,
        SemanticBlockIdV1,
        SemanticLocalIdV1,
        SemanticTypeIdV1,
    )> {
        match self.meanings[1].origin {
            SourceOrigin::RootRetainedCall {
                function,
                block,
                local,
                context_type,
            } => Some((function, block, local, context_type)),
            _ => None,
        }
    }
}
fn derive_reference_borrow<'a>(
    owner: &'a ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    statement_index: u32,
    budget: &mut Budget<'_>,
) -> ResultV1<CheckedReferenceBorrowV1<'a>> {
    let source = owner.source_semantic();
    if source.roots() != [function] || source.functions().len() > 32 {
        return refuse("snapshot borrow accessor requires the sole original root");
    }
    let declaration = source
        .functions()
        .get(function.index() as usize)
        .ok_or(ReferenceErrorV1::Unavailable("source root coordinate"))?;
    if declaration.role() != SemanticFunctionRoleV1::KernelRoot
        || declaration.kernel_entry().is_none()
    {
        return refuse("snapshot borrow accessor requires actual kernel root");
    }
    let statement = declaration
        .blocks()
        .get(block.index() as usize)
        .and_then(|b| b.statements().get(statement_index as usize))
        .ok_or(ReferenceErrorV1::Unavailable(
            "original shared borrow coordinate",
        ))?;
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return refuse("snapshot borrow is not original source assignment");
    };
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = assignment.value().kind()
    else {
        return refuse("snapshot borrow site is not original Shared Borrow");
    };
    if !assignment.destination().projections().is_empty() || !place.projections().is_empty() {
        return refuse("snapshot borrow accessor requires exact root locals");
    }
    let borrow_site = Site::Statement {
        block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
        statement: statement_index,
    };
    let mut original_definition = None;
    for event in rows(owner, function)?.events() {
        budget.charge_work(1)?;
        if event.site() == borrow_site
            && event.role() == EventRole::DestinationDefine
            && let Some(SsaResolvedEventV1::Define { variable, value }) = event.resolved()
            && variable.get() == assignment.destination().local().index()
        {
            if !event.is_reachable() || !event.is_promoted() {
                return refuse("source reference holder is unreachable or retained");
            }
            unique(&mut original_definition, value)?;
        }
    }
    if original_definition.is_none() {
        return refuse("source borrow has no exact original SSA definition");
    }
    let mut resolver = Resolver {
        owner,
        root: function,
        visits: 0,
    };
    // The retained-source proof is valid whether or not rustc also promoted this
    // particular aggregate. It never relabels addressed storage as an SSA value.
    let meaning = resolver.retained_snapshot(function, borrow_site, place, 0, budget)?;
    let Kind::Snapshot { lane_field } = meaning.kind else {
        return refuse("source borrow does not select original arithmetic snapshot");
    };
    let SourceOrigin::RootIssuedSsa {
        function: producer_function,
        value: producer_value,
    } = meaning.origin
    else {
        return refuse("root snapshot has no actual original lane SSA issuer");
    };
    if producer_function != function {
        return refuse("root snapshot does not have the current root's lane issuer");
    }
    resolver.shared(meaning, assignment.value().result_type())?;
    let mut constructor = None;
    for (bi, candidate_block) in declaration.blocks().iter().enumerate() {
        for (si, candidate) in candidate_block.statements().iter().enumerate() {
            budget.charge_work(1)?;
            if let SemanticStatementKindV1::Assign(candidate) = candidate.kind()
                && candidate.destination().local() == place.local()
            {
                // retained_snapshot independently established this exact unique,
                // dominating, whole aggregate initializer with no other writes.
                unique(
                    &mut constructor,
                    (SemanticBlockIdV1::from_index(bi as u32), si as u32),
                )?;
            }
        }
    }
    Ok(CheckedReferenceBorrowV1 {
        owner,
        function,
        block,
        statement: statement_index,
        source_rvalue: assignment.value(),
        reference_type: assignment.value().result_type(),
        snapshot_type: place.ty(),
        lane_field,
        lane_producer: (producer_function, producer_value),
        constructor: constructor.ok_or(ReferenceErrorV1::Unavailable(
            "original snapshot constructor absent",
        ))?,
    })
}
pub(super) fn with_checked_reference_borrow_v1<R: Copy + 'static>(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    statement: u32,
    budget: &mut Budget<'_>,
    inspect: impl FnOnce(&CheckedReferenceBorrowV1<'_>, &mut Budget<'_>) -> ResultV1<R>,
) -> ResultV1<R> {
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    let receipt = owner
        .occurrence_storage()
        .ok_or(ReferenceErrorV1::Unavailable(
            "captured source occurrences required",
        ))?;
    if budget.storage() < receipt.retained_storage() {
        return Err(Resource::Accounting.into());
    }
    let identity = budget.work_ledger_identity_v1();
    let slot = budget as *mut _ as usize;
    budget.reserve_storage(SCRATCH_V1)?;
    let floor = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let relation = derive_reference_borrow(owner, function, block, statement, budget)?;
        inspect(&relation, budget)
    }));
    if slot != budget as *mut _ as usize
        || identity != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        return Err(Resource::Accounting.into());
    }
    let denied = budget.failed_work().is_some() || budget.failed_storage().is_some();
    budget.release_storage(SCRATCH_V1)?;
    match outcome {
        Ok(Ok(_)) if denied => Err(Resource::Accounting.into()),
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(ReferenceErrorV1::CallbackPanicked)
        }
    }
}

/// One exact root SharedBorrow of retained context storage. The context is
/// authenticated by its original issuer call and lifetime, never by an SSA key.
pub(super) struct CheckedContextBorrowV1<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    statement: u32,
    source_rvalue: &'a SemanticRvalueV1,
    reference_type: SemanticTypeIdV1,
    context_type: SemanticTypeIdV1,
    issuer_block: SemanticBlockIdV1,
    issuer_local: SemanticLocalIdV1,
}
impl CheckedContextBorrowV1<'_> {
    pub(super) fn owner(&self) -> &ProductionSemanticSsaOwnerV1 {
        self.owner
    }
    pub(super) fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub(super) fn block(&self) -> SemanticBlockIdV1 {
        self.block
    }
    pub(super) fn statement(&self) -> u32 {
        self.statement
    }
    pub(super) fn source_rvalue(&self) -> &SemanticRvalueV1 {
        self.source_rvalue
    }
    pub(super) fn reference_type(&self) -> SemanticTypeIdV1 {
        self.reference_type
    }
    pub(super) fn context_type(&self) -> SemanticTypeIdV1 {
        self.context_type
    }
    pub(super) fn issuer(&self) -> (SemanticFunctionIdV1, SemanticBlockIdV1, SemanticLocalIdV1) {
        (self.function, self.issuer_block, self.issuer_local)
    }
    pub(super) fn source_call(&self) -> &SemanticDirectCallV1 {
        call_at(self.owner, self.function, self.issuer_block)
            .expect("checked immutable root context issuer call")
    }
}
fn derive_context_borrow<'a>(
    owner: &'a ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    statement_index: u32,
    budget: &mut Budget<'_>,
) -> ResultV1<CheckedContextBorrowV1<'a>> {
    let source = owner.source_semantic();
    if source.roots() != [function] || source.functions().len() > 32 {
        return refuse("context borrow requires the sole original root");
    }
    let declaration = source
        .functions()
        .get(function.index() as usize)
        .ok_or(ReferenceErrorV1::Unavailable("source root coordinate"))?;
    if declaration.role() != SemanticFunctionRoleV1::KernelRoot
        || declaration.kernel_entry().is_none()
    {
        return refuse("context borrow requires actual kernel root");
    }
    let statement = declaration
        .blocks()
        .get(block.index() as usize)
        .and_then(|b| b.statements().get(statement_index as usize))
        .ok_or(ReferenceErrorV1::Unavailable(
            "original context borrow coordinate",
        ))?;
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return refuse("context borrow is not original source assignment");
    };
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = assignment.value().kind()
    else {
        return refuse("context borrow site is not original Shared Borrow");
    };
    if !assignment.destination().projections().is_empty() || !place.projections().is_empty() {
        return refuse("context borrow requires exact root locals");
    }
    let borrow_site = Site::Statement {
        block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
        statement: statement_index,
    };
    let mut reference_definition = None;
    for event in rows(owner, function)?.events() {
        budget.charge_work(1)?;
        if event.site() == borrow_site
            && event.role() == EventRole::DestinationDefine
            && let Some(SsaResolvedEventV1::Define { variable, value }) = event.resolved()
            && variable.get() == assignment.destination().local().index()
        {
            if !event.is_reachable() || !event.is_promoted() {
                return refuse("context reference holder is unreachable or retained");
            }
            unique(&mut reference_definition, value)?;
        }
    }
    if reference_definition.is_none() {
        return refuse("context borrow has no exact original reference SSA definition");
    }
    let mut resolver = Resolver {
        owner,
        root: function,
        visits: 0,
    };
    let meaning = resolver.retained_context(function, borrow_site, place, 0, budget)?;
    resolver.shared(meaning, assignment.value().result_type())?;
    let SourceOrigin::RootRetainedCall {
        function: issuer_function,
        block: issuer_block,
        local: issuer_local,
        context_type,
    } = meaning.origin
    else {
        return refuse("context borrow requires original retained issuer call");
    };
    if meaning.kind != Kind::Context || issuer_function != function || context_type != place.ty() {
        return refuse("context borrow original issuer role/type changed");
    }
    Ok(CheckedContextBorrowV1 {
        owner,
        function,
        block,
        statement: statement_index,
        source_rvalue: assignment.value(),
        reference_type: assignment.value().result_type(),
        context_type,
        issuer_block,
        issuer_local,
    })
}
pub(super) fn with_checked_context_borrow_v1<R: Copy + 'static>(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    statement: u32,
    budget: &mut Budget<'_>,
    inspect: impl FnOnce(&CheckedContextBorrowV1<'_>, &mut Budget<'_>) -> ResultV1<R>,
) -> ResultV1<R> {
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    let receipt = owner
        .occurrence_storage()
        .ok_or(ReferenceErrorV1::Unavailable(
            "captured source occurrences required",
        ))?;
    if budget.storage() < receipt.retained_storage() {
        return Err(Resource::Accounting.into());
    }
    let identity = budget.work_ledger_identity_v1();
    let slot = budget as *mut _ as usize;
    budget.reserve_storage(SCRATCH_V1)?;
    let floor = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let relation = derive_context_borrow(owner, function, block, statement, budget)?;
        inspect(&relation, budget)
    }));
    if slot != budget as *mut _ as usize
        || identity != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        return Err(Resource::Accounting.into());
    }
    let denied = budget.failed_work().is_some() || budget.failed_storage().is_some();
    budget.release_storage(SCRATCH_V1)?;
    match outcome {
        Ok(Ok(_)) if denied => Err(Resource::Accounting.into()),
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(ReferenceErrorV1::CallbackPanicked)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use fe2o3_pliron::{
        ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    };

    #[derive(Clone, Copy, Debug)]
    enum Mutation {
        None,
        Rank,
        UnissuedLane,
        UnissuedContext,
        WrongLaneField,
        ReassignSnapshot,
        ForeignIncoming,
        SameIncoming,
        ForeignIncomingLane,
        DistinctIssuedLane,
        StorageDeadSnapshot,
        TerminalDeadSnapshot,
        TerminalDeadSnapshotAliasCopy,
        TerminalDeadSnapshotRawAlias,
        TerminalDeadSnapshotFormalRawAlias,
        DeadSnapshotFutureRead,
        DeadSnapshotBackedge,
        DeadSnapshotUnreachableIncoming,
        RawShuffleTerminalDeadSnapshot,
        InitialStorageLive,
        StorageLiveSnapshot,
        StorageLiveContext,
        DeadLiveSnapshot,
        DeadLiveContext,
        ProjectedExposure,
        MutableExposure,
        ContextCallOverwrite,
        ReassignSnapshotReference,
        ReassignContextReference,
        OverwriteSnapshotFormal,
        OverwriteContextFormal,
        RawSnapshotReference,
        RawContextReference,
        RawSnapshotFormal,
        RawContextFormal,
        RawBorrow,
        RawShuffle,
        HolderEntryLive,
        HolderLocalLive,
        HolderOrderedStorage,
        RawShuffleHolderOrderedStorage,
        HolderLateLive,
        HolderRestart,
        HolderUnreachableLive,
        HolderLiveBackedge,
        HolderDeadBackedge,
        HolderDeadBeforeCall,
        HolderAliasInitialLive,
        HolderFormalLive,
        HolderFormalDead,
        ShiftCounter,
        ShiftCounterZero,
        ShiftCounterOverwide,
        ShiftCounterZeroShift,
        ShiftCounterWideShift,
        ShiftCounterDynamicShift,
        ShiftCounterReassign,
        ShiftCounterInitReentry,
        ShiftCounterBorrow,
        ShiftCounterRawAddress,
        ShiftCounterSigned,
    }

    fn fixture(mutation: Mutation) -> Result<ProductionSemanticSsaOwnerV1, String> {
        let source = SemanticSourceProvenanceV1::unavailable();
        let shift_counter = matches!(mutation, Mutation::ShiftCounter | Mutation::ShiftCounterZero | Mutation::ShiftCounterOverwide | Mutation::ShiftCounterZeroShift | Mutation::ShiftCounterWideShift | Mutation::ShiftCounterDynamicShift | Mutation::ShiftCounterReassign | Mutation::ShiftCounterInitReentry | Mutation::ShiftCounterBorrow | Mutation::ShiftCounterRawAddress | Mutation::ShiftCounterSigned);
        let raw = shift_counter || matches!(mutation, Mutation::RawShuffle | Mutation::RawShuffleTerminalDeadSnapshot | Mutation::RawShuffleHolderOrderedStorage);
        let id = SemanticTypeIdV1::from_index;
        let unit = id(0);
        let word = id(1);
        let float = id(2);
        let lane = id(3);
        let context = id(4);
        let snapshot = id(5);
        let snapshot_ref = id(6);
        let context_ref = id(7);
        let word_ref = id(8);
        let snapshot_mut_ref = id(9);
        let raw_snapshot_ptr = id(10);
        let raw_context_ptr = id(11);
        let aggregate = |tag, fields: Vec<SemanticTypeIdV1>, bytes, align| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::aggregate_with_backend_repr(
                    Some(bytes),
                    align,
                    if tag == 4 {
                        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                            SemanticBackendPrimitiveV1::integer(false, 32, 4),
                            SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                        ))
                    } else {
                        SemanticBackendReprV1::memory(true)
                    },
                    false,
                    SemanticAggregateLayoutV1::new(vec![0; fields.len()], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
            )
        };
        let pointer = |tag, pointee, mutable| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        SemanticPointerKindV1::Reference,
                        if mutable {
                            SemanticMutabilityV1::Mutable
                        } else {
                            SemanticMutabilityV1::Immutable
                        },
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            if mutable {
                                SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                            } else {
                                SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                            },
                            if pointee == context { 0 } else { 4 },
                            if pointee == context { 1 } else { 4 },
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            )
        };
        let raw_pointer = |tag, pointee| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        SemanticPointerKindV1::Raw,
                        SemanticMutabilityV1::Mutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
                    ),
                    None,
                ),
            )
        };
        let mut types = vec![
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([1; 32]),
                SemanticLayoutIdentityV1::from_sha256([1; 32]),
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    0,
                    1,
                    SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::memory(true),
                    None,
                    false,
                    None,
                    1,
                    0,
                    SemanticTypeLayoutDetailsV1::None,
                )
                .unwrap(),
                SemanticTypeShapeV1::Unit,
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([2; 32]),
                SemanticLayoutIdentityV1::from_sha256([2; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(4),
                    4,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 32, 4),
                        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                }),
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([3; 32]),
                SemanticLayoutIdentityV1::from_sha256([3; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(4),
                    4,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::float(32, 4),
                        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
            ),
            aggregate(4, vec![word], 4, 4),
            aggregate(5, vec![], 0, 1),
            aggregate(6, vec![word, unit], 4, 4),
            pointer(7, snapshot, false),
            pointer(8, context, false),
            pointer(9, word, false),
            pointer(10, snapshot, true),
            raw_pointer(11, snapshot),
            raw_pointer(12, context),
        ];
        let signed_word = id(12);
        let raw_word_pointer = id(13);
        if shift_counter {
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([14; 32]), SemanticLayoutIdentityV1::from_sha256([14; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(Some(4), 4,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(true, 32, 4),
                        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()))), false).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: true, bits: 32 })));
            types.push(raw_pointer(15, word));
        }
        let direct = |ty| {
            let shared_reference = ty == snapshot_ref || ty == context_ref;
            let attributes = SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    shared_reference,
                    shared_reference.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    shared_reference,
                    shared_reference,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                if ty == snapshot_ref { 4 } else { 0 },
                (ty == snapshot_ref).then_some(4),
            )
            .unwrap();
            SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Direct(attributes))
        };
        let abi = |tag,
                   inputs: Vec<SemanticTypeIdV1>,
                   output,
                   ownership: Vec<SemanticSourceArgumentOwnershipV1>,
                   root| {
            SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                if root {
                    SemanticCanonAbiV1::GpuKernel
                } else {
                    SemanticCanonAbiV1::Rust
                },
                if root {
                    SemanticExternAbiV1::GpuKernel
                } else {
                    SemanticExternAbiV1::Rust
                },
                false,
                false,
                inputs.len() as u32,
                inputs
                    .into_iter()
                    .map(|ty| SemanticAbiArgumentV1::source(direct(ty)))
                    .collect(),
                if output == unit || output == context {
                    SemanticAbiValueV1::new(output, SemanticAbiPassModeV1::Ignore)
                } else {
                    direct(output)
                },
            )
            .unwrap()
            .with_source_argument_ownership(ownership)
            .unwrap()
        };
        let intrinsic = |tag, operation, output| SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
                source,
                abi(tag, vec![], output, vec![], false),
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
        };
        let mut callables = vec![
            // Defined callables occupy the canonical function-index prefix.
            SemanticCallableDeclV1::Defined {
                function: SemanticFunctionIdV1::from_index(0),
            },
            SemanticCallableDeclV1::Defined {
                function: SemanticFunctionIdV1::from_index(1),
            },
            intrinsic(
                20,
                SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
                    lane,
                    wave_width: 64,
                },
                lane,
            ),
            intrinsic(
                21,
                SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent { context },
                context,
            ),
        ];
        if raw {
            // This is an actual declared source intrinsic with its real typed
            // argument/return contract, not a factory activation predicate.
            callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1::from_sha256([22;32]),
                    SemanticItemDefinitionIdentityV1::from_sha256([22;32]),
                    SemanticMonomorphizationIdentityV1::from_sha256([22;32]),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256([22;32]),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256([22;32]),
                    source,
                    abi(22,vec![context_ref,float,word],float,vec![
                        SemanticSourceArgumentOwnershipV1::SharedBorrow,
                        SemanticSourceArgumentOwnershipV1::ByValue,
                        SemanticSourceArgumentOwnershipV1::ByValue,
                    ],false),
                ),
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942Wave64ShuffleIndex {context,element:float},
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([22;32]),
            });
        }
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let assignment = |local, ty, value| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(local, ty),
                    SemanticRvalueV1::new(ty, value),
                )),
            )
        };
        let constant = |ty, value| SemanticOperandV1::Constant(SemanticConstantV1::new(ty, value));
        let number = |n| {
            constant(
                word,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(n, 4).unwrap()),
            )
        };
        let zst = || constant(unit, SemanticConstantValueV1::ZeroSized);
        let snapshot_value = |operand| {
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Aggregate,
                    vec![operand, zst()],
                )
                .unwrap(),
            )
        };
        let call = |callee, arguments, destination, ty, next| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(callee),
                    arguments,
                    Some(SemanticCallDestinationV1::new(
                        place(destination, ty),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1::from_index(next),
                        ),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        };
        let block = |tag, statements, terminator| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, terminator),
            )
            .unwrap()
        };
        let lane_field = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(1),
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), word).unwrap()],
                word,
            )
            .unwrap(),
        );
        let mut statements = vec![
            assignment(
                3,
                snapshot,
                snapshot_value(if matches!(mutation, Mutation::WrongLaneField) {
                    number(7)
                } else {
                    lane_field
                }),
            ),
            assignment(
                4,
                snapshot_ref,
                if matches!(mutation, Mutation::RawBorrow) {
                    SemanticRvalueKindV1::AddressOf {
                        mutability: SemanticMutabilityV1::Immutable,
                        place: place(3, snapshot),
                    }
                } else {
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(3, snapshot),
                    }
                },
            ),
            assignment(
                5,
                context_ref,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(2, context),
                },
            ),
        ];
        match mutation {
            Mutation::ReassignSnapshotReference => statements.push(assignment(
                4,
                snapshot_ref,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(4, snapshot_ref))),
            )),
            Mutation::ReassignContextReference => statements.push(assignment(
                5,
                context_ref,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(5, context_ref))),
            )),
            Mutation::RawSnapshotReference | Mutation::HolderAliasInitialLive => statements.push(assignment(
                14,
                raw_snapshot_ptr,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Pointer,
                    operand: SemanticOperandV1::Copy(place(4, snapshot_ref)),
                },
            )),
            Mutation::RawContextReference => statements.push(assignment(
                15,
                raw_context_ptr,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Pointer,
                    operand: SemanticOperandV1::Copy(place(5, context_ref)),
                },
            )),
            _ => {}
        }
        if raw {
            // The real defined-call input is an original SSA use, with distinct
            // real source values available for coherent typed graph mutations.
            statements.push(assignment(6,float,SemanticRvalueKindV1::Use(constant(float,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0x3f800000,4).unwrap())))));
            statements.push(assignment(16,word,SemanticRvalueKindV1::Use(number(7))));
            statements.push(assignment(17,float,SemanticRvalueKindV1::Use(constant(float,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0x40000000,4).unwrap())))));
        }
        if matches!(mutation, Mutation::UnissuedLane) {
            statements.insert(
                0,
                assignment(
                    1,
                    lane,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Aggregate,
                            vec![number(3)],
                        )
                        .unwrap(),
                    ),
                ),
            );
        }
        if matches!(mutation, Mutation::UnissuedContext) {
            statements.insert(
                0,
                assignment(
                    2,
                    context,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Aggregate, vec![])
                            .unwrap(),
                    ),
                ),
            );
        }
        if matches!(
            mutation,
            Mutation::StorageLiveSnapshot | Mutation::DeadLiveSnapshot
        ) {
            if matches!(mutation, Mutation::DeadLiveSnapshot) {
                statements.push(SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)),
                ));
            }
            statements.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(3)),
            ));
        }
        if matches!(
            mutation,
            Mutation::StorageLiveContext | Mutation::DeadLiveContext
        ) {
            if matches!(mutation, Mutation::DeadLiveContext) {
                statements.push(SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(2)),
                ));
            }
            statements.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
            ));
        }
        if matches!(mutation, Mutation::StorageDeadSnapshot) {
            statements.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)),
            ));
        }
        if matches!(mutation, Mutation::ProjectedExposure) {
            statements.push(assignment(
                12,
                word_ref,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(3),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), word)
                                .unwrap(),
                        ],
                        word,
                    )
                    .unwrap(),
                },
            ));
        }
        if matches!(mutation, Mutation::MutableExposure) {
            statements.push(assignment(
                13,
                snapshot_mut_ref,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(3, snapshot),
                },
            ));
        }
        if matches!(mutation, Mutation::ReassignSnapshot) {
            statements.push(assignment(3, snapshot, snapshot_value(number(9))));
        }
        if matches!(mutation, Mutation::TerminalDeadSnapshotAliasCopy) {
            statements.push(assignment(11, snapshot_ref,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(4, snapshot_ref)))));
        }
        if matches!(mutation, Mutation::TerminalDeadSnapshotRawAlias) {
            statements.push(assignment(14, raw_snapshot_ptr, SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Pointer, operand: SemanticOperandV1::Copy(place(4, snapshot_ref)),
            }));
        }
        if matches!(mutation, Mutation::HolderDeadBeforeCall) {
            // The original reference holder dies after its real Borrow but
            // before the original helper's CallArgument(0) reads it.
            statements.push(SemanticStatementV1::new(source,
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(4))));
        }
        if matches!(mutation, Mutation::HolderLocalLive | Mutation::HolderLiveBackedge) {
            let index = statements.iter().position(|statement| matches!(statement.kind(),
                SemanticStatementKindV1::Assign(assignment) if assignment.destination().local().index() == 4)).unwrap();
            statements.insert(index, SemanticStatementV1::new(source,
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(4))));
        }
        let mut root_blocks = vec![
            // Keep both genuine compiler declarations in the root closure even
            // when the selected source value is independently overwritten.
            block(30, vec![], call(2, vec![], 1, lane, 1)),
            block(31, vec![], call(3, vec![], 2, context, 2)),
            block(
                32,
                statements,
                call(
                    1,
                    vec![
                        if matches!(mutation, Mutation::Rank) {
                            SemanticOperandV1::Copy(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(4),
                                    vec![
                                        SemanticProjectionV1::new(
                                            SemanticProjectionKindV1::Dereference,
                                            snapshot,
                                        )
                                        .unwrap(),
                                        SemanticProjectionV1::new(
                                            SemanticProjectionKindV1::Field(0),
                                            word,
                                        )
                                        .unwrap(),
                                    ],
                                    word,
                                )
                                .unwrap(),
                            )
                        } else {
                            SemanticOperandV1::Copy(place(4, snapshot_ref))
                        },
                        SemanticOperandV1::Copy(place(5, context_ref)),
                        if raw {
                            SemanticOperandV1::Copy(place(6,float))
                        } else {
                            constant(float,SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(0,4).unwrap()))
                        },
                    ],
                    6,
                    float,
                    3,
                ),
            ),
            block(33, vec![], SemanticTerminatorKindV1::Return),
        ];
        if raw {
            root_blocks[3]=block(33,vec![],SemanticTerminatorKindV1::Goto(
                SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::Goto,SemanticBlockIdV1::from_index(4))));
            root_blocks.push(block(34,vec![],SemanticTerminatorKindV1::Return));
        }
        if matches!(mutation, Mutation::TerminalDeadSnapshot | Mutation::TerminalDeadSnapshotAliasCopy
            | Mutation::TerminalDeadSnapshotRawAlias | Mutation::TerminalDeadSnapshotFormalRawAlias
            | Mutation::DeadSnapshotFutureRead
            | Mutation::DeadSnapshotBackedge | Mutation::DeadSnapshotUnreachableIncoming
            | Mutation::RawShuffleTerminalDeadSnapshot) {
            let dead = SemanticStatementV1::new(source,
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)));
            let terminal = if matches!(mutation, Mutation::RawShuffleTerminalDeadSnapshot) { 4 } else { 3 };
            let mut suffix = vec![dead];
            if matches!(mutation, Mutation::DeadSnapshotFutureRead | Mutation::TerminalDeadSnapshotAliasCopy) {
                let read_holder = if matches!(mutation, Mutation::TerminalDeadSnapshotAliasCopy) { 11 } else { 4 };
                suffix.push(assignment(16, word, SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(read_holder), vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, snapshot).unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), word).unwrap(),
                    ], word).unwrap()))));
            }
            let tail = if matches!(mutation, Mutation::DeadSnapshotBackedge) {
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto, SemanticBlockIdV1::from_index(2)))
            } else { SemanticTerminatorKindV1::Return };
            root_blocks[terminal] = block(33 + terminal as u8, suffix, tail);
            if matches!(mutation, Mutation::TerminalDeadSnapshotAliasCopy) {
                root_blocks[terminal] = block(36, vec![SemanticStatementV1::new(source,
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)))],
                    call(1, vec![SemanticOperandV1::Copy(place(11, snapshot_ref)),
                        SemanticOperandV1::Copy(place(5, context_ref)),
                        constant(float, SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0,4).unwrap()))],
                        6, float, 4));
                root_blocks.push(block(39, vec![], SemanticTerminatorKindV1::Return));
            }
            if matches!(mutation, Mutation::DeadSnapshotUnreachableIncoming) {
                root_blocks.push(block(37, vec![], call(1, vec![
                    SemanticOperandV1::Copy(place(4, snapshot_ref)),
                    SemanticOperandV1::Copy(place(5, context_ref)),
                    constant(float, SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap())),
                ], 6, float, 5)));
                root_blocks.push(block(38, vec![], SemanticTerminatorKindV1::Return));
            }
        }
        if matches!(mutation, Mutation::HolderEntryLive | Mutation::HolderOrderedStorage
            | Mutation::RawShuffleHolderOrderedStorage | Mutation::HolderRestart | Mutation::HolderAliasInitialLive) {
            root_blocks[0] = block(30, vec![
                SemanticStatementV1::new(source, SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(4))),
                SemanticStatementV1::new(source, SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(5))),
            ], call(2, vec![], 1, lane, 1));
        }
        if matches!(mutation, Mutation::HolderOrderedStorage | Mutation::RawShuffleHolderOrderedStorage) {
            let terminal = if matches!(mutation, Mutation::RawShuffleHolderOrderedStorage) { 4 } else { 3 };
            root_blocks[terminal] = block(81, vec![
                SemanticStatementV1::new(source, SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(4))),
                SemanticStatementV1::new(source, SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(5))),
            ], SemanticTerminatorKindV1::Return);
        }
        if matches!(mutation, Mutation::HolderLateLive | Mutation::HolderRestart) {
            root_blocks[3] = block(81, vec![SemanticStatementV1::new(source,
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(4)))], SemanticTerminatorKindV1::Return);
        }
        if matches!(mutation, Mutation::HolderUnreachableLive) {
            root_blocks.push(block(82, vec![SemanticStatementV1::new(source,
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(4)))], SemanticTerminatorKindV1::Return));
        }
        if matches!(mutation, Mutation::HolderLiveBackedge | Mutation::HolderDeadBackedge) {
            let statements = if matches!(mutation, Mutation::HolderDeadBackedge) {
                vec![SemanticStatementV1::new(source, SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(4)))]
            } else { vec![] };
            root_blocks[3] = block(81, statements, SemanticTerminatorKindV1::Goto(
                SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::Goto, SemanticBlockIdV1::from_index(2))));
        }
        if matches!(mutation, Mutation::InitialStorageLive) {
            root_blocks[0] = block(
                30,
                vec![
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
                    ),
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(3)),
                    ),
                ],
                call(2, vec![], 1, lane, 1),
            );
        }
        if matches!(mutation, Mutation::ForeignIncoming) {
            // A genuine first call must not excuse a second unissued context.
            root_blocks[3] = block(
                33,
                vec![
                    assignment(
                        7,
                        context,
                        SemanticRvalueKindV1::Aggregate(
                            SemanticAggregateRvalueV1::new(
                                SemanticAggregateKindV1::Aggregate,
                                vec![],
                            )
                            .unwrap(),
                        ),
                    ),
                    assignment(
                        8,
                        context_ref,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(7, context),
                        },
                    ),
                ],
                call(
                    1,
                    vec![
                        SemanticOperandV1::Copy(place(4, snapshot_ref)),
                        SemanticOperandV1::Copy(place(8, context_ref)),
                        constant(
                            float,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(0, 4).unwrap(),
                            ),
                        ),
                    ],
                    6,
                    float,
                    4,
                ),
            );
            root_blocks.push(block(34, vec![], SemanticTerminatorKindV1::Return));
        }
        if matches!(mutation, Mutation::SameIncoming) {
            root_blocks[3] = block(
                33,
                vec![],
                call(
                    1,
                    vec![
                        SemanticOperandV1::Copy(place(4, snapshot_ref)),
                        SemanticOperandV1::Copy(place(5, context_ref)),
                        constant(
                            float,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(0, 4).unwrap(),
                            ),
                        ),
                    ],
                    6,
                    float,
                    4,
                ),
            );
            root_blocks.push(block(34, vec![], SemanticTerminatorKindV1::Return));
        }
        if matches!(
            mutation,
            Mutation::ForeignIncomingLane | Mutation::DistinctIssuedLane
        ) {
            let new_lane = SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(9),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), word)
                            .unwrap(),
                    ],
                    word,
                )
                .unwrap(),
            );
            let mut second_statements = vec![
                assignment(10, snapshot, snapshot_value(new_lane)),
                assignment(
                    11,
                    snapshot_ref,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(10, snapshot),
                    },
                ),
            ];
            let next = if matches!(mutation, Mutation::DistinctIssuedLane) {
                5
            } else {
                4
            };
            let second_call = call(
                1,
                vec![
                    SemanticOperandV1::Copy(place(11, snapshot_ref)),
                    SemanticOperandV1::Copy(place(5, context_ref)),
                    constant(
                        float,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
                    ),
                ],
                6,
                float,
                next,
            );
            if matches!(mutation, Mutation::DistinctIssuedLane) {
                // A real second issuer is still a distinct original source SSA key.
                root_blocks[3] = block(33, vec![], call(2, vec![], 9, lane, 4));
                root_blocks.push(block(34, second_statements, second_call));
                root_blocks.push(block(35, vec![], SemanticTerminatorKindV1::Return));
            } else {
                second_statements.insert(
                    0,
                    assignment(
                        9,
                        lane,
                        SemanticRvalueKindV1::Aggregate(
                            SemanticAggregateRvalueV1::new(
                                SemanticAggregateKindV1::Aggregate,
                                vec![number(7)],
                            )
                            .unwrap(),
                        ),
                    ),
                );
                root_blocks[3] = block(33, second_statements, second_call);
                root_blocks.push(block(34, vec![], SemanticTerminatorKindV1::Return));
            }
        }
        if matches!(mutation, Mutation::ContextCallOverwrite) {
            root_blocks[3] = block(33, vec![], call(3, vec![], 2, context, 4));
            root_blocks.push(block(34, vec![], SemanticTerminatorKindV1::Return));
        }
        let function =
            |tag, root, types: Vec<(SemanticTypeIdV1, SemanticLocalRoleV1)>, abi, blocks| {
                SemanticFunctionDeclV1::new(
                    SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                    if root {
                        SemanticFunctionRoleV1::KernelRoot
                    } else {
                        SemanticFunctionRoleV1::InternalHelper
                    },
                    SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
                    SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
                    source,
                    abi,
                    types
                        .into_iter()
                        .enumerate()
                        .map(|(index, (ty, role))| {
                            SemanticLocalDeclV1::new(
                                SemanticLocalIdentityV1::from_sha256([tag + index as u8 + 1; 32]),
                                ty,
                                role,
                                source,
                            )
                        })
                        .collect(),
                    SemanticBlockIdV1::from_index(0),
                    blocks,
                )
                .unwrap()
            };
        let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
        let root = function(
            40,
            true,
            vec![
                (unit, SemanticLocalRoleV1::Return),
                (lane, SemanticLocalRoleV1::Temporary),
                (context, SemanticLocalRoleV1::Temporary),
                (snapshot, SemanticLocalRoleV1::Temporary),
                (snapshot_ref, SemanticLocalRoleV1::Temporary),
                (context_ref, SemanticLocalRoleV1::Temporary),
                (float, SemanticLocalRoleV1::Temporary),
                (context, SemanticLocalRoleV1::Temporary),
                (context_ref, SemanticLocalRoleV1::Temporary),
                (lane, SemanticLocalRoleV1::Temporary),
                (snapshot, SemanticLocalRoleV1::Temporary),
                (snapshot_ref, SemanticLocalRoleV1::Temporary),
                (word_ref, SemanticLocalRoleV1::Temporary),
                (snapshot_mut_ref, SemanticLocalRoleV1::Temporary),
                (raw_snapshot_ptr, SemanticLocalRoleV1::Temporary),
                (raw_context_ptr, SemanticLocalRoleV1::Temporary),
                (word, SemanticLocalRoleV1::Temporary),
                (float, SemanticLocalRoleV1::Temporary),
            ],
            abi(40, vec![], unit, vec![], true),
            root_blocks,
        )
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"synthetic_reference_relation".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([50; 32]),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                        .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        ));
        let mut helper_statements = vec![assignment(
            0,
            float,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, float))),
        )];
        match mutation {
            Mutation::OverwriteSnapshotFormal => helper_statements.insert(
                0,
                assignment(
                    1,
                    snapshot_ref,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, snapshot_ref))),
                ),
            ),
            Mutation::OverwriteContextFormal => helper_statements.insert(
                0,
                assignment(
                    2,
                    context_ref,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, context_ref))),
                ),
            ),
            Mutation::RawSnapshotFormal | Mutation::TerminalDeadSnapshotFormalRawAlias => helper_statements.insert(
                0,
                assignment(
                    4,
                    raw_snapshot_ptr,
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Pointer,
                        operand: SemanticOperandV1::Copy(place(1, snapshot_ref)),
                    },
                ),
            ),
            Mutation::RawContextFormal => helper_statements.insert(
                0,
                assignment(
                    5,
                    raw_context_ptr,
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Pointer,
                        operand: SemanticOperandV1::Copy(place(2, context_ref)),
                    },
                ),
            ),
            _ => {}
        }
        if matches!(mutation, Mutation::HolderFormalLive | Mutation::HolderFormalDead) {
            let kind = if matches!(mutation, Mutation::HolderFormalLive) {
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(1))
            } else { SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)) };
            helper_statements.insert(0, SemanticStatementV1::new(source, kind));
        }
        let counter_ty = if matches!(mutation, Mutation::ShiftCounterSigned) { signed_word } else { word };
        let mut helper_locals = vec![
                (float, SemanticLocalRoleV1::Return),
                (
                    if matches!(mutation, Mutation::Rank) {
                        word
                    } else {
                        snapshot_ref
                    },
                    SemanticLocalRoleV1::Argument(0),
                ),
                (context_ref, SemanticLocalRoleV1::Argument(1)),
                (float, SemanticLocalRoleV1::Argument(2)),
                (raw_snapshot_ptr, SemanticLocalRoleV1::Temporary),
                (raw_context_ptr, SemanticLocalRoleV1::Temporary),
                (word, SemanticLocalRoleV1::Temporary),
            ];
        if shift_counter {
            helper_locals.extend([(counter_ty, SemanticLocalRoleV1::Temporary),
                (word, SemanticLocalRoleV1::Temporary), (word, SemanticLocalRoleV1::Temporary),
                (word_ref, SemanticLocalRoleV1::Temporary), (raw_word_pointer, SemanticLocalRoleV1::Temporary)]);
            if matches!(mutation, Mutation::ShiftCounterDynamicShift) {
                helper_locals.push((signed_word, SemanticLocalRoleV1::Temporary));
            }
        }
        let helper_blocks = if shift_counter {
            let edge = |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
            let init = if matches!(mutation, Mutation::ShiftCounterZero) { 0 }
                else if matches!(mutation, Mutation::ShiftCounterOverwide) { 64 } else { 32 };
            let literal = |ty, value| constant(ty, SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()));
            let mut initialize = vec![
                SemanticStatementV1::new(source, SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(7))),
                assignment(7, counter_ty, SemanticRvalueKindV1::Use(literal(counter_ty, init))),
            ];
            if matches!(mutation, Mutation::ShiftCounterBorrow) {
                initialize.push(assignment(10, word_ref, SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared, place: place(7, word) }));
            }
            if matches!(mutation, Mutation::ShiftCounterRawAddress) {
                initialize.push(assignment(11, raw_word_pointer, SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable, place: place(7, word) }));
            }
            let shift = if matches!(mutation, Mutation::ShiftCounterZeroShift) { 0 }
                else if matches!(mutation, Mutation::ShiftCounterWideShift) { 32 } else { 1 };
            let right = if matches!(mutation, Mutation::ShiftCounterDynamicShift) {
                SemanticOperandV1::Copy(place(12, signed_word))
            } else { literal(signed_word, shift) };
            let mut step = vec![assignment(7, counter_ty, SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::ShiftRight,
                left: SemanticOperandV1::Copy(place(7, counter_ty)), right })];
            if matches!(mutation, Mutation::ShiftCounterDynamicShift) {
                step.insert(0, assignment(12, signed_word, SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand: SemanticOperandV1::Copy(place(9, word)),
                }));
            }
            if matches!(mutation, Mutation::ShiftCounterReassign) {
                step.push(assignment(7, counter_ty, SemanticRvalueKindV1::Use(literal(counter_ty, 64))));
            }
            vec![
                block(83, vec![assignment(0, float, SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, float))))],
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 5))),
                block(84, vec![], SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(7, counter_ty)),
                    targets: SemanticSwitchTargetsV1::new(vec![SemanticSwitchTargetV1::new(0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 4))], edge(SemanticEdgeRoleV1::SwitchOtherwise, 2)).unwrap(),
                }),
                block(85, vec![
                    assignment(6, word, SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(1), vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, snapshot).unwrap(),
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), word).unwrap()], word).unwrap()))),
                    assignment(9, word, if matches!(mutation, Mutation::ShiftCounterSigned) {
                        SemanticRvalueKindV1::Cast { kind: SemanticCastKindV1::Integer,
                            operand: SemanticOperandV1::Copy(place(7, signed_word)) }
                    } else { SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(7, word))) }),
                    assignment(8, word, SemanticRvalueKindV1::Binary { operation: SemanticBinaryOpV1::BitXor,
                        left: SemanticOperandV1::Copy(place(6, word)), right: SemanticOperandV1::Copy(place(9, word)) }),
                ], call(4, vec![SemanticOperandV1::Copy(place(2, context_ref)),
                    SemanticOperandV1::Copy(place(0, float)), SemanticOperandV1::Copy(place(8, word))], 0, float, 3)),
                block(86, step, SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto,
                    if matches!(mutation, Mutation::ShiftCounterInitReentry) { 5 } else { 1 }))),
                block(87, vec![SemanticStatementV1::new(source,
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(7)))], SemanticTerminatorKindV1::Return),
                block(88, initialize, SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1))),
            ]
        } else if raw {
                vec![
                    block(70,vec![assignment(6,word,SemanticRvalueKindV1::Use(
                        SemanticOperandV1::Copy(SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(1),vec![
                                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference,snapshot).unwrap(),
                                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0),word).unwrap(),
                            ],word).unwrap())))],
                        call(4,vec![
                            SemanticOperandV1::Copy(place(2,context_ref)),
                            SemanticOperandV1::Copy(place(3,float)),
                            SemanticOperandV1::Copy(place(6,word)),
                        ],0,float,1)),
                    block(71,vec![],SemanticTerminatorKindV1::Return),
                ]
            } else {
                vec![block(70,helper_statements,SemanticTerminatorKindV1::Return)]
            };
        let helper = function(
            60,
            false,
            helper_locals,
            abi(
                60,
                vec![
                    if matches!(mutation, Mutation::Rank) {
                        word
                    } else {
                        snapshot_ref
                    },
                    context_ref,
                    float,
                ],
                float,
                vec![
                    if matches!(mutation, Mutation::Rank) {
                        SemanticSourceArgumentOwnershipV1::ByValue
                    } else {
                        SemanticSourceArgumentOwnershipV1::SharedBorrow
                    },
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ],
                false,
            ),
            helper_blocks,
        );
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            types,
            vec![],
            vec![],
            vec![],
            vec![root, helper],
            callables,
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .map_err(|e| format!("{e:?}"))?
        .admit_current_production(SemanticMirLimitsV1::default())
        .map_err(|e| format!("{e:?}"))?;
        let semantic = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            ProductionSemanticMirLimitsV1::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut owner = ProductionSemanticSsaOwnerV1::try_new(
            semantic,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .map_err(|e| format!("{e:?}"))?;
        Ok(owner)
    }
    fn check(owner: &ProductionSemanticSsaOwnerV1) -> ResultV1<()> {
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        budget.reserve_storage(owner.occurrence_storage().unwrap().retained_storage())?;
        with_checked_reference_call_v1(
            owner,
            SemanticFunctionIdV1::from_index(0),
            SemanticBlockIdV1::from_index(2),
            &mut budget,
            |view, _| {
                assert!(std::ptr::eq(view.owner(), owner));
                assert_eq!(view.helper(), SemanticFunctionIdV1::from_index(1));
                assert!(matches!(
                    view.argument(0),
                    Some(ArgumentV1::Snapshot { lane_field: 0, .. } | ArgumentV1::Rank { .. })
                ));
                assert!(matches!(view.argument(1), Some(ArgumentV1::Context { .. })));
                assert!(matches!(
                    view.meanings[0].origin,
                    SourceOrigin::RootIssuedSsa { .. }
                ));
                assert!(
                    matches!(view.meanings[1].origin, SourceOrigin::RootRetainedCall {
                    function, block, local, context_type,
                } if function.index() == 0 && block.index() == 1 && local.index() == 2 && context_type.index() == 4)
                );
                Ok(())
            },
        )
    }
    #[test]
    fn original_initializer_and_actual_ssa_issuers_join_to_borrowed_call() {
        for mutation in [Mutation::None, Mutation::Rank, Mutation::InitialStorageLive] {
            let owner = fixture(mutation).expect("synthetic source admission");
            let borrowed = Site::Statement {
                block: fe2o3_mir_model::SsaBlockIdV1::new(2),
                statement: 1,
            };
            let occurrences = rows(&owner, SemanticFunctionIdV1::from_index(0)).unwrap();
            assert!(!occurrences.elisions().contains(&borrowed));
            let mut found = 0;
            for event in occurrences.events() {
                if event.site() == borrowed
                    && event.operand() == OperandRole::RvaluePlace
                    && event.role() == EventRole::BaseUse
                {
                    assert!(
                        matches!(event.event(), fe2o3_mir_model::SsaEventV1::Use(variable) if variable.get() == 3)
                    );
                    assert!(event.is_reachable());
                    assert!(!event.is_promoted());
                    assert!(event.resolved().is_none());
                    found += 1;
                }
            }
            assert_eq!(
                found, 1,
                "normal retained Borrow has exact syntax, not an SSA use or elision"
            );
            check(&owner).unwrap();
        }
    }
    #[test]
    fn source_origin_mutations_cannot_mint_reference_transport() {
        for mutation in [
            Mutation::UnissuedLane,
            Mutation::UnissuedContext,
            Mutation::WrongLaneField,
            Mutation::ReassignSnapshot,
        ] {
            let owner = fixture(mutation)
                .unwrap_or_else(|error| panic!("{mutation:?} must reach relation: {error}"));
            assert!(
                matches!(check(&owner), Err(ReferenceErrorV1::Unavailable(_))),
                "{mutation:?} must fail source relation"
            );
        }
    }
    #[test]
    fn raw_reference_type_mutation_is_rejected_by_original_source_schema() {
        let error = match fixture(Mutation::RawBorrow) {
            Ok(_) => panic!("raw address falsely typed as reference admitted"),
            Err(error) => error,
        };
        assert!(
            error.starts_with("InvalidTypeOperation { operation: Borrow,"),
            "{error}"
        );
    }
    #[test]
    fn one_short_scratch_and_zero_work_deny_before_callback_and_keep_sticky_failure() {
        let owner = fixture(Mutation::None).unwrap();
        let retained = owner.occurrence_storage().unwrap().retained_storage();
        for work_cap in [0, 10_000_000] {
            let mut work = Work::new(work_cap);
            let mut budget = Budget::new(
                &mut work,
                retained + SCRATCH_V1 - usize::from(work_cap != 0),
            );
            budget.reserve_storage(retained).unwrap();
            let entered = std::cell::Cell::new(false);
            let result = with_checked_reference_call_v1(
                &owner,
                SemanticFunctionIdV1::from_index(0),
                SemanticBlockIdV1::from_index(2),
                &mut budget,
                |_, _| {
                    entered.set(true);
                    Ok(())
                },
            );
            assert!(matches!(result, Err(ReferenceErrorV1::Resource(_))));
            assert!(!entered.get());
            assert_eq!(budget.storage(), retained);
            assert!(budget.failed_work().is_some() || budget.failed_storage().is_some());
        }
    }
    #[test]
    fn callback_denial_is_sticky_and_callback_allocations_are_not_refunded() {
        let owner = fixture(Mutation::None).unwrap();
        let retained = owner.occurrence_storage().unwrap().retained_storage();
        for deny in [false, true] {
            let mut work = Work::new(10_000_000);
            let mut budget = Budget::new(&mut work, 10_000_000);
            budget.reserve_storage(retained).unwrap();
            let identity = budget.work_ledger_identity_v1();
            let result = with_checked_reference_call_v1(
                &owner,
                SemanticFunctionIdV1::from_index(0),
                SemanticBlockIdV1::from_index(2),
                &mut budget,
                |view, budget| {
                    assert!(std::ptr::eq(view.owner(), &owner));
                    budget.reserve_storage(17)?;
                    if deny {
                        let _ = budget.charge_work(10_000_001);
                    }
                    Ok(())
                },
            );
            if deny {
                assert!(matches!(
                    result,
                    Err(ReferenceErrorV1::Resource(Resource::Accounting))
                ));
            } else {
                assert!(result.is_ok());
            }
            assert_eq!(budget.storage(), retained + 17);
            assert!(budget.work_ledger_identity_v1() == identity);
        }
    }
    #[test]
    fn helper_census_keeps_all_incoming_calls_under_same_borrowed_authority() {
        for mutation in [
            Mutation::None,
            Mutation::Rank,
            Mutation::SameIncoming,
            Mutation::ForeignIncoming,
            Mutation::ForeignIncomingLane,
            Mutation::DistinctIssuedLane,
        ] {
            let owner = fixture(mutation).expect("synthetic all-incoming source admission");
            let mut work = Work::new(10_000_000);
            let mut budget = Budget::new(&mut work, 10_000_000);
            budget
                .reserve_storage(owner.occurrence_storage().unwrap().retained_storage())
                .unwrap();
            check(&owner).expect("first source call is genuine");
            let entered = std::cell::Cell::new(false);
            let result = with_checked_reference_helper_v1(
                &owner,
                SemanticFunctionIdV1::from_index(1),
                &mut budget,
                |view, _| {
                    entered.set(true);
                    assert_eq!(
                        view.incoming_count(),
                        if matches!(mutation, Mutation::SameIncoming) {
                            2
                        } else {
                            1
                        }
                    );
                    assert!(std::ptr::eq(view.representative().owner(), &owner));
                    assert_eq!(
                        view.representative().helper(),
                        SemanticFunctionIdV1::from_index(1)
                    );
                    Ok(())
                },
            );
            if matches!(
                mutation,
                Mutation::ForeignIncoming
                    | Mutation::ForeignIncomingLane
                    | Mutation::DistinctIssuedLane
            ) {
                assert!(matches!(result, Err(ReferenceErrorV1::Unavailable(_))));
                assert!(!entered.get());
            } else {
                result.unwrap();
                assert!(entered.get());
            }
        }
    }

    #[test]
    fn saved_coordinates_do_not_join_distinct_live_source_owners() {
        let left = fixture(Mutation::None).expect("left source admission");
        let right = fixture(Mutation::None).expect("right source admission");
        let saved_helper = SemanticFunctionIdV1::from_index(1);
        let mut left_work = Work::new(10_000_000);
        let mut left_budget = Budget::new(&mut left_work, 10_000_000);
        left_budget
            .reserve_storage(left.occurrence_storage().unwrap().retained_storage())
            .unwrap();
        with_checked_reference_helper_v1(&left, saved_helper, &mut left_budget, |left_view, _| {
            let mut right_work = Work::new(10_000_000);
            let mut right_budget = Budget::new(&mut right_work, 10_000_000);
            right_budget
                .reserve_storage(right.occurrence_storage().unwrap().retained_storage())
                .unwrap();
            with_checked_reference_helper_v1(
                &right,
                saved_helper,
                &mut right_budget,
                |right_view, _| {
                    assert!(
                        !left_view
                            .representative()
                            .same_source_origins(right_view.representative())
                    );
                    assert!(!std::ptr::eq(
                        left_view.representative().source_call(),
                        right_view.representative().source_call()
                    ));
                    Ok(())
                },
            )
        })
        .unwrap();
    }

    #[test]
    fn root_borrow_accessor_retains_exact_source_initializer_and_lane_issuer() {
        let owner = fixture(Mutation::None).unwrap();
        let root = SemanticFunctionIdV1::from_index(0);
        let block = SemanticBlockIdV1::from_index(2);
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        budget
            .reserve_storage(owner.occurrence_storage().unwrap().retained_storage())
            .unwrap();
        let producer = result_value(&owner, root, SemanticBlockIdV1::from_index(0), &mut budget)
            .unwrap()
            .unwrap();
        with_checked_reference_borrow_v1(&owner, root, block, 1, &mut budget, |view, _| {
            assert!(std::ptr::eq(view.owner(), &owner));
            assert_eq!(view.function(), root);
            assert_eq!(view.block(), block);
            assert_eq!(view.statement(), 1);
            assert_eq!(view.reference_type(), SemanticTypeIdV1::from_index(6));
            assert_eq!(view.snapshot_type(), SemanticTypeIdV1::from_index(5));
            assert_eq!(view.lane_field(), 0);
            assert_eq!(view.lane_producer(), (root, producer));
            assert_eq!(view.constructor(), (block, 0));
            let SemanticStatementKindV1::Assign(original) =
                owner.source_semantic().functions()[0].blocks()[2].statements()[1].kind()
            else {
                unreachable!()
            };
            assert!(std::ptr::eq(view.source_rvalue(), original.value()));
            assert!(matches!(
                view.initializer_rvalue().kind(),
                SemanticRvalueKindV1::Aggregate(_)
            ));
            Ok(())
        })
        .unwrap();
        for (function, statement) in [
            (root, 0),
            (root, 2),
            (SemanticFunctionIdV1::from_index(1), 0),
        ] {
            assert!(matches!(
                with_checked_reference_borrow_v1(
                    &owner,
                    function,
                    block,
                    statement,
                    &mut budget,
                    |_, _| Ok(())
                ),
                Err(ReferenceErrorV1::Unavailable(_))
            ));
        }
    }

    #[test]
    fn retained_source_exposure_and_lifetime_mutations_reach_checker_and_refuse() {
        for mutation in [
            Mutation::StorageDeadSnapshot,
            Mutation::ProjectedExposure,
            Mutation::MutableExposure,
            Mutation::ContextCallOverwrite,
            Mutation::StorageLiveSnapshot,
            Mutation::StorageLiveContext,
            Mutation::DeadLiveSnapshot,
            Mutation::DeadLiveContext,
        ] {
            let owner = fixture(mutation)
                .unwrap_or_else(|e| panic!("{mutation:?} must reach source checker: {e}"));
            assert!(
                matches!(check(&owner), Err(ReferenceErrorV1::Unavailable(_))),
                "{mutation:?} must fail closed retained source relation"
            );
        }
    }
    #[test]
    fn root_context_borrow_retains_actual_issuer_call_without_fabricated_ssa() {
        for mutation in [Mutation::None, Mutation::InitialStorageLive] {
            let owner = fixture(mutation).expect("actual inert context borrow source admission");
            let root = SemanticFunctionIdV1::from_index(0);
            let block = SemanticBlockIdV1::from_index(2);
            let mut work = Work::new(10_000_000);
            let mut budget = Budget::new(&mut work, 10_000_000);
            budget
                .reserve_storage(owner.occurrence_storage().unwrap().retained_storage())
                .unwrap();
            with_checked_context_borrow_v1(&owner, root, block, 2, &mut budget, |view, budget| {
                assert!(std::ptr::eq(view.owner(), &owner));
                assert_eq!(
                    (view.function(), view.block(), view.statement()),
                    (root, block, 2)
                );
                assert_eq!(view.reference_type(), SemanticTypeIdV1::from_index(7));
                assert_eq!(view.context_type(), SemanticTypeIdV1::from_index(4));
                assert_eq!(
                    view.issuer(),
                    (
                        root,
                        SemanticBlockIdV1::from_index(1),
                        SemanticLocalIdV1::from_index(2)
                    )
                );
                assert!(std::ptr::eq(
                    view.source_call(),
                    call_at(&owner, root, SemanticBlockIdV1::from_index(1)).unwrap()
                ));
                let SemanticStatementKindV1::Assign(original) =
                    owner.source_semantic().functions()[0].blocks()[2].statements()[2].kind()
                else {
                    unreachable!()
                };
                assert!(std::ptr::eq(view.source_rvalue(), original.value()));
                assert!(
                    result_value(&owner, root, SemanticBlockIdV1::from_index(1), budget)?.is_none()
                );
                Ok(())
            })
            .unwrap();
            for (function, statement) in [
                (root, 0),
                (root, 1),
                (SemanticFunctionIdV1::from_index(1), 0),
            ] {
                assert!(matches!(
                    with_checked_context_borrow_v1(
                        &owner,
                        function,
                        block,
                        statement,
                        &mut budget,
                        |_, _| Ok(())
                    ),
                    Err(ReferenceErrorV1::Unavailable(_))
                ));
            }
        }
    }
    #[test]
    fn context_borrow_rejects_unissued_overwritten_or_restarted_original_storage() {
        for mutation in [
            Mutation::UnissuedContext,
            Mutation::StorageLiveContext,
            Mutation::DeadLiveContext,
            Mutation::ContextCallOverwrite,
        ] {
            let owner = fixture(mutation)
                .unwrap_or_else(|e| panic!("{mutation:?} must admit before context relation: {e}"));
            let mut work = Work::new(10_000_000);
            let mut budget = Budget::new(&mut work, 10_000_000);
            budget
                .reserve_storage(owner.occurrence_storage().unwrap().retained_storage())
                .unwrap();
            // UnissuedContext inserts one assignment before the original borrow.
            let statement = if matches!(mutation, Mutation::UnissuedContext) {
                3
            } else {
                2
            };
            let entered = std::cell::Cell::new(false);
            assert!(matches!(
                with_checked_context_borrow_v1(
                    &owner,
                    SemanticFunctionIdV1::from_index(0),
                    SemanticBlockIdV1::from_index(2),
                    statement,
                    &mut budget,
                    |_, _| {
                        entered.set(true);
                        Ok(())
                    }
                ),
                Err(ReferenceErrorV1::Unavailable(_))
            ));
            assert!(!entered.get());
        }
    }
    #[test]
    fn context_borrow_resource_denial_precedes_callback_and_stays_charged() {
        let owner = fixture(Mutation::None).expect("context resource fixture source admission");
        let retained = owner.occurrence_storage().unwrap().retained_storage();
        for work_cap in [0, 10_000_000] {
            let mut work = Work::new(work_cap);
            let mut budget = Budget::new(
                &mut work,
                retained + SCRATCH_V1 - usize::from(work_cap != 0),
            );
            budget.reserve_storage(retained).unwrap();
            let entered = std::cell::Cell::new(false);
            assert!(matches!(
                with_checked_context_borrow_v1(
                    &owner,
                    SemanticFunctionIdV1::from_index(0),
                    SemanticBlockIdV1::from_index(2),
                    2,
                    &mut budget,
                    |_, _| {
                        entered.set(true);
                        Ok(())
                    }
                ),
                Err(ReferenceErrorV1::Resource(_))
            ));
            assert!(!entered.get());
            assert_eq!(budget.storage(), retained);
            assert!(budget.failed_work().is_some() || budget.failed_storage().is_some());
        }
    }
    // Actual production holder/use guard under the same freshly borrowed call
    // relation. Ordinary original SSA ancestry deliberately still passes.
    #[test]
    fn original_reference_holders_and_formals_pass_actual_closed_source_guard() {
        let owner = fixture(Mutation::None).expect("original source fixture admission");
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        budget
            .reserve_storage(owner.occurrence_storage().unwrap().retained_storage())
            .unwrap();
        with_checked_reference_call_v1(
            &owner,
            SemanticFunctionIdV1::from_index(0),
            SemanticBlockIdV1::from_index(2),
            &mut budget,
            |view, budget| {
                assert!(std::ptr::eq(view.owner(), &owner));
                for (function, local, borrow) in [
                    (0, 4, Some((SemanticBlockIdV1::from_index(2), 1))),
                    (0, 5, Some((SemanticBlockIdV1::from_index(2), 2))),
                    (1, 1, None),
                    (1, 2, None),
                ] {
                    super::super::require_closed_wave64_reference_holder_v1(
                        &owner,
                        SemanticFunctionIdV1::from_index(function),
                        local,
                        borrow,
                        budget,
                    )
                    .expect("original admitted reference holder/formal must stay closed");
                }
                Ok(())
            },
        )
        .unwrap();
    }
    #[test]
    fn source_holder_reassignment_and_raw_aliases_fail_actual_guard_after_admission() {
        for (mutation, function, local, borrow) in [
            (
                Mutation::ReassignSnapshotReference,
                0,
                4,
                Some((SemanticBlockIdV1::from_index(2), 1)),
            ),
            (
                Mutation::ReassignContextReference,
                0,
                5,
                Some((SemanticBlockIdV1::from_index(2), 2)),
            ),
            (Mutation::OverwriteSnapshotFormal, 1, 1, None),
            (Mutation::OverwriteContextFormal, 1, 2, None),
            (
                Mutation::RawSnapshotReference,
                0,
                4,
                Some((SemanticBlockIdV1::from_index(2), 1)),
            ),
            (
                Mutation::RawContextReference,
                0,
                5,
                Some((SemanticBlockIdV1::from_index(2), 2)),
            ),
            (Mutation::RawSnapshotFormal, 1, 1, None),
            (Mutation::RawContextFormal, 1, 2, None),
        ] {
            let owner =
                fixture(mutation).unwrap_or_else(|e| panic!("{mutation:?} must admit: {e}"));
            check(&owner).expect("same SSA origins alone do not close holder/alias uses");
            let mut work = Work::new(10_000_000);
            let mut budget = Budget::new(&mut work, 10_000_000);
            budget
                .reserve_storage(owner.occurrence_storage().unwrap().retained_storage())
                .unwrap();
            with_checked_reference_call_v1(
                &owner,
                SemanticFunctionIdV1::from_index(0),
                SemanticBlockIdV1::from_index(2),
                &mut budget,
                |view, budget| {
                    assert!(std::ptr::eq(view.owner(), &owner));
                    assert!(
                        super::super::require_closed_wave64_reference_holder_v1(
                            &owner,
                            SemanticFunctionIdV1::from_index(function),
                            local,
                            borrow,
                            budget
                        )
                        .is_err(),
                        "{mutation:?} escaped actual closed holder/use guard"
                    );
                    Ok(())
                },
            )
            .unwrap();
        }
    }
    fn actual_raw_shuffle_whole_owner() -> ProductionSemanticKirOwnerV1 {
        let source=fixture(Mutation::RawShuffle).expect("declared raw-shuffle semantic source must admit");
        assert!(source.source_semantic().callables().iter().any(|callable|matches!(callable,
            SemanticCallableDeclV1::CompilerIntrinsic {operation:SemanticCompilerIntrinsicOperationV1::Gfx942Wave64ShuffleIndex {..},..})));
        let semantic=source.into_source_owner().expect("original SSA replay before normal owner construction");
        // Use the normal public constructor. A missing capture/transport step is
        // an actual test failure; no privately assembled owner may replace it.
        let owner=ProductionSemanticKirOwnerV1::try_lower(semantic,ProductionSemanticKirLimitsV1::default())
            .expect("normal source-to-whole-owner raw-shuffle construction");
        owner.verify_equivalence().expect("untouched whole-owner original-source replay");
        assert_eq!(owner.module().kernels[0].workgroup_size,Some(WorkgroupSize::new(64,1,1)));
        assert!(owner.module().functions.iter().flat_map(|f|f.body.iter()).flat_map(|b|&b.blocks)
            .flat_map(|b|&b.operations).any(|op|matches!(op.kind,OperationKind::Wave(WaveOperation {
                kind:WaveOperationKind::ShuffleIndex {..},..
            }))));
        owner
    }
    #[test]
    fn genuine_declared_raw_shuffle_constructs_and_replays_normal_whole_owner() {
        drop(actual_raw_shuffle_whole_owner());
    }
    #[derive(Clone,Copy,Debug)]
    enum WholeOwnerFault { Lane, Element, EntryEdge, Result, Continuation }
    #[test]
    fn coherent_raw_shuffle_graph_changes_fail_original_whole_owner_replay() {
        for fault in [WholeOwnerFault::Lane,WholeOwnerFault::Element,WholeOwnerFault::EntryEdge,
            WholeOwnerFault::Result,WholeOwnerFault::Continuation] {
            // Untouched public construction and whole-owner replay must succeed
            // before any negative mutation is permitted to count as evidence.
            let mut owner=actual_raw_shuffle_whole_owner();
            let mut module=owner.module().clone();
            let root=module.functions.iter().position(|f|f.role==fe2o3_kernel_ir::FunctionRole::KernelEntry).unwrap();
            let helper=module.functions.iter().position(|f|f.role==fe2o3_kernel_ir::FunctionRole::InternalHelper).unwrap();
            match fault {
                WholeOwnerFault::Lane|WholeOwnerFault::Element => {
                    let body=module.functions[root].body.as_mut().unwrap();
                    let block=body.blocks.iter_mut().find(|b|b.id==BlockId(2)).unwrap();
                    let alternative=block.operations.iter().find_map(|op|match (&fault,&op.kind) {
                        (WholeOwnerFault::Lane,OperationKind::Constant(Constant::U32(7)))=>Some(op.results[0].id),
                        (WholeOwnerFault::Element,OperationKind::Constant(Constant::F32Bits(bits))) if *bits==0x40000000=>Some(op.results[0].id),
                        _=>None,
                    }).expect("distinct genuine dominating source value");
                    let call=block.operations.iter_mut().find(|op|matches!(op.kind,OperationKind::Call {..})).unwrap();
                    let OperationKind::Call {arguments,..}=&mut call.kind else {unreachable!()};
                    let ordinal=usize::from(matches!(fault,WholeOwnerFault::Element));
                    assert_ne!(arguments[ordinal],alternative);
                    arguments[ordinal]=alternative;
                }
                WholeOwnerFault::EntryEdge => {
                    let block=module.functions[root].body.as_mut().unwrap().blocks.iter_mut().find(|b|b.id==BlockId(0)).unwrap();
                    let Some(Terminator::Branch {target,..})=&mut block.terminator else {panic!("source issuer normal entry edge")};
                    assert_eq!(*target,BlockId(1));*target=BlockId(2);
                }
                WholeOwnerFault::Result => {
                    let function=&mut module.functions[helper];
                    let input=function.body.as_ref().unwrap().parameters[1];
                    let block=function.body.as_mut().unwrap().blocks.iter_mut().find(|b|b.id==BlockId(1)).unwrap();
                    let Some(Terminator::Return {values})=&mut block.terminator else {panic!("actual shuffle result return")};
                    assert_eq!(values.len(),1);assert_ne!(values[0],input);values[0]=input;
                }
                WholeOwnerFault::Continuation => {
                    let block=module.functions[root].body.as_mut().unwrap().blocks.iter_mut().find(|b|b.id==BlockId(2)).unwrap();
                    let Some(Terminator::Branch {target,..})=&mut block.terminator else {panic!("source Defined-call normal continuation")};
                    assert_eq!(*target,BlockId(3));*target=BlockId(4);
                }
            }
            verify_module(&module).unwrap_or_else(|e|panic!("{fault:?} must remain typed KIR: {e:?}"));
            // Keep canonical graph bytes coherent with the changed module.
            // The immutable semantic source/SSA owner is deliberately untouched.
            owner.canonical_kernel_ir=ProductionCanonicalKernelIrV1::from_module(module.clone())
                .unwrap_or_else(|e|panic!("{fault:?} must retain canonical graph admission: {e:?}"));
            owner.module=RetainedProductionKirModuleV1::Legacy(module);
            owner.canonical_kernel_ir.revalidate().unwrap();
            assert!(matches!(owner.verify_equivalence(),Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)),
                "{fault:?} escaped full immutable-source owner replay");
        }
    }


#[test]
fn non_raw_captured_source_keeps_legacy_helper_and_parameter_accounting() {
    let owner = fixture(Mutation::None).expect("ordinary admitted source owner");
    assert!(owner.occurrence_storage().is_some());
    assert!(!super::super::ordinary_source_has_raw_wave64_shuffle_v1(owner.source_semantic()));
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 0);
    let mut closure = super::super::ReachableClosureBudgetV1::new(0);
    let plan = super::super::checked_wave64_helper_parameter_plan_v1(
        &owner, SemanticFunctionIdV1::from_index(0), SemanticFunctionIdV1::from_index(1),
        FunctionId::new("ordinary_non_raw_control"), 0, &mut closure, &mut budget,
    ).expect("non-raw source must not query the nominal helper view");
    assert!(plan.is_none());
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 0);
    assert!(budget.failed_work().is_none());
    assert!(budget.failed_storage().is_none());

    let target = Function::internal_helper(FunctionId::new("ordinary_non_raw_control"),
        Signature::new(vec![Type::Scalar(ScalarType::U32),Type::Scalar(ScalarType::F32)], vec![Type::Scalar(ScalarType::F32)]),
        vec![ValueId(1),ValueId(3)], vec![]);
    let instance = super::super::SemanticKirFunctionCorrespondenceV1 {
        correspondence_owner: SemanticFunctionIdV1::from_index(0),
        semantic_function: SemanticFunctionIdV1::from_index(1),
        kernel_ir_function: target.id.clone(),
        role: super::super::SemanticKirFunctionRoleV1::InternalHelper,
    };
    let mut direct_work=Work::new(10_000_000);
    let mut wrapped_work=Work::new(10_000_000);
    let mut direct_budget=Budget::new(&mut direct_work,10_000_000);
    let mut wrapped_budget=Budget::new(&mut wrapped_work,10_000_000);
    let direct=super::super::validate_parameter_correspondence_v1(owner.source_semantic(),&instance,&target,
        super::super::ArgumentTraceV1 {direct:&[],components:&[],ignored:&[]},&mut direct_budget);
    let wrapped=super::super::validate_parameter_correspondence_with_wave64_v1(&owner,&instance,&target,
        super::super::ArgumentTraceV1 {direct:&[],components:&[],ignored:&[]},&mut wrapped_budget);
    assert_eq!(format!("{direct:?}"),format!("{wrapped:?}"));
    assert_eq!(direct_budget.work(),wrapped_budget.work());
    assert_eq!(direct_budget.storage(),wrapped_budget.storage());
    assert_eq!(direct_budget.failed_work(),wrapped_budget.failed_work());
    assert_eq!(direct_budget.failed_storage(),wrapped_budget.failed_storage());
}

#[test]
fn retained_source_input_refunds_actual_receipt_on_success_and_error() {
    let owner=fixture(Mutation::RawShuffle).expect("genuine admitted raw-shuffle source owner");
    assert!(super::super::ordinary_source_has_raw_wave64_shuffle_v1(owner.source_semantic()));
    let receipt=owner.occurrence_storage().expect("real captured input receipt");
    let held=receipt.retained_storage();
    assert!(held>0);
    for refuse in [false,true] {
        let mut work=Work::new(100);
        let mut budget=Budget::new(&mut work,11+held+7);
        budget.reserve_storage(11).unwrap();
        let ledger=budget.work_ledger_identity_v1();
        let mut entered=false;
        let result=super::super::with_wave64_retained_source_input_v1(&owner,&mut budget,|budget| {
            entered=true;
            assert_eq!(budget.storage(),11+held);
            budget.reserve_storage(7)?;
            // The body owns this distinct output/scratch charge. On failure
            // it drops/refunds its scratch before returning; success retains it.
            if refuse { budget.release_storage(7)?; return Err(super::super::ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
            Ok(29u32)
        });
        assert!(entered);
        if refuse {
            assert!(matches!(result, Err(super::super::ProductionSemanticKirErrorV1::CorrespondenceMismatch)));
        } else {
            assert!(matches!(result, Ok(29)));
        }
        assert_eq!(budget.storage(),11+if refuse {0}else{7});
        assert!(budget.work_ledger_identity_v1()==ledger);
        assert!(budget.failed_work().is_none());
        assert!(budget.failed_storage().is_none());
        assert_eq!(owner.occurrence_storage().unwrap().retained_storage(),held);
    }
}

#[test]
fn retained_source_input_one_under_refuses_before_callback_and_preserves_failure_history() {
    let owner=fixture(Mutation::RawShuffle).expect("genuine admitted raw-shuffle source owner");
    let held=owner.occurrence_storage().unwrap().retained_storage();
    assert!(held>0);
    let mut work=Work::new(100);
    let mut budget=Budget::new(&mut work,11+held-1);
    budget.reserve_storage(11).unwrap();
    let mut entered=false;
    let result=super::super::with_wave64_retained_source_input_v1(&owner,&mut budget,|_| {
        entered=true;Ok(())
    });
    assert!(matches!(result, Err(super::super::ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Storage(_)))));
    assert!(!entered);
    assert_eq!(budget.storage(),11);
    assert_eq!(budget.failed_storage(),Some(11+held));
    assert!(budget.failed_work().is_none());
    drop(budget);
    let mut work=Work::new(0);
    let mut budget=Budget::new(&mut work,11+held);
    budget.reserve_storage(11).unwrap();
    let mut entered=false;
    let result=super::super::with_wave64_retained_source_input_v1(&owner,&mut budget,|budget| {
        entered=true;budget.charge_work(1)?;Ok(())
    });
    assert!(matches!(result, Err(super::super::ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Work(_)))));
    assert!(entered);
    assert_eq!(budget.storage(),11);
    assert_eq!(budget.failed_work(),Some(1));
    assert!(budget.failed_storage().is_none());
}

    #[test]
    fn issued_lane_field_is_plain_arithmetic_and_refuses_foreign_shapes() {
        let owner = fixture(Mutation::None).expect("admitted source owner");
        let semantic = owner.source_semantic();
        let types = semantic.types();
        let carrier = SemanticTypeIdV1::from_index(3);
        let word = SemanticTypeIdV1::from_index(1);
        let float = SemanticTypeIdV1::from_index(2);
        let snapshot = SemanticTypeIdV1::from_index(5);
        let projection = SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), word).unwrap();
        let lane = SemanticValueBindingV1::WaveLane { value: ValueId(42), wave: SemanticCurrentWaveV1::new(64) };
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let issued = super::super::compiler_issued_ssa_bindings_v1(types, semantic.callables(),
            &semantic.functions()[0], SemanticFunctionIdV1::from_index(0), None, Some(&mut budget)).unwrap();
        let arithmetic = super::super::wave_lane_arithmetic_field_v1(types, &issued, carrier,
            &projection, &lane, &mut budget).unwrap().expect("exact source lane field");
        assert!(matches!(arithmetic, SemanticValueBindingV1::Value { id: ValueId(42), ty: Type::Scalar(ScalarType::U32) }));
        assert!(super::super::require_current_wave_lane(SemanticBlockIdV1::from_index(0), arithmetic, 64,
            "arithmetic observation is not lane authority").is_err());
        for fault in 0..9 {
            let mut bindings = issued.clone();
            let mut source_types = types.to_vec();
            let mut source_carrier = carrier;
            let mut field = projection.clone();
            let mut source_binding = lane.clone();
            match fault {
                0 => bindings.clear(),
                1 => { bindings.insert(carrier, SemanticPromotedBindingV1::Ordinary); }
                2 => { bindings.insert(carrier, SemanticPromotedBindingV1::WaveLane { wave_width: 32 }); }
                3 => source_binding = SemanticValueBindingV1::WaveLane { value: ValueId(42), wave: SemanticCurrentWaveV1::new(32) },
                4 => source_carrier = snapshot,
                5 => field = SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), word).unwrap(),
                6 => field = SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), float).unwrap(),
                7 => source_binding = SemanticValueBindingV1::Value { id: ValueId(42), ty: Type::Scalar(ScalarType::U32) },
                8 => source_types[word.index() as usize] = types[float.index() as usize].clone(),
                _ => unreachable!(),
            }
            assert!(super::super::wave_lane_arithmetic_field_v1(&source_types, &bindings, source_carrier,
                &field, &source_binding, &mut budget).unwrap().is_none(), "field mutation {fault}");
        }
        let mut denied_work = Work::new(0);
        let mut denied = Budget::new(&mut denied_work, 0);
        assert!(super::super::wave_lane_arithmetic_field_v1(types, &issued, carrier,
            &projection, &lane, &mut denied).is_err());
        assert_eq!(denied.failed_work(), Some(4));
        assert_eq!(denied.storage(), 0);
    }

    #[test]
    fn snapshot_storage_end_requires_a_terminal_suffix_after_every_original_use() {
        for mutation in [Mutation::TerminalDeadSnapshot, Mutation::RawShuffleTerminalDeadSnapshot] {
            let owner = fixture(mutation).unwrap_or_else(|e| panic!("{mutation:?}: {e}"));
            check(&owner).unwrap_or_else(|e| panic!("{mutation:?}: {e:?}"));
            if matches!(mutation, Mutation::RawShuffleTerminalDeadSnapshot) {
                let semantic = owner.into_source_owner().expect("original source replay");
                let whole = ProductionSemanticKirOwnerV1::try_lower(semantic, ProductionSemanticKirLimitsV1::default())
                    .expect("normal whole-owner construction with terminal snapshot storage end");
                whole.verify_equivalence().expect("whole-owner replay with terminal snapshot storage end");
                assert!(whole.module().functions.iter().flat_map(|function| function.body.iter())
                    .flat_map(|body| &body.blocks).flat_map(|block| &block.operations)
                    .any(|operation| matches!(operation.kind, OperationKind::Wave(WaveOperation {
                        kind: WaveOperationKind::ShuffleIndex { .. }, ..
                    }))));
            }
        }
        for mutation in [Mutation::StorageDeadSnapshot, Mutation::DeadSnapshotFutureRead,
            Mutation::DeadSnapshotBackedge, Mutation::DeadLiveSnapshot,
            Mutation::TerminalDeadSnapshotAliasCopy, Mutation::TerminalDeadSnapshotRawAlias,
            Mutation::TerminalDeadSnapshotFormalRawAlias] {
            let owner = fixture(mutation).unwrap_or_else(|e| panic!("{mutation:?}: {e}"));
            assert!(matches!(check(&owner), Err(ReferenceErrorV1::Unavailable(_))), "{mutation:?}");
        }
        let unreachable = fixture(Mutation::DeadSnapshotUnreachableIncoming).expect("unreachable call source admits");
        check(&unreachable).expect("first reachable call is genuine");
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        budget.reserve_storage(unreachable.occurrence_storage().unwrap().retained_storage()).unwrap();
        assert!(matches!(with_checked_reference_helper_v1(&unreachable,
            SemanticFunctionIdV1::from_index(1), &mut budget, |_, _| Ok(())),
            Err(ReferenceErrorV1::Unavailable(_))));
        let owner = fixture(Mutation::TerminalDeadSnapshot).unwrap();
        let resolver = Resolver { owner: &owner, root: SemanticFunctionIdV1::from_index(0), visits: 0 };
        let mut zero_work = Work::new(0);
        let mut denied = Budget::new(&mut zero_work, 0);
        assert!(matches!(resolver.require_snapshot_dead_suffix(SemanticFunctionIdV1::from_index(0),
            SemanticLocalIdV1::from_index(3), 3, 0, &mut denied), Err(ReferenceErrorV1::Resource(Resource::Work(_)))));
        assert_eq!(denied.failed_work(), Some(1));
        assert_eq!(denied.storage(), 0);
    }

    fn original_holder_borrow(owner: &ProductionSemanticSsaOwnerV1, local: u32) -> (SemanticBlockIdV1, u32) {
        let mut original = None;
        for (bi, block) in owner.source_semantic().functions()[0].blocks().iter().enumerate() {
            for (si, statement) in block.statements().iter().enumerate() {
                if matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                    if assignment.destination().local().index() == local && matches!(assignment.value().kind(),
                        SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Shared, .. })) {
                    assert!(original.replace((SemanticBlockIdV1::from_index(bi as u32), si as u32)).is_none());
                }
            }
        }
        original.expect("unique original shared borrow")
    }
    #[test]
    fn holder_initial_live_and_terminal_dead_preserve_actual_source_and_whole_owner() {
        for mutation in [Mutation::HolderEntryLive, Mutation::HolderLocalLive,
            Mutation::HolderOrderedStorage, Mutation::RawShuffleHolderOrderedStorage] {
            let owner = fixture(mutation).unwrap_or_else(|error| panic!("{mutation:?}: {error}"));
            check(&owner).unwrap_or_else(|error| panic!("{mutation:?}: {error:?}"));
            let mut work = Work::new(10_000_000);
            let mut budget = Budget::new(&mut work, 10_000_000);
            budget.reserve_storage(owner.occurrence_storage().unwrap().retained_storage()).unwrap();
            with_checked_reference_call_v1(&owner, SemanticFunctionIdV1::from_index(0),
                SemanticBlockIdV1::from_index(2), &mut budget, |view, budget| {
                assert!(std::ptr::eq(view.owner(), &owner));
                for local in [4, 5] {
                    super::super::require_closed_wave64_reference_holder_v1(&owner,
                        SemanticFunctionIdV1::from_index(0), local, Some(original_holder_borrow(&owner, local)), budget)
                        .unwrap_or_else(|error| panic!("{mutation:?}, holder {local}: {error:?}"));
                }
                Ok(())
            }).unwrap();
            if matches!(mutation, Mutation::RawShuffleHolderOrderedStorage) {
                let whole = ProductionSemanticKirOwnerV1::try_lower(owner.into_source_owner().unwrap(),
                    ProductionSemanticKirLimitsV1::default()).expect("normal raw-shuffle owner with ordered holder storage");
                whole.verify_equivalence().expect("independent whole-owner source replay");
                assert!(whole.module().functions.iter().flat_map(|function| function.body.iter())
                    .flat_map(|body| &body.blocks).flat_map(|block| &block.operations)
                    .any(|operation| matches!(operation.kind, OperationKind::Wave(WaveOperation {
                        kind: WaveOperationKind::ShuffleIndex { .. }, .. }))));
            }
        }
    }
    #[test]
    fn holder_ordering_rejects_restarts_cycles_unreachable_lives_aliases_and_formal_markers() {
        for mutation in [Mutation::HolderLateLive, Mutation::HolderRestart, Mutation::HolderUnreachableLive,
            Mutation::HolderLiveBackedge, Mutation::HolderDeadBackedge,
            Mutation::HolderAliasInitialLive,
            Mutation::HolderFormalLive, Mutation::HolderFormalDead] {
            let owner = fixture(mutation).unwrap_or_else(|error| panic!("{mutation:?} must admit and capture original SSA: {error}"));
            let formal = matches!(mutation, Mutation::HolderFormalLive | Mutation::HolderFormalDead);
            let (function, local, borrow) = if formal { (1, 1, None) }
                else { (0, 4, Some(original_holder_borrow(&owner, 4))) };
            let mut work = Work::new(10_000_000);
            let mut budget = Budget::new(&mut work, 10_000_000);
            budget.reserve_storage(owner.occurrence_storage().unwrap().retained_storage()).unwrap();
            let result = super::super::require_closed_wave64_reference_holder_v1(&owner,
                SemanticFunctionIdV1::from_index(function), local, borrow, &mut budget);
            assert!(matches!(&result, Err(ProductionSemanticKirErrorV1::Unsupported { .. }
                | ProductionSemanticKirErrorV1::CorrespondenceMismatch)),
                "{mutation:?} must fail the semantic contract, not resource accounting: {result:?}");
        }
    }
    #[test]
    fn holder_dead_before_call_is_rejected_by_original_ssa_before_reference_transport() {
        // This source must fail in the original SSA planner, before a holder
        // relation can exist. Do not weaken admission to reach the later guard.
        match fixture(Mutation::HolderDeadBeforeCall) {
            Err(error) => assert_eq!(error,
                "Planner { function: SemanticFunctionIdV1(0), error: UndefinedAtUse { block: SsaBlockIdV1(2), event: 7, variable: SsaVariableIdV1(4) } }"),
            Ok(_) => panic!("original SSA accepted a call through a dead reference holder"),
        }
    }

    #[test]
    fn holder_storage_query_preserves_original_receipt_and_work_denial() {
        let owner = fixture(Mutation::HolderOrderedStorage).expect("actual holder lifetime source owner");
        let held = owner.occurrence_storage().unwrap().retained_storage();
        let borrow = original_holder_borrow(&owner, 4);
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, held);
        budget.reserve_storage(held).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        assert!(matches!(super::super::require_ordered_wave64_holder_storage_v1(&owner,
            SemanticFunctionIdV1::from_index(0), 4, borrow, Some((0, 0)), &mut budget),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Work(_)))));
        assert_eq!(budget.failed_work(), Some(4));
        assert_eq!(budget.storage(), held);
        assert!(budget.work_ledger_identity_v1() == ledger);
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, held - 1);
        budget.reserve_storage(held - 1).unwrap();
        assert!(matches!(super::super::require_ordered_wave64_holder_storage_v1(&owner,
            SemanticFunctionIdV1::from_index(0), 4, borrow, Some((0, 0)), &mut budget),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Accounting))));
        assert_eq!(budget.storage(), held - 1);
        assert!(budget.failed_storage().is_none());
    }

    #[test]
    fn decreasing_u32_counter_replays_genuine_raw_shuffle_whole_owner() {
        let source = fixture(Mutation::ShiftCounter).expect("original loop source admission and SSA capture");
        let held = source.occurrence_storage().unwrap().retained_storage();
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        budget.reserve_storage(held).unwrap();
        assert_eq!(super::super::checked_wave64_shift_counter_bound_v1(&source,
            SemanticFunctionIdV1::from_index(1), 7, &mut budget).unwrap(), Some(33));
        let owner = ProductionSemanticKirOwnerV1::try_lower(source.into_source_owner().unwrap(),
            ProductionSemanticKirLimitsV1::default()).expect("public lowering of actual source loop and raw shuffle");
        owner.verify_equivalence().expect("independent original-source whole-owner replay");
        assert!(owner.module().functions.iter().flat_map(|function| function.body.iter())
            .flat_map(|body| &body.blocks).flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::Wave(WaveOperation {
                kind: WaveOperationKind::ShuffleIndex { .. }, .. }))));
    }
    #[test]
    fn decreasing_counter_contract_refuses_changed_source_and_aliases() {
        for mutation in [Mutation::ShiftCounterZero, Mutation::ShiftCounterOverwide,
            Mutation::ShiftCounterZeroShift, Mutation::ShiftCounterWideShift, Mutation::ShiftCounterDynamicShift,
            Mutation::ShiftCounterReassign, Mutation::ShiftCounterInitReentry, Mutation::ShiftCounterBorrow,
            Mutation::ShiftCounterRawAddress, Mutation::ShiftCounterSigned] {
            let source = fixture(mutation).unwrap_or_else(|error| panic!("{mutation:?} must admit and capture before contract refusal: {error}"));
            let mut work = Work::new(10_000_000);
            let mut budget = Budget::new(&mut work, 10_000_000);
            budget.reserve_storage(source.occurrence_storage().unwrap().retained_storage()).unwrap();
            let result = super::super::checked_wave64_shift_counter_bound_v1(&source,
                SemanticFunctionIdV1::from_index(1), 7, &mut budget);
            assert!(matches!(&result, Ok(None)), "{mutation:?}: semantic refusal required, got {result:?}");
            assert!(budget.failed_work().is_none());
            assert!(budget.failed_storage().is_none());
        }
    }
    #[test]
    fn decreasing_counter_uses_exact_original_work_and_input_storage_limits() {
        let source = fixture(Mutation::ShiftCounter).unwrap();
        let held = source.occurrence_storage().unwrap().retained_storage();
        let query = |budget: &mut Budget<'_>| super::super::checked_wave64_shift_counter_bound_v1(
            &source, SemanticFunctionIdV1::from_index(1), 7, budget);
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, held);
        budget.reserve_storage(held).unwrap();
        assert_eq!(query(&mut budget).unwrap(), Some(33));
        let required = budget.work();
        assert!(required > 0);
        assert_eq!(budget.storage(), held);
        for limit in [0, required - 1] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, held);
            budget.reserve_storage(held).unwrap();
            let result = query(&mut budget);
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Work(_)))));
            assert!(budget.failed_work().is_some());
            assert_eq!(budget.storage(), held);
        }
        let mut work = Work::new(required);
        let mut budget = Budget::new(&mut work, held);
        budget.reserve_storage(held).unwrap();
        assert_eq!(query(&mut budget).unwrap(), Some(33));
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, held - 1);
        budget.reserve_storage(held - 1).unwrap();
        assert!(matches!(query(&mut budget), Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Accounting))));
        assert_eq!(budget.storage(), held - 1);
    }
}
