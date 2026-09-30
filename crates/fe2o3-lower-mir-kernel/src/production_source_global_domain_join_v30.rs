// Concrete endpoint checks shared by the existing scalar source route and
// full-chain callers. Callers must authenticate the source/endpoint relation and
// prepay the existing read/store domain frames plus the join frame below. These
// private helpers never construct a domain or confer source/refinement authority.
#[cfg(test)]
include!("production_source_global_domain_join_v30_tests.rs");
#[cfg(test)]
include!("production_source_global_domain_width_commands_v30_tests.rs");

fn source_global_domain_join_headers_v30() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a GlobalReadFactsV18<'a, 'a>,
        &'a GlobalReadFactV18<'a, 'a>,
        &'a GlobalStoreFactsV25<'a, 'a>,
        &'a GlobalStoreFactV25<'a, 'a>,
        &'a GlobalSourceAccessPairV18,
        &'a mut ArgumentBudgetV1<'a>,
        &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        Result<
            &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
        SourceOwnedResultV18<()>,
        Result<(), PendingGlobalReadConditionErrorV18>,
    );
    argument_sum_v1(&[
        argument_product_v1(2, size_of::<Frame<'_>>())?,
        argument_product_v1(2, std::mem::align_of::<Frame<'_>>())?,
    ])
}

fn source_domain_formal_error_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    error: fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
) -> PendingGlobalReadConditionErrorV18 {
    let source_refusal = optimized_source_observed_formal_error_v18(original, &error);
    PendingGlobalReadConditionErrorV18::Formal {
        error,
        source_refusal,
    }
}

fn check_source_read_endpoint_prepaid_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    facts: &GlobalReadFactsV18<'_, '_>,
    pair: &GlobalSourceAccessPairV18,
    fact: &GlobalReadFactV18<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), PendingGlobalReadConditionErrorV18> {
    original.check(budget)?;
    let owner = facts
        .owner(budget)
        .map_err(|error| source_domain_formal_error_v30(original, error))?;
    if !std::ptr::eq(owner, inventory.owner()) {
        return original
            .source
            .missing("pending global read formal scope differs")
            .map_err(Into::into);
    }
    original.retain_query((|| {
        budget.charge_work(96 + 4)?;
        let endpoint = &pair.output;
        let domain = *fact.domain();
        let root = optimized_source_definition_row_v18(inventory, endpoint.logical.root, budget)?;
        let index = optimized_source_definition_row_v18(inventory, endpoint.logical.index, budget)?;
        let address_index = optimized_source_definition_row_v18(inventory, endpoint.address_index, budget)?;
        let condition = optimized_source_definition_row_v18(inventory, endpoint.logical.guard_condition, budget)?;
        let guard = source_block_row_v18(inventory, endpoint.logical.guard_edge.source, budget)?;
        budget.charge_work(32)?;
        // This only rejoins actual control transport. Truth and the exact
        // taken edge still come from the independent formal fact below.
        let predicate = if condition.ty == &Type::BOOL {
            if !matches!(guard.block.terminator.as_ref(), Some(Terminator::ConditionalBranch {
                condition: actual, ..
            }) if condition.value == Some(*actual)) {
                return original.source.missing("pending global read changed exact control transport");
            }
            condition
        } else {
            if !matches!(condition.ty, Type::Scalar(ScalarType::U8 | ScalarType::U16 | ScalarType::U32 | ScalarType::U64 | ScalarType::I8 | ScalarType::I16 | ScalarType::I32 | ScalarType::I64))
                || !matches!(guard.block.terminator.as_ref(), Some(Terminator::Switch {
                    selector, ..
                }) if condition.value == Some(*selector)) {
                return original.source.missing("pending global read changed exact control transport");
            }
            let SliceDefinition::Result { operation: cast, result: 0 } = condition.coordinate else {
                return original.source.missing("pending global read changed exact control transport");
            };
            let cast = source_operation_row_v18(inventory, cast, budget)?.operation;
            if !matches!(&cast.kind, OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::ZeroExtend, value, to,
            } if *value == domain.predicate() && to == condition.ty)
                || !matches!(cast.results.as_slice(), [result]
                    if Some(result.id) == condition.value && &result.ty == condition.ty) {
                return original.source.missing("pending global read changed exact control transport");
            }
            inventory.definition_for_value(fact.operation().block.function, domain.predicate(), budget)
                .map_err(source_pointer_inventory_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("pending global read predicate definition absent"))?
        };
        let SliceDefinition::Result { operation: comparison, result: 0 } = predicate.coordinate else {
            return original.source.missing("pending global read changed exact local domain");
        };
        let comparison = source_operation_row_v18(inventory, comparison, budget)?.operation;
        let normalized_index = fact.normalized_index_origin();
        let (comparison_index, comparison_length) = fact.comparison_operands();
        let normalized_matches = match normalized_index {
            fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1::ProvenOrigin(value) =>
                index.value == Some(value),
            fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1::ExactBlockParameter(value) =>
                index.value == Some(value)
                    && matches!(index.coordinate, SliceDefinition::BlockArgument { .. }),
        };
        let definition_function = |coordinate| match coordinate {
            SliceDefinition::FunctionArgument { function, .. } => function,
            SliceDefinition::BlockArgument { block, .. } => block.function,
            SliceDefinition::Result { operation, .. } => operation.block.function,
        };
        let length = source_operation_row_v18(inventory, endpoint.logical.length, budget)?.operation;
        let edge_index = guard.edges.start.checked_add(endpoint.logical.guard_edge.successor as usize)
            .filter(|index| *index < guard.edges.end).ok_or(ArgumentResourceV1::Arithmetic)?;
        let edge = inventory.edges().get(edge_index).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("pending global read guard edge absent"))?;
        let Some(bits) = endpoint.scalar.bit_width() else {
            return original.source.missing("pending global read scalar byte width");
        };
        let [length_value] = length.results.as_slice() else {
            return original.source.missing("pending global read length result");
        };
        if pair.output.writing || endpoint.memory.volatile
            || !global_source_memory_space_v26(&pair.origin, endpoint.memory.address_space)
            || !std::ptr::eq(fact.owner(), inventory.owner())
            || fact.operation() != endpoint.logical.access.operation
            || root.coordinate != (SliceDefinition::FunctionArgument {
                function: fact.operation().block.function,
                argument: domain.allocation().parameter_index(),
            })
            || root.value != Some(domain.slice()) || !normalized_matches
            || !matches!(index.ty, Type::Scalar(ScalarType::Index | ScalarType::U64))
            || address_index.value != Some(domain.index()) || address_index.ty != &Type::INDEX
            || definition_function(index.coordinate) != fact.operation().block.function
            || definition_function(address_index.coordinate) != fact.operation().block.function
            || definition_function(condition.coordinate) != fact.operation().block.function
            || definition_function(predicate.coordinate) != fact.operation().block.function
            || endpoint.logical.length.block.function != fact.operation().block.function
            || endpoint.logical.guard_edge.source.function != fact.operation().block.function
            || length_value.id != fact.normalized_length_origin() || length_value.ty != Type::INDEX
            || predicate.value != Some(domain.predicate()) || predicate.ty != &Type::BOOL
            || !matches!(&comparison.kind, OperationKind::Compare {
                predicate: fe2o3_kernel_ir::ComparePredicate::LessThan, lhs, rhs,
            } if (*lhs, *rhs) == (comparison_index, comparison_length))
            || !matches!(comparison.results.as_slice(), [result]
                if Some(result.id) == predicate.value && result.ty == Type::BOOL)
            || !matches!(root.ty, Type::Slice(slice)
                if slice.address_space == AddressSpace::Global && *slice.element == Type::Scalar(endpoint.scalar))
            || domain.pointer() != endpoint.pointer
            || bits % 8 != 0 || !matches!(bits / 8, 1 | 2 | 4 | 8)
            || domain.element_bytes() != u64::from(bits / 8)
            || !endpoint.memory.alignment.is_power_of_two()
            || u64::from(endpoint.memory.alignment) > domain.element_bytes()
            || edge.coordinate != endpoint.logical.guard_edge
            || domain.path() != (fe2o3_kernel_ir::FormalGuardedPathV1::TrueEdge {
                source: guard.block.id,
                ordinal: endpoint.logical.guard_edge.successor as usize,
                target: edge.target_id,
            })
        {
            return original.source.missing("pending global read changed exact local domain");
        }
        Ok(())
    })())?;
    let predicate = facts
        .true_at(fact.operation(), fact.domain().predicate(), budget)
        .map_err(|error| source_domain_formal_error_v30(original, error))?;
    original
        .retain_query((|| {
            budget.charge_work(8)?;
            let domain = *fact.domain();
            if !predicate.as_ref().is_some_and(|predicate| {
                std::ptr::eq(predicate.owner(), inventory.owner())
                    && predicate.at() == fact.operation()
                    && predicate.is_true()
                    && predicate.value() == domain.predicate()
                    && domain.path()
                        == (fe2o3_kernel_ir::FormalGuardedPathV1::TrueEdge {
                            source: predicate.edge().0,
                            ordinal: predicate.edge().1,
                            target: predicate.edge().2,
                        })
            }) {
                return original
                    .source
                    .missing("pending global read changed actual guard truth");
            }
            Ok(())
        })())
        .map_err(Into::into)
}

fn check_source_store_endpoint_prepaid_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    facts: &GlobalStoreFactsV25<'_, '_>,
    pair: &GlobalSourceAccessPairV18,
    fact: &GlobalStoreFactV25<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.check(budget)?;
    original.retain_query((|| {
        let owner = facts.owner(budget)
            .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?;
        if !std::ptr::eq(owner, inventory.owner()) || !std::ptr::eq(fact.owner(), inventory.owner()) {
            return original.source.missing("slice Store fact owner differs");
        }
        budget.charge_work(160 + 4)?;
        let endpoint = &pair.output;
        let domain = fact.domain();
        let root =
            optimized_source_definition_row_v18(inventory, endpoint.logical.root, budget)?;
        let index =
            optimized_source_definition_row_v18(inventory, endpoint.logical.index, budget)?;
        let address_index =
            optimized_source_definition_row_v18(inventory, endpoint.address_index, budget)?;
        let condition = optimized_source_definition_row_v18(
            inventory,
            endpoint.logical.guard_condition,
            budget,
        )?;
        let guard =
            source_block_row_v18(inventory, endpoint.logical.guard_edge.source, budget)?;
        let length =
            source_operation_row_v18(inventory, endpoint.logical.length, budget)?.operation;
        let predicate = if condition.ty == &Type::BOOL {
            if !matches!(guard.block.terminator.as_ref(), Some(Terminator::ConditionalBranch { condition: actual, .. }) if condition.value == Some(*actual))
            {
                return original
                    .source
                    .missing("slice Store guard transport differs");
            }
            condition
        } else {
            if !matches!(condition.ty, Type::Scalar(ScalarType::U8 | ScalarType::U16 | ScalarType::U32 | ScalarType::U64 | ScalarType::I8 | ScalarType::I16 | ScalarType::I32 | ScalarType::I64))
                || !matches!(guard.block.terminator.as_ref(), Some(Terminator::Switch { selector, .. }) if condition.value == Some(*selector))
            {
                return original
                    .source
                    .missing("slice Store guard transport differs");
            }
            let SliceDefinition::Result {
                operation: cast,
                result: 0,
            } = condition.coordinate
            else {
                return original
                    .source
                    .missing("slice Store guard representation differs");
            };
            let cast = source_operation_row_v18(inventory, cast, budget)?.operation;
            if !matches!(&cast.kind, OperationKind::Cast { kind: fe2o3_kernel_ir::CastKind::ZeroExtend, value, to }
                if *value == domain.predicate() && to == condition.ty)
                || !matches!(cast.results.as_slice(), [result] if Some(result.id) == condition.value && &result.ty == condition.ty)
            {
                return original
                    .source
                    .missing("slice Store guard representation differs");
            }
            inventory
                .definition_for_value(
                    fact.operation().block.function,
                    domain.predicate(),
                    budget,
                )
                .map_err(source_pointer_inventory_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "slice Store predicate definition absent",
                ))?
        };
        let SliceDefinition::Result {
            operation: comparison,
            result: 0,
        } = predicate.coordinate
        else {
            return original.source.missing("slice Store comparison is absent");
        };
        let comparison = source_operation_row_v18(inventory, comparison, budget)?.operation;
        let normalized_matches = match fact.normalized_index_origin() {
            fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1::ProvenOrigin(value) => {
                index.value == Some(value)
            }
            fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1::ExactBlockParameter(value) => {
                index.value == Some(value)
                    && matches!(index.coordinate, SliceDefinition::BlockArgument { .. })
            }
        };
        let edge_index = guard
            .edges
            .start
            .checked_add(endpoint.logical.guard_edge.successor as usize)
            .filter(|index| *index < guard.edges.end)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let edge = inventory.edges().get(edge_index).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("slice Store guard edge absent"),
        )?;
        let Some(bits) = endpoint.scalar.bit_width() else {
            return original
                .source
                .missing("slice Store scalar width is unknown");
        };
        let [length_value] = length.results.as_slice() else {
            return original
                .source
                .missing("slice Store length occurrence differs");
        };
        let definition_function = |coordinate| match coordinate {
            SliceDefinition::FunctionArgument { function, .. } => function,
            SliceDefinition::BlockArgument { block, .. } => block.function,
            SliceDefinition::Result { operation, .. } => operation.block.function,
        };
        if !endpoint.writing
            || !pair.input.writing
            || endpoint.memory.volatile
            || !global_source_memory_space_v26(&pair.origin, endpoint.memory.address_space)
            || fact.operation() != endpoint.logical.access.operation
            || root.coordinate
                != (SliceDefinition::FunctionArgument {
                    function: fact.operation().block.function,
                    argument: domain.allocation().parameter_index(),
                })
            || root.value != Some(domain.slice())
            || !normalized_matches
            || !matches!(index.ty, Type::Scalar(ScalarType::Index | ScalarType::U64))
            || address_index.value != Some(domain.index())
            || address_index.ty != &Type::INDEX
            || definition_function(index.coordinate) != fact.operation().block.function
            || definition_function(address_index.coordinate) != fact.operation().block.function
            || definition_function(condition.coordinate) != fact.operation().block.function
            || definition_function(predicate.coordinate) != fact.operation().block.function
            || endpoint.logical.length.block.function != fact.operation().block.function
            || endpoint.logical.guard_edge.source.function != fact.operation().block.function
            || length_value.id != fact.normalized_length_origin()
            || length_value.ty != Type::INDEX
            || predicate.value != Some(domain.predicate())
            || predicate.ty != &Type::BOOL
            || !matches!(&comparison.kind, OperationKind::Compare { predicate: fe2o3_kernel_ir::ComparePredicate::LessThan, lhs, rhs } if (*lhs, *rhs) == fact.comparison_operands())
            || !matches!(comparison.results.as_slice(), [result] if Some(result.id) == predicate.value && result.ty == Type::BOOL)
            || !matches!(root.ty, Type::Slice(slice) if slice.address_space == AddressSpace::Global && *slice.element == Type::Scalar(endpoint.scalar))
            || domain.pointer() != endpoint.pointer
            || bits % 8 != 0
            || !matches!(bits / 8, 1 | 2 | 4 | 8)
            || domain.element_bytes() != u64::from(bits / 8)
            || !endpoint.memory.alignment.is_power_of_two()
            || u64::from(endpoint.memory.alignment) > domain.element_bytes()
            || edge.coordinate != endpoint.logical.guard_edge
            || domain.path()
                != (fe2o3_kernel_ir::FormalGuardedPathV1::TrueEdge {
                    source: guard.block.id,
                    ordinal: endpoint.logical.guard_edge.successor as usize,
                    target: edge.target_id,
                })
        {
            return original.source.missing("slice Store exact domain differs");
        }
        let truth = facts
            .true_at(fact.operation(), domain.predicate(), budget)
            .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?;
        budget.charge_work(8)?;
        if !truth.as_ref().is_some_and(|truth| {
            std::ptr::eq(truth.owner(), inventory.owner())
                && truth.at() == fact.operation()
                && truth.is_true()
                && truth.value() == domain.predicate()
                && domain.path()
                    == (fe2o3_kernel_ir::FormalGuardedPathV1::TrueEdge {
                        source: truth.edge().0,
                        ordinal: truth.edge().1,
                        target: truth.edge().2,
                    })
        }) {
            return original
                .source
                .missing("slice Store exact guard truth differs");
        }
        Ok(())
    })())
}
