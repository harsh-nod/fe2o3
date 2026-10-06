// Genuine optimized slice replay; legacy and original query implementations remain unchanged.

struct OptimizedSliceFactsV18<'a> {
    access: SliceAccess,
    address_operation: SliceOperation,
    data_operation: SliceOperation,
    length_operation: SliceOperation,
    data_carrier: SliceDefinition,
    length_carrier: SliceDefinition,
    input: SliceDefinition,
    index: SliceDefinition,
    memory: MemoryAccess,
    loaded_type: &'a Type,
    guard_condition: SliceDefinition,
    guard_edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
}

#[derive(Clone, Copy)]
enum OptimizedSliceOwnerV18<'a> {
    Optimized {
        relation: &'a ProductionSourceCorrespondenceV18<'a>,
        optimized: &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        root: usize,
        instance: usize,
        retained_allocations: &'a [SliceOperation],
    },
}

struct OptimizedSliceQueryV18<'a, 's, O = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12> {
    owner: OptimizedSliceOwnerV18<'a>,
    inventory: &'a CanonicalKirInventoryV1<'a, O>,
    function: &'a CanonicalKirFunctionRefV1<'a>,
    site: ProductionSliceAccessSiteV1,
    origins: &'s super::value_origin_v1::WholeValueOriginsV1<'a, O>,
}

impl<'a, O> OptimizedSliceQueryV18<'a, '_, O> {
    fn operation(
        &self,
        coordinate: SliceOperation,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<&'a CanonicalKirOperationRefV1<'a>> {
        budget.charge_work(4)?;
        if coordinate.block.function != self.function.coordinate {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let blocks = self
            .inventory
            .blocks()
            .get(self.function.blocks.clone())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let block = blocks
            .get(coordinate.block.block as usize)
            .filter(|block| block.coordinate == coordinate.block)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        self.inventory
            .operations()
            .get(block.operations.clone())
            .and_then(|operations| operations.get(coordinate.operation as usize))
            .filter(|operation| operation.coordinate == coordinate)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }

    fn origin(&self, value: ValueId, budget: &mut SliceBudget<'_>) -> SliceResult<SliceDefinition> {
        self.origins
            .resolve(value, budget)
            .map_err(slice_inventory_error)?
            .ok_or_else(|| {
                self.site
                    .unsupported("slice access has conflicting or ungrounded SSA origins")
            })
    }

    fn descriptor_origin(
        &self,
        mut value: ValueId,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<SliceDefinition> {
        let steps = self
            .function
            .definitions
            .len()
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        for _ in 0..steps {
            budget.charge_work(1)?;
            let origin = self.origin(value, budget)?;
            if let SliceDefinition::Result {
                operation,
                result: 0,
            } = origin
            {
                let operation = self.operation(operation, budget)?;
                if let OperationKind::Cast {
                    kind: CastKind::SliceToGeneric,
                    value: input,
                    to,
                } = &operation.operation.kind
                {
                    let definition = self
                        .inventory
                        .definition_for_value(self.function.coordinate, *input, budget)
                        .map_err(slice_inventory_error)?
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    if !source_descriptor_widening_v29(definition.ty, to)
                        || operation.operation.results.len() != 1
                        || operation.operation.results[0].ty != *to
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    value = *input;
                    continue;
                }
            }
            return Ok(origin);
        }
        Err(self
            .site
            .unsupported("slice descriptor transport is cyclic"))
    }

    fn defining_operation(
        &self,
        value: ValueId,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<&'a CanonicalKirOperationRefV1<'a>> {
        match self.origin(value, budget)? {
            SliceDefinition::Result {
                operation,
                result: 0,
            } => self.operation(operation, budget),
            _ => Err(self
                .site
                .unsupported("slice access operand has no supported defining operation")),
        }
    }

    fn is_retained_private(
        &self,
        mut pointer: ValueId,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<bool> {
        for _ in 0..self.function.definitions.len() {
            budget.charge_work(1)?;
            let origin = self
                .origins
                .resolve(pointer, budget)
                .map_err(slice_inventory_error)?;
            let origin = origin.ok_or_else(|| {
                self.site.unsupported(
                    "retained private storage has conflicting or ungrounded SSA origins",
                )
            })?;
            let SliceDefinition::Result {
                operation,
                result: 0,
            } = origin
            else {
                return Ok(false);
            };
            let actual = self.operation(operation, budget)?;
            let block = &self.inventory.blocks()
                [self.function.blocks.start + operation.block.block as usize];
            let _location =
                FunctionOperationLocation::new(block.block.id, operation.operation as usize);
            let OptimizedSliceOwnerV18::Optimized {
                retained_allocations,
                ..
            } = self.owner;
            let retained = {
                budget.charge_work(call_splice_search_work_v1(retained_allocations.len()))?;
                retained_allocations
                    .binary_search_by_key(&(operation.block.block, operation.operation), |row| {
                        (row.block.block, row.operation)
                    })
                    .is_ok()
            };
            if retained {
                return Ok(true);
            }
            match &actual.operation.kind {
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value,
                    ..
                } => pointer = *value,
                _ => return Ok(false),
            }
        }
        Err(self
            .site
            .unsupported("retained private storage transport is cyclic"))
    }

    fn visit_site_operations(
        &self,
        budget: &mut SliceBudget<'_>,
        mut visit: impl FnMut(
            &'a CanonicalKirOperationRefV1<'a>,
            &mut SliceBudget<'_>,
        ) -> SliceResult<()>,
    ) -> SliceResult<()> {
        let OptimizedSliceOwnerV18::Optimized {
            optimized,
            root,
            instance,
            ..
        } = self.owner;
        {
            return optimized
                .visit_source_operations(
                    root,
                    instance,
                    self.site.block,
                    self.site.statement,
                    budget,
                    |disposition, budget| {
                        if let ProductionOptimizedSourceSpanV18::Operation(
                            ProductionOptimizedSourceOperationV18::Retained { output, .. },
                        ) = disposition
                        {
                            visit(
                                self.operation(output, budget)
                                    .map_err(source_emission_error_v18)?,
                                budget,
                            )
                            .map_err(source_emission_error_v18)?;
                        }
                        Ok(())
                    },
                )
                .map_err(source_slice_query_error_v18);
        }
    }

    fn access(&self, budget: &mut SliceBudget<'_>) -> SliceResult<SliceAccess> {
        let mut ordinal = 0_u32;
        let mut selected = None;
        self.visit_site_operations(budget, |operation, budget| {
            let effects = self
                .inventory
                .effects()
                .get(operation.effects.clone())
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            budget.charge_work(effects.len())?;
            let mut effects = effects.iter().filter(|effect| {
                matches!(
                    effect.effect,
                    KirLocalMemoryEffectRefV1::Read(_)
                        | KirLocalMemoryEffectRefV1::Write(_)
                        | KirLocalMemoryEffectRefV1::VolatileRead(_)
                        | KirLocalMemoryEffectRefV1::VolatileWrite(_)
                        | KirLocalMemoryEffectRefV1::Atomic { .. }
                )
            });
            try_visit_kir_memory_accesses_v1(
                operation.operation,
                |(pointer, access, space, atomic)| -> SliceResult<()> {
                    budget.charge_work(1)?;
                    let effect = effects
                        .next()
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    let plain_private = space == dialect_kernel::MemorySpaceAttr::Private
                        && atomic.is_none()
                        && matches!(
                            (&operation.operation.kind, access),
                            (
                                OperationKind::Load { .. },
                                dialect_kernel::AccessKindAttr::Read
                            ) | (
                                OperationKind::Store { .. },
                                dialect_kernel::AccessKindAttr::Write
                            )
                        );
                    if plain_private && self.is_retained_private(pointer, budget)? {
                        return Ok(());
                    }
                    if ordinal == self.site.access {
                        selected = Some(effect.coordinate);
                    }
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    Ok(())
                },
            )?;
            if effects.next().is_some() {
                return Err(self
                    .site
                    .unsupported("source slice span has an unmodeled memory effect"));
            }
            Ok(())
        })?;
        selected.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }

    fn index_origin(
        &self,
        mut value: ValueId,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<SliceDefinition> {
        for _ in 0..self.function.definitions.len() {
            budget.charge_work(1)?;
            let definition = self
                .inventory
                .definition_for_value(self.function.coordinate, value, budget)
                .map_err(slice_inventory_error)?
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if !matches!(
                definition.ty,
                Type::Scalar(ScalarType::Index | ScalarType::U64)
            ) {
                return Err(self
                    .site
                    .unsupported("slice bounds require the exact unsigned index domain"));
            }
            let origin = self.origin(value, budget)?;
            if let SliceDefinition::Result {
                operation,
                result: 0,
            } = origin
            {
                let operation = self.operation(operation, budget)?;
                if let OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: input,
                    to,
                } = &operation.operation.kind
                    && matches!(to, Type::Scalar(ScalarType::Index | ScalarType::U64))
                {
                    value = *input;
                    continue;
                }
            }
            return Ok(origin);
        }
        Err(self.site.unsupported("slice index transport is cyclic"))
    }

    fn facts(&self, budget: &mut SliceBudget<'_>) -> SliceResult<OptimizedSliceFactsV18<'a>> {
        self.descriptor_facts(false, budget)
    }

    fn descriptor_facts(
        &self,
        write: bool,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<OptimizedSliceFactsV18<'a>> {
        let access = self.access(budget)?;
        let read = self.operation(access.operation, budget)?;
        let (pointer, memory, loaded_type) = match (&read.operation.kind, write) {
            (OperationKind::Load { pointer, access }, false) => {
                let [result] = read.operation.results.as_slice() else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                (pointer, access, &result.ty)
            }
            (
                OperationKind::Store {
                    pointer,
                    value,
                    access,
                },
                true,
            ) if read.operation.results.is_empty() => {
                budget.reserve_storage(descriptor_write_lookup_headers_v18()?)?;
                let definition = self
                    .inventory
                    .definition_for_value(self.function.coordinate, *value, budget)
                    .map_err(slice_inventory_error)?
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                if definition.ty.as_scalar().is_none() {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                (pointer, access, definition.ty)
            }
            _ => {
                return Err(self.site.unsupported(if write {
                    "checked slice correspondence requires a plain scalar write"
                } else {
                    "checked slice correspondence requires a plain read"
                }));
            }
        };
        if !matches!(
            memory.address_space,
            AddressSpace::Global | AddressSpace::Generic
        ) || memory.volatile
            || access.effect != 0
        {
            return Err(self
                .site
                .unsupported("checked slice correspondence requires a nonvolatile global read"));
        }
        let address = self.defining_operation(*pointer, budget)?;
        let OperationKind::GetElementPointer { base, offset } = address.operation.kind else {
            return Err(self
                .site
                .unsupported("slice read address is not an exact element projection"));
        };
        let data = self.defining_operation(base, budget)?;
        let OperationKind::SliceData { slice: data_slice } = data.operation.kind else {
            return Err(self
                .site
                .unsupported("slice read address has no exact slice data carrier"));
        };
        let (definition, success_edge) = self.assertion_endpoint(budget)?;
        let edges = self
            .inventory
            .edges()
            .get(self.function.edges.clone())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        budget.charge_work(edges.len())?;
        let mut predecessors = edges
            .iter()
            .filter(|edge| edge.target == access.operation.block);
        if predecessors.next().map(|edge| edge.coordinate) != Some(success_edge)
            || predecessors.next().is_some()
        {
            return Err(self
                .site
                .unsupported("slice read requires the unique assertion-success predecessor"));
        }
        let SliceDefinition::Result {
            operation: compare,
            result: 0,
        } = definition
        else {
            return Err(self
                .site
                .unsupported("slice assertion has no exact comparison definition"));
        };
        let compare = self.operation(compare, budget)?;
        let OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        } = compare.operation.kind
        else {
            return Err(self
                .site
                .unsupported("slice assertion is not an exact less-than comparison"));
        };
        let index = self.index_origin(offset, budget)?;
        if index != self.index_origin(lhs, budget)? {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let SliceDefinition::Result {
            operation: length,
            result: 0,
        } = self.index_origin(rhs, budget)?
        else {
            return Err(self
                .site
                .unsupported("slice assertion does not use a slice length"));
        };
        let length = self.operation(length, budget)?;
        let OperationKind::SliceLength {
            slice: length_slice,
        } = length.operation.kind
        else {
            return Err(self
                .site
                .unsupported("slice assertion does not use a slice length"));
        };
        let input = self.descriptor_origin(data_slice, budget)?;
        if input != self.descriptor_origin(length_slice, budget)? {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        if !matches!(input, SliceDefinition::FunctionArgument { .. }) {
            return Err(self
                .site
                .unsupported("slice carrier is not exact whole-entry transport"));
        }
        let SliceDefinition::FunctionArgument { function, argument } = input else {
            unreachable!()
        };
        let Type::Slice(root_slice) = self
            .function
            .function
            .signature
            .parameters
            .get(argument as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let Type::Slice(data_type) = self
            .inventory
            .definition_for_value(self.function.coordinate, data_slice, budget)
            .map_err(slice_inventory_error)?
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .ty
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if function != self.function.coordinate
            || root_slice.address_space != AddressSpace::Global
            || data_type.address_space != memory.address_space
            || root_slice.element != data_type.element
            || root_slice.access != data_type.access
            || (write && root_slice.access != AccessMode::ReadWrite)
            || (write && root_slice.element.as_ref() != loaded_type)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let carrier = |value, budget: &mut SliceBudget<'_>| -> SliceResult<SliceDefinition> {
            let definition = self
                .inventory
                .definition_for_value(self.function.coordinate, value, budget)
                .map_err(slice_inventory_error)?
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if !matches!(definition.ty, Type::Slice(_)) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            Ok(definition.coordinate)
        };
        Ok(OptimizedSliceFactsV18 {
            access,
            address_operation: address.coordinate,
            data_operation: data.coordinate,
            length_operation: length.coordinate,
            data_carrier: carrier(data_slice, budget)?,
            length_carrier: carrier(length_slice, budget)?,
            input,
            index,
            memory: *memory,
            loaded_type,
            guard_condition: definition,
            guard_edge: success_edge,
        })
    }

    fn assertion_endpoint(
        &self,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<(
        SliceDefinition,
        fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    )> {
        let OptimizedSliceOwnerV18::Optimized {
            relation,
            root,
            instance,
            ..
        } = self.owner;
        let assertion = relation
            .assertion(root, instance, self.site.assertion, budget)
            .map_err(source_slice_query_error_v18)?;
        let SemanticKirAssertConditionOutcomeV1::Emitted {
            definition: _,
            success_edge: _,
            ..
        } = assertion.outcome()
        else {
            return Err(self
                .site
                .unsupported("slice assertion was elided by an existing rule"));
        };
        if !assertion.expected() || assertion.semantic_success() != self.site.block {
            return Err(self
                .site
                .unsupported("slice read is not the selected positive assertion successor"));
        }
        let OptimizedSliceOwnerV18::Optimized {
            optimized,
            root,
            instance,
            ..
        } = self.owner;
        {
            let SemanticKirOptimizedAssertOutcomeV1::Conditional {
                condition, success, ..
            } = optimized
                .assertion(root, instance, self.site.assertion, budget)
                .map_err(source_slice_query_error_v18)?
            else {
                return Err(self.site.unsupported(
                    "optimized selected or elided slice assertion needs formal bounds continuation",
                ));
            };
            let fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Retained(success) = success
            else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            return Ok((condition.definition, success));
        }
    }
}
