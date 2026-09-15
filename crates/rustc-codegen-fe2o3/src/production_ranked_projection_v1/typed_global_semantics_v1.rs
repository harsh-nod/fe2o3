// Facts collected only during replay of the converged capability states.
#[derive(Default)]
struct ProjectedGlobalSemanticUsesV1 {
    source_unsigned_comparisons:
        BTreeMap<(usize, usize), source_unsigned_switch_v1::SourceComparisonV1>,
    comparisons: BTreeMap<(usize, usize), AllocationContractV1>,
    discriminants: BTreeMap<(usize, usize), usize>,
    load_operands: HashMap<usize, Vec<(*const SemanticPlaceV1, SemanticTypeIdV1)>>,
    metadata_assignments: HashSet<*const fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1>,
    work: usize,
}

impl ProjectedGlobalSemanticUsesV1 {
    fn record_load_operand(
        &mut self,
        operand: &SemanticOperandV1,
        state: &ProjectedCapabilityStateV1,
        dominance: &SemanticEnumPayloadDominanceV1,
        block: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge_capability_dataflow_work_v1(&mut self.work, 1)?;
        let Some(ProjectedCapabilityValueV1::Known(
            ProjectedCapabilityOriginV1::GlobalLoadedScalar { block: producer },
        )) = capability_origin_from_assignment_operand_v1(
            operand,
            state,
            dominance,
            SemanticBlockIdV1::from_index(block as u32),
        )
        else {
            return Ok(());
        };
        let Some(place) = raw_operand_place(operand) else {
            return Ok(());
        };
        self.load_operands.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "typed global payload-use storage cannot be reserved",
            )
        })?;
        let uses = self.load_operands.entry(producer).or_default();
        uses.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "typed global payload-use storage cannot be reserved",
            )
        })?;
        uses.push((place as *const _, place.ty()));
        Ok(())
    }

    fn record_assignment(
        &mut self,
        assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        metadata_only: bool,
        state: &ProjectedCapabilityStateV1,
        dominance: &SemanticEnumPayloadDominanceV1,
        block: usize,
        statement: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge_capability_dataflow_work_v1(&mut self.work, 1)?;
        assignment.value().kind().try_visit_operands(|operand| {
            self.record_load_operand(operand, state, dominance, block)
        })?;
        if !assignment.destination().projections().is_empty() {
            return Ok(());
        }
        if metadata_only {
            self.metadata_assignments.try_reserve(1).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "typed global metadata-use storage cannot be reserved",
                )
            })?;
            // Replay authenticates this exact assignment before its operands
            // are consumed. The retained pointer is only an identity key.
            self.metadata_assignments.insert(assignment as *const _);
        }
        self.record_source_unsigned_comparison_v1(assignment, state, (block, statement))?;
        match assignment.value().kind() {
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                right,
                ..
            } => {
                if let Some(ProjectedCapabilityOriginV1::GlobalExtent(allocation)) =
                    capability_known_origin_v1(state, right)
                {
                    self.comparisons.insert((block, statement), allocation);
                }
            }
            SemanticRvalueKindV1::Discriminant(place) if place.projections().is_empty() => {
                if let Some(ProjectedCapabilityValueV1::Known(
                    ProjectedCapabilityOriginV1::GlobalLoadResult { block: producer },
                )) = state.get(&(place.local().index() as usize))
                {
                    self.discriminants.insert((block, statement), *producer);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn global_metadata_observation_v1(
    types: &[SemanticTypeDeclV1],
    value: &SemanticRvalueKindV1,
    origin: Option<ProjectedCapabilityValueV1>,
) -> bool {
    let Some(ProjectedCapabilityValueV1::Known(origin)) = origin else {
        return false;
    };
    match (value, origin) {
        (
            SemanticRvalueKindV1::Length(_)
            | SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                ..
            },
            ProjectedCapabilityOriginV1::GlobalExtent(_),
        )
        | (
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                ..
            },
            ProjectedCapabilityOriginV1::GlobalPhysical { .. },
        ) => true,
        (
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(_)),
            ProjectedCapabilityOriginV1::GlobalPhysical {
                view,
                ty,
                borrowed: false,
            },
        ) => {
            // Copying a shared slice handle observes no element. In particular,
            // neither a mutable handle nor a raw pointer gains this exemption.
            let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return false;
            };
            pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.mutability() == SemanticMutabilityV1::Immutable
                && pointer.address_space() == 0
                && pointer.pointer_width_bits() == 64
                && pointer.metadata() == SemanticPointerMetadataV1::SliceLength
                && matches!(types.get(pointer.pointee().index() as usize).map(SemanticTypeDeclV1::shape),
                    Some(SemanticTypeShapeV1::Slice { element }) if *element == view.element)
        }
        _ => false,
    }
}

fn global_shared_slice_reference_v1(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    slice: SemanticTypeIdV1,
) -> bool {
    matches!(
        types
            .get(slice.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Slice { .. })
    ) && matches!(types.get(reference.index() as usize).map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Pointer(pointer))
                if pointer.pointee() == slice
                    && pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.mutability() == SemanticMutabilityV1::Immutable
                    && pointer.address_space() == 0
                    && pointer.pointer_width_bits() == 64
                    && pointer.metadata() == SemanticPointerMetadataV1::SliceLength)
}

fn global_mutable_metadata_snapshot_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block: usize,
    statement: usize,
    origin: Option<ProjectedCapabilityValueV1>,
    work: &mut usize,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    let Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalPhysical {
        view,
        ty,
        borrowed: false,
    })) = origin
    else {
        return Ok(false);
    };
    if view.contract != SemanticCapabilityMemoryContractV1::global_exclusive_read_write() {
        return Ok(false);
    }
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Ok(false);
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Mutable
        || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
    {
        return Ok(false);
    }
    let Some([first, second, third]) = function.blocks()[block]
        .statements()
        .get(statement..)
        .and_then(|tail| tail.get(..3))
    else {
        return Ok(false);
    };
    let [
        SemanticStatementKindV1::Assign(snapshot),
        SemanticStatementKindV1::Assign(borrow),
        SemanticStatementKindV1::Assign(metadata),
    ] = [first.kind(), second.kind(), third.kind()]
    else {
        return Ok(false);
    };
    let snapshot_local = snapshot.destination().local();
    let borrow_local = borrow.destination().local();
    if !matches!(
        snapshot.value().kind(),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(_))
    ) || !snapshot.destination().projections().is_empty()
        || !borrow.destination().projections().is_empty()
        || !metadata.destination().projections().is_empty()
        || snapshot_local == borrow_local
        || [snapshot_local, borrow_local].contains(&metadata.destination().local())
        || [snapshot_local, borrow_local].iter().any(|local| {
            function.locals()[local.index() as usize].role() != SemanticLocalRoleV1::Temporary
        })
        || !global_shared_slice_reference_v1(types, borrow.destination().ty(), pointer.pointee())
        || unsigned_index_bits_v1(types, metadata.destination().ty()) != Some(64)
        || !matches!(borrow.value().kind(), SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared, place,
        } if place.local() == snapshot_local && place.ty() == pointer.pointee()
            && matches!(place.projections(), [projection]
                if projection.kind() == SemanticProjectionKindV1::Dereference
                    && projection.result_type() == pointer.pointee()))
        || !matches!(metadata.value().kind(), SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::PointerMetadata, operand,
        } if simple_operand_local(operand) == Some(borrow_local)
            && operand.ty() == borrow.destination().ty())
    {
        return Ok(false);
    }

    // The origin above is still mandatory. This audit only proves that the
    // two temporary handles have no observer outside this exact sequence.
    global_metadata_use_audit_v1::closed(
        function,
        [snapshot, borrow, metadata],
        [snapshot_local, borrow_local],
        work,
    )
}

fn global_physical_place_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
) -> Option<(ProjectedGlobalViewV1, SemanticTypeIdV1)> {
    let mut projections = place.projections().iter();
    let origin = state.get(&(place.local().index() as usize))?;
    let (view, mut ty) = match *origin {
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view)) => {
            let base = function.locals().get(place.local().index() as usize)?.ty();
            if view.borrow.is_some() {
                let dereference = projections.next()?;
                if dereference.kind() != SemanticProjectionKindV1::Dereference
                    || dereference.result_type() != view.view
                    || !is_exact_reference_to_v1(
                        types,
                        base,
                        view.view,
                        if view.borrow == Some(SemanticBorrowKindV1::Shared) {
                            SemanticMutabilityV1::Immutable
                        } else {
                            SemanticMutabilityV1::Mutable
                        },
                    )
                {
                    return None;
                }
            } else if base != view.view {
                return None;
            }
            let field = projections.next()?;
            let declaration = types.get(view.view.index() as usize)?;
            let SemanticTypeShapeV1::Aggregate(fields) = declaration.shape() else {
                return None;
            };
            let fe2o3_mir_model::semantic_mir_v1::SemanticTypeLayoutDetailsV1::Aggregate(layout) =
                declaration.layout().details()
            else {
                return None;
            };
            if field.kind() != SemanticProjectionKindV1::Field(0)
                || field.result_type() != view.physical
                || fields.fields().first() != Some(&view.physical)
                || layout.field_offsets().first() != Some(&0)
                || !layout.padding().is_empty()
                || declaration.layout().size_bytes()
                    != types
                        .get(view.physical.index() as usize)?
                        .layout()
                        .size_bytes()
                || fields.fields().iter().skip(1).any(|ty| {
                    types
                        .get(ty.index() as usize)
                        .is_none_or(|ty| ty.layout().size_bytes() != Some(0))
                })
            {
                return None;
            }
            (view, view.physical)
        }
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalPhysical {
            view,
            ty,
            borrowed,
        }) => {
            if borrowed {
                let dereference = projections.next()?;
                if dereference.kind() != SemanticProjectionKindV1::Dereference
                    || dereference.result_type() != ty
                {
                    return None;
                }
            }
            (view, ty)
        }
        _ => return None,
    };
    if let Some(projection) = projections.next() {
        let SemanticTypeShapeV1::Pointer(pointer) = types.get(ty.index() as usize)?.shape() else {
            return None;
        };
        if projection.kind() != SemanticProjectionKindV1::Dereference
            || pointer.kind() != SemanticPointerKindV1::Reference
            || (pointer.mutability() != SemanticMutabilityV1::Immutable
                && view.contract
                    != SemanticCapabilityMemoryContractV1::global_exclusive_read_write())
            || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
            || projection.result_type() != pointer.pointee()
        {
            return None;
        }
        ty = pointer.pointee();
    }
    (projections.next().is_none() && place.ty() == ty).then_some((view, ty))
}

fn global_metadata_assignment_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
) -> Option<ProjectedCapabilityValueV1> {
    let destination = assignment.destination().ty();
    let metadata = |view: ProjectedGlobalViewV1, ty: SemanticTypeIdV1| {
        (matches!(types.get(ty.index() as usize)?.shape(), SemanticTypeShapeV1::Slice { element } if *element == view.element)
            && matches!(types.get(destination.index() as usize)?.shape(), SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits: 64 })))
            .then_some(ProjectedCapabilityOriginV1::GlobalExtent(view.allocation))
    };
    let origin = match assignment.value().kind() {
        SemanticRvalueKindV1::Use(operand) => {
            let (view, ty) =
                global_physical_place_v1(types, function, state, raw_operand_place(operand)?)?;
            if destination != ty {
                return None;
            }
            ProjectedCapabilityOriginV1::GlobalPhysical {
                view,
                ty,
                borrowed: false,
            }
        }
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place,
        } => {
            let (view, ty) = global_physical_place_v1(types, function, state, place)?;
            let slice = matches!(
                types.get(ty.index() as usize)?.shape(),
                SemanticTypeShapeV1::Slice { .. }
            );
            if !(if slice {
                global_shared_slice_reference_v1(types, destination, ty)
            } else {
                is_exact_shared_reference_to_v1(types, destination, ty)
            }) {
                return None;
            }
            ProjectedCapabilityOriginV1::GlobalPhysical {
                view,
                ty: if slice { destination } else { ty },
                borrowed: !slice,
            }
        }
        SemanticRvalueKindV1::Length(place) => {
            let (view, ty) = global_physical_place_v1(types, function, state, place)?;
            metadata(view, ty)?
        }
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::PointerMetadata,
            operand,
        } => {
            let (view, ty) =
                global_physical_place_v1(types, function, state, raw_operand_place(operand)?)?;
            let SemanticTypeShapeV1::Pointer(pointer) = types.get(ty.index() as usize)?.shape()
            else {
                return None;
            };
            if pointer.metadata() != SemanticPointerMetadataV1::SliceLength {
                return None;
            }
            metadata(view, pointer.pointee())?
        }
        _ => return None,
    };
    Some(ProjectedCapabilityValueV1::Known(origin))
}

fn typed_global_source_call_v1<'a>(
    function: &'a SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    intrinsic: &IntrinsicProjectionV1,
    source: &ProjectedAccessSourceV1,
    view: ProductionRankedValueV1,
    indices: &[ProductionRankedValueV1],
    access: AccessKindAttr,
) -> Result<
    (
        &'a SemanticDirectCallV1,
        ProjectedGlobalAccessContractV1,
        ProjectedGlobalViewV1,
    ),
    &'static str,
> {
    let site = source
        .semantic_site
        .ok_or("typed global effect lacks its semantic site")?;
    let block = function
        .blocks()
        .get(site.block)
        .ok_or("typed global effect source block is out of bounds")?;
    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
        return Err("typed global effect source is not a call");
    };
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        binding, operation, ..
    }) = callables.get(call.callee().index() as usize)
    else {
        return Err("typed global effect source has no exact intrinsic contract");
    };
    let contract = global_access_contract_v1(operation)
        .ok_or("typed global effect source is not a modeled scalar access")?;
    let effects = if access == AccessKindAttr::Read {
        &intrinsic.direct_read_effects
    } else {
        &intrinsic.direct_write_effects
    };
    let expected = effects
        .get(site.block)
        .and_then(Option::as_ref)
        .ok_or("typed global effect lacks its authenticated call-site projection")?;
    let bound = intrinsic
        .global_views
        .get(site.block)
        .copied()
        .flatten()
        .ok_or("typed global effect lacks its authenticated allocation binding")?;
    if site.statement.is_some()
        || source.memory_space != MemorySpaceAttr::Global
        || source.access != access
        || source.source != block.terminator().source()
        || expected.source != source.source
        || expected.view
            != match view {
                ProductionRankedValueV1::Local(view) => view,
                _ => return Err("typed global effect view is not an exact ranked definition"),
            }
        || expected.indices != indices
        || expected.access != access
        || contract.access != access
        || contract.source_identity != binding.identity()
        || contract.view != bound.view
        || contract.element != bound.element
        || contract.contract != bound.contract
        || contract.provenance != bound.provenance
        || !contract.shape.arguments_match(call, contract.element)
        || !contract.shape.source_inputs_match(
            call, binding.abi().source_input_types(), binding.abi().source_argument_ownership(),
        )
        || call.destination().is_none_or(|destination| {
            !destination.place().projections().is_empty()
                || destination.place().ty() != contract.result
        })
        || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
    {
        return Err("typed global effect differs from its exact source contract");
    }
    Ok((call, contract, bound))
}

impl<'a> GpuSemanticExpressionResolverV2<'a> {
    fn bind_typed_global_read_v2(
        &mut self,
        callables: &[SemanticCallableDeclV1],
        intrinsic: &IntrinsicProjectionV1,
        blocks: &[ProductionRankedBlockV1],
        sources: &[ProjectedAccessSourceV1],
        source: &ProjectedAccessSourceV1,
        allocations: &HashMap<ProductionRankedValueV1, u64>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let Some(site) = source.semantic_site else {
            return Ok(());
        };
        let Some(SemanticTerminatorKindV1::Call(call)) = self
            .function
            .blocks()
            .get(site.block)
            .map(|block| block.terminator().kind())
        else {
            return Ok(());
        };
        if !matches!(
            callables.get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::CapabilityGlobalLoad { .. }
                    | SemanticCompilerIntrinsicOperationV1::CapabilityGlobalExclusiveLoad { .. },
                ..
            })
        ) {
            return Ok(());
        }
        let Some(ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Read,
            view,
            indices,
        }) = blocks
            .get(source.block)
            .and_then(|block| block.operations().get(source.operation))
        else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "typed global load lacks one exact ranked read",
            ));
        };
        let (_, contract, bound) = typed_global_source_call_v1(
            self.function,
            callables,
            intrinsic,
            source,
            *view,
            indices,
            AccessKindAttr::Read,
        )
        .map_err(ProductionRankedProjectionErrorV1::Incomplete)?;
        if bound.allocation.writable
            && !self
                .memory_versions
                .contains_key(&(source.block, source.operation))
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "typed global mutable load lacks an authenticated reaching memory version",
            ));
        }
        if allocations.get(view).copied() != Some(bound.allocation.allocation_origin)
            || sources
                .iter()
                .filter(|candidate| {
                    candidate.semantic_site == source.semantic_site
                        && candidate.access.reads_memory()
                })
                .count()
                != 1
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "typed global load allocation or source custody changed",
            ));
        }
        let load = ProductionSemanticLoadV2 {
            block: u32::try_from(source.block).map_err(|_| {
                ProductionRankedProjectionErrorV1::Incomplete("typed global load block exceeds u32")
            })?,
            operation: u32::try_from(source.operation).map_err(|_| {
                ProductionRankedProjectionErrorV1::Incomplete(
                    "typed global load operation exceeds u32",
                )
            })?,
            scalar: self
                .scalar_v2(contract.element)
                .map_err(ProductionRankedProjectionErrorV1::Incomplete)?,
            read_mode: fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedVolatile,
            allocation_origin: bound.allocation.allocation_origin,
            view: *view,
            indices: indices.clone().into_boxed_slice(),
        };
        if self.retain_reference_reads {
            self.reference_reads.record(source, &load, !bound.allocation.writable)?;
        }
        for &(place, ty) in intrinsic
            .global_uses
            .load_operands
            .get(&site.block)
            .into_iter()
            .flatten()
        {
            self.charge_v2()
                .map_err(ProductionRankedProjectionErrorV1::Incomplete)?;
            if ty != contract.element || self.place_loads.insert(place, load.clone()).is_some() {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "typed global payload use changed type or producer",
                ));
            }
        }
        Ok(())
    }

    fn resolve_versioned_load_v1(
        &mut self,
        load: ProductionSemanticLoadV2,
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        Self::require_depth_v2(depth)?;
        match self
            .memory_versions
            .get(&(load.block as usize, load.operation as usize))
            .copied()
        {
            None | Some(TypedGlobalMemoryVersionV1::Initial) => {
                Ok(ProductionSemanticExpressionV2::Load(load))
            }
            Some(TypedGlobalMemoryVersionV1::Store { semantic_block }) => {
                self.charge_v2()?;
                if !self.visiting_stores.insert(semantic_block) {
                    return Err("mutable global store value has a cyclic memory dependency");
                }
                let result = self.resolve_reaching_store_rhs_v1(semantic_block, depth + 1);
                self.visiting_stores.remove(&semantic_block);
                result
            }
            Some(TypedGlobalMemoryVersionV1::Conflict) => {
                Err("mutable global load has conflicting reaching writes")
            }
        }
    }
}
