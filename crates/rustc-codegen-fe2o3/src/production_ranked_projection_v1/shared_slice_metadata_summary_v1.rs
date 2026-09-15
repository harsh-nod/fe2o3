// This grammar transports an immutable descriptor, never its pointee effects.
// It does not certify results for the separate deterministic scalar evaluator.
fn immutable_metadata_slice_type_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return false;
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata()
            != fe2o3_mir_model::semantic_mir_v1::SemanticPointerMetadataV1::SliceLength
    {
        return false;
    }
    let Some(SemanticTypeShapeV1::Slice { element }) = types
        .get(pointer.pointee().index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return false;
    };
    matches!(
        types
            .get(element.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
    )
}

fn immutable_metadata_slice_abi_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    work: &mut usize,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    let abi = function.abi();
    if function.role() != fe2o3_mir_model::semantic_mir_v1::SemanticFunctionRoleV1::InternalHelper
        || function.export().is_some()
        || abi.canon_abi() != fe2o3_mir_model::semantic_mir_v1::SemanticCanonAbiV1::Rust
        || abi.extern_abi() != fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::Rust
        || abi.can_unwind()
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || abi.arguments().len() != abi.source_input_types().len()
        || abi.arguments().len() != abi.source_argument_ownership().len()
        || !scalar_direct_abi_value_v1(types, abi.return_value())
    {
        return Ok(false);
    }
    // Each descriptor follows two type-table edges; all scans are prepaid.
    let scan_work = abi
        .arguments()
        .len()
        .checked_mul(4)
        .and_then(|arguments| {
            function
                .locals()
                .len()
                .checked_mul(3)
                .and_then(|locals| arguments.checked_add(locals))
        })
        .and_then(|total| total.checked_add(1))
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "shared-slice metadata-summary work count overflowed",
        ))?;
    charge_defined_callable_summary_work_v1(work, scan_work)?;
    let mut has_slice = false;
    for ((argument, ty), ownership) in abi
        .arguments()
        .iter()
        .zip(abi.source_input_types())
        .zip(abi.source_argument_ownership())
    {
        if argument.role() != SemanticAbiArgumentRoleV1::Source
            || argument.value().source_ty() != *ty
        {
            return Ok(false);
        }
        if immutable_metadata_slice_type_v1(types, *ty) {
            has_slice = true;
            if *ownership != SemanticSourceArgumentOwnershipV1::SharedBorrow
                || argument.value().adjusted().is_some()
                || argument.value().pointee_override().is_some()
                || !matches!(argument.mode(), SemanticAbiPassModeV1::Pair { .. })
            {
                return Ok(false);
            }
        } else if !scalar_direct_abi_value_v1(types, argument.value()) {
            return Ok(false);
        }
    }
    Ok(has_slice
        && function.locals().iter().all(|local| {
            scalar_or_checked_carrier_type_v1(types, local.ty())
                || immutable_metadata_slice_type_v1(types, local.ty())
        }))
}

struct ImmutableSliceMetadataSummaryV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    function_count: usize,
    callables: &'a [SemanticCallableDeclV1],
}

impl ImmutableSliceMetadataSummaryV1<'_> {
    fn descriptor_place(
        &self,
        place: &SemanticPlaceV1,
        work: &mut usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        charge_defined_callable_summary_work_v1(work, 4 + place.projections().len())?;
        Ok(place.projections().is_empty()
            && self
                .function
                .locals()
                .get(place.local().index() as usize)
                .is_some_and(|local| local.ty() == place.ty())
            && immutable_metadata_slice_type_v1(self.types, place.ty()))
    }

    fn descriptor_operand(
        &self,
        operand: &SemanticOperandV1,
        work: &mut usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        charge_defined_callable_summary_work_v1(work, 1)?;
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.descriptor_place(place, work)
            }
            SemanticOperandV1::Constant(_) => Ok(false),
        }
    }

    fn statement(
        &self,
        statement: &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
        work: &mut usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        charge_defined_callable_summary_work_v1(work, 1)?;
        match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                let value = assignment.value();
                if immutable_metadata_slice_type_v1(self.types, value.result_type()) {
                    return Ok(assignment.destination().ty() == value.result_type()
                        && self.descriptor_place(assignment.destination(), work)?
                        && matches!(value.kind(), SemanticRvalueKindV1::Use(operand)
                            if operand.ty() == value.result_type() && self.descriptor_operand(operand, work)?));
                }
                let is_length = match value.kind() {
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand,
                    } => Some(self.descriptor_operand(operand, work)?),
                    SemanticRvalueKindV1::Length(place) => {
                        charge_defined_callable_summary_work_v1(
                            work,
                            4 + place.projections().len(),
                        )?;
                        let local = self.function.locals().get(place.local().index() as usize);
                        Some(
                            matches!((local, place.projections()), (Some(local), [projection])
                            if immutable_metadata_slice_type_v1(self.types, local.ty())
                                && matches!(projection.kind(), SemanticProjectionKindV1::Dereference)
                                && matches!(self.types.get(local.ty().index() as usize).map(SemanticTypeDeclV1::shape), Some(SemanticTypeShapeV1::Pointer(pointer)) if pointer.pointee() == place.ty())
                                && projection.result_type() == place.ty()),
                        )
                    }
                    _ => None,
                };
                if let Some(is_length) = is_length {
                    return Ok(is_length
                        && unsigned_index_bits_v1(self.types, value.result_type()) == Some(64)
                        && scalar_defined_callable_destination_v1(
                            self.types,
                            self.function,
                            assignment.destination(),
                            work,
                        )?);
                }
                scalar_defined_callable_statement_v1(self.types, self.function, statement, work)
            }
            SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local)
                if self
                    .function
                    .locals()
                    .get(local.index() as usize)
                    .is_some_and(|local| {
                        immutable_metadata_slice_type_v1(self.types, local.ty())
                    }) =>
            {
                Ok(true)
            }
            _ => scalar_defined_callable_statement_v1(self.types, self.function, statement, work),
        }
    }

    fn call(
        &self,
        call: &SemanticDirectCallV1,
        callees: &mut Vec<usize>,
        call_edges: &mut usize,
        work: &mut usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        charge_defined_callable_summary_work_v1(work, 1 + call.arguments().len())?;
        let mut eligible = call.variadic_argument_abis().is_empty()
            && matches!(
                call.unwind(),
                SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
            );
        for argument in call.arguments() {
            eligible &= if immutable_metadata_slice_type_v1(self.types, argument.ty()) {
                self.descriptor_operand(argument, work)?
            } else {
                scalar_defined_callable_operand_v1(self.types, self.function, argument, work)?
            };
        }
        if let Some(destination) = call.destination() {
            eligible &= scalar_defined_callable_destination_v1(
                self.types,
                self.function,
                destination.place(),
                work,
            )?;
        }
        let Some(SemanticCallableDeclV1::Defined { function }) =
            self.callables.get(call.callee().index() as usize)
        else {
            return Ok(false);
        };
        let callee = function.index() as usize;
        if callee >= self.function_count {
            return Ok(false);
        }
        *call_edges =
            call_edges
                .checked_add(1)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "defined-callable effect-summary edge count overflowed",
                ))?;
        if *call_edges > MAX_DEFINED_CALLABLE_SUMMARY_EDGES_V1 {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "defined-callable effect-summary edge count exceeded its production limit",
            ));
        }
        callees.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "defined-callable effect-summary edge storage cannot be reserved",
            )
        })?;
        callees.push(callee);
        Ok(eligible)
    }

    fn terminator(
        &self,
        terminator: &SemanticTerminatorKindV1,
        callees: &mut Vec<usize>,
        call_edges: &mut usize,
        work: &mut usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        charge_defined_callable_summary_work_v1(work, 1)?;
        match terminator {
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::Unreachable => Ok(true),
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                scalar_defined_callable_operand_v1(self.types, self.function, discriminant, work)
            }
            SemanticTerminatorKindV1::Call(call) => self.call(call, callees, call_edges, work),
            _ => Ok(false),
        }
    }
}
