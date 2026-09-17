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
