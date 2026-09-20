//! Body-derived address-observation summaries, not capability or lifetime proofs.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionRoleV1, SemanticMutabilityV1, SemanticPointerKindV1, SemanticRustTypeKindV1,
    SemanticSourceArgumentOwnershipV1,
};

#[derive(Clone, Copy)]
pub(super) struct NominalReferenceParameterV29 {
    pub(super) ordinal: u32,
    pub(super) local: u32,
    pub(super) reference_type: SemanticTypeIdV1,
    pub(super) pointee: SemanticTypeIdV1,
    closed: bool,
}

pub(super) struct NominalReferenceEffectsV29 {
    parameters: Vec<Vec<Option<NominalReferenceParameterV29>>>,
    parameter_count: usize,
    scratch_peak: usize,
}

type Error = ProductionSemanticSsaErrorV1;

impl NominalReferenceEffectsV29 {
    pub(super) fn derive(
        semantic: &AdmittedInertSemanticMirV1,
        limits: ProductionSemanticSsaLimitsV1,
        summary: &mut ProductionSemanticSsaSummaryV1,
    ) -> Result<Self, Error> {
        let mut meter = Meter { limits, summary };
        meter.storage(product(semantic.functions().len(), 8)?)?;
        let mut result = Self {
            parameters: Vec::with_capacity(semantic.functions().len()),
            parameter_count: 0,
            scratch_peak: 0,
        };
        for function in semantic.functions() {
            meter.work(1)?;
            let count = function.abi().source_input_types().len();
            meter.storage(product(count, 16)?)?;
            meter.work(count)?;
            let mut parameters = vec![None; count];
            if function.role() == SemanticFunctionRoleV1::InternalHelper {
                for (local_index, local) in function.locals().iter().enumerate() {
                    meter.work(8)?;
                    let SemanticLocalRoleV1::Argument(ordinal) = local.role() else {
                        continue;
                    };
                    let reference_type = local.ty();
                    if function.abi().source_input_types().get(ordinal as usize)
                        != Some(&reference_type)
                    {
                        continue;
                    }
                    let Some(declaration) = semantic.types().get(reference_type.index() as usize)
                    else {
                        continue;
                    };
                    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
                        continue;
                    };
                    if pointer.kind() != SemanticPointerKindV1::Reference
                        || !semantic
                            .types()
                            .get(pointer.pointee().index() as usize)
                            .is_some_and(|pointee| {
                                matches!(
                                    pointee.rust_type_kind(),
                                    SemanticRustTypeKindV1::Execution(_)
                                )
                            })
                    {
                        continue;
                    }
                    let ownership = match pointer.mutability() {
                        SemanticMutabilityV1::Mutable => {
                            SemanticSourceArgumentOwnershipV1::UniqueBorrow
                        }
                        SemanticMutabilityV1::Immutable => {
                            SemanticSourceArgumentOwnershipV1::SharedBorrow
                        }
                    };
                    if function
                        .abi()
                        .source_argument_ownership()
                        .get(ordinal as usize)
                        != Some(&ownership)
                    {
                        continue;
                    }
                    parameters[ordinal as usize] = Some(NominalReferenceParameterV29 {
                        ordinal,
                        local: local_index as u32,
                        reference_type,
                        pointee: pointer.pointee(),
                        closed: false,
                    });
                    result.parameter_count = sum(result.parameter_count, 1)?;
                }
            }
            result.parameters.push(parameters);
        }
        // Only established leaves can close a caller. Recursive dependencies
        // without an independently closed body remain unapproved.
        for _ in 0..=result.parameter_count {
            let mut changed = false;
            for (index, function) in semantic.functions().iter().enumerate() {
                let parameters = &result.parameters[index];
                meter.work(sum(1, parameters.len())?)?;
                let (roots, unresolved) = parameters
                    .iter()
                    .flatten()
                    .fold((0, false), |(count, unresolved), parameter| {
                        (count + 1, unresolved || !parameter.closed)
                    });
                if !unresolved {
                    continue;
                }
                result.prepay_scan(function, roots, &mut meter)?;
                let (sites, closed) = adapter::analyze_borrow_uses_v29(
                    function,
                    semantic.callables(),
                    &result.parameters[index],
                    Some(&result),
                );
                drop(sites);
                for ordinal in closed {
                    meter.work(1)?;
                    let parameter = result.parameters[index][ordinal as usize]
                        .as_mut()
                        .ok_or(Error::ReplayMismatch)?;
                    changed |= !parameter.closed;
                    parameter.closed = true;
                }
            }
            if !changed {
                return Ok(result);
            }
        }
        Err(Error::ReplayMismatch)
    }

    pub(super) fn accepts(
        &self,
        function: SemanticFunctionIdV1,
        ordinal: usize,
        reference_type: SemanticTypeIdV1,
        pointee: SemanticTypeIdV1,
    ) -> bool {
        self.parameters
            .get(function.index() as usize)
            .and_then(|parameters| parameters.get(ordinal))
            .and_then(Option::as_ref)
            .is_some_and(|parameter| {
                parameter.closed
                    && parameter.reference_type == reference_type
                    && parameter.pointee == pointee
            })
    }

    pub(super) fn borrow_sites(
        &mut self,
        function: &SemanticFunctionDeclV1,
        callables: &[SemanticCallableDeclV1],
        limits: ProductionSemanticSsaLimitsV1,
        summary: &mut ProductionSemanticSsaSummaryV1,
    ) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, Error> {
        if self.parameter_count == 0 {
            return Ok(transparent_borrow_sites_v1(function, callables));
        }
        self.prepay_scan(function, 0, &mut Meter { limits, summary })?;
        Ok(adapter::analyze_borrow_uses_v29(function, callables, &[], Some(self)).0)
    }

    fn prepay_scan(
        &mut self,
        function: &SemanticFunctionDeclV1,
        roots: usize,
        meter: &mut Meter<'_>,
    ) -> Result<(), Error> {
        let (units, candidates) = scan_size(function, roots, meter)?;
        let tree_height = (usize::BITS
            - sum(candidates, function.locals().len())?
                .max(1)
                .leading_zeros()) as usize
            + 1;
        meter.work(sum(
            product(product(units, 32)?, tree_height)?,
            product(product(product(candidates, candidates)?, 8)?, tree_height)?,
        )?)?;
        // Conservative logical words include geometric Vec growth, tree nodes,
        // maximum source-node event scratch, and concurrent chain/result sets.
        let scratch = sum(64, sum(product(units, 16)?, product(candidates, 128)?)?)?;
        if scratch > self.scratch_peak {
            meter.storage(scratch - self.scratch_peak)?;
            self.scratch_peak = scratch;
        }
        Ok(())
    }
}

struct Meter<'a> {
    limits: ProductionSemanticSsaLimitsV1,
    summary: &'a mut ProductionSemanticSsaSummaryV1,
}

impl Meter<'_> {
    fn work(&mut self, units: usize) -> Result<(), Error> {
        self.summary.work_units = sum(self.summary.work_units, units)?;
        accounting::enforce_module_resource_limits_v1(*self.summary, self.limits)
    }

    fn storage(&mut self, words: usize) -> Result<(), Error> {
        self.summary.storage_words = sum(self.summary.storage_words, words)?;
        accounting::enforce_module_resource_limits_v1(*self.summary, self.limits)
    }
}

fn sum(left: usize, right: usize) -> Result<usize, Error> {
    left.checked_add(right).ok_or(Error::ResourceOverflow)
}

fn product(left: usize, right: usize) -> Result<usize, Error> {
    left.checked_mul(right).ok_or(Error::ResourceOverflow)
}

// Allocation-free sizing only. Source occurrences still come exclusively from
// the existing adapter and its shared plain/capture emission driver.
struct SyntaxSize<'a, 'b> {
    units: usize,
    candidates: usize,
    meter: &'a mut Meter<'b>,
}

impl SyntaxSize<'_, '_> {
    fn add(&mut self, units: usize) -> Result<(), Error> {
        self.meter.work(units)?;
        self.units = sum(self.units, units)?;
        Ok(())
    }

    fn place(&mut self, place: &SemanticPlaceV1) -> Result<(), Error> {
        self.add(sum(1, place.projections().len())?)
    }

    fn operand(&mut self, operand: &SemanticOperandV1) -> Result<(), Error> {
        self.add(1)?;
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => self.place(place),
            SemanticOperandV1::Constant(_) => Ok(()),
        }
    }

    fn rvalue(&mut self, value: &SemanticRvalueKindV1) -> Result<(), Error> {
        self.add(1)?;
        value.try_visit_operands(|operand| self.operand(operand))?;
        match value {
            SemanticRvalueKindV1::Borrow { place, .. } => {
                self.candidates = sum(self.candidates, 1)?;
                self.place(place)
            }
            SemanticRvalueKindV1::AddressOf { place, .. }
            | SemanticRvalueKindV1::Length(place)
            | SemanticRvalueKindV1::Discriminant(place) => self.place(place),
            SemanticRvalueKindV1::Load(load) => self.place(load.source()),
            SemanticRvalueKindV1::Use(_)
            | SemanticRvalueKindV1::Unary { .. }
            | SemanticRvalueKindV1::Binary { .. }
            | SemanticRvalueKindV1::CheckedBinary(_)
            | SemanticRvalueKindV1::UncheckedBinary(_)
            | SemanticRvalueKindV1::Cast { .. }
            | SemanticRvalueKindV1::Aggregate(_) => Ok(()),
        }
    }

    fn statement(&mut self, statement: &SemanticStatementKindV1) -> Result<(), Error> {
        self.add(1)?;
        match statement {
            SemanticStatementKindV1::Assign(assignment) => {
                self.place(assignment.destination())?;
                self.rvalue(assignment.value().kind())
            }
            SemanticStatementKindV1::Store(store) => {
                self.place(store.destination())?;
                self.operand(store.value())
            }
            SemanticStatementKindV1::AtomicRmw(operation) => {
                self.place(operation.destination())?;
                self.place(operation.address())?;
                self.operand(operation.value())
            }
            SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                self.place(operation.destination())?;
                self.place(operation.address())?;
                self.operand(operation.expected())?;
                self.operand(operation.replacement())
            }
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => self.place(place),
            SemanticStatementKindV1::Assume(operand) => self.operand(operand),
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Nop => Ok(()),
        }
    }

    fn terminator(&mut self, terminator: &SemanticTerminatorKindV1) -> Result<(), Error> {
        self.add(sum(1, terminator.edge_count())?)?;
        match terminator {
            SemanticTerminatorKindV1::Call(call) => {
                for argument in call.arguments() {
                    self.operand(argument)?;
                }
                if let Some(destination) = call.destination() {
                    self.place(destination.place())?;
                }
                Ok(())
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for argument in call.arguments() {
                    self.operand(argument)?;
                }
                Ok(())
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => self.operand(discriminant),
            SemanticTerminatorKindV1::Drop { place, .. } => self.place(place),
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                self.operand(condition)?;
                partial_moves::visit_assert_operands_v1(message, &mut |operand| {
                    self.operand(operand)
                })
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

fn scan_size(
    function: &SemanticFunctionDeclV1,
    roots: usize,
    meter: &mut Meter<'_>,
) -> Result<(usize, usize), Error> {
    let mut size = SyntaxSize {
        units: 0,
        candidates: roots,
        meter,
    };
    size.add(sum(
        roots,
        sum(
            function.locals().len(),
            function.abi().source_input_types().len(),
        )?,
    )?)?;
    for block in function.blocks() {
        size.add(1)?;
        for statement in block.statements() {
            size.statement(statement.kind())?;
        }
        size.terminator(block.terminator().kind())?;
    }
    Ok((size.units, size.candidates))
}
