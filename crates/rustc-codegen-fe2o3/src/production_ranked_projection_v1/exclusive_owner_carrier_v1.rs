//! Closed whole-value origin transport for borrowed ExclusiveOwner carriers.
//!
//! This is allocation-origin custody, not a general alias or initialization proof.
//! The source/N and capability checks remain mandatory. In particular, the only
//! permitted carrier address users are direct borrowed receivers consumed by
//! descriptor-preserving, authenticated compiler intrinsics.

use super::bf16_nominal_preparation_resources_v1::{PreparationResourcesV1, resource};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::mem::size_of;

pub(super) fn exclusive_owner_value_origins_v1(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    definitions: &[u8],
) -> Result<Vec<Option<u32>>, ProductionRankedProjectionErrorV1> {
    origins_with_limit(
        callables,
        function,
        definitions,
        MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1,
    )
    .map(|(origins, _)| origins)
}

/// Same carrier predicates; accepted reservations belong to the enclosing
/// genuine source-preparation factory and remain owned until its scope ends.
pub(super) fn exclusive_owner_value_origins_with_resources_v1(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    definitions: &[u8],
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<Vec<Option<u32>>, ProductionRankedProjectionErrorV1> {
    origins_with_limit_and_resources(
        callables,
        function,
        definitions,
        MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1,
        resources,
    )
    .map(|(origins, _)| origins)
}

pub(super) fn origins_with_limit(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    definitions: &[u8],
    limit: usize,
) -> Result<(Vec<Option<u32>>, usize), ProductionRankedProjectionErrorV1> {
    origins_with_limit_and_resources(
        callables,
        function,
        definitions,
        limit,
        &mut PreparationResourcesV1::unmetered(),
    )
}

/// Private limit seam shared by legacy/resource controls. The local census
/// counter and its refusal remain unchanged; original work is additional.
pub(super) fn origins_with_limit_and_resources(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    definitions: &[u8],
    limit: usize,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(Vec<Option<u32>>, usize), ProductionRankedProjectionErrorV1> {
    let count = function.locals().len();
    if definitions.len() != count {
        return Err(failure(
            "ExclusiveOwner carrier definitions do not match the local table",
        ));
    }
    // Fixed lexical headers are prepaid separately from vector capacities.
    resources.reserve_storage(carrier_frame_storage_v1()?)?;
    let mut work = 0;
    charge(&mut work, limit, count, resources)?;
    charge(
        &mut work,
        limit,
        function.abi().source_argument_ownership().len(),
        resources,
    )?;
    let mut origins = resources.filled(count, None)?;
    if !function
        .abi()
        .source_argument_ownership()
        .contains(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
    {
        return Ok((origins, work));
    }
    let mut scan = CarrierScan::new(count, limit, work, resources)?;
    scan.charge(count, resources)?;
    let mut copies = resources.nested::<usize>(count)?;
    let mut borrows = Vec::new();
    let mut edge_count = 0;
    for (index, local) in function.locals().iter().enumerate() {
        scan.charge(1, resources)?;
        if local.role() == SemanticLocalRoleV1::Return {
            scan.bad_carrier[index] = true;
            scan.bad_receiver[index] = true;
        }
    }
    for block in function.blocks() {
        scan.charge(1, resources)?;
        for statement in block.statements() {
            scan.charge(1, resources)?;
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    let destination = assignment.destination();
                    let destination_index = destination.local().index() as usize;
                    let unique = destination.projections().is_empty()
                        && definitions.get(destination_index) == Some(&1);
                    if !destination.projections().is_empty() {
                        scan.place(destination, false, false, resources)?;
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
                                    && assignment.value().result_type() == destination_local.ty()
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
                            scan.operand(operand, exact.is_some(), false, resources)?;
                            if let Some(source) = exact {
                                scan.charge(1, resources)?;
                                push_local_provenance_edge_with_resources_v1(
                                    &mut copies,
                                    source,
                                    destination_index,
                                    &mut edge_count,
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
                            scan.place(place, true, false, resources)?;
                            scan.charge(1, resources)?;
                            resources.push(
                                &mut borrows,
                                (place.local().index() as usize, destination_index),
                            )?;
                        }
                        value => {
                            value.try_visit_operands(|operand| {
                                scan.operand(operand, false, false, resources)
                            })?;
                            match value {
                                SemanticRvalueKindV1::Borrow { place, .. }
                                | SemanticRvalueKindV1::AddressOf { place, .. }
                                | SemanticRvalueKindV1::Length(place)
                                | SemanticRvalueKindV1::Discriminant(place) => {
                                    scan.place(place, false, false, resources)?
                                }
                                SemanticRvalueKindV1::Load(load) => {
                                    scan.place(load.source(), false, false, resources)?
                                }
                                _ => {}
                            }
                        }
                    }
                }
                SemanticStatementKindV1::Store(store) => {
                    scan.place(store.destination(), false, false, resources)?;
                    scan.operand(store.value(), false, false, resources)?;
                }
                SemanticStatementKindV1::AtomicRmw(atomic) => {
                    scan.place(atomic.destination(), false, false, resources)?;
                    scan.place(atomic.address(), false, false, resources)?;
                    scan.operand(atomic.value(), false, false, resources)?;
                }
                SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                    scan.place(atomic.destination(), false, false, resources)?;
                    scan.place(atomic.address(), false, false, resources)?;
                    scan.operand(atomic.expected(), false, false, resources)?;
                    scan.operand(atomic.replacement(), false, false, resources)?;
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => {
                    scan.place(place, false, false, resources)?
                }
                SemanticStatementKindV1::Assume(operand) => {
                    scan.operand(operand, false, false, resources)?
                }
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        scan.charge(1, resources)?;
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                let receiver_preserved = callables
                    .get(call.callee().index() as usize)
                    .is_some_and(descriptor_preserving);
                for (ordinal, operand) in call.arguments().iter().enumerate() {
                    scan.operand(
                        operand,
                        false,
                        ordinal == 0 && receiver_preserved,
                        resources,
                    )?;
                }
                if let Some(destination) = call.destination()
                    && !destination.place().projections().is_empty()
                {
                    scan.place(destination.place(), false, false, resources)?;
                }
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for operand in call.arguments() {
                    scan.operand(operand, false, false, resources)?;
                }
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                scan.operand(discriminant, false, false, resources)?
            }
            SemanticTerminatorKindV1::Drop { place, .. } => {
                scan.place(place, false, false, resources)?
            }
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                scan.operand(condition, false, false, resources)?;
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
                        scan.operand(left, false, false, resources)?;
                        scan.operand(right, false, false, resources)?;
                    }
                    Message::DivisionByZero(operand) | Message::RemainderByZero(operand) => {
                        scan.operand(operand, false, false, resources)?
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
    for (carrier, receiver) in borrows {
        scan.charge(1, resources)?;
        if scan.bad_receiver[receiver] || scan.receiver_uses[receiver] == 0 {
            scan.bad_carrier[carrier] = true;
        }
    }
    for (index, local) in function.locals().iter().enumerate() {
        scan.charge(1, resources)?;
        if let SemanticLocalRoleV1::Argument(argument) = local.role()
            && definitions[index] == 0
            && !scan.bad_carrier[index]
            && function
                .abi()
                .source_argument_ownership()
                .get(argument as usize)
                == Some(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
            && function.abi().source_input_types().get(argument as usize) == Some(&local.ty())
        {
            origins[index] = Some(argument);
        }
    }
    // Filtering before propagation makes an unsafe intermediate invalidate every
    // descendant, even when its mutation appears after the copy in source order.
    for (source, edges) in copies.iter_mut().enumerate() {
        scan.charge(1, resources)?;
        if scan.bad_carrier[source] {
            edges.clear();
        } else {
            scan.charge(edges.len(), resources)?;
            edges.retain(|&destination| !scan.bad_carrier[destination]);
        }
    }
    // The propagation helper scans all locals, initializes a bounded queue,
    // pops each reached local at most once, and visits each edge at most once.
    scan.charge(
        count
            .checked_mul(3)
            .and_then(|value| value.checked_add(edge_count))
            .ok_or(failure("ExclusiveOwner carrier work accounting overflowed"))?,
        resources,
    )?;
    propagate_exact_local_origins_with_resources_v1(
        &mut origins,
        &copies,
        "an ExclusiveOwner carrier has conflicting argument origins",
        resources,
    )?;
    Ok((origins, scan.work))
}

fn descriptor_preserving(callable: &SemanticCallableDeclV1) -> bool {
    matches!(
        callable,
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceLen { .. }
                | SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut { .. }
                | SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite { .. },
            ..
        }
    )
}

fn failure(message: &'static str) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Unsupported(message)
}

/// Logical stack/header reservation; no allocator/RSS claim. Nested inner Vec
/// headers are paid by resources.nested; all payloads by the resource adapter.
fn carrier_frame_storage_v1() -> Result<usize, ProductionRankedProjectionErrorV1> {
    [
        size_of::<CarrierScan>(),
        size_of::<Vec<Option<u32>>>(),
        size_of::<Vec<Vec<usize>>>(),
        size_of::<Vec<(usize, usize)>>(),
        size_of::<std::vec::IntoIter<(usize, usize)>>(),
        size_of::<Result<(Vec<Option<u32>>, usize), ProductionRankedProjectionErrorV1>>(),
        // Bounded nonrecursive iterator/borrow/counter frame.
        512,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}

struct CarrierScan {
    bad_carrier: Vec<bool>,
    bad_receiver: Vec<bool>,
    receiver_uses: Vec<u8>,
    work: usize,
    limit: usize,
}

impl CarrierScan {
    fn new(
        count: usize,
        limit: usize,
        mut work: usize,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        charge(
            &mut work,
            limit,
            count
                .checked_mul(3)
                .ok_or(failure("ExclusiveOwner carrier work accounting overflowed"))?,
            resources,
        )?;
        Ok(Self {
            bad_carrier: resources.filled(count, false)?,
            bad_receiver: resources.filled(count, false)?,
            receiver_uses: resources.filled(count, 0)?,
            work,
            limit,
        })
    }

    fn charge(
        &mut self,
        amount: usize,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge(&mut self.work, self.limit, amount, resources)
    }

    fn local(
        &mut self,
        local: SemanticLocalIdV1,
        carrier: bool,
        receiver: bool,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.charge(1, resources)?;
        let index = local.index() as usize;
        let Some(bad) = self.bad_carrier.get_mut(index) else {
            return Err(failure(
                "ExclusiveOwner carrier use is outside the local table",
            ));
        };
        *bad |= !carrier;
        self.bad_receiver[index] |= !receiver;
        if receiver {
            self.receiver_uses[index] = self.receiver_uses[index].saturating_add(1);
        }
        Ok(())
    }

    fn place(
        &mut self,
        place: &SemanticPlaceV1,
        carrier: bool,
        receiver: bool,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let whole = place.projections().is_empty();
        self.local(
            place.local(),
            carrier && whole,
            receiver && whole,
            resources,
        )?;
        for projection in place.projections() {
            self.charge(1, resources)?;
            if let SemanticProjectionKindV1::Index(local) = projection.kind() {
                self.local(local, false, false, resources)?;
            }
        }
        Ok(())
    }

    fn operand(
        &mut self,
        operand: &SemanticOperandV1,
        carrier: bool,
        receiver: bool,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.charge(1, resources)?;
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.place(place, carrier, receiver, resources)
            }
            SemanticOperandV1::Constant(_) => Ok(()),
        }
    }
}

fn charge(
    work: &mut usize,
    limit: usize,
    amount: usize,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    *work = work
        .checked_add(amount)
        .ok_or(failure("ExclusiveOwner carrier work accounting overflowed"))?;
    if *work > limit {
        return Err(failure(
            "ExclusiveOwner carrier census exceeds the projection work limit",
        ));
    }
    resources.work(amount)
}
