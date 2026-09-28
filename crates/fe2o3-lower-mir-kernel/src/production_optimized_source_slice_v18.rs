include!("production_optimized_source_slice_query_v18.rs");

/// A distinct actual-output slice view retaining its original argument view.
/// This proves neither initializedness nor final reference/launch permission.
pub struct ProductionOptimizedSliceAccessViewV18<'scope> {
    original: &'scope ProductionSliceAccessViewV1<'scope>,
    facts: &'scope OptimizedSliceFactsV18<'scope>,
}

impl ProductionOptimizedSliceAccessViewV18<'_> {
    /// Borrows the still-live original slice view whose root argument and
    /// source identity were joined to this actual-output access.
    pub fn original(&self) -> &ProductionSliceAccessViewV1<'_> {
        self.original
    }
    /// Returns the exact output Load occurrence and its memory-effect ordinal.
    pub const fn access(&self) -> SliceAccess {
        self.facts.access
    }
    /// Returns the checked output definition carrying this slice's data pointer.
    pub const fn data_carrier(&self) -> SliceDefinition {
        self.facts.data_carrier
    }
    /// Returns the checked output definition carrying this slice's length.
    pub const fn length_carrier(&self) -> SliceDefinition {
        self.facts.length_carrier
    }
    /// Returns the exact output root-argument definition corresponding to the
    /// unchanged original slice argument slot.
    pub const fn input(&self) -> SliceDefinition {
        self.facts.input
    }
    /// Returns the actual output index definition selected by this access.
    pub const fn index(&self) -> SliceDefinition {
        self.facts.index
    }
    /// Returns the output access payload, checked against the original payload;
    /// these attributes alone do not establish initializedness or currentness.
    pub const fn memory(&self) -> MemoryAccess {
        self.facts.memory
    }
    /// Borrows the loaded scalar type from the immutable output operation.
    pub fn loaded_type(&self) -> &Type {
        self.facts.loaded_type
    }
}

#[derive(Clone, Copy)]
struct OptimizedSliceOriginalV18 {
    access: SliceAccess,
    data: SliceDefinition,
    length: SliceDefinition,
    input: SliceDefinition,
    index: SliceDefinition,
    memory: MemoryAccess,
    scalar: ScalarType,
    local: Option<SemanticLocalIdV1>,
}

fn optimized_slice_original_v18(
    view: &ProductionSliceAccessViewV1<'_>,
    local: Option<SemanticLocalIdV1>,
) -> SliceResult<OptimizedSliceOriginalV18> {
    Ok(OptimizedSliceOriginalV18 {
        access: view.access(),
        data: view.data_carrier(),
        length: view.length_carrier(),
        input: view.input(),
        index: view.index(),
        memory: view.memory(),
        scalar: view
            .loaded_type()
            .as_scalar()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
        local,
    })
}

fn optimized_slice_descendant_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: SliceDefinition,
    output: SliceDefinition,
    budget: &mut SliceBudget<'_>,
) -> SourceOwnedResultV18<()> {
    let mut matched = false;
    for row in optimized.definition_descendants(input, budget)? {
        budget.charge_work(1)?;
        if row.output == output {
            if matched {
                return relation
                    .source
                    .missing("optimized slice repeated definition descendant");
            }
            matched = true;
        }
    }
    if !matched {
        return relation
            .source
            .missing("optimized slice changed actual carrier or index");
    }
    Ok(())
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Lends a distinct slice view after rechecking original argument custody,
    /// exact output Load/carrier/index transport, and the supported assertion
    /// profile. Ambiguous descendants or unsupported selected/elided guards
    /// refuse rather than inheriting the original proof. Final memory,
    /// reference, and launch obligations remain the joint consumer's duty.
    pub fn with_optimized_checked_slice_access_v18<R>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        instance: usize,
        site: ProductionSliceAccessSiteV1,
        budget: &mut SliceBudget<'_>,
        use_view: impl for<'scope> FnOnce(
            &ProductionOptimizedSliceAccessViewV18<'scope>,
            Option<SemanticLocalIdV1>,
        ) -> SliceResult<R>,
    ) -> SourceOwnedResultV18<R> {
        self.with_optimized_descriptor_access_v18(
            optimized, root, instance, site, false, budget, use_view,
        )
    }

    fn with_optimized_descriptor_access_v18<R>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        instance: usize,
        site: ProductionSliceAccessSiteV1,
        write: bool,
        budget: &mut SliceBudget<'_>,
        use_view: impl for<'scope> FnOnce(
            &ProductionOptimizedSliceAccessViewV18<'scope>,
            Option<SemanticLocalIdV1>,
        ) -> SliceResult<R>,
    ) -> SourceOwnedResultV18<R> {
        optimized_source_endpoints_v18(self, optimized, budget)?;
        let floor = budget.storage();
        scoped_source_attempt_v29(self.source.cleanup, budget, floor, |budget| {
            let floor = budget.storage();
            self.retain_query((|| {
                budget.charge_work(1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29)?;
                budget.reserve_storage(argument_sum_v1(&[
                    size_of::<OptimizedSliceOriginalV18>(),
                    size_of::<ProductionOptimizedSliceAccessViewV18<'_>>(),
                    size_of::<OptimizedSliceFactsV18<'_>>(),
                    size_of::<Vec<SliceOperation>>(),
                    size_of::<
                        OptimizedSliceQueryV18<'_, '_, fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18>,
                    >(),
                    size_of::<Result<R, ProductionSourceOwnedViewErrorV18>>(),
                    source_reference_cleanup_headers_v29()?,
                ])?)?;
                Ok(())
            })())?;
            let original = self.with_descriptor_access_v18(
                root,
                instance,
                site,
                write,
                budget,
                optimized_slice_original_v18,
            )?;
            let output = optimized.output_inventory(budget)?;
            let function =
                optimized_source_root_function_v18(self, optimized, root, budget)?;
            let output_function = function.coordinate;
            let original_root = self.source.root_row(root)?;
            let mut retained = emission_vec_v1(original_root.source_slots.slots.len(), budget)
                .map_err(source_emission_error_v18)?;
            for operation in &self.inventory.operations()[self.inventory.functions()
                [original_root.function_ordinal]
                .operations
                .clone()]
            {
                budget.charge_work(1)?;
                if !matches!(operation.operation.kind, OperationKind::Alloca { .. }) {
                    continue;
                }
                if !self.retained_scalar_allocation(root, operation.coordinate, budget)? {
                    continue;
                }
                match optimized.operation(operation.coordinate, budget)? {
                    ProductionOptimizedSourceOperationV18::Retained { output, .. } => {
                        if retained.len() == retained.capacity() {
                            return Err(SourceConsumerErrorV18(ProductionSourceOwnedViewErrorV18::Binding(
                                "optimized slice retained allocation census exceeded original slots",
                            )));
                        }
                        retained.push(output)
                    }
                    ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {}
                    ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                        return Err(SourceConsumerErrorV18(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "optimized slice lost retained allocation",
                            ),
                        ));
                    }
                }
            }
            private_array_heapsort_v1(
                &mut retained,
                |row| {
                    [
                        row.block.function.0 as usize,
                        row.block.block as usize,
                        row.operation as usize,
                    ]
                },
                &mut SourceCorrespondenceWorkV18(budget),
                || ArgumentResourceV1::Arithmetic.into(),
            )?;
            for pair in retained.windows(2) {
                budget.charge_work(1)?;
                if pair[0] == pair[1] {
                    return Err(SourceConsumerErrorV18(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "optimized slice merged retained allocations",
                        ),
                    ));
                }
            }
            let facts = super::value_origin_v1::with_optimized_whole_value_origins_v18(
                self,
                optimized,
                output,
                output_function,
                budget,
                |origins, budget| {
                    OptimizedSliceQueryV18 {
                        owner: OptimizedSliceOwnerV18::Optimized {
                            relation: self,
                            optimized,
                            root,
                            instance,
                            retained_allocations: &retained,
                        },
                        inventory: output,
                        function,
                        site,
                        origins,
                    }
                    .descriptor_facts(write, budget)
                    .map_err(source_emission_error_v18)
                },
            )?;
            self.retain_query((|| {
                let ProductionOptimizedSourceOperationV18::Retained { output: actual, .. } =
                    optimized.operation(original.access.operation, budget)?
                else {
                    return self
                        .source
                        .missing("optimized slice read has no retained occurrence");
                };
                if facts.access.operation != actual
                    || facts.access.effect != original.access.effect
                    || facts.memory != original.memory
                    || facts.loaded_type.as_scalar() != Some(original.scalar)
                {
                    return self
                        .source
                        .missing("optimized slice changed actual read or access payload");
                }
                optimized_source_actual_operand_v18(
                    self,
                    optimized,
                    fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                        operation: original.access.operation,
                        operand: 0,
                    },
                    actual,
                    budget,
                )?;
                for (input, output) in [
                    (original.input, facts.input),
                    (original.data, facts.data_carrier),
                    (original.length, facts.length_carrier),
                    (original.index, facts.index),
                ] {
                    optimized_slice_descendant_v18(self, optimized, input, output, budget)?;
                }
                let (
                    SliceDefinition::FunctionArgument {
                        argument: before, ..
                    },
                    SliceDefinition::FunctionArgument {
                        function: actual,
                        argument: after,
                    },
                ) = (original.input, facts.input)
                else {
                    return self
                        .source
                        .missing("optimized slice entry transport is not a root argument");
                };
                if before != after || actual != output_function {
                    return self
                        .source
                        .missing("optimized slice changed exact root argument");
                }
                Ok(())
            })())?;
            let retained_bytes = budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let result =
                self.with_descriptor_access_v18(root, instance, site, write, budget, |view, local| {
                    let checked = optimized_slice_original_v18(view, local)?;
                    if checked.access != original.access
                        || checked.input != original.input
                        || checked.local != original.local
                        || local != original.local
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    use_view(
                        &ProductionOptimizedSliceAccessViewV18 {
                            original: view,
                            facts: &facts,
                        },
                        local,
                    )
                });
            drop(facts);
            drop(retained);
            let value = result?;
            if let Err(error) =
                self.retain_query(budget.release_storage(retained_bytes).map_err(Into::into))
            {
                source_reference_discard_v29(Ok::<R, ProductionSourceOwnedViewErrorV18>(value));
                return Err(SourceConsumerErrorV18(error));
            }
            Ok(value)
        })
        .map_err(|error: SourceConsumerErrorV18<ProductionSourceOwnedViewErrorV18>| error.0)
    }
}
