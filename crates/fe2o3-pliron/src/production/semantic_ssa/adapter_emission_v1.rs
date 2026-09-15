//! One event grammar for real planner input and auxiliary borrow analysis.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticConstantV1;
use std::convert::Infallible;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::semantic_ssa) enum SemanticSsaEmissionSiteV1 {
    Auxiliary,
    Function,
    Local(usize),
    Block(usize),
    Statement { block: usize, statement: usize },
    Terminator { block: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::semantic_ssa) enum SemanticSsaVisitV1 {
    Function,
    EntryCandidate,
    Block,
    Statement,
    Rvalue,
    Operand,
    Place,
    Projection,
    Terminator,
    AssertMessage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::semantic_ssa) enum SemanticSsaOperandRoleV1 {
    RvalueOperand(usize),
    RvaluePlace,
    Destination,
    StoreValue,
    StoreDestination,
    AtomicAddress,
    AtomicValue,
    AtomicExpected,
    AtomicReplacement,
    AtomicDestination,
    StatementPlace,
    Assume,
    StorageLive,
    StorageDead,
    CallArgument(usize),
    CallDestinationAddress,
    TailCallArgument(usize),
    SwitchDiscriminant,
    DropPlace,
    AssertCondition,
    AssertMessage(usize),
    ReturnValue,
    ElidedBorrowDestination,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::semantic_ssa) enum SemanticSsaEventRoleV1 {
    BaseUse,
    ProjectionIndexUse(usize),
    MoveKill,
    DestinationDefine,
    StorageKill,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::semantic_ssa) enum SemanticSsaEntryOriginV1 {
    Argument(u32),
    ImplicitCapability,
}

// Callbacks run before the corresponding event/definition push. Implementations
// must stop on denial; auxiliary analysis always uses the no-op observer.
pub(in crate::production::semantic_ssa) trait SemanticSsaEmissionObserverV1 {
    type Error;
    fn block_pass_begin(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn entry_pass_begin(&mut self, _: usize, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn visit(
        &mut self,
        kind: SemanticSsaVisitV1,
        site: SemanticSsaEmissionSiteV1,
    ) -> Result<(), Self::Error>;
    fn event(
        &mut self,
        site: SemanticSsaEmissionSiteV1,
        operand: SemanticSsaOperandRoleV1,
        role: SemanticSsaEventRoleV1,
        ordinal: usize,
        event: SsaEventV1,
    ) -> Result<(), Self::Error>;
    fn constant(
        &mut self,
        site: SemanticSsaEmissionSiteV1,
        operand: SemanticSsaOperandRoleV1,
        next_event: usize,
        constant: &SemanticConstantV1,
    ) -> Result<(), Self::Error>;
    fn successor(
        &mut self,
        block: usize,
        ordinal: usize,
        edge: SemanticControlFlowEdgeV1,
    ) -> Result<(), Self::Error>;
    fn edge_definition(
        &mut self,
        block: usize,
        edge_ordinal: usize,
        edge: SemanticControlFlowEdgeV1,
        definition_ordinal: usize,
        variable: SsaVariableIdV1,
    ) -> Result<(), Self::Error>;
    fn entry_definition(
        &mut self,
        ordinal: usize,
        variable: SsaVariableIdV1,
        origin: SemanticSsaEntryOriginV1,
    ) -> Result<(), Self::Error>;
    fn elided_borrow(&mut self, site: SemanticSsaEmissionSiteV1) -> Result<(), Self::Error>;
    fn statement_elision_lookup(
        &mut self,
        site: SemanticSsaEmissionSiteV1,
        candidates: usize,
    ) -> Result<(), Self::Error>;
    fn block_complete(
        &mut self,
        block: usize,
        events: usize,
        successors: usize,
    ) -> Result<(), Self::Error>;
    fn input_complete(
        &mut self,
        blocks: usize,
        entry_definitions: usize,
    ) -> Result<(), Self::Error>;
}

pub(super) struct NoSemanticSsaEmissionObserverV1;

impl SemanticSsaEmissionObserverV1 for NoSemanticSsaEmissionObserverV1 {
    type Error = Infallible;
    fn visit(
        &mut self,
        _: SemanticSsaVisitV1,
        _: SemanticSsaEmissionSiteV1,
    ) -> Result<(), Infallible> {
        Ok(())
    }
    fn event(
        &mut self,
        _: SemanticSsaEmissionSiteV1,
        _: SemanticSsaOperandRoleV1,
        _: SemanticSsaEventRoleV1,
        _: usize,
        _: SsaEventV1,
    ) -> Result<(), Infallible> {
        Ok(())
    }
    fn constant(
        &mut self,
        _: SemanticSsaEmissionSiteV1,
        _: SemanticSsaOperandRoleV1,
        _: usize,
        _: &SemanticConstantV1,
    ) -> Result<(), Infallible> {
        Ok(())
    }
    fn successor(
        &mut self,
        _: usize,
        _: usize,
        _: SemanticControlFlowEdgeV1,
    ) -> Result<(), Infallible> {
        Ok(())
    }
    fn edge_definition(
        &mut self,
        _: usize,
        _: usize,
        _: SemanticControlFlowEdgeV1,
        _: usize,
        _: SsaVariableIdV1,
    ) -> Result<(), Infallible> {
        Ok(())
    }
    fn entry_definition(
        &mut self,
        _: usize,
        _: SsaVariableIdV1,
        _: SemanticSsaEntryOriginV1,
    ) -> Result<(), Infallible> {
        Ok(())
    }
    fn elided_borrow(&mut self, _: SemanticSsaEmissionSiteV1) -> Result<(), Infallible> {
        Ok(())
    }
    fn statement_elision_lookup(
        &mut self,
        _: SemanticSsaEmissionSiteV1,
        _: usize,
    ) -> Result<(), Infallible> {
        Ok(())
    }
    fn block_complete(&mut self, _: usize, _: usize, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
    fn input_complete(&mut self, _: usize, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
}

pub(super) fn infallible_v1<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => match error {},
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::production::semantic_ssa) enum SemanticSsaEmissionErrorV1<O, E> {
    Observer(O),
    Output(E),
}

pub(in crate::production::semantic_ssa) fn observer_error_v1<O>(
    error: SemanticSsaEmissionErrorV1<O, Infallible>,
) -> O {
    match error {
        SemanticSsaEmissionErrorV1::Observer(error) => error,
        SemanticSsaEmissionErrorV1::Output(error) => match error {},
    }
}

type EmissionResultV1<O, E> = Result<(), SemanticSsaEmissionErrorV1<O, E>>;

// The grammar does not require a second planner input for a future count-only
// output. Real emission uses the original event Vec and its actual length.
pub(in crate::production::semantic_ssa) trait SemanticSsaEventBufferV1 {
    type Error;

    fn event_count(&self) -> usize;
    fn push_event(&mut self, event: SsaEventV1) -> Result<(), Self::Error>;
}

impl SemanticSsaEventBufferV1 for Vec<SsaEventV1> {
    type Error = Infallible;

    fn event_count(&self) -> usize {
        self.len()
    }
    fn push_event(&mut self, event: SsaEventV1) -> Result<(), Infallible> {
        self.push(event);
        Ok(())
    }
}

struct EmitterV1<'a, B, O> {
    events: &'a mut B,
    observer: &'a mut O,
    site: SemanticSsaEmissionSiteV1,
}

pub(in crate::production::semantic_ssa) fn emit_statement_events_v1<
    B: SemanticSsaEventBufferV1<Error = Infallible>,
    O: SemanticSsaEmissionObserverV1,
>(
    statement: &SemanticStatementKindV1,
    elided_borrow: bool,
    site: SemanticSsaEmissionSiteV1,
    events: &mut B,
    observer: &mut O,
) -> Result<(), O::Error> {
    emit_statement_events_with_buffer_v1(statement, elided_borrow, site, events, observer)
        .map_err(observer_error_v1)
}

pub(in crate::production::semantic_ssa) fn emit_terminator_events_v1<
    B: SemanticSsaEventBufferV1<Error = Infallible>,
    O: SemanticSsaEmissionObserverV1,
>(
    terminator: &SemanticTerminatorKindV1,
    return_local: Option<usize>,
    site: SemanticSsaEmissionSiteV1,
    events: &mut B,
    observer: &mut O,
) -> Result<(), O::Error> {
    emit_terminator_events_with_buffer_v1(terminator, return_local, site, events, observer)
        .map_err(observer_error_v1)
}

pub(in crate::production::semantic_ssa) fn emit_statement_events_with_buffer_v1<
    B: SemanticSsaEventBufferV1,
    O: SemanticSsaEmissionObserverV1,
>(
    statement: &SemanticStatementKindV1,
    elided_borrow: bool,
    site: SemanticSsaEmissionSiteV1,
    events: &mut B,
    observer: &mut O,
) -> EmissionResultV1<O::Error, B::Error> {
    let mut emitter = EmitterV1 {
        events,
        observer,
        site,
    };
    emitter.visit(SemanticSsaVisitV1::Statement)?;
    if elided_borrow {
        let SemanticStatementKindV1::Assign(assignment) = statement else {
            unreachable!("authenticated borrow site must be an assignment");
        };
        emitter
            .observer
            .elided_borrow(site)
            .map_err(SemanticSsaEmissionErrorV1::Observer)?;
        emitter.definition(
            assignment.destination(),
            SemanticSsaOperandRoleV1::ElidedBorrowDestination,
        )
    } else {
        emitter.statement(statement)
    }
}

pub(in crate::production::semantic_ssa) fn emit_terminator_events_with_buffer_v1<
    B: SemanticSsaEventBufferV1,
    O: SemanticSsaEmissionObserverV1,
>(
    terminator: &SemanticTerminatorKindV1,
    return_local: Option<usize>,
    site: SemanticSsaEmissionSiteV1,
    events: &mut B,
    observer: &mut O,
) -> EmissionResultV1<O::Error, B::Error> {
    let mut emitter = EmitterV1 {
        events,
        observer,
        site,
    };
    emitter.visit(SemanticSsaVisitV1::Terminator)?;
    emitter.terminator(terminator, return_local)
}

impl<B: SemanticSsaEventBufferV1, O: SemanticSsaEmissionObserverV1> EmitterV1<'_, B, O> {
    fn visit(&mut self, kind: SemanticSsaVisitV1) -> EmissionResultV1<O::Error, B::Error> {
        self.observer
            .visit(kind, self.site)
            .map_err(SemanticSsaEmissionErrorV1::Observer)
    }
    fn event(
        &mut self,
        operand: SemanticSsaOperandRoleV1,
        role: SemanticSsaEventRoleV1,
        event: SsaEventV1,
    ) -> EmissionResultV1<O::Error, B::Error> {
        self.observer
            .event(self.site, operand, role, self.events.event_count(), event)
            .map_err(SemanticSsaEmissionErrorV1::Observer)?;
        self.events
            .push_event(event)
            .map_err(SemanticSsaEmissionErrorV1::Output)
    }
    fn statement(
        &mut self,
        statement: &SemanticStatementKindV1,
    ) -> EmissionResultV1<O::Error, B::Error> {
        use SemanticSsaOperandRoleV1 as R;
        match statement {
            SemanticStatementKindV1::Assign(assignment) => {
                self.rvalue(assignment.value().kind())?;
                self.definition(assignment.destination(), R::Destination)
            }
            SemanticStatementKindV1::Store(store) => {
                self.operand(store.value(), R::StoreValue)?;
                self.place(store.destination(), R::StoreDestination)
            }
            SemanticStatementKindV1::AtomicRmw(operation) => {
                self.place(operation.address(), R::AtomicAddress)?;
                self.operand(operation.value(), R::AtomicValue)?;
                self.definition(operation.destination(), R::AtomicDestination)
            }
            SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                self.place(operation.address(), R::AtomicAddress)?;
                self.operand(operation.expected(), R::AtomicExpected)?;
                self.operand(operation.replacement(), R::AtomicReplacement)?;
                self.definition(operation.destination(), R::AtomicDestination)
            }
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => self.place(place, R::StatementPlace),
            SemanticStatementKindV1::Assume(condition) => self.operand(condition, R::Assume),
            SemanticStatementKindV1::StorageLive(local) => self.event(
                R::StorageLive,
                SemanticSsaEventRoleV1::StorageKill,
                SsaEventV1::Kill(SsaVariableIdV1::new(local.index())),
            ),
            SemanticStatementKindV1::StorageDead(local) => self.event(
                R::StorageDead,
                SemanticSsaEventRoleV1::StorageKill,
                SsaEventV1::Kill(SsaVariableIdV1::new(local.index())),
            ),
            SemanticStatementKindV1::Nop => Ok(()),
        }
    }
    fn rvalue(&mut self, value: &SemanticRvalueKindV1) -> EmissionResultV1<O::Error, B::Error> {
        use SemanticSsaOperandRoleV1 as R;
        self.visit(SemanticSsaVisitV1::Rvalue)?;
        match value {
            SemanticRvalueKindV1::Use(operand)
            | SemanticRvalueKindV1::Unary { operand, .. }
            | SemanticRvalueKindV1::Cast { operand, .. } => {
                self.operand(operand, R::RvalueOperand(0))
            }
            SemanticRvalueKindV1::Binary { left, right, .. } => {
                self.operand(left, R::RvalueOperand(0))?;
                self.operand(right, R::RvalueOperand(1))
            }
            SemanticRvalueKindV1::CheckedBinary(operation) => {
                self.operand(operation.left(), R::RvalueOperand(0))?;
                self.operand(operation.right(), R::RvalueOperand(1))
            }
            SemanticRvalueKindV1::UncheckedBinary(operation) => {
                self.operand(operation.left(), R::RvalueOperand(0))?;
                self.operand(operation.right(), R::RvalueOperand(1))
            }
            SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. }
            | SemanticRvalueKindV1::Length(place)
            | SemanticRvalueKindV1::Discriminant(place) => self.place(place, R::RvaluePlace),
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                for (ordinal, operand) in aggregate.operands().iter().enumerate() {
                    self.operand(operand, R::RvalueOperand(ordinal))?;
                }
                Ok(())
            }
            SemanticRvalueKindV1::Load(load) => self.place(load.source(), R::RvaluePlace),
        }
    }
    fn operand(
        &mut self,
        operand: &SemanticOperandV1,
        role: SemanticSsaOperandRoleV1,
    ) -> EmissionResultV1<O::Error, B::Error> {
        self.visit(SemanticSsaVisitV1::Operand)?;
        match operand {
            SemanticOperandV1::Copy(place) => self.place(place, role),
            SemanticOperandV1::Move(place) => {
                self.place(place, role)?;
                if place.projections().is_empty() {
                    self.event(
                        role,
                        SemanticSsaEventRoleV1::MoveKill,
                        SsaEventV1::Kill(SsaVariableIdV1::new(place.local().index())),
                    )?;
                }
                Ok(())
            }
            SemanticOperandV1::Constant(constant) => self
                .observer
                .constant(self.site, role, self.events.event_count(), constant)
                .map_err(SemanticSsaEmissionErrorV1::Observer),
        }
    }
    fn place(
        &mut self,
        place: &SemanticPlaceV1,
        role: SemanticSsaOperandRoleV1,
    ) -> EmissionResultV1<O::Error, B::Error> {
        self.visit(SemanticSsaVisitV1::Place)?;
        self.place_contents(place, role)
    }
    fn place_contents(
        &mut self,
        place: &SemanticPlaceV1,
        role: SemanticSsaOperandRoleV1,
    ) -> EmissionResultV1<O::Error, B::Error> {
        self.event(
            role,
            SemanticSsaEventRoleV1::BaseUse,
            SsaEventV1::Use(SsaVariableIdV1::new(place.local().index())),
        )?;
        for (ordinal, projection) in place.projections().iter().enumerate() {
            self.visit(SemanticSsaVisitV1::Projection)?;
            if let SemanticProjectionKindV1::Index(local) = projection.kind() {
                self.event(
                    role,
                    SemanticSsaEventRoleV1::ProjectionIndexUse(ordinal),
                    SsaEventV1::Use(SsaVariableIdV1::new(local.index())),
                )?;
            }
        }
        Ok(())
    }
    fn call_destination_address(
        &mut self,
        destination: &SemanticPlaceV1,
    ) -> EmissionResultV1<O::Error, B::Error> {
        self.visit(SemanticSsaVisitV1::Place)?;
        let mut indirect = false;
        for projection in destination.projections() {
            self.visit(SemanticSsaVisitV1::Projection)?;
            indirect |= projection.kind() == SemanticProjectionKindV1::Dereference;
        }
        let role = SemanticSsaOperandRoleV1::CallDestinationAddress;
        if indirect {
            self.event(
                role,
                SemanticSsaEventRoleV1::BaseUse,
                SsaEventV1::Use(SsaVariableIdV1::new(destination.local().index())),
            )?;
        }
        for (ordinal, projection) in destination.projections().iter().enumerate() {
            self.visit(SemanticSsaVisitV1::Projection)?;
            if let SemanticProjectionKindV1::Index(local) = projection.kind() {
                self.event(
                    role,
                    SemanticSsaEventRoleV1::ProjectionIndexUse(ordinal),
                    SsaEventV1::Use(SsaVariableIdV1::new(local.index())),
                )?;
            }
        }
        Ok(())
    }

    fn definition(
        &mut self,
        place: &SemanticPlaceV1,
        role: SemanticSsaOperandRoleV1,
    ) -> EmissionResultV1<O::Error, B::Error> {
        self.visit(SemanticSsaVisitV1::Place)?;
        if place.projections().is_empty() {
            self.event(
                role,
                SemanticSsaEventRoleV1::DestinationDefine,
                SsaEventV1::Define(SsaVariableIdV1::new(place.local().index())),
            )
        } else {
            self.place_contents(place, role)
        }
    }
    fn terminator(
        &mut self,
        terminator: &SemanticTerminatorKindV1,
        return_local: Option<usize>,
    ) -> EmissionResultV1<O::Error, B::Error> {
        use SemanticSsaOperandRoleV1 as R;
        match terminator {
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                self.operand(discriminant, R::SwitchDiscriminant)
            }
            SemanticTerminatorKindV1::Call(call) => {
                if let Some(destination) = call.destination()
                    && !destination.place().projections().is_empty()
                {
                    self.call_destination_address(destination.place())?;
                }
                for (ordinal, argument) in call.arguments().iter().enumerate() {
                    self.operand(argument, R::CallArgument(ordinal))?;
                }
                Ok(())
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for (ordinal, argument) in call.arguments().iter().enumerate() {
                    self.operand(argument, R::TailCallArgument(ordinal))?;
                }
                Ok(())
            }
            SemanticTerminatorKindV1::Drop { place, .. } => self.place(place, R::DropPlace),
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                self.operand(condition, R::AssertCondition)?;
                self.assert_message(message)
            }
            SemanticTerminatorKindV1::Return => {
                if let Some(local) = return_local {
                    self.event(
                        R::ReturnValue,
                        SemanticSsaEventRoleV1::BaseUse,
                        SsaEventV1::Use(SsaVariableIdV1::new(local as u32)),
                    )?;
                }
                Ok(())
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => Ok(()),
        }
    }
    fn assert_message(
        &mut self,
        message: &SemanticAssertMessageV1,
    ) -> EmissionResultV1<O::Error, B::Error> {
        use SemanticSsaOperandRoleV1::AssertMessage;
        self.visit(SemanticSsaVisitV1::AssertMessage)?;
        match message {
            SemanticAssertMessageV1::BoundsCheck { length, index } => {
                self.operand(length, AssertMessage(0))?;
                self.operand(index, AssertMessage(1))
            }
            SemanticAssertMessageV1::Overflow { left, right, .. } => {
                self.operand(left, AssertMessage(0))?;
                self.operand(right, AssertMessage(1))
            }
            SemanticAssertMessageV1::DivisionByZero(operand)
            | SemanticAssertMessageV1::RemainderByZero(operand) => {
                self.operand(operand, AssertMessage(0))
            }
            SemanticAssertMessageV1::MisalignedPointerDereference {
                required_alignment,
                found_alignment,
            } => {
                self.operand(required_alignment, AssertMessage(0))?;
                self.operand(found_alignment, AssertMessage(1))
            }
            SemanticAssertMessageV1::NullPointerDereference
            | SemanticAssertMessageV1::ResumedAfterReturn
            | SemanticAssertMessageV1::ResumedAfterPanic => Ok(()),
        }
    }
}
