//! Inert source eligibility for whole scalar retained slots. This is not an
//! initialization, memory-value, or fresh source/N equivalence proof.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

const ELIGIBLE: u8 = 1;
const EXPLICIT: u8 = 2;
const BLOCKED: u8 = 4;

fn resource(error: Resource) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
    )
}

fn malformed() -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Unsupported(
        "scalar private singleton census has malformed local or type custody",
    )
}

fn charge(
    facts: &mut dyn ProjectedAssertionFactsV1,
    amount: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    facts.charge_private_array_work(amount)
}

pub(super) fn eligible(census: &[u8], local: SemanticLocalIdV1) -> bool {
    census.get(local.index() as usize) == Some(&(ELIGIBLE | EXPLICIT))
}

fn has_explicit_whole_place(
    function: &SemanticFunctionDeclV1,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    for block in function.blocks() {
        charge(facts, 1)?;
        for statement in block.statements() {
            charge(facts, 3)?;
            let place = match statement.kind() {
                SemanticStatementKindV1::Store(store) => Some(store.destination()),
                SemanticStatementKindV1::Assign(assignment) => match assignment.value().kind() {
                    SemanticRvalueKindV1::Load(load) => Some(load.source()),
                    _ => None,
                },
                _ => None,
            };
            if place.is_some_and(|place| place.projections().is_empty()) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

struct Census<'a, 'f> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    flags: &'a mut [u8],
    facts: &'f mut dyn ProjectedAssertionFactsV1,
}

impl Census<'_, '_> {
    fn place(
        &mut self,
        place: &SemanticPlaceV1,
        block: bool,
        explicit: bool,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let work = place
            .projections()
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(8))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        charge(self.facts, work)?;
        let local = self
            .function
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(malformed)?;
        if self.types.get(place.ty().index() as usize).is_none()
            || (place.projections().is_empty() && place.ty() != local.ty())
        {
            return Err(malformed());
        }
        for projection in place.projections() {
            if self
                .types
                .get(projection.result_type().index() as usize)
                .is_none()
            {
                return Err(malformed());
            }
            if let SemanticProjectionKindV1::Index(index) = projection.kind()
                && self.function.locals().get(index.index() as usize).is_none()
            {
                return Err(malformed());
            }
        }
        let flag = self
            .flags
            .get_mut(place.local().index() as usize)
            .ok_or_else(malformed)?;
        if block || !place.projections().is_empty() {
            *flag |= BLOCKED;
        }
        if explicit {
            *flag |= EXPLICIT;
        }
        Ok(())
    }

    fn operand(
        &mut self,
        operand: &SemanticOperandV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge(self.facts, 2)?;
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.place(place, false, false)
            }
            SemanticOperandV1::Constant(constant) => {
                if self.types.get(constant.ty().index() as usize).is_none() {
                    return Err(malformed());
                }
                Ok(())
            }
        }
    }

    fn value(&mut self, value: &SemanticRvalueV1) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge(self.facts, 3)?;
        if self
            .types
            .get(value.result_type().index() as usize)
            .is_none()
        {
            return Err(malformed());
        }
        match value.kind() {
            SemanticRvalueKindV1::Use(operand)
            | SemanticRvalueKindV1::Unary { operand, .. }
            | SemanticRvalueKindV1::Cast { operand, .. } => self.operand(operand),
            SemanticRvalueKindV1::Binary { left, right, .. } => {
                self.operand(left)?;
                self.operand(right)
            }
            SemanticRvalueKindV1::CheckedBinary(binary) => {
                self.operand(binary.left())?;
                self.operand(binary.right())
            }
            SemanticRvalueKindV1::UncheckedBinary(binary) => {
                self.operand(binary.left())?;
                self.operand(binary.right())
            }
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                for operand in aggregate.operands() {
                    self.operand(operand)?;
                }
                Ok(())
            }
            SemanticRvalueKindV1::Load(load) => self.place(
                load.source(),
                load.volatility() != SemanticVolatilityV1::NonVolatile || load.atomic().is_some(),
                true,
            ),
            SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. } => self.place(place, true, false),
            SemanticRvalueKindV1::Length(place) | SemanticRvalueKindV1::Discriminant(place) => {
                self.place(place, true, false)
            }
        }
    }

    fn statement(
        &mut self,
        kind: &SemanticStatementKindV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge(self.facts, 2)?;
        match kind {
            SemanticStatementKindV1::Assign(assignment) => {
                self.place(assignment.destination(), false, false)?;
                self.value(assignment.value())
            }
            SemanticStatementKindV1::Store(store) => {
                self.place(
                    store.destination(),
                    store.volatility() != SemanticVolatilityV1::NonVolatile
                        || store.atomic().is_some(),
                    true,
                )?;
                self.operand(store.value())
            }
            SemanticStatementKindV1::AtomicRmw(atomic) => {
                self.place(atomic.address(), true, false)?;
                self.place(atomic.destination(), true, false)?;
                self.operand(atomic.value())
            }
            SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                self.place(atomic.address(), true, false)?;
                self.place(atomic.destination(), true, false)?;
                self.operand(atomic.expected())?;
                self.operand(atomic.replacement())
            }
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => self.place(place, true, false),
            SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local) => {
                charge(self.facts, 1)?;
                if self.function.locals().get(local.index() as usize).is_none() {
                    return Err(malformed());
                }
                Ok(())
            }
            SemanticStatementKindV1::Assume(operand) => self.operand(operand),
            SemanticStatementKindV1::Nop => Ok(()),
        }
    }

    fn message(
        &mut self,
        message: &SemanticAssertMessageV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
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
            SemanticAssertMessageV1::DivisionByZero(operand)
            | SemanticAssertMessageV1::RemainderByZero(operand) => self.operand(operand),
            SemanticAssertMessageV1::NullPointerDereference
            | SemanticAssertMessageV1::ResumedAfterReturn
            | SemanticAssertMessageV1::ResumedAfterPanic => Ok(()),
        }
    }

    fn terminator(
        &mut self,
        kind: &SemanticTerminatorKindV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge(self.facts, 2)?;
        match kind {
            SemanticTerminatorKindV1::Call(call) => {
                for argument in call.arguments() {
                    self.operand(argument)?;
                }
                if let Some(destination) = call.destination() {
                    self.place(destination.place(), false, false)?;
                }
                Ok(())
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for argument in call.arguments() {
                    self.operand(argument)?;
                }
                Ok(())
            }
            SemanticTerminatorKindV1::Drop { place, .. } => self.place(place, true, false),
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

/// New scratch is bounded by this ledger. Existing projector allocations retain
/// their prior limits. The closure cannot return a borrow of the census.
pub(super) fn with_scalar_private_singletons_v1<T, F: ProjectedAssertionFactsV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    facts: &mut F,
    action: impl FnOnce(&[u8], &mut F) -> Result<T, ProductionRankedProjectionErrorV1>,
) -> Result<T, ProductionRankedProjectionErrorV1> {
    charge(facts, 4)?;
    if !has_explicit_whole_place(function, facts)? {
        return action(&[], facts);
    }
    let count = function.locals().len();
    let requested = count
        .checked_add(std::mem::size_of::<Vec<u8>>())
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    let floor = facts.scalar_private_storage_v1()?;
    let mut retained = 0_usize;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        facts.reserve_scalar_private_storage_v1(requested)?;
        retained = requested;
        let mut flags = Vec::new();
        flags
            .try_reserve_exact(count)
            .map_err(|_| resource(Resource::Allocation))?;
        let excess = flags
            .capacity()
            .checked_sub(count)
            .ok_or_else(|| resource(Resource::Accounting))?;
        facts.reserve_scalar_private_storage_v1(excess)?;
        retained = retained
            .checked_add(excess)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        charge(
            facts,
            count
                .checked_mul(5)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        for local in function.locals() {
            let ty = types
                .get(local.ty().index() as usize)
                .ok_or_else(malformed)?;
            let is_scalar = matches!(ty.shape(), SemanticTypeShapeV1::Scalar(_));
            flags.push(if !local.role().is_entry_argument() && is_scalar {
                ELIGIBLE
            } else {
                0
            });
        }
        let mut census = Census {
            types,
            function,
            flags: &mut flags,
            facts,
        };
        for block in function.blocks() {
            charge(census.facts, 1)?;
            for statement in block.statements() {
                census.statement(statement.kind())?;
            }
            census.terminator(block.terminator().kind())?;
        }
        action(&flags, facts)
    }));
    let expected = floor
        .checked_add(retained)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    if facts.scalar_private_storage_v1()? != expected {
        return Err(resource(Resource::Accounting));
    }
    facts.release_scalar_private_storage_v1(retained)?;
    if facts.scalar_private_storage_v1()? != floor {
        return Err(resource(Resource::Accounting));
    }
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
