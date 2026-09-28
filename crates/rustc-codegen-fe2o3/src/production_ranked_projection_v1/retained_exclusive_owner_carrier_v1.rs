//! Retained ExclusiveOwner census DATA. No genuine source or readiness authority.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::RetainedExactOriginWorklistV1;
#[cfg(test)]
use crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::retained_origin_worklist_frame_v1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
type Error = ProductionRankedProjectionErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Resources<'b, 'w> = PreparationResourcesV1<'b, 'w>;
type Ledger = (usize, CanonicalKernelIrWorkLedgerIdentityV1);
type SliceIdentity = (usize, usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}

/// A future authenticated factory must keep this pending owner and its source
/// loan alive across the true checked postflight and drop before credit refund.
pub(in crate::production_ranked_projection_v1) struct RetainedExclusiveCarrierV1 {
    phase: Phase,
    ledger: Option<Ledger>,
    function: Option<usize>,
    callables: Option<SliceIdentity>,
    definitions: Option<SliceIdentity>,
    origins: Vec<Option<u32>>,
    scan: CarrierScan,
    scan_invoked: bool,
    copies: Vec<Vec<usize>>,
    borrows: Vec<(usize, usize)>,
    edge_count: usize,
    initial_work: usize,
    propagation: RetainedExactOriginWorklistV1,
}
impl RetainedExclusiveCarrierV1 {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            ledger: None,
            function: None,
            callables: None,
            definitions: None,
            origins: Vec::new(),
            scan: CarrierScan {
                bad_carrier: Vec::new(),
                bad_receiver: Vec::new(),
                receiver_uses: Vec::new(),
                work: 0,
                limit: 0,
            },
            scan_invoked: false,
            copies: Vec::new(),
            borrows: Vec::new(),
            edge_count: 0,
            initial_work: 0,
            propagation: RetainedExactOriginWorklistV1::new(),
        }
    }
    pub(in crate::production_ranked_projection_v1) fn prepare_into(
        &mut self,
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
        definitions: &[u8],
        limit: usize,
        resources: &mut Resources<'_, '_>,
    ) -> Result<()> {
        let fresh = self.phase == Phase::Fresh && self.ledger.is_none() && self.function.is_none();
        self.phase = Phase::Terminal;
        if !fresh || !resources.is_metered() || resources.has_denial() {
            return Err(resource(Resource::Accounting));
        }
        let ledger = resources
            .original_ledger_v1()
            .ok_or_else(|| resource(Resource::Accounting))?;
        resources.work(32)?;
        resources.reserve_storage(retained_carrier_frame_v1()?)?;
        self.ledger = Some(ledger);
        self.function = Some(function as *const SemanticFunctionDeclV1 as usize);
        self.callables = Some((callables.as_ptr() as usize, callables.len()));
        self.definitions = Some((definitions.as_ptr() as usize, definitions.len()));
        self.prepare_attached(callables, function, definitions, limit, resources)?;
        if resources.has_denial()
            || resources.original_ledger_v1() != Some(ledger)
            || (self.scan_invoked && !self.propagation.completed(resources))
        {
            return Err(resource(Resource::Accounting));
        }
        self.phase = Phase::Complete;
        Ok(())
    }
    /// Lexical identity and same-ledger completion only, not source authority.
    pub(in crate::production_ranked_projection_v1) fn completed_for<'a>(
        &'a self,
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
        definitions: &[u8],
        resources: &Resources<'_, '_>,
    ) -> Result<(&'a [Option<u32>], usize)> {
        if self.phase != Phase::Complete
            || self.ledger.is_none()
            || self.ledger != resources.original_ledger_v1()
            || resources.has_denial()
            || self.function != Some(function as *const SemanticFunctionDeclV1 as usize)
            || self.callables != Some((callables.as_ptr() as usize, callables.len()))
            || self.definitions != Some((definitions.as_ptr() as usize, definitions.len()))
        {
            return Err(resource(Resource::Accounting));
        }
        Ok((
            &self.origins,
            if self.scan_invoked {
                self.scan.work
            } else {
                self.initial_work
            },
        ))
    }

    fn prepare_attached(
        &mut self,
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
        definitions: &[u8],
        limit: usize,
        resources: &mut Resources<'_, '_>,
    ) -> Result<()> {
        let count = function.locals().len();
        if definitions.len() != count {
            return Err(failure(
                "ExclusiveOwner carrier definitions do not match the local table",
            ));
        }
        // Fixed lexical headers are prepaid separately from vector capacities.
        resources.reserve_storage(carrier_frame_storage_v1()?)?;
        charge(&mut self.initial_work, limit, count, resources)?;
        charge(
            &mut self.initial_work,
            limit,
            function.abi().source_argument_ownership().len(),
            resources,
        )?;
        fill_attached(&mut self.origins, count, None, resources)?;
        if !function
            .abi()
            .source_argument_ownership()
            .contains(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
        {
            return Ok(());
        }
        self.scan_invoked = true;
        self.scan
            .prepare_retained_into(count, limit, self.initial_work, resources)?;
        self.scan.charge(count, resources)?;
        nested_attached(&mut self.copies, count, resources)?;
        for (index, local) in function.locals().iter().enumerate() {
            self.scan.charge(1, resources)?;
            if local.role() == SemanticLocalRoleV1::Return {
                self.scan.bad_carrier[index] = true;
                self.scan.bad_receiver[index] = true;
            }
        }
        for block in function.blocks() {
            self.scan.charge(1, resources)?;
            for statement in block.statements() {
                self.scan.charge(1, resources)?;
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        let destination = assignment.destination();
                        let destination_index = destination.local().index() as usize;
                        let unique = destination.projections().is_empty()
                            && definitions.get(destination_index) == Some(&1);
                        if !destination.projections().is_empty() {
                            self.scan.place(destination, false, false, resources)?;
                        }
                        match assignment.value().kind() {
                            SemanticRvalueKindV1::Use(operand) if unique => {
                                // simple_operand_local first scans the complete transparent
                                // projection spine. Pay that separate walk BEFORE entering
                                // the helper; scan.operand below retains its unchanged local
                                // census charge for its own later projection walk.
                                resources.work(
                                    raw_operand_place(operand)
                                        .map_or(0, |place| place.projections().len()),
                                )?;
                                let source =
                                    simple_operand_local(operand).map(|id| id.index() as usize);
                                let exact = source.filter(|&source| {
                                    let Some(source_local) = function.locals().get(source) else {
                                        return false;
                                    };
                                    let Some(destination_local) =
                                        function.locals().get(destination_index)
                                    else {
                                        return false;
                                    };
                                    source_local.ty() == destination_local.ty()
                                        && assignment.value().result_type()
                                            == destination_local.ty()
                                        && !matches!(
                                            destination_local.role(),
                                            SemanticLocalRoleV1::Argument(_)
                                        )
                                        && match source_local.role() {
                                            SemanticLocalRoleV1::Argument(_) => {
                                                definitions[source] == 0
                                            }
                                            _ => definitions[source] == 1,
                                        }
                                });
                                self.scan
                                    .operand(operand, exact.is_some(), false, resources)?;
                                if let Some(source) = exact {
                                    self.scan.charge(1, resources)?;
                                    push_local_provenance_edge_with_resources_v1(
                                        &mut self.copies,
                                        source,
                                        destination_index,
                                        &mut self.edge_count,
                                        resources,
                                    )?;
                                }
                            }
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Mutable | SemanticBorrowKindV1::Shared,
                                place,
                            } if unique && place.projections().is_empty() => {
                                // Defer carrier permission until the complete receiver
                                // use census has excluded aliasing, writes and escapes.
                                self.scan.place(place, true, false, resources)?;
                                self.scan.charge(1, resources)?;
                                resources.push(
                                    &mut self.borrows,
                                    (place.local().index() as usize, destination_index),
                                )?;
                            }
                            value => {
                                value.try_visit_operands(|operand| {
                                    self.scan.operand(operand, false, false, resources)
                                })?;
                                match value {
                                    SemanticRvalueKindV1::Borrow { place, .. }
                                    | SemanticRvalueKindV1::AddressOf { place, .. }
                                    | SemanticRvalueKindV1::Length(place)
                                    | SemanticRvalueKindV1::Discriminant(place) => {
                                        self.scan.place(place, false, false, resources)?
                                    }
                                    SemanticRvalueKindV1::Load(load) => {
                                        self.scan.place(load.source(), false, false, resources)?
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    SemanticStatementKindV1::Store(store) => {
                        self.scan
                            .place(store.destination(), false, false, resources)?;
                        self.scan.operand(store.value(), false, false, resources)?;
                    }
                    SemanticStatementKindV1::AtomicRmw(atomic) => {
                        self.scan
                            .place(atomic.destination(), false, false, resources)?;
                        self.scan.place(atomic.address(), false, false, resources)?;
                        self.scan.operand(atomic.value(), false, false, resources)?;
                    }
                    SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                        self.scan
                            .place(atomic.destination(), false, false, resources)?;
                        self.scan.place(atomic.address(), false, false, resources)?;
                        self.scan
                            .operand(atomic.expected(), false, false, resources)?;
                        self.scan
                            .operand(atomic.replacement(), false, false, resources)?;
                    }
                    SemanticStatementKindV1::SetDiscriminant { place, .. }
                    | SemanticStatementKindV1::Deinitialize(place) => {
                        self.scan.place(place, false, false, resources)?
                    }
                    SemanticStatementKindV1::Assume(operand) => {
                        self.scan.operand(operand, false, false, resources)?
                    }
                    SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Nop => {}
                }
            }
            self.scan.charge(1, resources)?;
            match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => {
                    let receiver_preserved = callables
                        .get(call.callee().index() as usize)
                        .is_some_and(descriptor_preserving);
                    for (ordinal, operand) in call.arguments().iter().enumerate() {
                        self.scan.operand(
                            operand,
                            false,
                            ordinal == 0 && receiver_preserved,
                            resources,
                        )?;
                    }
                    if let Some(destination) = call.destination()
                        && !destination.place().projections().is_empty()
                    {
                        self.scan
                            .place(destination.place(), false, false, resources)?;
                    }
                }
                SemanticTerminatorKindV1::TailCall(call) => {
                    for operand in call.arguments() {
                        self.scan.operand(operand, false, false, resources)?;
                    }
                }
                SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                    self.scan.operand(discriminant, false, false, resources)?
                }
                SemanticTerminatorKindV1::Drop { place, .. } => {
                    self.scan.place(place, false, false, resources)?
                }
                SemanticTerminatorKindV1::Assert {
                    condition, message, ..
                } => {
                    self.scan.operand(condition, false, false, resources)?;
                    use fe2o3_mir_model::semantic_mir_v1::SemanticAssertMessageV1 as Message;
                    match message {
                        Message::BoundsCheck {
                            length: left,
                            index: right,
                        }
                        | Message::Overflow { left, right, .. }
                        | Message::MisalignedPointerDereference {
                            required_alignment: left,
                            found_alignment: right,
                        } => {
                            self.scan.operand(left, false, false, resources)?;
                            self.scan.operand(right, false, false, resources)?;
                        }
                        Message::DivisionByZero(operand) | Message::RemainderByZero(operand) => {
                            self.scan.operand(operand, false, false, resources)?
                        }
                        Message::NullPointerDereference
                        | Message::ResumedAfterReturn
                        | Message::ResumedAfterPanic => {}
                    }
                }
                SemanticTerminatorKindV1::Goto(_)
                | SemanticTerminatorKindV1::FalseEdge { .. }
                | SemanticTerminatorKindV1::Return
                | SemanticTerminatorKindV1::UnwindResume
                | SemanticTerminatorKindV1::UnwindTerminate
                | SemanticTerminatorKindV1::Abort
                | SemanticTerminatorKindV1::Unreachable => {}
            }
        }
        for (carrier, receiver) in self.borrows.iter().copied() {
            self.scan.charge(1, resources)?;
            if self.scan.bad_receiver[receiver] || self.scan.receiver_uses[receiver] == 0 {
                self.scan.bad_carrier[carrier] = true;
            }
        }
        for (index, local) in function.locals().iter().enumerate() {
            self.scan.charge(1, resources)?;
            if let SemanticLocalRoleV1::Argument(argument) = local.role()
                && definitions[index] == 0
                && !self.scan.bad_carrier[index]
                && function
                    .abi()
                    .source_argument_ownership()
                    .get(argument as usize)
                    == Some(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
                && function.abi().source_input_types().get(argument as usize) == Some(&local.ty())
            {
                self.origins[index] = Some(argument);
            }
        }
        // Filtering before propagation makes an unsafe intermediate invalidate every
        // descendant, even when its mutation appears after the copy in source order.
        for (source, edges) in self.copies.iter_mut().enumerate() {
            self.scan.charge(1, resources)?;
            if self.scan.bad_carrier[source] {
                edges.clear();
            } else {
                self.scan.charge(edges.len(), resources)?;
                edges.retain(|&destination| !self.scan.bad_carrier[destination]);
            }
        }
        // The propagation helper scans all locals, initializes a bounded queue,
        // pops each reached local at most once, and visits each edge at most once.
        self.scan.charge(
            count
                .checked_mul(3)
                .and_then(|value| value.checked_add(self.edge_count))
                .ok_or(failure("ExclusiveOwner carrier work accounting overflowed"))?,
            resources,
        )?;
        self.propagation.prepare_into(
            &mut self.origins,
            &self.copies,
            "an ExclusiveOwner carrier has conflicting argument origins",
            resources,
        )?;
        Ok(())
    }
}
fn fill_attached<T: Clone>(
    values: &mut Vec<T>,
    count: usize,
    value: T,
    resources: &mut Resources<'_, '_>,
) -> Result<()> {
    resources.work(count)?;
    resources.reserve(values, count)?;
    values.resize(count, value);
    Ok(())
}
fn nested_attached(
    values: &mut Vec<Vec<usize>>,
    count: usize,
    resources: &mut Resources<'_, '_>,
) -> Result<()> {
    resources.work(count)?;
    resources.reserve(values, count)?;
    values.resize_with(count, Vec::new);
    Ok(())
}
const FRAME_ROWS: usize = 22;
fn typed_rows() -> Result<[usize; FRAME_ROWS]> {
    Ok([
        size_of::<RetainedExclusiveCarrierV1>(),
        size_of::<(
            CarrierScan,
            RetainedExactOriginWorklistV1,
            Vec<Option<u32>>,
            Vec<Vec<usize>>,
            Vec<(usize, usize)>,
            Phase,
            Option<Ledger>,
            Option<usize>,
            Option<SliceIdentity>,
            Option<SliceIdentity>,
            bool,
            usize,
            usize,
        )>(),
        size_of::<(
            &mut RetainedExclusiveCarrierV1,
            &[SemanticCallableDeclV1],
            &SemanticFunctionDeclV1,
            &[u8],
            usize,
            &mut Resources<'static, 'static>,
            bool,
            Ledger,
            Option<Ledger>,
            Result<()>,
        )>(),
        size_of::<(
            &RetainedExclusiveCarrierV1,
            &[SemanticCallableDeclV1],
            &SemanticFunctionDeclV1,
            &[u8],
            &Resources<'static, 'static>,
            Option<Ledger>,
            Option<usize>,
            Option<SliceIdentity>,
            Option<SliceIdentity>,
            Result<(&[Option<u32>], usize)>,
        )>(),
        size_of::<(
            &mut RetainedExclusiveCarrierV1,
            &[SemanticCallableDeclV1],
            &SemanticFunctionDeclV1,
            &[u8],
            usize,
            &mut Resources<'static, 'static>,
            usize,
            bool,
            Result<()>,
        )>(),
        size_of::<(
            &mut CarrierScan,
            usize,
            usize,
            usize,
            &mut Resources<'static, 'static>,
            Option<usize>,
            Result<()>,
        )>(),
        fill_frame::<Option<u32>>(),
        fill_frame::<bool>(),
        fill_frame::<bool>(),
        fill_frame::<u8>(),
        size_of::<(
            &mut Vec<Vec<usize>>,
            usize,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
        size_of::<(
            std::iter::Copied<std::slice::Iter<'static, (usize, usize)>>,
            usize,
            usize,
            &(usize, usize),
        )>(),
        size_of::<(
            &mut Vec<Vec<usize>>,
            &mut [Vec<usize>],
            &mut usize,
            usize,
            usize,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
        size_of::<(
            &mut Vec<(usize, usize)>,
            (usize, usize),
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
        size_of::<(
            &mut CarrierScan,
            &mut Resources<'static, 'static>,
            &SemanticOperandV1,
            Result<()>,
        )>(),
        size_of::<(
            &mut CarrierScan,
            &mut Resources<'static, 'static>,
            &SemanticPlaceV1,
            Result<()>,
        )>(),
        size_of::<(
            &mut RetainedExactOriginWorklistV1,
            &mut [Option<u32>],
            &[Vec<usize>],
            &'static str,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
        size_of::<(
            &RetainedExactOriginWorklistV1,
            &Resources<'static, 'static>,
            bool,
        )>(),
        size_of::<(Error, Resource, Result<()>, Option<Ledger>, bool)>(),
        size_of::<(
            [usize; FRAME_ROWS],
            Result<[usize; FRAME_ROWS]>,
            std::array::IntoIter<usize, FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
        )>(),
        size_of::<(Result<usize>, usize, Option<usize>, Resource, Error)>(),
        size_of::<(
            &mut usize,
            usize,
            usize,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
    ])
}
fn fill_frame<T>() -> usize {
    size_of::<(
        &mut Vec<T>,
        usize,
        T,
        &mut Resources<'static, 'static>,
        Result<()>,
        Result<()>,
    )>()
}
pub(in crate::production_ranked_projection_v1) fn retained_carrier_frame_v1() -> Result<usize> {
    typed_rows()?.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}
#[cfg(test)]
#[path = "retained_exclusive_owner_carrier_v1_tests.rs"]
mod tests;

impl CarrierScan {
    fn prepare_retained_into(
        &mut self,
        count: usize,
        limit: usize,
        work: usize,
        resources: &mut Resources<'_, '_>,
    ) -> Result<()> {
        self.work = work;
        self.limit = limit;
        charge(
            &mut self.work,
            limit,
            count
                .checked_mul(3)
                .ok_or(failure("ExclusiveOwner carrier work accounting overflowed"))?,
            resources,
        )?;
        fill_attached(&mut self.bad_carrier, count, false, resources)?;
        fill_attached(&mut self.bad_receiver, count, false, resources)?;
        fill_attached(&mut self.receiver_uses, count, 0, resources)?;
        Ok(())
    }
}
