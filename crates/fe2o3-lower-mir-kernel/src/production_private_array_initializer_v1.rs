fn private_array_initializer_value_v1<W: PrivateArrayChargeV1>(
    statement: &SemanticStatementKindV1,
    body: &FunctionBody,
    slot: &PrivateArraySlotV1,
    effect: &PrivateArrayEffectV1,
    component: u32,
    binding: PrivateArrayInitializerValueV1,
    work: &mut W,
) -> Result<u64, PrivateArrayRelationErrorV1<W::Error>> {
    use PrivateArrayRelationErrorV1::{Incomplete, InvalidSource, Mismatch};
    work.charge_private_array_work(12)?;
    let SemanticStatementKindV1::Assign(assignment) = statement else {
        return Err(InvalidSource(
            "private initializer is not an array assignment",
        ));
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        return Err(Incomplete(
            "private initializer requires an exact Array Aggregate",
        ));
    };
    if aggregate.kind() != &SemanticAggregateKindV1::Array
        || assignment.value().result_type() != slot.semantic_type
        || !assignment.destination().projections().is_empty()
        || effect.role != fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination
        || effect.access != PrivateArrayAccessV1::Write
        || effect.semantic_type != slot.semantic_type
        || u64::try_from(aggregate.operands().len()).ok() != Some(slot.length)
        || u64::from(component) >= slot.length
    {
        return Err(Mismatch(
            "private initializer source shape or component changed",
        ));
    }
    work.charge_private_array_work(6)?;
    let Some(SemanticOperandV1::Constant(constant)) = aggregate.operands().get(component as usize)
    else {
        return Err(Incomplete(
            "private initializer value requires a separate SSA operand relation",
        ));
    };
    let SemanticConstantValueV1::Scalar(bits) = constant.value() else {
        return Err(Incomplete(
            "private initializer constant is not a literal scalar",
        ));
    };
    let PrivateRetainedElementFactsV1::Scalar(scalar) = slot.element_facts.element else {
        return Err(Incomplete(
            "private initializer element requires a separate value relation",
        ));
    };
    if constant.ty() != slot.element_type || u64::from(bits.size_bytes()) != slot.element_facts.size
    {
        return Err(Mismatch(
            "private initializer source scalar type or width changed",
        ));
    }
    let PrivateArrayInitializerValueV1::LiteralScalar { value, definition } = binding;
    // Scalar decoding has fixed bounded width; use the existing exact bit conversion.
    work.charge_private_array_work(12)?;
    let expected = lower_constant(Type::Scalar(scalar), *bits)
        .map_err(|_| Incomplete("private initializer scalar representation is unsupported"))?;
    if definition.block != effect.memory_location.block
        || definition.block_ordinal != effect.memory_location.block_ordinal
        || definition.operation
            != effect
                .source_first_operation
                .checked_add(component as usize)
                .ok_or(Mismatch(
                    "private initializer definition coordinate overflows",
                ))?
        || definition.operation >= effect.memory_location.operation
        || effect
            .source_first_operation
            .checked_add(aggregate.operands().len())
            .is_none_or(|end| end > effect.gep_location.operation)
    {
        return Err(Mismatch(
            "private initializer definition is outside its exact source recipe",
        ));
    }
    let actual = private_array_operation_v1(body, definition, work)?
        .ok_or(Mismatch("private initializer value definition is absent"))?;
    work.charge_private_array_work(5)?;
    let [result] = actual.results.as_slice() else {
        return Err(Mismatch("private initializer value result count changed"));
    };
    if result.id != value
        || result.ty != Type::Scalar(scalar)
        || !matches!(&actual.kind, OperationKind::Constant(actual) if *actual == expected)
    {
        return Err(Mismatch(
            "private initializer value differs from its exact source literal",
        ));
    }
    let memory = private_array_operation_v1(body, effect.memory_location, work)?
        .ok_or(Mismatch("private initializer Store is absent"))?;
    work.charge_private_array_work(2)?;
    if !matches!(memory.kind, OperationKind::Store { value: actual, .. } if actual == value) {
        return Err(Mismatch("private initializer Store uses a different value"));
    }
    Ok(u64::from(component))
}

struct PrivateArrayInitializerContextV1<'a> {
    root: usize,
    function_id: SemanticFunctionIdV1,
    body: &'a FunctionBody,
    semantic: &'a AdmittedInertSemanticMirV1,
    function: &'a SemanticFunctionDeclV1,
}

fn source_private_array_error_v18(
    error: PrivateArrayRelationErrorV1<ProductionSourceOwnedViewErrorV18>,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        PrivateArrayRelationErrorV1::Work(error) => error,
        PrivateArrayRelationErrorV1::InvalidSource(detail)
        | PrivateArrayRelationErrorV1::Incomplete(detail)
        | PrivateArrayRelationErrorV1::Mismatch(detail) => {
            ProductionSourceOwnedViewErrorV18::Binding(detail)
        }
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    fn private_array_attachment_key(
        root: usize,
        instance: usize,
        row: usize,
        field: TileAttachmentFieldV29,
    ) -> TileAttachmentKeyV29 {
        TileAttachmentKeyV29 {
            root,
            family: TileAttachmentFamilyV29::PrivateArray,
            instance,
            row,
            field,
            component: 0,
            part: 0,
        }
    }

    fn private_array_mapped_location(
        &self,
        key: TileAttachmentKeyV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<PrivateArrayPhysicalLocationV1> {
        let [row] = self.attachment_range(key, budget)? else {
            return self
                .source
                .missing("private array point is not one actual operation");
        };
        let TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(point)) = row.location
        else {
            return self
                .source
                .missing("private array point has no actual operation");
        };
        self.private_array_physical_point(key.root, point, budget)
    }

    fn private_array_physical_point(
        &self,
        root: usize,
        point: TileScalarPointV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<PrivateArrayPhysicalLocationV1> {
        budget.charge_work(4)?;
        let row = self.source.root_row(root)?;
        if point.function != row.function_ordinal {
            return self
                .source
                .missing("private array point changed physical root");
        }
        let body = self
            .inventory
            .functions()
            .get(point.function)
            .and_then(|row| row.function.body.as_ref())
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "private array body",
            ))?;
        let block = body
            .blocks
            .get(point.block)
            .filter(|block| point.operation < block.operations.len())
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "private array operation",
            ))?;
        Ok(PrivateArrayPhysicalLocationV1 {
            block_ordinal: point.block,
            block: block.id,
            operation: point.operation,
        })
    }

    fn private_array_source_range(
        &self,
        key: TileAttachmentKeyV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(PrivateArrayPhysicalLocationV1, usize)> {
        let rows = self.attachment_range(key, budget)?;
        let mut first: Option<PrivateArrayPhysicalLocationV1> = None;
        let mut end = 0;
        for row in rows {
            budget.charge_work(2)?;
            let TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(point)) =
                row.location
            else {
                return self
                    .source
                    .missing("private initializer range contains non-operation output");
            };
            let location = self.private_array_physical_point(key.root, point, budget)?;
            if let Some(start) = first {
                if location.block != start.block
                    || location.block_ordinal != start.block_ordinal
                    || location.operation != end
                {
                    return self
                        .source
                        .missing("private initializer range is not its contiguous source recipe");
                }
            } else {
                first = Some(location);
            }
            end = location
                .operation
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        Ok((
            first.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "empty private initializer range",
            ))?,
            end,
        ))
    }

    /// Checks a retained literal-array initializer in its exact source instance.
    /// Each relocated address and written value uses the existing independent
    /// private-array relation. A count is descriptive, not execution authority.
    pub fn private_array_initializer_count(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<u64>> {
        self.retain_query((|| {
            self.query(budget)?;
            budget.charge_work(12)?;
            let function_id = self.source.instance(root, instance, budget)?.0;
            let root_row = self.source.root_row(root)?;
            let owner = &self.source.owner.inner.source.owner;
            let semantic = owner.source_semantic();
            let function = semantic
                .functions()
                .get(function_id.index() as usize)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "private initializer source function",
                ))?;
            let source = function
                .blocks()
                .get(block.index() as usize)
                .and_then(|block| block.statements().get(statement as usize))
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "private initializer source occurrence",
                ))?;
            let SemanticStatementKindV1::Assign(assignment) = source.kind() else {
                return self
                    .source
                    .missing("private initializer is not an assignment");
            };
            let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                return self
                    .source
                    .missing("private initializer is not an array aggregate");
            };
            let place = assignment.destination();
            let local = function
                .locals()
                .get(place.local().index() as usize)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "private initializer source local",
                ))?;
            if !place.projections().is_empty()
                || local.role().is_entry_argument()
                || place.ty() != local.ty()
                || assignment.value().result_type() != local.ty()
                || aggregate.kind() != &SemanticAggregateKindV1::Array
            {
                return self
                    .source
                    .missing("private initializer is not one whole nonargument array");
            }
            let plan = owner.plan_for_function(function_id).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("private initializer SSA plan"),
            )?;
            let promoted = private_array_binary_search_v1(
                plan.plan().promoted_variables(),
                |value| [value.get() as usize],
                [place.local().index() as usize],
                &mut SourceCorrespondenceWorkV18(budget),
            )?
            .is_ok();
            let arrays = &self.source.sidecar(root, instance, budget)?.private_arrays;
            budget.charge_work(arrays.slots.len())?;
            let mut slots = arrays
                .slots
                .iter()
                .enumerate()
                .filter(|(_, slot)| slot.local == place.local().index());
            let Some((slot_index, selected_slot)) = slots.next() else {
                return if promoted {
                    Ok(None)
                } else {
                    self.source
                        .missing("retained private initializer has no source slot")
                };
            };
            if slots.next().is_some() {
                return self.source.missing("ambiguous private initializer slot");
            }
            if promoted {
                return self
                    .source
                    .missing("retained private initializer contradicts source promotion");
            }
            let mut slot = *selected_slot;
            let facts = private_retained_array_facts_v1(
                semantic.types(),
                local.ty(),
                self.source.owner.inner.limits.max_operations,
                &mut SourceCorrespondenceWorkV18(budget),
            )?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "private initializer fixed layout",
            ))?;
            if slot.owner != root_row.coordinates.root
                || slot.function != function_id
                || slot.semantic_type != local.ty()
                || slot.length != facts.length
                || slot.element_type != facts.element_type
                || slot.element_facts != facts.element
                || u64::try_from(aggregate.operands().len()).ok() != Some(facts.length)
                || !matches!(
                    facts.element.element,
                    PrivateRetainedElementFactsV1::Scalar(_)
                )
            {
                return self
                    .source
                    .missing("private initializer source layout or component count changed");
            }
            use TileAttachmentFieldV29 as Field;
            let key = |row, field| Self::private_array_attachment_key(root, instance, row, field);
            slot.count_location = self.private_array_mapped_location(
                key(slot_index, Field::ArrayCountLocation),
                budget,
            )?;
            slot.alloca_location = self
                .private_array_mapped_location(key(slot_index, Field::ArrayAllocation), budget)?;
            // Instance sidecars retain emission order, not the legacy merged
            // correspondence's sorted order. Do not binary-search raw rows.
            budget.charge_work(arrays.effects.len())?;
            let effects = arrays.effects.iter().enumerate().filter(|(_, row)| {
                row.semantic_block == block.index() && row.semantic_statement == statement
            });
            let body = self
                .inventory
                .functions()
                .get(root_row.function_ordinal)
                .and_then(|row| row.function.body.as_ref())
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "private initializer physical body",
                ))?;
            budget.charge_work(1)?;
            let allocation_block = body.blocks.first()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "private initializer allocation entry",
                ))?.id;
            let mut span = None;
            let mut previous = None;
            let mut count = 0usize;
            for (component, (effect_index, original)) in effects.enumerate() {
                budget.charge_work(12)?;
                let row = arrays
                    .slots
                    .len()
                    .checked_add(effect_index)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let mut effect = *original;
                if effect.owner != root_row.coordinates.root
                    || effect.function != function_id
                    || effect.local != place.local().index()
                    || effect.semantic_block != block.index()
                    || effect.semantic_statement != statement
                {
                    return self
                        .source
                        .missing("private initializer source instance differs");
                }
                let raw_span = (effect.source_first_operation, effect.source_end_operation);
                let (first, end) = match span {
                    Some((expected, first, end)) if expected == raw_span => (first, end),
                    Some(_) => {
                        return self.source.missing(
                            "private initializer components name different source ranges",
                        );
                    }
                    None => {
                        let (first, end) = self.private_array_source_range(
                            key(row, Field::ArraySourceRange),
                            budget,
                        )?;
                        span = Some((raw_span, first, end));
                        (first, end)
                    }
                };
                effect.source_first_operation = first.operation;
                effect.source_end_operation = end;
                effect.gep_location =
                    self.private_array_mapped_location(key(row, Field::ArrayGepLocation), budget)?;
                effect.memory_location = self
                    .private_array_mapped_location(key(row, Field::ArrayMemoryLocation), budget)?;
                effect.offset_location = effect
                    .offset_location
                    .map(|_| {
                        self.private_array_mapped_location(
                            key(row, Field::ArrayOffsetLocation),
                            budget,
                        )
                    })
                    .transpose()?;
                let PrivateArrayIndexV1::InitializerElement {
                    component: actual,
                    value: PrivateArrayInitializerValueV1::LiteralScalar { value, .. },
                } = effect.original_index
                else {
                    return self
                        .source
                        .missing("private initializer is not a literal component");
                };
                if actual as usize != component
                    || effect.memory_location.block != first.block
                    || previous.is_some_and(|prior| prior >= effect.memory_location.operation)
                {
                    return self
                        .source
                        .missing("private initializer components are not exact and ordered");
                }
                effect.original_index = PrivateArrayIndexV1::InitializerElement {
                    component: actual,
                    value: PrivateArrayInitializerValueV1::LiteralScalar {
                        value,
                        definition: self.private_array_mapped_location(
                            key(row, Field::ArrayLiteralDefinition),
                            budget,
                        )?,
                    },
                };
                let actual = private_array_exact_physical_relation_v1(
                    semantic.types(),
                    function,
                    body,
                    root_row.coordinates.root,
                    function_id,
                    &slot,
                    &effect,
                    PrivateArrayPhysicalBlocksV1 { allocation: allocation_block, access: first.block },
                    self.source.owner.inner.limits.max_operations,
                    &mut SourceCorrespondenceWorkV18(budget),
                )
                .map_err(source_private_array_error_v18)?;
                if actual != component as u64 {
                    return self
                        .source
                        .missing("private initializer normalized component differs");
                }
                previous = Some(effect.memory_location.operation);
                count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            if u64::try_from(count).ok() != Some(facts.length) {
                return self.source.missing("private initializer effect census");
            }
            Ok(Some(facts.length))
        })())
    }
}

impl ProductionPreRankedKirOwnerV1 {
    fn private_array_initializer_context_v1<'a>(
        &'a self,
        selected_root: SemanticFunctionIdV1,
        expected_body: SemanticFunctionIdV1,
        work: &mut PrivateArrayQueryWorkV1<'_, '_>,
    ) -> Result<PrivateArrayInitializerContextV1<'a>, SemanticKirPrivateArrayQueryErrorV1> {
        use SemanticKirPrivateArrayQueryErrorV1::{InvalidSource, Mismatch};
        private_array_binary_search_v1(
            &self.launch_roots,
            |row| [row.selected_root.index() as usize],
            [selected_root.index() as usize],
            work,
        )?
        .map_err(|_| InvalidSource("selected root is absent from this owner"))?;
        // Use only already sealed function instances. This lookup neither
        // admits a helper nor substitutes absence for source promotion.
        let index = private_array_binary_search_v1(
            &self.assert_origins.functions,
            |row| [row.owner.index() as usize, row.function.index() as usize],
            [
                selected_root.index() as usize,
                expected_body.index() as usize,
            ],
            work,
        )?
        .map_err(|_| InvalidSource("initializer body is absent from this owner"))?;
        work.charge_private_array_work(12)?;
        let row = self.assert_origins.functions[index];
        let root = row.canonical.0 as usize;
        let lowered = self
            .executable
            .module()
            .functions
            .get(root)
            .ok_or(Mismatch("initializer physical function is absent"))?;
        let body = lowered
            .body
            .as_ref()
            .ok_or(Mismatch("initializer physical body is absent"))?;
        let semantic = self.semantic_ssa.source_semantic();
        let function = semantic
            .functions()
            .get(expected_body.index() as usize)
            .ok_or(Mismatch("initializer source body is absent"))?;
        let plan = self
            .semantic_ssa
            .plan_for_function(expected_body)
            .ok_or(Mismatch("initializer source SSA body is absent"))?;
        if row.owner != selected_root
            || row.function != expected_body
            || plan.function_identity() != function.identity()
            || plan.plan().reverse_postorder().len() != row.reachable_blocks
        {
            return Err(Mismatch(
                "initializer sealed function or SSA association changed",
            ));
        }
        Ok(PrivateArrayInitializerContextV1 {
            root,
            function_id: expected_body,
            body,
            semantic,
            function,
        })
    }

    /// Checks every element of one retained literal-scalar Array Aggregate.
    /// The inert count grants no functional, artifact or launch authority.
    /// `None` is restricted to the sealed promoted-local case; a retained but
    /// unsupported, missing or malformed initializer remains an error. Keep
    /// this owner's graph, assertion-origin and helper-memory payloads reserved
    /// in `budget`.
    pub fn materialized_private_array_initializer_count(
        &self,
        selected_root: SemanticFunctionIdV1,
        expected_body: SemanticFunctionIdV1,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Option<u64>, SemanticKirPrivateArrayQueryErrorV1> {
        use SemanticKirPrivateArrayQueryErrorV1::{Incomplete, InvalidSource, Mismatch, Resource};
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as ResourceError;
        budget.charge_work(3).map_err(Resource)?;
        let floor = self.retained_analysis_storage_v1();
        if budget.storage() < floor {
            return Err(Resource(ResourceError::Accounting));
        }
        let mut work = PrivateArrayQueryWorkV1 { budget };
        let context =
            self.private_array_initializer_context_v1(selected_root, expected_body, &mut work)?;
        // Statement/local/type/count/role checks before any element walk.
        work.charge_private_array_work(16)?;
        let fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =
            site
        else {
            return Err(Incomplete(
                "private initializer is not a statement occurrence",
            ));
        };
        let source = context
            .function
            .blocks()
            .get(block.get() as usize)
            .and_then(|block| block.statements().get(statement as usize))
            .ok_or(InvalidSource(
                "private initializer source coordinate is absent",
            ))?;
        let SemanticStatementKindV1::Assign(assignment) = source.kind() else {
            return Err(InvalidSource(
                "private initializer source is not an assignment",
            ));
        };
        let place = assignment.destination();
        let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
            return Err(Incomplete(
                "private initializer requires an Array Aggregate",
            ));
        };
        let local = context
            .function
            .locals()
            .get(place.local().index() as usize)
            .ok_or(InvalidSource("private initializer local is absent"))?;
        if !place.projections().is_empty()
            || local.role().is_entry_argument()
            || place.ty() != local.ty()
            || assignment.value().result_type() != local.ty()
            || aggregate.kind() != &SemanticAggregateKindV1::Array
        {
            return Err(Incomplete(
                "private initializer is not one whole nonargument Array Aggregate",
            ));
        }
        work.charge_private_array_work(2)?;
        let plan = self
            .semantic_ssa
            .plan_for_function(context.function_id)
            .ok_or(Mismatch("selected source SSA plan is absent"))?;
        let promoted = private_array_binary_search_v1(
            plan.plan().promoted_variables(),
            |value| [value.get() as usize],
            [place.local().index() as usize],
            &mut work,
        )?
        .is_ok();
        let rows = &self.correspondence.private_arrays;
        let instance =
            private_array_instance_v1(rows, selected_root, context.function_id, &mut work)?;
        let Some(instance) = instance else {
            return if promoted {
                Ok(None)
            } else {
                Err(Mismatch("retained initializer instance is absent"))
            };
        };
        work.charge_private_array_work(12)?;
        let lowered = self
            .correspondence
            .lowered_functions
            .get(instance.lowered_function_ordinal)
            .ok_or(Mismatch(
                "private initializer correspondence function is absent",
            ))?;
        if instance.owner != selected_root
            || instance.function != expected_body
            || instance.module_function_ordinal != context.root
            || lowered.correspondence_owner != selected_root
            || lowered.semantic_function != expected_body
            || !private_array_equal_bytes_v1(
                lowered.kernel_ir_function.as_str().as_bytes(),
                self.executable.module().functions[context.root]
                    .id
                    .as_str()
                    .as_bytes(),
                &mut work,
            )?
        {
            return Err(Mismatch("private initializer instance coordinates changed"));
        }
        let slots = rows
            .slots
            .get(instance.slot_start..instance.slot_end)
            .ok_or(Mismatch("private initializer slot range changed"))?;
        let effects = rows
            .effects
            .get(instance.effect_start..instance.effect_end)
            .ok_or(Mismatch("private initializer effect range changed"))?;
        let slot_index = private_array_binary_search_v1(
            slots,
            |row| [row.local as usize],
            [place.local().index() as usize],
            &mut work,
        )?;
        let Ok(slot_index) = slot_index else {
            return if promoted {
                Ok(None)
            } else {
                Err(Mismatch("retained initializer slot is absent"))
            };
        };
        work.charge_private_array_work(2)?;
        if promoted {
            return Err(Mismatch(
                "retained initializer contradicts sealed SSA promotion",
            ));
        }
        let slot = &slots[slot_index];
        let facts = private_retained_array_facts_v1(
            context.semantic.types(),
            local.ty(),
            self.limits.max_operations,
            &mut work,
        )?
        .ok_or(Incomplete(
            "private initializer has no exact supported fixed layout",
        ))?;
        work.charge_private_array_work(2)?;
        if u64::try_from(aggregate.operands().len()).ok() != Some(facts.length)
            || !matches!(
                facts.element.element,
                PrivateRetainedElementFactsV1::Scalar(_)
            )
        {
            return Err(Incomplete(
                "private initializer element or count is unsupported",
            ));
        }
        for operand in aggregate.operands() {
            work.charge_private_array_work(5)?;
            let SemanticOperandV1::Constant(constant) = operand else {
                return Err(Incomplete(
                    "private initializer value requires a separate SSA operand relation",
                ));
            };
            let SemanticConstantValueV1::Scalar(bits) = constant.value() else {
                return Err(Incomplete(
                    "private initializer constant is not a literal scalar",
                ));
            };
            if constant.ty() != facts.element_type
                || u64::from(bits.size_bytes()) != facts.element.size
            {
                return Err(Mismatch("private initializer scalar type or width changed"));
            }
        }
        let key = [block.get() as usize, statement as usize];
        let start = private_array_partition_v1(
            effects,
            |row| [row.semantic_block as usize, row.semantic_statement as usize],
            key,
            false,
            &mut work,
        )?;
        let end = private_array_partition_v1(
            effects,
            |row| [row.semantic_block as usize, row.semantic_statement as usize],
            key,
            true,
            &mut work,
        )?;
        work.charge_private_array_work(3)?;
        let effects = effects
            .get(start..end)
            .ok_or(Mismatch("private initializer occurrence range changed"))?;
        if u64::try_from(effects.len()).ok() != Some(facts.length) {
            return Err(Mismatch(
                "private initializer component census is incomplete",
            ));
        }
        let mut previous = None;
        for (component, effect) in effects.iter().enumerate() {
            work.charge_private_array_work(7)?;
            if !matches!(effect.original_index, PrivateArrayIndexV1::InitializerElement { component: actual, .. } if actual as usize == component)
                || effect.local != place.local().index()
                || effect.semantic_block != block.get()
                || effect.semantic_statement != statement
                || previous.is_some_and(|previous| previous >= effect.memory_location.operation)
            {
                return Err(Mismatch(
                    "private initializer components are not exact and ordered",
                ));
            }
            let actual = private_array_exact_relation_v1(
                context.semantic.types(),
                context.function,
                context.body,
                selected_root,
                context.function_id,
                slot,
                effect,
                self.limits.max_operations,
                &mut work,
            )
            .map_err(private_array_query_error_v1)?;
            work.charge_private_array_work(2)?;
            if actual != component as u64 {
                return Err(Mismatch("private initializer component offset changed"));
            }
            previous = Some(effect.memory_location.operation);
        }
        Ok(Some(facts.length))
    }
}
