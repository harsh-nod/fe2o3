// Closed source-global evidence, shared only with the composing compiler.
// Runtime premises are retained data, never a caller-authored proof producer.
include!("production_source_aggregate_global_transport_v30.rs");
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CompletedGlobalOperationV26 {
    Read,
    Write,
    Data,
    Length,
    Address,
    PointerTransport,
}

include!("production_source_descriptor_length_completion_v30.rs");
include!("production_source_descriptor_length_leaves_v40.rs");

/// An exact source-bound parameter contract whose concrete allocation and
/// launch requirements must be transported to the runtime, not assumed here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMixedSliceRuntimePremiseV26 {
    root: usize,
    original_argument: u32,
    parameter: SliceDefinition,
    ty: SemanticTypeIdV1,
    identity: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1,
    scalar: ScalarType,
    reads: usize,
    writes: usize,
    axis: Option<fe2o3_kernel_ir::Axis>,
    launch: fe2o3_kernel_ir::ExplicitLaunchExtent,
    width: fe2o3_kernel_ir::FormalIndexWidth,
    exclusive_contract: bool,
}

impl ProductionMixedSliceRuntimePremiseV26 {
    /// Returns the original source root ordinal that owns this argument.
    pub const fn root(&self) -> usize {
        self.root
    }
    /// Returns the original semantic argument ordinal, not a physical ABI offset.
    pub const fn original_argument(&self) -> u32 {
        self.original_argument
    }
    /// Returns the exact retained output parameter definition for this slice.
    pub const fn parameter(&self) -> SliceDefinition {
        self.parameter
    }
    /// Returns the semantic type ID in the retained original source owner.
    pub const fn source_type(&self) -> SemanticTypeIdV1 {
        self.ty
    }
    /// Returns semantic type identity, distinct from the Rust ABI layout identity.
    pub const fn source_identity(
        &self,
    ) -> fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1 {
        self.identity
    }
    /// Returns the authenticated scalar element type of the accessed slice.
    pub const fn scalar(&self) -> ScalarType {
        self.scalar
    }
    /// Returns complete read and write occurrence counts, in that order.
    pub const fn access_counts(&self) -> [usize; 2] {
        [self.reads, self.writes]
    }
    /// Returns the source-bound invocation axis required by admitted writes.
    pub const fn invocation_axis(&self) -> Option<fe2o3_kernel_ir::Axis> {
        self.axis
    }
    /// Returns the explicit launch condition retained during source completion.
    pub const fn launch(&self) -> fe2o3_kernel_ir::ExplicitLaunchExtent {
        self.launch
    }
    /// Returns the formal pointer/index width used for the conditional analysis.
    pub const fn index_width(&self) -> fe2o3_kernel_ir::FormalIndexWidth {
        self.width
    }
    /// Reports whether memory accesses require a valid, aligned extent.
    /// Unused parameters still require complete argument/buffer transport.
    pub const fn requires_valid_aligned_extent(&self) -> bool {
        self.reads != 0 || self.writes != 0
    }
    /// Reports whether any read requires an initialized runtime extent.
    pub const fn requires_initialized_extent(&self) -> bool {
        self.reads != 0
    }
    /// Reports whether writes require exclusive, cross-argument nonoverlap checks.
    pub const fn requires_exclusive_nonoverlapping_runtime_binding(&self) -> bool {
        self.writes != 0
    }
    /// Reports whether write disjointness requires the exact retained launch.
    pub const fn requires_exact_launch_binding(&self) -> bool {
        self.writes != 0
    }
    /// Reports the original source exclusivity contract, not a live alias proof.
    pub const fn source_exclusive_contract(&self) -> bool {
        self.exclusive_contract
    }
    /// Always false: this premise is data and grants no executable authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// One exact source/output memory occurrence. The address-formation coordinate
/// is deliberately separate from the guarded access domain: forming an address
/// before a branch does not inherit the branch's dereference condition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMixedRuntimeOccurrenceV26 {
    premise: usize,
    source: GlobalSourceAccessPairV18,
    domain: fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26,
    projection: Option<(fe2o3_kernel_ir::Axis, ValueId)>,
}
impl ProductionMixedRuntimeOccurrenceV26 {
    /// Returns the associated accessed-argument row in the retained premise roster.
    pub const fn premise_index(&self) -> usize {
        self.premise
    }
    /// Returns the exact original source instance ordinal within its root.
    pub const fn original_instance(&self) -> usize {
        self.source.instance
    }
    /// Returns the original canonical memory-operation occurrence.
    pub const fn original_operation(&self) -> SliceOperation {
        self.source.input.logical.access.operation
    }
    /// Returns the retained output memory-operation occurrence.
    pub const fn output_operation(&self) -> SliceOperation {
        self.source.output.logical.access.operation
    }
    /// Returns the original address-formation occurrence, separate from access.
    pub const fn original_address_formation(&self) -> SliceOperation {
        self.source.input.logical.address
    }
    /// Returns output address formation; it need not be dominated by the access guard.
    pub const fn output_address_formation(&self) -> SliceOperation {
        self.source.output.logical.address
    }
    /// Returns the exact output SSA definition used to form the address index.
    pub const fn output_address_index(&self) -> SliceDefinition {
        self.source.output.address_index
    }
    /// Returns the exact output edge whose condition guards the memory access.
    pub const fn output_guard_edge(&self) -> fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
        self.source.output.logical.guard_edge
    }
    /// Returns the output SSA definition of the authenticated access condition.
    pub const fn output_guard_condition(&self) -> SliceDefinition {
        self.source.output.logical.guard_condition
    }
    /// Returns the conditional read or write domain with its exact raw value IDs.
    pub const fn domain(&self) -> fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26 {
        self.domain
    }
    /// Returns an authenticated direct Global invocation projection when available.
    pub const fn invocation_projection(&self) -> Option<(fe2o3_kernel_ir::Axis, ValueId)> {
        self.projection
    }
    /// Returns the exact output access width, alignment, address space, and flags.
    pub const fn memory_access(&self) -> MemoryAccess {
        self.source.output.memory
    }
    /// Always true: address representability needs an independently justified domain.
    pub const fn requires_address_formation_domain(&self) -> bool {
        true
    }
    /// Always false: occurrence identity alone does not admit artifact execution.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

pub(super) struct CompletedGlobalSourcesV26<'s, 'g> {
    original: &'s ProductionSourceCorrespondenceV18<'s>,
    optimized: &'s ProductionOptimizedSourceCorrespondenceV18<'s>,
    native: &'s fe2o3_pliron::PendingCanonicalMixedMemoryPoliciesV26<'s, 'g>,
    operations: &'s [Option<CompletedGlobalOperationV26>],
    premises: &'s [ProductionMixedSliceRuntimePremiseV26],
    occurrences: &'s [ProductionMixedRuntimeOccurrenceV26],
    roots: usize,
    operation_count: usize,
    scope: DescriptorRoleScopeV18,
}

impl CompletedGlobalSourcesV26<'_, '_> {
    pub(super) fn check_source_subject(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        if let Err(error) = self.scope.observe(self.original, budget) {
            self.native.refuse_retained_custody();
            return self.original.retain_query(Err(error));
        }
        self.original.retain_query((|| {
            budget.charge_work(4)?;
            if !std::ptr::eq(original, self.original) || !std::ptr::eq(optimized, self.optimized) {
                return original
                    .source
                    .missing("mixed global completed source owner differs");
            }
            original.global_expression_entry_v23(optimized, budget)?;
            let output = optimized.output_inventory(budget)?;
            let native_owner = self
                .native
                .owner(budget)
                .map_err(slice_entry_native_error_v25)?;
            if !std::ptr::eq(output.owner(), native_owner)
                || self.operations.len() != output.operations().len()
                || self.roots != original.source.root_count(budget)?
            {
                return original
                    .source
                    .missing("mixed global completed census changed");
            }
            Ok(())
        })())
    }

    pub(super) fn exact_operation(
        &self,
        coordinate: SliceOperation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<CompletedGlobalOperationV26>> {
        self.check_source_subject(self.original, self.optimized, budget)?;
        let output = self.optimized.output_inventory(budget)?;
        let index = completed_global_index_v26(output, coordinate, budget)?;
        Ok(self.operations[index])
    }

    pub(super) fn runtime_premises(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[ProductionMixedSliceRuntimePremiseV26]> {
        self.check_source_subject(self.original, self.optimized, budget)?;
        Ok(self.premises)
    }
    pub(super) fn census(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(usize, usize)> {
        self.check_source_subject(self.original, self.optimized, budget)?;
        Ok((self.roots, self.operation_count))
    }
    pub(super) fn runtime_occurrences(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[ProductionMixedRuntimeOccurrenceV26]> {
        self.check_source_subject(self.original, self.optimized, budget)?;
        Ok(self.occurrences)
    }
    pub(super) const fn runtime_requirements_are_discharged(&self) -> bool {
        false
    }
    pub(super) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn completed_global_index_v26(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    at: SliceOperation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let block = source_block_row_v18(inventory, at.block, budget)?;
    budget.charge_work(4)?;
    let index = block
        .operations
        .start
        .checked_add(at.operation as usize)
        .filter(|index| *index < block.operations.end)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "mixed global operation is absent",
        ))?;
    if inventory
        .operations()
        .get(index)
        .is_none_or(|row| row.coordinate != at)
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "mixed global operation coordinate differs",
        ));
    }
    Ok(index)
}

fn global_completion_headers_v26<F>() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        CompletedGlobalSourcesV26<'a, 'a>,
        ProductionMixedSliceRuntimePremiseV26,
        Option<CompletedGlobalOperationV26>,
        Vec<Option<CompletedGlobalOperationV26>>,
        Vec<ProductionMixedSliceRuntimePremiseV26>,
        Vec<ProductionMixedRuntimeOccurrenceV26>,
        ProductionMixedRuntimeOccurrenceV26,
        DescriptorRoleScopeV18,
        Vec<usize>,
        Vec<Option<usize>>,
        Vec<bool>,
        &'a SourceSliceRootCompletionV25<'a, 'a, 'a>,
        &'a fe2o3_pliron::PendingCanonicalMixedMemoryPoliciesV26<'a, 'a>,
        &'a fe2o3_kernel_ir::CheckedCanonicalConditionalSliceDomainsV26<'a, 'a>,
        &'a fe2o3_kernel_ir::CanonicalConditionalSliceParameterV26,
        Option<&'a fe2o3_kernel_ir::CanonicalConditionalSliceParameterV26>,
        &'a fe2o3_kernel_ir::CanonicalConditionalSliceAccessV26,
        Option<&'a fe2o3_kernel_ir::CanonicalConditionalSliceAccessV26>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        Option<(
            fe2o3_kernel_ir::ExplicitLaunchExtent,
            fe2o3_kernel_ir::FormalIndexWidth,
            usize,
            usize,
        )>,
        std::iter::Enumerate<std::slice::Iter<'a, Option<SourceSliceArgumentCompletionV25>>>,
        std::slice::Iter<'a, DescriptorSourceRoleRowV18>,
        std::slice::Iter<'a, Option<CompletedGlobalOperationV26>>,
        [usize; 20],
        [&'a (); 32],
        SliceDefinition,
        SliceOperation,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Frame<'_>>>())?,
        argument_product_v1(8, size_of::<&mut F>())?,
    ])
}

pub(super) fn with_completed_global_sources_v26<'work, F>(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    native: &fe2o3_pliron::PendingCanonicalMixedMemoryPoliciesV26<'_, '_>,
    reads: &GlobalReadFactsV18<'_, '_>,
    stores: &GlobalStoreFactsV25<'_, '_>,
    launches: &[fe2o3_kernel_ir::ExplicitLaunchExtent],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: &mut F,
) -> SourceOwnedResultV18<()>
where
    F: for<'s, 'g> FnMut(
        &CompletedGlobalSourcesV26<'s, 'g>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
{
    original.global_expression_entry_v23(optimized, budget)?;
    let run = |budget: &mut ArgumentBudgetV1<'work>| {
        budget.reserve_storage(global_completion_headers_v26::<F>()?)?;
        let globals = native
            .conditional_globals(budget)
            .map_err(slice_entry_native_error_v25)?;
        let accesses = native
            .global_accesses(budget)
            .map_err(slice_entry_native_error_v25)?;
        let output = optimized.output_inventory(budget)?;
        accesses
            .check_owner(output.owner(), budget)
            .map_err(slice_entry_native_error_v25)?;
        let roots = original.source.root_count(budget)?;
        if launches.len() != roots {
            return original
                .source
                .missing("mixed global source launch roster differs");
        }
        let mut operations = source_reference_emission_vec_v29(output.operations().len(), budget)
            .map_err(source_argument_error_v18)?;
        budget.charge_work(output.operations().len())?;
        operations.resize(output.operations().len(), None);
        let mut parameter_capacity = 0usize;
        let mut parameter_offsets = source_reference_emission_vec_v29(
            output
                .functions()
                .len()
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?,
            budget,
        )
        .map_err(source_argument_error_v18)?;
        parameter_offsets.push(0usize);
        for function in output.functions() {
            budget.charge_work(3)?;
            parameter_capacity = parameter_capacity
                .checked_add(function.function.signature.parameters.len())
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            parameter_offsets.push(parameter_capacity);
        }
        let mut parameters = source_reference_emission_vec_v29(parameter_capacity, budget)
            .map_err(source_argument_error_v18)?;
        budget.charge_work(parameter_capacity)?;
        parameters.resize(parameter_capacity, false);
        let mut premises = source_reference_emission_vec_v29(parameter_capacity, budget)
            .map_err(source_argument_error_v18)?;
        let mut occurrences = source_reference_emission_vec_v29(output.operations().len(), budget)
            .map_err(source_argument_error_v18)?;
        let mut premise_slots = source_reference_emission_vec_v29(parameter_capacity, budget)
            .map_err(source_argument_error_v18)?;
        budget.charge_work(parameter_capacity)?;
        premise_slots.resize(parameter_capacity, None);
        let mut completed_roots = 0usize;
        let mut effects = 0usize;
        let mut operation_count = 0usize;
        let mut root_consumer =
            |root: usize,
             source: &PendingGlobalSourceAccessesV18<'_>,
             budget: &mut ArgumentBudgetV1<'work>| {
                budget.charge_work(2)?;
                if root != completed_roots {
                    return original
                        .source
                        .missing("mixed global source root order differs");
                }
                source.with_shared_entry_regions_v18(budget, |entries, budget| {
                    entries.with_complete_slice_domains_v25(
                        accesses,
                        reads,
                        stores,
                        launches[root],
                        width,
                        budget,
                        &mut |completion, budget| {
                            let function = optimized_source_root_function_v18(
                                original, optimized, root, budget,
                            )?;
                            let conditions = globals
                                .function_conditions(function.coordinate, budget)
                                .map_err(|error| {
                                    optimized_source_observed_formal_error_v18(original, &error)
                                })?
                                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                    "mixed global function conditions are absent",
                                ))?;
                            if (conditions.0, conditions.1) != (completion.launch, completion.width)
                            {
                                return original
                                    .source
                                    .missing("mixed global source launch or index width differs");
                            }
                            let mut counts = [0usize; 2];
                            for (argument, row) in completion.arguments.iter().enumerate() {
                                budget.charge_work(64)?;
                                let Some(row) = row else {
                                    continue;
                                };
                                let SliceDefinition::FunctionArgument {
                                    function: parameter_function,
                                    argument: parameter,
                                } = row.parameter
                                else {
                                    return original.source.missing(
                                        "mixed global source slice is not an output parameter",
                                    );
                                };
                                let actual = globals
                                    .parameter(parameter_function, parameter, budget)
                                    .map_err(|error| {
                                        optimized_source_observed_formal_error_v18(original, &error)
                                    })?
                                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                        "mixed global conditional parameter is absent",
                                    ))?;
                                let definition = optimized_source_definition_row_v18(
                                    output,
                                    row.parameter,
                                    budget,
                                )?;
                                if parameter_function != function.coordinate
                                    || definition.value != Some(actual.value())
                                    || row.scalar != actual.scalar()
                                    || row.reads != actual.reads()
                                    || row.writes != actual.writes()
                                    || row.axis != actual.invocation_axis()
                                    || (row.writes != 0 && !row.exclusive)
                                    || (row.writes != 0
                                        && !actual.requires_exclusive_runtime_binding())
                                    || (row.writes != 0 && !actual.requires_exact_launch_binding())
                                    || (row.reads != 0 && !actual.requires_initialized_extent())
                                    || actual.requires_valid_aligned_extent()
                                        != (row.reads != 0 || row.writes != 0)
                                {
                                    return original.source.missing(
                                        "mixed global source and conditional parameter differ",
                                    );
                                }
                                budget.charge_work(6)?;
                                let start = *parameter_offsets
                                    .get(parameter_function.0 as usize)
                                    .ok_or(ArgumentResourceV1::Accounting)?;
                                let end = *parameter_offsets
                                    .get(
                                        (parameter_function.0 as usize)
                                            .checked_add(1)
                                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                                    )
                                    .ok_or(ArgumentResourceV1::Accounting)?;
                                let slot = start
                                    .checked_add(parameter as usize)
                                    .filter(|slot| *slot < end)
                                    .ok_or(ArgumentResourceV1::Accounting)?;
                                if parameters[slot] {
                                    return original
                                        .source
                                        .missing("mixed global source parameter completed twice");
                                }
                                parameters[slot] = true;
                                premise_slots[slot] = Some(premises.len());
                                if premises.len() == premises.capacity() {
                                    return Err(ArgumentResourceV1::Accounting.into());
                                }
                                premises.push(ProductionMixedSliceRuntimePremiseV26 {
                                    root,
                                    original_argument: u32::try_from(argument)
                                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                                    parameter: row.parameter,
                                    ty: row.ty,
                                    identity: row.identity,
                                    scalar: row.scalar,
                                    reads: row.reads,
                                    writes: row.writes,
                                    axis: row.axis,
                                    launch: completion.launch,
                                    width: completion.width,
                                    exclusive_contract: row.exclusive,
                                });
                            }
                            for row in completion.source.source.roles.rows {
                                budget.charge_work(8)?;
                                let role = match row.role {
                                    Some(DescriptorSourceRoleV18::Read) => {
                                        CompletedGlobalOperationV26::Read
                                    }
                                    Some(DescriptorSourceRoleV18::Write) => {
                                        CompletedGlobalOperationV26::Write
                                    }
                                    Some(DescriptorSourceRoleV18::Data) => {
                                        CompletedGlobalOperationV26::Data
                                    }
                                    Some(DescriptorSourceRoleV18::Length) => {
                                        CompletedGlobalOperationV26::Length
                                    }
                                    Some(DescriptorSourceRoleV18::Address) => {
                                        CompletedGlobalOperationV26::Address
                                    }
                                    Some(DescriptorSourceRoleV18::PointerTransport) => {
                                        CompletedGlobalOperationV26::PointerTransport
                                    }
                                    None => continue,
                                };
                                let index = completed_global_index_v26(output, row.output, budget)?;
                                if operations[index].is_some() {
                                    return original
                                        .source
                                        .missing("mixed global source occurrence completed twice");
                                }
                                if matches!(
                                    role,
                                    CompletedGlobalOperationV26::Read
                                        | CompletedGlobalOperationV26::Write
                                ) {
                                    let fact = globals
                                        .access_at(row.output, budget)
                                        .map_err(|error| {
                                            optimized_source_observed_formal_error_v18(
                                                original, &error,
                                            )
                                        })?
                                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                            "mixed global source effect lacks conditional fact",
                                        ))?;
                                    let writing = role == CompletedGlobalOperationV26::Write;
                                    if fact.domain().writing() != writing {
                                        return original
                                            .source
                                            .missing("mixed global source effect changed kind");
                                    }
                                    // Includes the fixed, lossless original and
                                    // output endpoint copies in the retained row.
                                    budget.charge_work(128)?;
                                    let pair = completion
                                        .source
                                        .source
                                        .access(row.output, budget)?
                                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                            "mixed global occurrence lacks authentic source pair",
                                        ))?;
                                    let SliceDefinition::FunctionArgument {
                                        function: parameter_function,
                                        argument,
                                    } = pair.output.logical.root
                                    else {
                                        return original.source.missing(
                                            "mixed global occurrence is not rooted at a parameter",
                                        );
                                    };
                                    let start = *parameter_offsets
                                        .get(parameter_function.0 as usize)
                                        .ok_or(ArgumentResourceV1::Accounting)?;
                                    let end = *parameter_offsets
                                        .get(
                                            (parameter_function.0 as usize)
                                                .checked_add(1)
                                                .ok_or(ArgumentResourceV1::Arithmetic)?,
                                        )
                                        .ok_or(ArgumentResourceV1::Accounting)?;
                                    let slot = start
                                        .checked_add(argument as usize)
                                        .filter(|slot| *slot < end)
                                        .ok_or(ArgumentResourceV1::Accounting)?;
                                    let premise = premise_slots[slot].ok_or(
                                        ProductionSourceOwnedViewErrorV18::Binding(
                                            "mixed global occurrence lacks exact parameter premise",
                                        ),
                                    )?;
                                    if pair.output.logical.access.operation != fact.operation()
                                        || pair.output.writing != writing
                                        || pair.output.pointer != fact.domain().pointer()
                                        || premises[premise].root != root
                                        || occurrences.len() == occurrences.capacity()
                                    {
                                        return original
                                            .source
                                            .missing("mixed global occurrence contract differs");
                                    }
                                    occurrences.push(ProductionMixedRuntimeOccurrenceV26 {
                                        premise,
                                        source: *pair,
                                        domain: fact.domain(),
                                        projection: fact.invocation_projection(),
                                    });
                                    counts[usize::from(writing)] = counts[usize::from(writing)]
                                        .checked_add(1)
                                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                                    effects = effects
                                        .checked_add(1)
                                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                                }
                                operations[index] = Some(role);
                                operation_count = operation_count
                                    .checked_add(1)
                                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                            }
                            if counts != [conditions.2, conditions.3] {
                                return original
                                    .source
                                    .missing("mixed global root effect census differs");
                            }
                            complete_source_descriptor_lengths_v30(
                                original,
                                optimized,
                                root,
                                completion,
                                &mut operations,
                                &mut operation_count,
                                budget,
                            )?;
                            Ok(())
                        },
                    )
                })?;
                completed_roots = completed_roots
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                Ok(())
            };
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of_val(&root_consumer),
            std::mem::align_of_val(&root_consumer),
        ])?)?;
        original.with_global_source_expressions_v23(optimized, budget, &mut root_consumer)?;
        drop(root_consumer);
        if completed_roots != roots
            || effects
                != globals
                    .access_count(budget)
                    .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?
        {
            return original
                .source
                .missing("mixed global whole-module source census is incomplete");
        }
        let completed = CompletedGlobalSourcesV26 {
            original,
            optimized,
            native,
            operations: &operations,
            premises: &premises,
            occurrences: &occurrences,
            roots,
            operation_count,
            scope: DescriptorRoleScopeV18::new(budget),
        };
        completed.check_source_subject(original, optimized, budget)?;
        shared_entry_consume_v18(original, budget, |budget| consume(&completed, budget))?;
        completed.check_source_subject(original, optimized, budget)
    };
    let frame = argument_sum_v1(&[std::mem::size_of_val(&run), std::mem::align_of_val(&run)])?;
    original.retain_query(source_scalar_normalization_scratch_v18(
        original.source.cleanup,
        budget,
        frame,
        run,
    ))
}
