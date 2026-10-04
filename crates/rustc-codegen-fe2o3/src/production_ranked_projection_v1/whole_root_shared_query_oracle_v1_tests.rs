//! All source operand/read/place queries at their real statement/terminator site.
//! Observable contains answers and work only; not private row/duplicate equality.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
pub(super) struct QueryComparison<'a, 's, 'r, 'i, 'g, 'b, 'w> {
    owner: &'s ProductionPreRankedKirOwnerV1,
    function_id: SemanticFunctionIdV1,
    function: &'s SemanticFunctionDeclV1,
    original: &'a ProductionSemanticSharedReadsV1<'s>,
    retained: &'a ProductionSemanticSharedReadsV1<'s>,
    facts: &'a mut CanonicalSourceAssertionFactsV1<'r, 'i, 'g, 'b, 'w>,
    owned: &'a mut usize,
    pub(super) visits: usize,
    pub(super) accepted: usize,
}
impl QueryComparison<'_, '_, '_, '_, '_, '_, '_> {
    fn place(
        &mut self,
        site: ProjectedSemanticAccessSiteV1,
        place: &SemanticPlaceV1,
    ) -> BResult<()> {
        self.facts.charge_private_array_work(2)?;
        let before = self.facts.retained_whole_root_snapshot_v1(
            self.owner,
            self.function_id,
            self.owned,
            None,
        )?;
        let expected =
            self.facts
                .shared_value_read_v1(self.original, self.function, site, place)?;
        let middle = self.facts.retained_whole_root_snapshot_v1(
            self.owner,
            self.function_id,
            self.owned,
            None,
        )?;
        let actual = self
            .facts
            .shared_value_read_v1(self.retained, self.function, site, place)?;
        let after = self.facts.retained_whole_root_snapshot_v1(
            self.owner,
            self.function_id,
            self.owned,
            None,
        )?;
        if expected != actual
            || middle.work.checked_sub(before.work) != after.work.checked_sub(middle.work)
            || before.storage != middle.storage
            || middle.storage != after.storage
            || before.owned != after.owned
        {
            return Err(accounting());
        }
        self.visits = self.visits.checked_add(1).ok_or_else(arithmetic)?;
        if expected {
            self.accepted = self.accepted.checked_add(1).ok_or_else(arithmetic)?;
        }
        Ok(())
    }
    fn operand(
        &mut self,
        site: ProjectedSemanticAccessSiteV1,
        operand: &SemanticOperandV1,
    ) -> BResult<()> {
        self.facts.charge_private_array_work(1)?;
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.place(site, place)
            }
            SemanticOperandV1::Constant(_) => Ok(()),
        }
    }
    fn rvalue(
        &mut self,
        site: ProjectedSemanticAccessSiteV1,
        value: &SemanticRvalueKindV1,
    ) -> BResult<()> {
        value.try_visit_operands(|operand| self.operand(site, operand))?;
        match value {
            SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. }
            | SemanticRvalueKindV1::Length(place)
            | SemanticRvalueKindV1::Discriminant(place) => self.place(site, place),
            SemanticRvalueKindV1::Load(load) => self.place(site, load.source()),
            _ => Ok(()),
        }
    }
    fn all(&mut self) -> BResult<()> {
        for (block, body) in self.function.blocks().iter().enumerate() {
            self.facts.charge_private_array_work(1)?;
            for (statement, value) in body.statements().iter().enumerate() {
                self.facts.charge_private_array_work(1)?;
                let site = ProjectedSemanticAccessSiteV1 {
                    block,
                    statement: Some(statement),
                };
                match value.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        self.rvalue(site, assignment.value().kind())?;
                        self.place(site, assignment.destination())?;
                    }
                    SemanticStatementKindV1::Store(store) => {
                        self.operand(site, store.value())?;
                        self.place(site, store.destination())?;
                    }
                    SemanticStatementKindV1::AtomicRmw(atomic) => {
                        self.operand(site, atomic.value())?;
                        self.place(site, atomic.address())?;
                        self.place(site, atomic.destination())?;
                    }
                    SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                        self.operand(site, atomic.expected())?;
                        self.operand(site, atomic.replacement())?;
                        self.place(site, atomic.address())?;
                        self.place(site, atomic.destination())?;
                    }
                    SemanticStatementKindV1::Assume(operand) => self.operand(site, operand)?,
                    SemanticStatementKindV1::Deinitialize(place)
                    | SemanticStatementKindV1::SetDiscriminant { place, .. } => {
                        self.place(site, place)?
                    }
                    SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Nop => {}
                }
            }
            let site = ProjectedSemanticAccessSiteV1 {
                block,
                statement: None,
            };
            self.facts.charge_private_array_work(1)?;
            match body.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => {
                    for operand in call.arguments() {
                        self.operand(site, operand)?;
                    }
                    if let Some(destination) = call.destination() {
                        self.place(site, destination.place())?;
                    }
                }
                SemanticTerminatorKindV1::TailCall(call) => {
                    for operand in call.arguments() {
                        self.operand(site, operand)?;
                    }
                }
                SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                    self.operand(site, discriminant)?
                }
                SemanticTerminatorKindV1::Assert {
                    condition, message, ..
                } => {
                    self.operand(site, condition)?;
                    visit_assert_operands_v1(message, &mut |operand| self.operand(site, operand))?;
                }
                SemanticTerminatorKindV1::Drop { place, .. } => self.place(site, place)?,
                SemanticTerminatorKindV1::Goto(_)
                | SemanticTerminatorKindV1::Return
                | SemanticTerminatorKindV1::UnwindResume
                | SemanticTerminatorKindV1::UnwindTerminate
                | SemanticTerminatorKindV1::Abort
                | SemanticTerminatorKindV1::Unreachable
                | SemanticTerminatorKindV1::FalseEdge { .. } => {}
            }
        }
        Ok(())
    }
}
pub(super) fn compare<'s>(
    owner: &'s ProductionPreRankedKirOwnerV1,
    function_id: SemanticFunctionIdV1,
    function: &'s SemanticFunctionDeclV1,
    original: &ProductionSemanticSharedReadsV1<'s>,
    retained: &ProductionSemanticSharedReadsV1<'s>,
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
    owned: &mut usize,
) -> BResult<(usize, usize)> {
    let mut comparison = QueryComparison {
        owner,
        function_id,
        function,
        original,
        retained,
        facts,
        owned,
        visits: 0,
        accepted: 0,
    };
    comparison.all()?;
    Ok((comparison.visits, comparison.accepted))
}
pub(super) fn frame() -> usize {
    size_of::<(
        QueryComparison<'static, 'static, 'static, 'static, 'static, 'static, 'static>,
        &mut QueryComparison<'static, 'static, 'static, 'static, 'static, 'static, 'static>,
        &ProductionPreRankedKirOwnerV1,
        SemanticFunctionIdV1,
        &SemanticFunctionDeclV1,
        &ProductionSemanticSharedReadsV1<'static>,
        &mut usize,
        &mut CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>,
        Snapshot,
        BResult<Snapshot>,
        usize,
        Option<usize>,
        bool,
        BResult<bool>,
        BResult<()>,
        (usize, usize),
        BResult<(usize, usize)>,
        ProjectedSemanticAccessSiteV1,
        Option<usize>,
        &SemanticPlaceV1,
        &SemanticOperandV1,
        &SemanticRvalueKindV1,
        &SemanticStatementKindV1,
        &SemanticTerminatorKindV1,
        &SemanticTerminatorV1,
        &SemanticBasicBlockV1,
        &SemanticStatementV1,
        &[SemanticBasicBlockV1],
        &[SemanticStatementV1],
        &SemanticRvalueV1,
        &SemanticAssignmentV1,
        &SemanticMemoryStoreV1,
        &SemanticMemoryLoadV1,
        &SemanticAtomicRmwV1,
        &SemanticAtomicCompareExchangeV1,
        &SemanticRvalueV1,
        &SemanticDirectCallV1,
        &SemanticDirectTailCallV1,
        Option<&SemanticCallDestinationV1>,
        &SemanticCallDestinationV1,
        &SemanticAssertMessageV1,
        std::iter::Enumerate<std::slice::Iter<'static, SemanticBasicBlockV1>>,
        std::iter::Enumerate<std::slice::Iter<'static, SemanticStatementV1>>,
        std::slice::Iter<'static, SemanticOperandV1>,
        &[SemanticOperandV1],
        &SemanticCheckedBinaryRvalueV1,
        &SemanticUncheckedBinaryRvalueV1,
        &SemanticAggregateRvalueV1,
        &mut dyn FnMut(&SemanticOperandV1) -> BResult<()>,
        Resource,
        Backend,
        CanonicalAssertionErrorV1,
    )>()
}

pub(super) fn visit_assert_operands_v1<E>(
    message: &SemanticAssertMessageV1,
    visitor: &mut impl FnMut(&SemanticOperandV1) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => {
            visitor(length)?;
            visitor(index)
        }
        SemanticAssertMessageV1::Overflow { left, right, .. } => {
            visitor(left)?;
            visitor(right)
        }
        SemanticAssertMessageV1::DivisionByZero(operand)
        | SemanticAssertMessageV1::RemainderByZero(operand) => visitor(operand),
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => {
            visitor(required_alignment)?;
            visitor(found_alignment)
        }
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => Ok(()),
    }
}
