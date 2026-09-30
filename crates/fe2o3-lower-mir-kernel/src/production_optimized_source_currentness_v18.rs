include!("production_optimized_source_alias_transport_v18.rs");

struct OptimizedSourceMemoryOccurrenceV18 {
    original: usize,
    input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    input_access: u32,
    output: Option<(
        fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        u32,
        ValueId,
    )>,
}

fn optimized_currentness_occurrence_key_v18(
    row: &OptimizedSourceMemoryOccurrenceV18,
) -> [usize; 4] {
    [
        row.input.block.function.0 as usize,
        row.input.block.block as usize,
        row.input.operation as usize,
        row.input_access as usize,
    ]
}

// This is only an actual-output currentness relation. Ranked memory effects,
// scalar/read-from correspondence, and final reference replay remain separate.
pub(super) struct CheckedOptimizedSourceMemoryV18<'scope> {
    original: &'scope CheckedSourceMemoryV29<'scope>,
    optimized: &'scope ProductionOptimizedSourceCorrespondenceV18<'scope>,
    output_memory: &'scope fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'scope, 'scope>,
    accesses: &'scope [OptimizedSourceMemoryOccurrenceV18],
    required: usize,
}

impl CheckedOptimizedSourceMemoryV18<'_> {
    pub(super) fn check_scope_v18(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        original.retain_query((|| {
            self.check(budget)?;
            budget.charge_work(3)?;
            if !std::ptr::eq(original, self.original.correspondence)
                || !std::ptr::eq(optimized, self.optimized)
                || root != self.original.root
            {
                return original
                    .source
                    .missing("retained footprint changed original or optimized owner");
            }
            Ok(())
        })())
    }

    // The complete footprint key is intentional. Value queries still require a
    // whole-value role; CopyObject's roles never collapse when pointers alias.
    pub(super) fn retained_value_footprint_v18(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        instance: usize,
        input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        access: u32,
        pointer: ValueId,
        output: Option<(
            fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
            u32,
            ValueId,
        )>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<bool> {
        original.retain_query((|| {
            self.check_scope_v18(original, optimized, root, budget)?;
            let key = [
                input.block.function.0 as usize,
                input.block.block as usize,
                input.operation as usize,
                access as usize,
            ];
            let ordinal = private_array_partition_v1(
                self.accesses,
                optimized_currentness_occurrence_key_v18,
                key,
                false,
                &mut SourceCorrespondenceWorkV18(budget),
            )?;
            budget.charge_work(1)?;
            let Some(row) = self
                .accesses
                .get(ordinal)
                .filter(|row| optimized_currentness_occurrence_key_v18(row) == key)
            else {
                return Ok(false);
            };
            let pending = self.original.pending.accesses.get(row.original).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "retained footprint original pending index",
                ),
            )?;
            budget.charge_work(4)?;
            if pending.instance.index() != instance
                || row.output != output
                || pending.alternatives.is_empty()
            {
                return original.source.missing(
                    "retained footprint changed source occurrence or lacks activation evidence",
                );
            }
            let (actual, actual_pointer) =
                immutable_memory_access_v29(original, root, pending, budget)?;
            if actual != input
                || actual_pointer != pointer
                || row.input_access != pending.physical.footprint
            {
                return original
                    .source
                    .missing("retained value footprint changed its exact input role");
            }
            let slot = original
                .source
                .root_row(root)?
                .source_slots
                .slots
                .get(pending.physical.slot)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "retained footprint original backing",
                ))?;
            let operation = source_operation_row_v18(original.inventory, input, budget)?.operation;
            match slot.representation {
                ScopedSlotRepresentationV29::ScalarArray(_) => {
                    slot.scalar_array().map_err(immutable_memory_error_v29)?;
                    if !matches!(
                        operation.kind,
                        OperationKind::Load { .. } | OperationKind::Store { .. }
                    ) {
                        return original
                            .source
                            .missing("scalar backing acquired a typed currentness role");
                    }
                }
                ScopedSlotRepresentationV29::Object { .. } => {
                    if !matches!(
                        operation.kind,
                        OperationKind::Storage(
                            ScopedObjectOperationV29::ReadValue { .. }
                                | ScopedObjectOperationV29::WriteValue { .. }
                        )
                    ) {
                        return original
                            .source
                            .missing("object backing lost its typed currentness role");
                    }
                }
            }
            Ok(true)
        })())
    }

    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.original.observe_custody(budget)?;
        if budget.storage() < self.required {
            self.original.correspondence.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.original
            .correspondence
            .retain_query(self.observe_custody(budget))?;
        self.original.check(budget)?;
        optimized_source_endpoints_v18(self.original.correspondence, self.optimized, budget)?;
        if !self
            .output_memory
            .belongs_to(self.optimized.output_inventory(budget)?)
        {
            return self
                .original
                .correspondence
                .source
                .missing("optimized currentness changed actual memory-version owner");
        }
        Ok(())
    }

    pub(super) fn visit_accesses(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        mut consume: impl FnMut(
            usize,
            usize,
            fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
            Option<(fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1, ValueId)>,
            &mut ArgumentBudgetV1<'_>,
        ) -> SourceOwnedResultV18<()>,
    ) -> SourceOwnedResultV18<()> {
        self.original.correspondence.retain_query((|| {
            self.check(budget)?;
            for row in self.accesses {
                budget.charge_work(2)?;
                if row.input_access != 0 || row.output.is_some_and(|(_, access, _)| access != 0) {
                    return self.original.correspondence.source.missing(
                        "multiple footprints cannot use the whole-value currentness visitor",
                    );
                }
                let source = self.original.pending.accesses.get(row.original).ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "optimized currentness lost original access",
                    ),
                )?;
                consume(
                    source.instance.index(),
                    source.anchor,
                    row.input,
                    row.output
                        .map(|(operation, _, pointer)| (operation, pointer)),
                    budget,
                )?;
                self.check(budget)?;
            }
            Ok(())
        })())
    }
}

fn optimized_currentness_memory_error_v18(
    error: fe2o3_kernel_analysis::CanonicalKirMemorySsaErrorV1,
) -> ProductionSourceOwnedViewErrorV18 {
    fe2o3_pliron::CanonicalAnalysisScopeErrorV1::MemorySsa(error).into()
}

fn optimized_source_retained_load_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<(fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1, ValueId)>> {
    let input_row = source_operation_row_v18(relation.inventory, input, budget)?;
    if !matches!(input_row.operation.kind, OperationKind::Load { .. })
        || !matches!(input_row.operation.results.as_slice(), [result] if result.id == value)
    {
        return relation
            .source
            .missing("optimized index changed original Load result");
    }
    let actual = match optimized.operation(input, budget)? {
        ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
        ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => return Ok(None),
        ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
            return relation
                .source
                .missing("optimized index erased an ordered Load");
        }
    };
    let output = optimized.output_inventory(budget)?;
    let row = source_operation_row_v18(output, actual, budget)?;
    let [result] = row.operation.results.as_slice() else {
        return relation.source.missing("optimized index Load result arity");
    };
    if !matches!(row.operation.kind, OperationKind::Load { .. })
        || !optimized_source_value_descends_v18(
            relation,
            optimized,
            input.block.function,
            value,
            actual.block.function,
            result.id,
            budget,
        )?
    {
        return relation
            .source
            .missing("optimized index Load changed its exact descendant");
    }
    Ok(Some((actual, result.id)))
}

// Reuse the original source/read/guard checks, then give the existing history
// solver separate actual-output locators. These copied rows are hypotheses,
// not rebinding of the original source table or its inventory.
fn optimized_source_index_locations_v18(
    original: &CheckedSourceMemoryV29<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(
    Vec<SourceIndexLocationV29>,
    Vec<SourceIndexGuardLocationV29>,
)> {
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Definition,
        CanonicalKirOperationCoordinateV1 as Coordinate, CanonicalKirUseCoordinateV1 as Usage,
    };
    let relation = original.correspondence;
    original.check(budget)?;
    optimized_source_endpoints_v18(relation, optimized, budget)?;
    budget.reserve_storage(argument_sum_v1(&[
        size_of::<(
            Vec<SourceIndexLocationV29>,
            Vec<SourceIndexGuardLocationV29>,
        )>(),
        size_of::<
            SourceOwnedResultV18<(
                Vec<SourceIndexLocationV29>,
                Vec<SourceIndexGuardLocationV29>,
            )>,
        >(),
    ])?)?;
    let input_function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
        u32::try_from(relation.source.root_row(original.root)?.function_ordinal)
            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
    );
    let output_function =
        optimized_source_root_function_v18(relation, optimized, original.root, budget)?.coordinate;
    let output = optimized.output_inventory(budget)?;
    let (input_indices, input_guards) =
        immutable_index_locations_v29(relation, original.root, original.pending, budget)?;
    let mut indices =
        emission_vec_v1(input_indices.len(), budget).map_err(immutable_memory_error_v29)?;
    for input in &input_indices {
        budget.charge_work(3)?;
        let mut source = input.source;
        let input_block = relation
            .inventory
            .block_for_id(input_function, source.block, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized index original block",
            ))?
            .coordinate;
        let at = Coordinate {
            block: input_block,
            operation: u32::try_from(source.operation)
                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
        };
        let original_operation =
            source_operation_row_v18(relation.inventory, at, budget)?.operation;
        if !matches!(original_operation.kind, OperationKind::GetElementPointer { base, offset }
                if base == source.base && offset == source.offset)
            || !matches!(original_operation.results.as_slice(), [result] if result.id == source.pointer)
        {
            return relation
                .source
                .missing("optimized index original pointer recipe");
        }
        let output_at = match optimized.operation(at, budget)? {
            ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
            ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => continue,
            ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                let mut replacement = None;
                for descendant in optimized.definition_descendants(
                    Definition::Result {
                        operation: at,
                        result: 0,
                    },
                    budget,
                )? {
                    budget.charge_work(1)?;
                    let Definition::Result {
                        operation,
                        result: 0,
                    } = descendant.output
                    else {
                        return relation
                            .source
                            .missing("optimized index replacement is not an actual GEP result");
                    };
                    if replacement.replace(operation).is_some() {
                        return relation
                            .source
                            .missing("optimized index has ambiguous executable GEP descendants");
                    }
                }
                replacement.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized index lost its GEP descendant",
                ))?
            }
        };
        if output_at.block.function != output_function {
            return relation
                .source
                .missing("optimized index changed physical function");
        }
        let actual = source_operation_row_v18(output, output_at, budget)?.operation;
        let OperationKind::GetElementPointer { base, offset } = actual.kind else {
            return relation
                .source
                .missing("optimized index output is not an actual GEP");
        };
        let [result] = actual.results.as_slice() else {
            return relation
                .source
                .missing("optimized index output GEP result arity");
        };
        for (old, new) in [
            (source.base, base),
            (source.offset, offset),
            (source.pointer, result.id),
        ] {
            if !optimized_source_value_descends_v18(
                relation,
                optimized,
                input_function,
                old,
                output_function,
                new,
                budget,
            )? {
                return relation
                    .source
                    .missing("optimized index operand/result descendant substitution");
            }
        }
        if matches!(
            optimized.operation(at, budget)?,
            ProductionOptimizedSourceOperationV18::Retained { .. }
        ) {
            for (operand, expected) in [(0, base), (1, offset)] {
                let (_, actual) = optimized_source_actual_operand_v18(
                    relation,
                    optimized,
                    Usage::OperationOperand {
                        operation: at,
                        operand,
                    },
                    output_at,
                    budget,
                )?;
                if actual != expected {
                    return relation
                        .source
                        .missing("optimized index retained operand occurrence substitution");
                }
            }
        }
        let mut value = offset;
        for (kind, scalar) in plan_integer_cast_v1(source.scalar, ScalarType::Index)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized index cast profile",
            ))?
            .into_iter()
            .flatten()
            .rev()
        {
            budget.charge_work(2)?;
            let definition = output
                .definition_for_value(output_function, value, budget)
                .map_err(source_pointer_inventory_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized index cast definition",
                ))?;
            let Definition::Result {
                operation,
                result: 0,
            } = definition.coordinate
            else {
                return relation
                    .source
                    .missing("optimized index cast result coordinate");
            };
            let cast = source_operation_row_v18(output, operation, budget)?.operation;
            let OperationKind::Cast {
                kind: actual,
                value: input,
                ref to,
            } = cast.kind
            else {
                return relation
                    .source
                    .missing("optimized index changed required cast shape");
            };
            if actual != kind || to != &Type::Scalar(scalar) {
                return relation
                    .source
                    .missing("optimized index changed cast semantics");
            }
            value = input;
        }
        if !optimized_source_value_descends_v18(
            relation,
            optimized,
            input_function,
            source.original,
            output_function,
            value,
            budget,
        )? {
            return relation
                .source
                .missing("optimized index changed source value descendant");
        }
        let load = match input.load {
            None => None,
            Some(load) => {
                let (load, result) = optimized_source_retained_load_v18(
                    relation,
                    optimized,
                    load,
                    source.original,
                    budget,
                )?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "reachable optimized index lost its Load",
                ))?;
                if result != value {
                    return relation
                        .source
                        .missing("optimized index uses a different Load result");
                }
                Some(load)
            }
        };
        source.original = value;
        source.base = base;
        source.offset = offset;
        source.pointer = result.id;
        source.block = source_block_row_v18(output, output_at.block, budget)?
            .block
            .id;
        source.operation = output_at.operation as usize;
        indices.push(SourceIndexLocationV29 { source, load });
    }
    private_array_heapsort_v1(
        &mut indices,
        |row| [row.source.pointer.0 as usize],
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    let mut guards =
        emission_vec_v1(input_guards.len(), budget).map_err(immutable_memory_error_v29)?;
    for input in &input_guards {
        budget.charge_work(2)?;
        let mut source = input.source;
        let outcome = optimized.assertion(
            original.root,
            source.instance.index(),
            source.assertion,
            budget,
        )?;
        let (condition, success, failure) = match outcome {
            SemanticKirOptimizedAssertOutcomeV1::Conditional {
                condition,
                success,
                failure,
            } => (condition, success, failure),
            SemanticKirOptimizedAssertOutcomeV1::SelectedSuccess { .. }
            | SemanticKirOptimizedAssertOutcomeV1::SelectedFailure { .. }
            | SemanticKirOptimizedAssertOutcomeV1::RemovedUnreachable { .. } => continue,
            SemanticKirOptimizedAssertOutcomeV1::SourceRuleElision { .. } => {
                return relation
                    .source
                    .missing("original emitted index guard became a source-rule elision");
            }
        };
        let (
            fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Retained(success),
            fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Retained(failure),
        ) = (success, failure)
        else {
            return relation
                .source
                .missing("conditional index guard has nonexecuting edges");
        };
        let Usage::TerminatorOperand { block, operand: 0 } = condition.coordinate else {
            return relation
                .source
                .missing("conditional index guard has a noncondition output use");
        };
        if block.function != output_function
            || success.source != block
            || failure.source != block
            || success.successor != 0
            || failure.successor != 1
        {
            return relation
                .source
                .missing("conditional index guard changed edge polarity");
        }
        let block = source_block_row_v18(output, block, budget)?.block;
        let Some(Terminator::ConditionalBranch {
            condition: value,
            then_target,
            else_target,
            ..
        }) = block.terminator.as_ref()
        else {
            return relation
                .source
                .missing("conditional index guard lost output terminator");
        };
        let condition_definition =
            optimized_source_definition_row_v18(output, condition.definition, budget)?;
        if condition_definition.value != Some(*value)
            || !optimized_source_value_descends_v18(
                relation,
                optimized,
                input_function,
                source.condition,
                output_function,
                *value,
                budget,
            )?
        {
            return relation
                .source
                .missing("conditional index guard changed output condition definition");
        }
        let (load, loaded) = optimized_source_retained_load_v18(
            relation,
            optimized,
            input.load,
            source.value,
            budget,
        )?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "conditional output index guard lost its Load",
        ))?;
        source.value = loaded;
        source.condition = *value;
        source.block = block.id;
        source.success = *then_target;
        source.failure = *else_target;
        guards.push(SourceIndexGuardLocationV29 { source, load });
    }
    let bytes = argument_sum_v1(&[
        argument_product_v1(
            input_indices.capacity(),
            std::mem::size_of::<SourceIndexLocationV29>(),
        )?,
        argument_product_v1(
            input_guards.capacity(),
            std::mem::size_of::<SourceIndexGuardLocationV29>(),
        )?,
        immutable_index_header_v29()?,
    ])?;
    drop((input_indices, input_guards));
    budget.release_storage(bytes)?;
    Ok((indices, guards))
}

fn check_optimized_source_memory_v18(
    original: &CheckedSourceMemoryV29<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    memory: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<OptimizedSourceMemoryOccurrenceV18>> {
    check_optimized_source_memory_equations_v18(original, optimized, memory, budget, |_, _, _| {
        Ok(())
    })
}

fn check_optimized_source_memory_equations_v18(
    original: &CheckedSourceMemoryV29<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    memory: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
    observe: impl FnOnce(
        &OptimizedSourceCurrentnessEquationsV18<'_, '_>,
        &OptimizedSourceAliasTransportV18,
        &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<Vec<OptimizedSourceMemoryOccurrenceV18>> {
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirUseCoordinateV1 as Usage,
    };
    original.check(budget)?;
    budget.reserve_storage(std::mem::size_of_val(&observe))?;
    let relation = original.correspondence;
    optimized_source_endpoints_v18(relation, optimized, budget)?;
    let output = optimized.output_inventory(budget)?;
    if !memory.belongs_to(output) {
        return relation
            .source
            .missing("optimized currentness foreign output memory versions");
    }
    let function_row =
        optimized_source_root_function_v18(relation, optimized, original.root, budget)?;
    let function_coordinate = function_row.coordinate;
    let typed =
        require_optimized_source_typed_roles_v18(relation, optimized, original.root, budget)?;
    let mut physical_typed = OptimizedSourceObjectCensusV18::default();
    let function = function_row.function;
    let pending = original.pending;
    let slots = optimized_source_slots_v18(original, optimized, budget)?;
    let prefix = OptimizedMemoryGapPrefixV18::build(relation, optimized, budget)?;
    let mut occurrences =
        emission_vec_v1(pending.accesses.len(), budget).map_err(immutable_memory_error_v29)?;
    let mut accesses =
        emission_vec_v1(pending.accesses.len(), budget).map_err(immutable_memory_error_v29)?;
    for (ordinal, row) in pending.accesses.iter().enumerate() {
        budget.charge_work(1)?;
        let (input, _) = immutable_memory_access_v29(relation, original.root, row, budget)?;
        let input_operation = source_operation_row_v18(relation.inventory, input, budget)?;
        let input_footprint =
            source_address_footprint_v33(input_operation.operation, row.physical.footprint, budget)
                .map_err(immutable_memory_error_v29)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized currentness input footprint ordinal",
                ))?;
        let is_typed = input_footprint.typed;
        let mapped = match optimized.operation(input, budget)? {
            ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
            ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                if is_typed {
                    physical_typed.unreachable_footprints =
                        argument_sum_v1(&[physical_typed.unreachable_footprints, 1])?;
                }
                occurrences.push(OptimizedSourceMemoryOccurrenceV18 {
                    original: ordinal,
                    input,
                    input_access: row.physical.footprint,
                    output: None,
                });
                continue;
            }
            ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                return relation
                    .source
                    .missing("reachable memory access removed by scalar rewrite");
            }
        };
        if mapped.block.function != function_coordinate {
            return relation
                .source
                .missing("optimized physical access moved to another function");
        }
        let (_, pointer) = optimized_source_actual_operand_v18(
            relation,
            optimized,
            Usage::OperationOperand {
                operation: input,
                operand: input_footprint.operand,
            },
            mapped,
            budget,
        )?;
        let operation = source_operation_row_v18(output, mapped, budget)?.operation;
        let actual = source_address_footprint_v33(operation, row.physical.footprint, budget)
            .map_err(immutable_memory_error_v29)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized currentness output footprint ordinal",
            ))?;
        if actual.pointer != pointer
            || input_footprint.role != actual.role
            || input_footprint.operand != actual.operand
            || input_footprint.access != actual.access
            || is_typed != actual.typed
        {
            return relation
                .source
                .missing("optimized physical access changed actual pointer");
        }
        if is_typed {
            physical_typed.retained_footprints =
                argument_sum_v1(&[physical_typed.retained_footprints, 1])?;
        }
        if memory
            .operation(mapped, budget)
            .map_err(optimized_currentness_memory_error_v18)?
            .is_none()
        {
            return relation
                .source
                .missing("optimized physical access absent from fresh memory versions");
        }
        let block = source_block_row_v18(output, mapped.block, budget)?.block.id;
        accesses.push(SourceAddressAccessV29 {
            footprint: row.physical.footprint,
            block,
            operation: mapped.operation as usize,
            slot: row.physical.slot,
        });
        occurrences.push(OptimizedSourceMemoryOccurrenceV18 {
            original: ordinal,
            input,
            input_access: row.physical.footprint,
            output: Some((mapped, row.physical.footprint, pointer)),
        });
    }
    // Exact input keys are made unique below. Together with the complete typed
    // source/output census, these counts rule out a typed effect omitted from
    // the physical equation system, including unreachable original effects.
    sort_optimized_currentness_occurrences_v18(&mut occurrences, budget)?;
    let mut prior = None;
    for row in &occurrences {
        budget.charge_work(1)?;
        if prior == Some(row.input) {
            continue;
        }
        prior = Some(row.input);
        let operation = source_operation_row_v18(relation.inventory, row.input, budget)?.operation;
        if matches!(operation.kind, OperationKind::Storage(_)) {
            if row.output.is_some() {
                physical_typed.retained = argument_sum_v1(&[physical_typed.retained, 1])?;
            } else {
                physical_typed.unreachable = argument_sum_v1(&[physical_typed.unreachable, 1])?;
            }
        }
    }
    let mut kills =
        emission_vec_v1(pending.kills.len(), budget).map_err(immutable_memory_error_v29)?;
    for row in &pending.kills {
        if let Some((block, gap)) = prefix.boundary(
            relation,
            optimized,
            original.root,
            row.block,
            row.gap,
            budget,
        )? {
            kills.push(SourceAddressKillV29 { block, gap, ..*row });
        }
    }
    let mut lifetimes =
        emission_vec_v1(pending.lifetimes.len(), budget).map_err(immutable_memory_error_v29)?;
    for (source_ordinal, row) in pending.lifetimes.iter().enumerate() {
        if let Some((block, gap)) = prefix.boundary(
            relation,
            optimized,
            original.root,
            row.block,
            row.gap,
            budget,
        )? {
            // A temporary exact source-row locator. The checked segment-order
            // producer replaces it with actual chronological sequence below.
            lifetimes.push(SourceAddressLifetimeV29 {
                block,
                gap,
                sequence: source_ordinal,
                ..*row
            });
        }
    }
    let mut births =
        emission_vec_v1(pending.births.len(), budget).map_err(immutable_memory_error_v29)?;
    for row in &pending.births {
        let input =
            source_input_operation_v18(relation, original.root, row.block, row.operation, budget)?;
        let mapped = match optimized.operation(input, budget)? {
            ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
            ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => continue,
            // The output-use producer below must independently reconstruct
            // this source birth through exact checked rewrite lineage.
            ProductionOptimizedSourceOperationV18::Rewritten { .. } => continue,
        };
        let original_result = optimized_source_definition_row_v18(
            relation.inventory,
            Definition::Result {
                operation: input,
                result: 0,
            },
            budget,
        )?;
        if original_result.value != Some(row.result) {
            return relation
                .source
                .missing("optimized pointer birth changed original result");
        }
        let actual = Definition::Result {
            operation: mapped,
            result: 0,
        };
        let mut found = false;
        for descendant in optimized.definition_descendants(original_result.coordinate, budget)? {
            budget.charge_work(1)?;
            if descendant.output == actual {
                if found {
                    return relation
                        .source
                        .missing("optimized pointer birth repeated result");
                }
                found = true;
            }
        }
        if !found {
            return relation
                .source
                .missing("optimized pointer birth missing descendant");
        }
        let result = optimized_source_definition_row_v18(output, actual, budget)?
            .value
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized pointer birth has no value",
            ))?;
        let block = source_block_row_v18(output, mapped.block, budget)?.block.id;
        births.push(SourceAddressBirthV29 {
            block,
            operation: mapped.operation as usize,
            result,
        });
    }
    let mut failures = emission_vec_v1(pending.index_failures.len(), budget)
        .map_err(immutable_memory_error_v29)?;
    for row in &pending.index_failures {
        if let Some((block, gap)) = prefix.boundary(
            relation,
            optimized,
            original.root,
            row.block,
            row.gap,
            budget,
        )? {
            failures.push(SourceIndexFailureV29 { block, gap, ..*row });
        }
    }
    private_array_heapsort_v1(
        &mut accesses,
        |row| [row.block.0 as usize, row.operation, row.footprint as usize],
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    private_array_heapsort_v1(
        &mut kills,
        |row| [row.block.0 as usize, row.gap, row.slot],
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    // Erased operations and merged segments can collapse repeated clears of
    // the same holder into one actual gap. Origin/currentness/history solvers
    // all interpret Kill as an idempotent clear. The alias producer still joins
    // every original occurrence and proves complete output-key coverage.
    budget.charge_work(kills.len())?;
    kills.dedup_by_key(|row| (row.block, row.gap, row.slot));
    private_array_heapsort_v1(
        &mut lifetimes,
        |row| [row.block.0 as usize, row.gap, row.sequence],
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    private_array_heapsort_v1(
        &mut births,
        |row| [row.block.0 as usize, row.operation],
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    private_array_heapsort_v1(
        &mut failures,
        |row| {
            [
                row.block.0 as usize,
                row.gap,
                row.instance.index(),
                row.anchor,
            ]
        },
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    let prepared = SourceAddressMemoryV29::prepare_inventory(
        output,
        function_coordinate,
        &slots,
        &accesses,
        budget,
    )
    .map_err(immutable_memory_error_v29)?;
    let indexed = source_array_geometry_v29(&slots, !pending.indices.is_empty(), budget)?;
    let (graph, geometry) = if !indexed {
        (
            prepared
                .solve(&slots, &accesses, &kills, budget)
                .map_err(immutable_memory_error_v29)?,
            SourceAddressGeometryV29::Scalar,
        )
    } else {
        let checked = prepared
            .solve_pending_indices(&slots, &accesses, &kills, budget)
            .map_err(immutable_memory_error_v29)?;
        (checked.graph, SourceAddressGeometryV29::PendingIndices)
    };
    let projects = check_optimized_source_projects_v33(original, optimized, &graph, budget)?;
    physical_typed.retained = argument_sum_v1(&[physical_typed.retained, projects.retained])?;
    physical_typed.unreachable =
        argument_sum_v1(&[physical_typed.unreachable, projects.unreachable])?;
    if typed != physical_typed {
        return relation.source.missing(
            "optimized typed effects lack complete physical footprint and Project coverage",
        );
    }
    let transport = optimized_source_alias_transport_v18(
        original,
        optimized,
        &graph,
        &slots,
        &prefix,
        &mut lifetimes,
        &kills,
        budget,
    )?;
    let equations = OptimizedSourceCurrentnessEquationsV18 {
        function,
        graph: &graph,
        slots: &slots,
        accesses: &accesses,
        kills: &kills,
        initial: &pending.initial,
        lifetimes: &lifetimes,
        births: &births,
        failures: &failures,
        geometry,
    };
    // This read-only observer has no admission token. Production supplies a
    // no-op; internal tests can inspect and challenge the generated hypotheses.
    observe(&equations, &transport, budget)?;
    equations
        .check(transport.equations(), budget)
        .map_err(immutable_memory_error_v29)?;
    if !indexed {
        scoped_slot_uses_v29::check_expanded_scalar_addresses_with_failures_v29(
            function, &graph, &slots, &accesses, &kills, &failures, budget,
        )
        .map_err(immutable_memory_error_v29)?;
    } else {
        let (selected, guards) = optimized_source_index_locations_v18(original, optimized, budget)?;
        scoped_slot_uses_v29::check_expanded_index_addresses_v29(
            function,
            &graph,
            &slots,
            &accesses,
            &kills,
            &lifetimes,
            &failures,
            &selected,
            &guards,
            output,
            Some(memory),
            function_coordinate,
            budget,
        )
        .map_err(immutable_memory_error_v29)?;
        drop((selected, guards));
    }
    drop((
        graph, slots, prefix, accesses, kills, lifetimes, births, failures, transport,
    ));
    Ok(occurrences)
}

fn sort_optimized_currentness_occurrences_v18(
    occurrences: &mut [OptimizedSourceMemoryOccurrenceV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    private_array_heapsort_v1(
        occurrences,
        optimized_currentness_occurrence_key_v18,
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    for pair in occurrences.windows(2) {
        budget.charge_work(1)?;
        if optimized_currentness_occurrence_key_v18(&pair[0])
            >= optimized_currentness_occurrence_key_v18(&pair[1])
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized currentness repeats an input footprint",
            ));
        }
    }
    Ok(())
}

fn optimized_currentness_value_footprint_v18(
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(ValueId, bool)> {
    budget.charge_work(1)?;
    match operation.kind {
        OperationKind::Load { pointer, .. } | OperationKind::Store { pointer, .. } => {
            Ok((pointer, false))
        }
        OperationKind::Storage(
            ScopedObjectOperationV29::ReadValue { address, .. }
            | ScopedObjectOperationV29::WriteValue { address, .. },
        ) => Ok((address, true)),
        _ => Err(ProductionSourceOwnedViewErrorV18::Binding(
            "operation lacks one admitted whole-value currentness footprint",
        )),
    }
}

fn optimized_currentness_query_headers_v18() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<[usize; 4]>(),
        size_of::<SourceOwnedResultV18<bool>>(),
        size_of::<SourceOwnedResultV18<(ValueId, bool)>>(),
        size_of::<Result<Option<SourceAddressFootprintV33>, ProductionSemanticKirErrorV1>>(),
        argument_product_v1(2, size_of::<SourceAddressFootprintV33>())?,
        size_of::<Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>>(),
        size_of::<
            Option<(
                fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
                u32,
                ValueId,
            )>,
        >(),
        size_of::<Result<ScopedScalarArraySlotV29, ProductionSemanticKirErrorV1>>(),
    ])
}

#[cfg(test)]
mod optimized_currentness_occurrence_tests {
    use super::*;

    #[test]
    fn whole_value_footprints_keep_typed_address_roles_and_reject_other_storage_effects() {
        let access = MemoryAccess::new(AddressSpace::Private, 4);
        let address = ValueId(3);
        let value = ValueId(7);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        for kind in [
            OperationKind::Load {
                pointer: address,
                access,
            },
            OperationKind::Store {
                pointer: address,
                value,
                access,
            },
            OperationKind::Storage(ScopedObjectOperationV29::ReadValue { address, access }),
            OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                address,
                value,
                access,
            }),
        ] {
            let typed = matches!(kind, OperationKind::Storage(_));
            let operation = Operation {
                results: vec![],
                kind,
            };
            assert_eq!(
                optimized_currentness_value_footprint_v18(&operation, &mut budget).unwrap(),
                (address, typed)
            );
        }
        for kind in [
            ScopedObjectOperationV29::CopyObject {
                source: address,
                destination: address,
                source_access: access,
                destination_access: access,
                overlap: fe2o3_kernel_ir::StorageCopyOverlapV1::MayOverlap,
            },
            ScopedObjectOperationV29::ReadDiscriminant { address, access },
        ] {
            let operation = Operation {
                results: vec![],
                kind: OperationKind::Storage(kind),
            };
            assert!(matches!(
                optimized_currentness_value_footprint_v18(&operation, &mut budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "operation lacks one admitted whole-value currentness footprint"
                ))
            ));
        }
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn complete_footprint_keys_keep_both_copy_roles_with_aliased_pointers() {
        let input = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
            block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(3),
                block: 7,
            },
            operation: 11,
        };
        let pointer = ValueId(19);
        let copy = Operation {
            results: vec![],
            kind: OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::CopyObject {
                source: pointer,
                destination: pointer,
                source_access: MemoryAccess::new(AddressSpace::Private, 8),
                destination_access: MemoryAccess::new(AddressSpace::Private, 8),
                overlap: fe2o3_kernel_ir::StorageCopyOverlapV1::MayOverlap,
            }),
        };
        let mut rows = [
            OptimizedSourceMemoryOccurrenceV18 {
                original: 29,
                input,
                input_access: 1,
                output: Some((input, 1, pointer)),
            },
            OptimizedSourceMemoryOccurrenceV18 {
                original: 23,
                input,
                input_access: 0,
                output: Some((input, 0, pointer)),
            },
        ];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        sort_optimized_currentness_occurrences_v18(&mut rows, &mut budget).unwrap();
        assert_eq!((rows[0].original, rows[1].original), (23, 29));
        for access in 0..2 {
            let key = [3, 7, 11, access];
            let index = private_array_partition_v1(
                &rows,
                optimized_currentness_occurrence_key_v18,
                key,
                false,
                &mut SourceCorrespondenceWorkV18(&mut budget),
            )
            .unwrap();
            assert_eq!(index, access);
            assert_eq!(rows[index].output, Some((input, access as u32, pointer)));
        }
        assert!(optimized_currentness_value_footprint_v18(&copy, &mut budget).is_err());
        let OperationKind::Storage(operation) = &copy.kind else {
            unreachable!()
        };
        let mut count = 0;
        operation
            .try_visit_memory_accesses(|actual_pointer, _, _| {
                assert_eq!(actual_pointer, pointer);
                assert_eq!(rows[count].input_access, count as u32);
                count += 1;
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
        assert_eq!(count, 2);
        rows[0].input_access = 1;
        assert!(matches!(
            sort_optimized_currentness_occurrences_v18(&mut rows, &mut budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized currentness repeats an input footprint"
            ))
        ));
        // This tests the key and refusal boundary only. Neither row above is a
        // checked currentness view, and no typed-source completion is asserted.
    }

    #[test]
    fn currentness_query_header_charge_is_independent_and_precedes_queries() {
        let expected = size_of::<[usize; 4]>()
            + size_of::<SourceOwnedResultV18<bool>>()
            + size_of::<SourceOwnedResultV18<(ValueId, bool)>>()
            + size_of::<Result<Option<SourceAddressFootprintV33>, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceAddressFootprintV33>()
            + size_of::<Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>>()
            + size_of::<
                Option<(
                    fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
                    u32,
                    ValueId,
                )>,
            >()
            + size_of::<Result<ScopedScalarArraySlotV29, ProductionSemanticKirErrorV1>>();
        assert_eq!(optimized_currentness_query_headers_v18().unwrap(), expected);
        for limit in [expected, expected - 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget = ArgumentBudgetV1::new(&mut work, limit);
            let result = budget.reserve_storage(optimized_currentness_query_headers_v18().unwrap());
            assert_eq!(result.is_ok(), limit == expected);
            if result.is_err() {
                assert_eq!(budget.failed_storage(), Some(expected));
                assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                    if (error.actual(), error.limit()) == (expected, limit)));
            }
            assert_eq!(budget.work(), 0);
        }
    }
}

pub(super) fn with_checked_optimized_source_memory_v18<'work, T, E>(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    input_memory: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    output_memory: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl for<'scope> FnOnce(
        &CheckedOptimizedSourceMemoryV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<ProductionSourceOwnedViewErrorV18>,
{
    optimized_source_endpoints_v18(relation, optimized, budget)?;
    with_checked_source_memory_v29(
        relation,
        root,
        Some(input_memory),
        budget,
        |original, budget| {
            let floor = budget.storage();
            scoped_source_attempt_v29(relation.source.cleanup, budget, floor, |budget| {
                let floor = budget.storage();
                let accesses = relation.source.retain_construction(|| {
                    budget.charge_work(1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29)?;
                    budget.reserve_storage(argument_sum_v1(&[
                        size_of::<CheckedOptimizedSourceMemoryV18<'_>>(),
                        size_of::<std::thread::Result<Result<T, E>>>(),
                        argument_product_v1(9, size_of::<Vec<usize>>())?,
                        argument_product_v1(3, size_of::<OptimizedSourceObjectCensusV18>())?,
                        optimized_currentness_query_headers_v18()?,
                        physical_discard_headers_v29::<T, E>()?,
                    ])?)?;
                    check_optimized_source_memory_v18(original, optimized, output_memory, budget)
                })?;
                let retained = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let view = CheckedOptimizedSourceMemoryV18 {
                    original,
                    optimized,
                    output_memory,
                    accesses: &accesses,
                    required: budget.storage(),
                };
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    consume(&view, budget)
                }));
                let first = relation.source.guard.first.get();
                let postflight = if matches!(&caught, Ok(Ok(_))) {
                    view.check(budget)
                } else {
                    view.observe_custody(budget)
                };
                drop(view);
                drop(accesses);
                let value = match caught {
                    Err(payload) => std::panic::resume_unwind(payload),
                    Ok(Err(error)) => {
                        let selected = match first {
                            Some(first) => {
                                source_reference_discard_v29(Err::<T, E>(error));
                                first.error().into()
                            }
                            None => error,
                        };
                        return Err(SourceConsumerErrorV18(selected));
                    }
                    Ok(Ok(value)) => match postflight {
                        Ok(()) => value,
                        Err(error) => {
                            source_reference_discard_v29(Ok::<T, E>(value));
                            return Err(SourceConsumerErrorV18(error.into()));
                        }
                    },
                };
                if let Err(error) = relation
                    .source
                    .retain_construction(|| budget.release_storage(retained).map_err(Into::into))
                {
                    relation.source.cleanup.deny_refund();
                    source_reference_discard_v29(Ok::<T, E>(value));
                    return Err(SourceConsumerErrorV18(error.into()));
                }
                Ok(value)
            })
            .map_err(|error: SourceConsumerErrorV18<E>| error.0)
        },
    )
}
