type GlobalStoreFactsV25<'s, 'g> = fe2o3_kernel_ir::CheckedCanonicalGuardedGlobalStoresV24<'s, 'g>;
type GlobalStoreFactV25<'s, 'g> = fe2o3_kernel_ir::CanonicalGuardedGlobalStoreFactV24<'s, 'g>;

fn slice_store_domain_headers_v25() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_analysis::{
        CanonicalKirBlockRefV1, CanonicalKirEdgeRefV1, CanonicalKirInventoryV18,
        CanonicalKirOperationRefV1,
    };
    type Frame<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a GlobalStoreFactsV25<'a, 'a>,
        &'a GlobalStoreFactV25<'a, 'a>,
        &'a GlobalSourceAccessPairV18,
        &'a GlobalSourceAccessEndpointV18,
        &'a CanonicalKirInventoryV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        fe2o3_kernel_ir::CanonicalGuardedGlobalStoreDomainV24,
        [&'a CanonicalKirDefinitionRefV1<'a>; 7],
        [&'a CanonicalKirOperationRefV1<'a>; 3],
        &'a CanonicalKirBlockRefV1<'a>,
        &'a CanonicalKirEdgeRefV1<'a>,
        fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1,
        Option<fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'a, 'a>>,
        fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'a, 'a>,
        [ValueId; 4],
        [usize; 3],
        u16,
        bool,
        SourceOwnedResultV18<()>,
        fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Frame<'_>>>())?,
    ])
}

impl PendingGlobalSourceAccessesV18<'_> {
    // This rejoins a genuine Store-only fact. No Store is converted to a read
    // fact, and the legacy read entrance and its accepted grammar are unchanged.
    fn check_local_store_domain_v25(
        &self,
        facts: &GlobalStoreFactsV25<'_, '_>,
        pair: &GlobalSourceAccessPairV18,
        fact: &GlobalStoreFactV25<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let original = self.original();
        original.global_expression_entry_v23(self.optimized(), budget)?;
        original.retain_query(self.roles.observe_custody(budget))?;
        let inventory = self
            .optimized()
            .pending_global_output_v18(original, budget)?;
        original.retain_query((|| {
            if !std::ptr::eq(fact.owner(), inventory.owner()) {
                return original.source.missing("slice Store fact owner differs");
            }
            Ok(())
        })())?;
        let run = |budget: &mut ArgumentBudgetV1<'_>| -> SourceOwnedResultV18<()> {
            budget.reserve_storage(slice_store_domain_headers_v25()?)?;
            let owner = facts
                .owner(budget)
                .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?;
            if !std::ptr::eq(owner, inventory.owner())
                || self.access(pair.output.logical.access.operation, budget)? != Some(pair)
            {
                return original
                    .source
                    .missing("slice Store source or formal scope differs");
            }
            budget.charge_work(160)?;
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
                if condition.ty != &Type::Scalar(ScalarType::U32)
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
                    if *value == domain.predicate() && to == &Type::Scalar(ScalarType::U32))
                    || !matches!(cast.results.as_slice(), [result] if Some(result.id) == condition.value && result.ty == Type::Scalar(ScalarType::U32))
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
                || endpoint.memory.address_space != AddressSpace::Global
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
        };
        let frame = argument_sum_v1(&[std::mem::size_of_val(&run), std::mem::align_of_val(&run)])?;
        original.retain_query(source_scalar_normalization_scratch_v18(
            original.source.cleanup,
            budget,
            frame,
            run,
        ))
    }
}

include!("production_optimized_source_slice_completion_v25.rs");
