//! Narrow source occurrence transport, not an initialization or alias theorem
//! for arbitrary native memory. Fresh source/N and final private checks remain.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkLedgerIdentityV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssignmentV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
    SemanticTargetArchitectureV1, SemanticTargetDataLayoutV1,
};

type R<T> = Result<T, ProductionRankedProjectionErrorV1>;
type Site = ProjectedSemanticAccessSiteV1;

fn resource(error: Resource) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
    )
}
fn malformed() -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Unsupported("exact scalar-borrow source occurrence custody")
}
fn charge(facts: &mut dyn ProjectedAssertionFactsV1, amount: usize) -> R<()> {
    facts.charge_private_array_work(amount)
}

#[derive(Clone, Copy)]
struct Candidate {
    root: usize,
    alias: usize,
    block: usize,
    statement: usize,
}

#[derive(Clone, Copy, Default)]
struct Local {
    root_alias: Option<usize>,
    candidate: Option<Candidate>,
    initialized: bool,
    defined_once: bool,
    alias_alive: bool,
    blocked: bool,
}

#[derive(Clone, Copy)]
struct Read<'a> {
    place: &'a SemanticPlaceV1,
    alias: usize,
}

/// Only the enclosing root callback may retain this borrowed source census.
pub(super) struct ScalarPrivateBorrowsV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    types: &'a [SemanticTypeDeclV1],
    target: SemanticTargetDataLayoutV1,
    ledger: (usize, CanonicalKernelIrWorkLedgerIdentityV1),
    live_floor: usize,
    locals: Vec<Local>,
    starts: Vec<usize>,
    reads: Vec<Option<Read<'a>>>,
}

impl ScalarPrivateBorrowsV1<'_> {
    /// The existing provenance result is an independent conjunction, not a
    /// replacement for exact occurrence/lifetime/nonescape validation here.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve(
        &self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        target: SemanticTargetDataLayoutV1,
        site: Site,
        place: &SemanticPlaceV1,
        access: AccessKindAttr,
        atomic: Option<SemanticAtomicAccessV1>,
        provenance: Option<LocalAllocationProvenanceV1>,
        facts: &mut dyn ProjectedAssertionFactsV1,
    ) -> R<Option<SemanticLocalIdV1>> {
        if facts.helper_value_ledger_v1()? != self.ledger
            || facts.scalar_private_storage_v1()? < self.live_floor
        {
            return Err(resource(Resource::Accounting));
        }
        charge(facts, 12)?;
        if !std::ptr::eq(self.function, function)
            || !std::ptr::eq(self.types, types)
            || self.target != target
        {
            return Err(malformed());
        }
        let Some(statement) = site.statement else {
            return Ok(None);
        };
        let start = *self.starts.get(site.block).ok_or_else(malformed)?;
        let end = *self.starts.get(site.block + 1).ok_or_else(malformed)?;
        let ordinal = start
            .checked_add(statement)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        if ordinal >= end {
            return Err(malformed());
        }
        let Some(read) = self.reads[ordinal] else {
            return Ok(None);
        };
        if !std::ptr::eq(read.place, place) {
            // Other operands at the same source statement gain no permission.
            return Ok(None);
        }
        let alias = self.locals[read.alias];
        let candidate = alias.candidate.ok_or_else(malformed)?;
        let root = self.locals[candidate.root];
        if alias.blocked || root.blocked || access != AccessKindAttr::Read || atomic.is_some() {
            return Ok(None);
        }
        let root = SemanticLocalIdV1::from_index(
            u32::try_from(candidate.root).map_err(|_| resource(Resource::Arithmetic))?,
        );
        if provenance != Some(LocalAllocationProvenanceV1::Private(root)) {
            return Err(malformed());
        }
        Ok(Some(root))
    }
}

#[allow(clippy::too_many_arguments)]
fn candidate(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    target: SemanticTargetDataLayoutV1,
    block: usize,
    statement: usize,
    kind: &SemanticStatementKindV1,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> R<Option<Candidate>> {
    charge(facts, 3)?;
    let SemanticStatementKindV1::Assign(assignment) = kind else {
        return Ok(None);
    };
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Mutable,
        place,
    } = assignment.value().kind()
    else {
        return Ok(None);
    };
    charge(facts, 20)?;
    let destination = assignment.destination();
    if !place.projections().is_empty() || !destination.projections().is_empty() {
        return Ok(None);
    }
    let root = place.local().index() as usize;
    let alias = destination.local().index() as usize;
    let root_local = function.locals().get(root).ok_or_else(malformed)?;
    let alias_local = function.locals().get(alias).ok_or_else(malformed)?;
    if root_local.role() != SemanticLocalRoleV1::Temporary
        || alias_local.role() != SemanticLocalRoleV1::Temporary
        || root_local.ty() != place.ty()
        || alias_local.ty() != destination.ty()
        || assignment.value().result_type() != destination.ty()
    {
        return Ok(None);
    }
    let scalar = types
        .get(root_local.ty().index() as usize)
        .ok_or_else(malformed)?;
    let SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { bits, .. }) = scalar.shape()
    else {
        return Ok(None);
    };
    if !matches!(*bits, 8 | 16 | 32 | 64)
        || scalar.layout().size_bytes() != Some(u64::from(*bits / 8))
    {
        return Ok(None);
    }
    let reference = types
        .get(alias_local.ty().index() as usize)
        .ok_or_else(malformed)?;
    let SemanticTypeShapeV1::Pointer(pointer) = reference.shape() else {
        return Ok(None);
    };
    let width = match target.architecture() {
        SemanticTargetArchitectureV1::AmdGpuGfx942 => 64,
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Mutable
        || pointer.metadata() != SemanticPointerMetadataV1::None
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != width
        || reference.layout().size_bytes() != Some(u64::from(width / 8))
        || pointer.pointee() != place.ty()
    {
        return Ok(None);
    }
    Ok(Some(Candidate {
        root,
        alias,
        block,
        statement,
    }))
}

fn allocate<T: Clone>(
    count: usize,
    value: T,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> R<Vec<T>> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    facts.reserve_scalar_private_storage_v1(bytes)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| resource(Resource::Allocation))?;
    let actual = output
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    facts.reserve_scalar_private_storage_v1(
        actual
            .checked_sub(bytes)
            .ok_or_else(|| resource(Resource::Accounting))?,
    )?;
    charge(facts, count)?;
    output.resize(count, value);
    Ok(output)
}

struct Scan<'a, 'c, 'f> {
    census: &'c mut ScalarPrivateBorrowsV1<'a>,
    facts: &'f mut dyn ProjectedAssertionFactsV1,
    block: usize,
    statement: Option<usize>,
    allowed_place: Option<&'a SemanticPlaceV1>,
    allowed_destination: Option<&'a SemanticPlaceV1>,
}

impl<'a> Scan<'a, '_, '_> {
    fn local(&mut self, local: SemanticLocalIdV1) -> R<()> {
        charge(self.facts, 2)?;
        let value = self
            .census
            .locals
            .get_mut(local.index() as usize)
            .ok_or_else(malformed)?;
        if value.root_alias.is_some() || value.candidate.is_some() {
            value.blocked = true;
        }
        Ok(())
    }

    fn place(&mut self, place: &SemanticPlaceV1) -> R<()> {
        charge(self.facts, 4)?;
        if self.census.types.get(place.ty().index() as usize).is_none() {
            return Err(malformed());
        }
        let local = self
            .census
            .function
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(malformed)?;
        if place.projections().is_empty() && local.ty() != place.ty() {
            return Err(malformed());
        }
        let permitted = self.allowed_place.is_some_and(|p| std::ptr::eq(p, place))
            || self
                .allowed_destination
                .is_some_and(|p| std::ptr::eq(p, place));
        if !permitted {
            self.local(place.local())?;
        }
        for projection in place.projections() {
            charge(self.facts, 2)?;
            if self
                .census
                .types
                .get(projection.result_type().index() as usize)
                .is_none()
            {
                return Err(malformed());
            }
            if let SemanticProjectionKindV1::Index(local) = projection.kind() {
                self.local(local)?;
            }
        }
        Ok(())
    }

    fn operand(&mut self, operand: &SemanticOperandV1) -> R<()> {
        charge(self.facts, 2)?;
        match operand {
            SemanticOperandV1::Copy(place) => self.place(place),
            SemanticOperandV1::Move(place) => {
                // A Move never receives the exact-read waiver.
                self.local(place.local())?;
                self.place(place)
            }
            SemanticOperandV1::Constant(value) => {
                if self.census.types.get(value.ty().index() as usize).is_none() {
                    return Err(malformed());
                }
                Ok(())
            }
        }
    }

    fn value(&mut self, value: &'a SemanticRvalueV1) -> R<()> {
        charge(self.facts, 2)?;
        if self
            .census
            .types
            .get(value.result_type().index() as usize)
            .is_none()
        {
            return Err(malformed());
        }
        value
            .kind()
            .try_visit_operands(|operand| self.operand(operand))?;
        match value.kind() {
            SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. }
            | SemanticRvalueKindV1::Length(place)
            | SemanticRvalueKindV1::Discriminant(place) => self.place(place),
            SemanticRvalueKindV1::Load(load) => self.place(load.source()),
            SemanticRvalueKindV1::Use(_)
            | SemanticRvalueKindV1::Unary { .. }
            | SemanticRvalueKindV1::Cast { .. }
            | SemanticRvalueKindV1::Binary { .. }
            | SemanticRvalueKindV1::CheckedBinary(_)
            | SemanticRvalueKindV1::UncheckedBinary(_)
            | SemanticRvalueKindV1::Aggregate(_) => Ok(()),
        }
    }

    fn initialize(&mut self, place: &'a SemanticPlaceV1) -> R<()> {
        charge(self.facts, 5)?;
        let index = place.local().index() as usize;
        let row = *self.census.locals.get(index).ok_or_else(malformed)?;
        let Some(alias) = row.root_alias else {
            return Ok(());
        };
        let candidate = self.census.locals[alias].candidate.ok_or_else(malformed)?;
        if !place.projections().is_empty()
            || self.block != candidate.block
            || self.statement.is_none_or(|i| i >= candidate.statement)
            || row.defined_once
        {
            return Ok(());
        }
        self.census.locals[index].defined_once = true;
        self.census.locals[index].initialized = true;
        self.allowed_destination = Some(place);
        Ok(())
    }

    fn borrow(&mut self, assignment: &'a SemanticAssignmentV1) -> R<()> {
        charge(self.facts, 6)?;
        let local = assignment.destination().local().index() as usize;
        let Some(row) = self.census.locals.get(local).copied() else {
            return Err(malformed());
        };
        let Some(candidate) = row.candidate else {
            return Ok(());
        };
        if candidate.block != self.block || Some(candidate.statement) != self.statement {
            return Ok(());
        }
        let SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place,
        } = assignment.value().kind()
        else {
            return Ok(());
        };
        if self.census.locals[candidate.root].initialized && !row.alias_alive {
            self.census.locals[local].alias_alive = true;
            self.allowed_place = Some(place);
            self.allowed_destination = Some(assignment.destination());
        }
        Ok(())
    }

    fn read(&mut self, assignment: &'a SemanticAssignmentV1) -> R<()> {
        charge(self.facts, 8)?;
        let place = match assignment.value().kind() {
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => place,
            SemanticRvalueKindV1::Load(load)
                if load.volatility() == SemanticVolatilityV1::NonVolatile
                    && load.atomic().is_none() =>
            {
                load.source()
            }
            _ => return Ok(()),
        };
        let alias = place.local().index() as usize;
        let row = *self.census.locals.get(alias).ok_or_else(malformed)?;
        let Some(candidate) = row.candidate else {
            return Ok(());
        };
        let root = self.census.function.locals()[candidate.root].ty();
        if candidate.block != self.block
            || !row.alias_alive
            || !self.census.locals[candidate.root].initialized
            || self.statement.is_none_or(|i| i <= candidate.statement)
            || place.ty() != root
            || assignment.value().result_type() != root
            || !matches!(place.projections(), [p] if p.kind() == SemanticProjectionKindV1::Dereference && p.result_type() == root)
        {
            return Ok(());
        }
        let ordinal = self.census.starts[self.block]
            .checked_add(self.statement.ok_or_else(malformed)?)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        self.census.reads[ordinal] = Some(Read { place, alias });
        self.allowed_place = Some(place);
        Ok(())
    }

    fn lifetime(&mut self, local: SemanticLocalIdV1, live: bool) -> R<()> {
        charge(self.facts, 6)?;
        let index = local.index() as usize;
        let row = *self.census.locals.get(index).ok_or_else(malformed)?;
        let candidate = row.candidate.or_else(|| {
            row.root_alias
                .and_then(|alias| self.census.locals[alias].candidate)
        });
        let Some(candidate) = candidate else {
            return Ok(());
        };
        // Only value accesses must be same-block. A later lifetime end in an
        // exit block does not carry a pointer or authorize another access.
        if candidate.block != self.block {
            return Ok(());
        }
        let current = &mut self.census.locals[index];
        if live && (current.defined_once || current.alias_alive) {
            current.blocked = true;
        }
        if !live
            && ((current.root_alias.is_some() && !current.defined_once)
                || (current.candidate.is_some() && !current.alias_alive))
        {
            current.blocked = true;
        }
        current.initialized = false;
        current.alias_alive = false;
        Ok(())
    }

    fn statement(&mut self, kind: &'a SemanticStatementKindV1) -> R<()> {
        charge(self.facts, 2)?;
        self.allowed_place = None;
        self.allowed_destination = None;
        match kind {
            SemanticStatementKindV1::Assign(assignment) => {
                self.initialize(assignment.destination())?;
                self.borrow(assignment)?;
                self.read(assignment)?;
                self.place(assignment.destination())?;
                self.value(assignment.value())
            }
            SemanticStatementKindV1::Store(store) => {
                if store.volatility() == SemanticVolatilityV1::NonVolatile
                    && store.atomic().is_none()
                {
                    self.initialize(store.destination())?;
                }
                self.place(store.destination())?;
                self.operand(store.value())
            }
            SemanticStatementKindV1::AtomicRmw(atomic) => {
                self.place(atomic.address())?;
                self.place(atomic.destination())?;
                self.operand(atomic.value())
            }
            SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                self.place(atomic.address())?;
                self.place(atomic.destination())?;
                self.operand(atomic.expected())?;
                self.operand(atomic.replacement())
            }
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => self.place(place),
            SemanticStatementKindV1::StorageLive(local) => self.lifetime(*local, true),
            SemanticStatementKindV1::StorageDead(local) => self.lifetime(*local, false),
            SemanticStatementKindV1::Assume(operand) => self.operand(operand),
            SemanticStatementKindV1::Nop => Ok(()),
        }
    }

    fn message(&mut self, message: &'a SemanticAssertMessageV1) -> R<()> {
        charge(self.facts, 2)?;
        match message {
            SemanticAssertMessageV1::BoundsCheck { length, index }
            | SemanticAssertMessageV1::Overflow {
                left: length,
                right: index,
                ..
            }
            | SemanticAssertMessageV1::MisalignedPointerDereference {
                required_alignment: length,
                found_alignment: index,
            } => {
                self.operand(length)?;
                self.operand(index)
            }
            SemanticAssertMessageV1::DivisionByZero(value)
            | SemanticAssertMessageV1::RemainderByZero(value) => self.operand(value),
            SemanticAssertMessageV1::NullPointerDereference
            | SemanticAssertMessageV1::ResumedAfterReturn
            | SemanticAssertMessageV1::ResumedAfterPanic => Ok(()),
        }
    }

    fn terminator(&mut self, kind: &'a SemanticTerminatorKindV1) -> R<()> {
        charge(self.facts, 2)?;
        self.allowed_place = None;
        self.allowed_destination = None;
        self.statement = None;
        match kind {
            SemanticTerminatorKindV1::Call(call) => {
                for operand in call.arguments() {
                    self.operand(operand)?;
                }
                if let Some(destination) = call.destination() {
                    self.place(destination.place())?;
                }
                Ok(())
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for operand in call.arguments() {
                    self.operand(operand)?;
                }
                Ok(())
            }
            SemanticTerminatorKindV1::Drop { place, .. } => self.place(place),
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => self.operand(discriminant),
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                self.operand(condition)?;
                self.message(message)
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => Ok(()),
        }
    }
}

fn build<'a>(
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    target: SemanticTargetDataLayoutV1,
    statements: usize,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> R<ScalarPrivateBorrowsV1<'a>> {
    facts.reserve_scalar_private_storage_v1(std::mem::size_of::<ScalarPrivateBorrowsV1<'_>>())?;
    let locals = allocate(function.locals().len(), Local::default(), facts)?;
    let starts = allocate(
        function
            .blocks()
            .len()
            .checked_add(1)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
        0usize,
        facts,
    )?;
    let reads = allocate(statements, None, facts)?;
    let mut census = ScalarPrivateBorrowsV1 {
        function,
        types,
        target,
        ledger: facts.helper_value_ledger_v1()?,
        live_floor: 0,
        locals,
        starts,
        reads,
    };
    let mut offset = 0usize;
    for (block, body) in function.blocks().iter().enumerate() {
        charge(facts, 3)?;
        census.starts[block] = offset;
        offset = offset
            .checked_add(body.statements().len())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        for (statement, value) in body.statements().iter().enumerate() {
            if let Some(c) = candidate(
                types,
                function,
                target,
                block,
                statement,
                value.kind(),
                facts,
            )? {
                charge(facts, 6)?;
                if let Some(previous) = census.locals[c.alias].candidate {
                    census.locals[previous.root].blocked = true;
                    census.locals[c.alias].blocked = true;
                    census.locals[c.root].blocked = true;
                } else {
                    census.locals[c.alias].candidate = Some(c);
                }
                if census.locals[c.root].root_alias.is_some() {
                    census.locals[c.root].blocked = true;
                } else {
                    census.locals[c.root].root_alias = Some(c.alias);
                }
            }
        }
    }
    census.starts[function.blocks().len()] = offset;
    for (block, body) in function.blocks().iter().enumerate() {
        charge(facts, 2)?;
        let mut scan = Scan {
            census: &mut census,
            facts,
            block,
            statement: None,
            allowed_place: None,
            allowed_destination: None,
        };
        for (statement, value) in body.statements().iter().enumerate() {
            scan.statement = Some(statement);
            scan.statement(value.kind())?;
        }
        scan.terminator(body.terminator().kind())?;
    }
    census.live_floor = facts.scalar_private_storage_v1()?;
    Ok(census)
}

/// New canonical-ledger scratch only; inherited projector domains are unchanged.
/// V397's exact slot/ledger identity query is a prerequisite, not a fake budget.
pub(super) fn with_scalar_private_borrows_v1<T, F: ProjectedAssertionFactsV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    target: SemanticTargetDataLayoutV1,
    facts: &mut F,
    next: impl for<'s> FnOnce(Option<&'s ScalarPrivateBorrowsV1<'s>>, &mut F) -> R<T>,
) -> R<T> {
    charge(facts, 6)?;
    let mut found = false;
    let mut statements = 0usize;
    for (block, body) in function.blocks().iter().enumerate() {
        charge(facts, 3)?;
        statements = statements
            .checked_add(body.statements().len())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        for (statement, value) in body.statements().iter().enumerate() {
            found |= candidate(
                types,
                function,
                target,
                block,
                statement,
                value.kind(),
                facts,
            )?
            .is_some();
        }
    }
    if !found {
        return next(None, facts);
    }
    let floor = facts.scalar_private_storage_v1()?;
    let identity = facts.helper_value_ledger_v1()?;
    let mut callback_floor = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let census = build(types, function, target, statements, facts)?;
        callback_floor = Some(facts.scalar_private_storage_v1()?);
        next(Some(&census), facts)
    }));
    let cleanup = (|| {
        // Never debit or release a replacement ledger. If the callback is
        // unwinding, accounting failures must not replace its original panic.
        if facts.helper_value_ledger_v1()? != identity {
            return Err(resource(Resource::Accounting));
        }
        let live = facts.scalar_private_storage_v1()?;
        let excess = live
            .checked_sub(floor)
            .ok_or_else(|| resource(Resource::Accounting))?;
        let balanced = callback_floor.is_none_or(|expected| expected == live);
        facts.release_scalar_private_storage_v1(excess)?;
        if facts.scalar_private_storage_v1()? != floor || !balanced {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    })();
    match result {
        Ok(result) => {
            cleanup?;
            result
        }
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
