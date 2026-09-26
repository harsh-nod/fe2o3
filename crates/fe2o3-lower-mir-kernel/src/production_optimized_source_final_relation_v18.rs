/// Inert projected access metadata with its original invocation identity.
/// Construction and private solver key assignment grant no proof authority.
pub struct ProductionSourceRankedAccessV18 {
    instance: usize,
    function: SemanticFunctionIdV1,
    projected: ProductionRankedAccessSourceV1,
    solver: Option<SemanticAccessSiteV1>,
}

impl ProductionSourceRankedAccessV18 {
    /// Retains the entire original projection record, including extent metadata.
    pub const fn new(instance: usize, function: SemanticFunctionIdV1,
        projected: ProductionRankedAccessSourceV1) -> Self
    {
        Self { instance, function, projected, solver: None }
    }

    fn original(&self) -> SourceEffectSiteV18 {
        SourceEffectSiteV18 {
            instance: self.instance, function: self.function,
            block: SemanticBlockIdV1::from_index(self.projected.semantic_block),
            statement: self.projected.semantic_statement,
            ordinal: self.projected.semantic_access_ordinal,
        }
    }
}

fn source_ranked_key_after_v18(
    previous: Option<(SourceEffectSiteV18, SemanticAccessSiteV1)>,
    site: SourceEffectSiteV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SemanticAccessSiteV1> {
    budget.charge_work(8)?;
    let Some((before, key)) = previous else {
        return Ok(SemanticAccessSiteV1 { block: 0, statement: site.statement, ordinal: 0 });
    };
    if source_flow_site_key_v18(before) >= source_flow_site_key_v18(site) {
        return Err(ProductionSourceOwnedViewErrorV18::Binding("qualified ranked source sites repeat or are unordered"));
    }
    let same_block = (before.instance, before.function, before.block)
        == (site.instance, site.function, site.block);
    Ok(SemanticAccessSiteV1 {
        block: if same_block { key.block } else {
            key.block.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?
        },
        statement: site.statement,
        ordinal: if same_block && before.statement == site.statement {
            key.ordinal.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?
        } else { 0 },
    })
}

fn check_source_ranked_site_identity_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    site: SourceEffectSiteV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let (function, _) = original.source.instance(root, site.instance, budget)?;
    let source = original.source.source_semantic(budget)?;
    budget.charge_work(4)?;
    let block = source.functions().get(function.index() as usize)
        .and_then(|function| function.blocks().get(site.block.index() as usize))
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("qualified ranked original source block"))?;
    if function != site.function
        || site.statement.is_some_and(|statement| statement as usize >= block.statements().len())
    {
        return original.source.missing("qualified ranked source changed its invocation or site");
    }
    // Original access ordinals are retained, not inferred from these coordinates.
    // The complete physical/source census must still join each actual access.
    Ok(())
}

fn prepare_source_ranked_accesses_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    records: &mut [ProductionSourceRankedAccessV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.retain_query((|| {
        optimized_source_endpoints_v18(original, optimized, budget)?;
        original.source.root(root, budget)?;
        let headers = source_ranked_key_scratch_headers_v18()?;
        budget.reserve_storage(headers)?;
        private_array_heapsort_v1(records, |row| source_flow_site_key_v18(row.original()),
            &mut SourceCorrespondenceWorkV18(budget), || ArgumentResourceV1::Arithmetic.into())?;
        {
            let mut previous = None;
            for row in records {
                let site = row.original();
                check_source_ranked_site_identity_v18(original, root, site, budget)?;
                let key = source_ranked_key_after_v18(previous, site, budget)?;
                row.solver = Some(key);
                previous = Some((site, key));
            }
        }
        budget.release_storage(headers)?;
        optimized_source_endpoints_v18(original, optimized, budget)
    })())
}

fn check_source_ranked_accesses_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    records: &[ProductionSourceRankedAccessV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.retain_query((|| {
        optimized_source_endpoints_v18(original, optimized, budget)?;
        original.source.root(root, budget)?;
        let headers = source_ranked_key_scratch_headers_v18()?;
        budget.reserve_storage(headers)?;
        {
            let mut previous = None;
            for row in records {
                let site = row.original();
                check_source_ranked_site_identity_v18(original, root, site, budget)?;
                let key = source_ranked_key_after_v18(previous, site, budget)?;
                budget.charge_work(1)?;
                if row.solver != Some(key) {
                    return original.source.missing("qualified ranked solver key differs from its original instance group");
                }
                previous = Some((site, key));
            }
        }
        budget.release_storage(headers)?;
        Ok(())
    })())
}

fn source_ranked_key_scratch_headers_v18() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<Option<(SourceEffectSiteV18, SemanticAccessSiteV1)>>(),
        size_of::<SourceEffectSiteV18>(), size_of::<SemanticAccessSiteV1>(),
        size_of::<SourceOwnedResultV18<SemanticAccessSiteV1>>(),
        size_of::<SourceOwnedResultV18<()>>(), size_of::<[usize; 6]>(),
    ])
}

// Original instance metadata joined to every actual output memory footprint.
// This census is an input to final ranked/currentness checking, not its proof.
#[derive(Clone, Copy)]
enum OptimizedSourceAccessOriginV18 {
    Unresolved,
    SourceSpan(SourceEffectSiteV18),
    GeneratedPending,
    RetainedStorage,
    NonSpan,
}

#[derive(Clone, Copy)]
struct OptimizedSourceMemoryBindingV18 {
    original: SourceMemoryBindingV18,
    input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    input_access: u32,
    origin: OptimizedSourceAccessOriginV18,
}

struct OptimizedSourceEffectCensusV18<'scope> {
    original: &'scope ProductionSourceCorrespondenceV18<'scope>,
    optimized: &'scope ProductionOptimizedSourceCorrespondenceV18<'scope>,
    root: usize,
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    effects: Vec<SourceMemoryEffectV18>,
    bindings: Vec<OptimizedSourceMemoryBindingV18>,
    removed_unreachable: usize,
}

// This view proves source-span access numbering, not that a ranked access or
// generated recipe is equivalent. Non-span obligations are still explicit.
struct OptimizedSourceAccessSitesV18<'scope> {
    census: OptimizedSourceEffectCensusV18<'scope>,
    currentness: &'scope scoped_raw_admission_v29::CheckedOptimizedSourceMemoryV18<'scope>,
}

fn optimized_source_access_sites_v18<'scope>(
    original: &'scope ProductionSourceCorrespondenceV18<'scope>,
    optimized: &'scope ProductionOptimizedSourceCorrespondenceV18<'scope>,
    root: usize,
    input_spaces: &SourcePointerSpacesV18<'_, '_>,
    output_spaces: &SourcePointerSpacesV18<'_, '_>,
    currentness: &'scope scoped_raw_admission_v29::CheckedOptimizedSourceMemoryV18<'scope>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<OptimizedSourceAccessSitesV18<'scope>> {
    original.retain_query((|| {
        currentness.check_scope_v18(original, optimized, root, budget)?;
        budget.reserve_storage(argument_sum_v1(&[
            size_of::<OptimizedSourceAccessSitesV18<'_>>(),
            size_of::<SourceOwnedResultV18<OptimizedSourceAccessSitesV18<'_>>>(),
        ])?)?;
        let census = optimized_source_effect_census_core_v18(original, optimized, root,
            input_spaces, output_spaces, Some(currentness), budget)?;
        Ok(OptimizedSourceAccessSitesV18 { census, currentness })
    })())
}

impl OptimizedSourceAccessSitesV18<'_> {
    fn check_location(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        root: usize,
        location: &SourceEffectLocationV18,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        original.retain_query((|| {
            self.currentness.check_scope_v18(original, self.census.optimized, root, budget)?;
            self.census.check_location(original, root, location, budget)?;
            let key = [location.physical.block.block as usize,
                location.physical.operation as usize, location.access as usize];
            let index = private_array_partition_v1(&self.census.effects, source_effect_key_v18,
                key, false, &mut SourceCorrespondenceWorkV18(budget))?;
            budget.charge_work(1)?;
            if !matches!(self.census.bindings.get(index).map(|row| row.origin),
                Some(OptimizedSourceAccessOriginV18::SourceSpan(site)) if site == location.site)
            {
                return original.source.missing("ranked access changed its exact original access ordinal or disposition");
            }
            Ok(())
        })())
    }
}

fn optimized_source_ordinal_scratch_headers_v18() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<Option<(usize, usize)>>(),
        size_of::<Option<SourceEffectSiteV18>>(),
        size_of::<SourceEffectSiteV18>(),
        size_of::<Option<usize>>(),
        size_of::<SourceOwnedResultV18<Option<usize>>>(),
        size_of::<SourceOwnedResultV18<bool>>(),
        size_of::<std::ops::Range<usize>>(),
        size_of::<Option<(fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1, u32, ValueId)>>(),
        size_of::<OptimizedSourceAccessOriginV18>(),
        size_of::<bool>(),
        argument_product_v1(5, size_of::<usize>())?,
    ])
}

// Number the immutable input's attachment parts, not reordered output blocks.
// The preexisting binding census already rejects duplicate span attachment of
// an input footprint, including footprints removed by checked dead control.
fn optimized_source_access_ordinals_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    currentness: &scoped_raw_admission_v29::CheckedOptimizedSourceMemoryV18<'_>,
    input: &[SourceMemoryEffectV18],
    input_bindings: &[SourceMemoryBindingV18],
    output: &[SourceMemoryEffectV18],
    bindings: &mut [OptimizedSourceMemoryBindingV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    currentness.check_scope_v18(original, optimized, root, budget)?;
    if input.len() != input_bindings.len() || output.len() != bindings.len() {
        return original.source.missing("source access numbering changed its physical census");
    }
    let headers = optimized_source_ordinal_scratch_headers_v18()?;
    budget.reserve_storage(headers)?;
    let mut expected = 0usize;
    for binding in input_bindings {
        budget.charge_work(1)?;
        if binding.span.is_some() { expected = argument_sum_v1(&[expected, 1])?; }
    }
    let mut group = None;
    let mut part = 0usize;
    let mut source_site = None;
    let mut generated = false;
    let mut access_ordinal = 0u32;
    let mut visited = 0usize;
    for attachment in original.attachments {
        budget.charge_work(1)?;
        let key = attachment.key;
        if key.root != root || key.family != TileAttachmentFamilyV29::InstanceSpans
            || key.field != TileAttachmentFieldV29::Span { continue; }
        let next = (key.instance, key.row);
        if group != Some(next) {
            budget.charge_work(1)?;
            if group.is_some_and(|before| before >= next) {
                return original.source.missing("source access span groups are not unique and ordered");
            }
            group = Some(next);
            part = 0;
            access_ordinal = 0;
            let span = original.source.root_row(root)?.coordinates.spans.rows.get(key.row)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("source access original span index"))?;
            let function = original.source.instance(root, key.instance, budget)?.0;
            budget.charge_work(2)?;
            if span.instance.index() != key.instance || span.source.coordinates().1 != function {
                return original.source.missing("source access span changed original instance or function");
            }
            source_site = match span.source {
                InstanceSpanSourceV1::Statement(site) => Some(SourceEffectSiteV18 {
                    instance: key.instance, function, block: site.semantic_block,
                    statement: Some(site.statement_ordinal), ordinal: 0,
                }),
                InstanceSpanSourceV1::Terminator(site) => Some(SourceEffectSiteV18 {
                    instance: key.instance, function, block: site.semantic_block,
                    statement: None, ordinal: 0,
                }),
                InstanceSpanSourceV1::Synthetic(_) | InstanceSpanSourceV1::InvocationEntry(_) => None,
            };
            if let Some(site) = source_site {
                check_source_ranked_site_identity_v18(original, root, site, budget)?;
            }
            generated = match source_site {
                Some(site) if site.statement.is_none() => {
                    let semantic = original.source.source_semantic(budget)?;
                    budget.charge_work(3)?;
                    let block = semantic.functions().get(function.index() as usize)
                        .and_then(|function| function.blocks().get(site.block.index() as usize))
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("generated source access block"))?;
                    match block.terminator().kind() {
                        SemanticTerminatorKindV1::Call(call) => matches!(
                            semantic.callables().get(call.callee().index() as usize),
                            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                                operation: SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum { .. }
                                    | SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupScanSum { .. }, ..
                            })),
                        _ => false,
                    }
                }
                _ => false,
            };
        }
        budget.charge_work(2)?;
        if key.component != 0 || key.part != part {
            return original.source.missing("source access span changed component or part sequence");
        }
        part = argument_sum_v1(&[part, 1])?;
        let ProductionSourceOperationV18::Operation(operation) =
            original.mapped_source_operation(attachment.location, budget)? else { continue; };
        let first = private_array_partition_v1(input, source_effect_key_v18,
            [operation.block.block as usize, operation.operation as usize, 0], false,
            &mut SourceCorrespondenceWorkV18(budget))?;
        let last = private_array_partition_v1(input, source_effect_key_v18,
            [operation.block.block as usize, operation.operation as usize, u32::MAX as usize], true,
            &mut SourceCorrespondenceWorkV18(budget))?;
        let effects = input.get(first..last)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("source access footprint range"))?;
        for (offset, effect) in effects.iter().enumerate() {
            budget.charge_work(4)?;
            let before = input_bindings.get(first.checked_add(offset).ok_or(ArgumentResourceV1::Arithmetic)?)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("source access binding index"))?;
            if effect.coordinate != operation || before.instance != Some(key.instance)
                || before.span != Some(key.row)
            {
                return original.source.missing("source access footprint changed its exact original attachment");
            }
            visited = argument_sum_v1(&[visited, 1])?;
            let index = optimized_source_effect_output_index_v18(original, optimized, effect, output, budget)?;
            let actual = index.map(|index| &output[index]);
            let retained = currentness.retained_value_footprint_v18(original, optimized, root,
                key.instance, effect.coordinate, effect.ordinal, effect.pointer,
                actual.map(|effect| (effect.coordinate, effect.ordinal, effect.pointer)), budget)?;
            if effect.storage_operation && !retained {
                return original.source.missing("typed source access lacks exact whole-value currentness");
            }
            let origin = if retained {
                OptimizedSourceAccessOriginV18::RetainedStorage
            } else if generated {
                OptimizedSourceAccessOriginV18::GeneratedPending
            } else if let Some(mut site) = source_site {
                site.ordinal = access_ordinal;
                access_ordinal = access_ordinal.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                OptimizedSourceAccessOriginV18::SourceSpan(site)
            } else { OptimizedSourceAccessOriginV18::NonSpan };
            if let Some(index) = index {
                let binding = bindings.get_mut(index)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding("source access output binding"))?;
                budget.charge_work(3)?;
                if binding.input != effect.coordinate || binding.input_access != effect.ordinal
                    || !matches!(binding.origin, OptimizedSourceAccessOriginV18::Unresolved)
                {
                    return original.source.missing("source access output repeats or substitutes an original footprint");
                }
                binding.origin = origin;
            }
        }
    }
    if visited != expected {
        return original.source.missing("source access span footprint census is incomplete");
    }
    for binding in bindings {
        budget.charge_work(1)?;
        if binding.original.span.is_none() {
            binding.origin = OptimizedSourceAccessOriginV18::NonSpan;
        } else if matches!(binding.origin, OptimizedSourceAccessOriginV18::Unresolved) {
            return original.source.missing("output source access lacks an original ordinal disposition");
        }
    }
    currentness.check_scope_v18(original, optimized, root, budget)?;
    budget.release_storage(headers)?;
    Ok(())
}

fn optimized_source_effect_output_index_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: &SourceMemoryEffectV18,
    output: &[SourceMemoryEffectV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<usize>> {
    let operation = match optimized.operation(input.coordinate, budget)? {
        ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => return Ok(None),
        ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
        ProductionOptimizedSourceOperationV18::Rewritten { .. } =>
            return original.source.missing("source access ordinal lost an ordered memory operation"),
    };
    let key = [operation.block.block as usize, operation.operation as usize, input.ordinal as usize];
    let index = private_array_partition_v1(output, source_effect_key_v18, key, false,
        &mut SourceCorrespondenceWorkV18(budget))?;
    budget.charge_work(1)?;
    if !output.get(index).is_some_and(|effect|
        effect.coordinate == operation && effect.ordinal == input.ordinal)
    {
        return original.source.missing("source access ordinal lost its actual output footprint");
    }
    Ok(Some(index))
}

fn optimized_source_effect_census_v18<'scope>(
    relation: &'scope ProductionSourceCorrespondenceV18<'scope>,
    optimized: &'scope ProductionOptimizedSourceCorrespondenceV18<'scope>,
    root: usize,
    input_spaces: &SourcePointerSpacesV18<'_, '_>,
    output_spaces: &SourcePointerSpacesV18<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<OptimizedSourceEffectCensusV18<'scope>> {
    optimized_source_effect_census_core_v18(relation, optimized, root,
        input_spaces, output_spaces, None, budget)
}

fn optimized_source_effect_census_core_v18<'scope>(
    relation: &'scope ProductionSourceCorrespondenceV18<'scope>,
    optimized: &'scope ProductionOptimizedSourceCorrespondenceV18<'scope>,
    root: usize,
    input_spaces: &SourcePointerSpacesV18<'_, '_>,
    output_spaces: &SourcePointerSpacesV18<'_, '_>,
    currentness: Option<&scoped_raw_admission_v29::CheckedOptimizedSourceMemoryV18<'_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<OptimizedSourceEffectCensusV18<'scope>> {
    relation.retain_query((|| {
        optimized_source_endpoints_v18(relation, optimized, budget)?;
        let max_operations = relation.source.limits(budget)?.max_operations;
        budget.reserve_storage(argument_sum_v1(&[
            size_of::<OptimizedSourceEffectCensusV18<'_>>(),
            size_of::<SourceOwnedResultV18<OptimizedSourceEffectCensusV18<'_>>>(),
            size_of::<Vec<SourceMemoryEffectV18>>(),
            size_of::<Vec<SourceMemoryBindingV18>>(),
        ])?)?;
        let input =
            source_memory_effects_v18(relation, root, input_spaces, max_operations, budget)?;
        let original_bindings = source_memory_bindings_v18(relation, root, &input, budget)?;
        let output = optimized_source_memory_effects_v18(
            relation,
            optimized,
            root,
            output_spaces,
            max_operations,
            budget,
        )?;
        let mut bindings =
            emission_vec_v1(output.len(), budget).map_err(source_emission_error_v18)?;
        budget.charge_work(output.len())?;
        // These rows are never exposed before all output effects have an input.
        // A complete occurrence key keeps multi-footprint operations distinct.
        bindings.resize(output.len(), OptimizedSourceMemoryBindingV18 {
            original: SourceMemoryBindingV18::default(),
            input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0), block: 0,
                }, operation: 0,
            },
            input_access: 0,
            origin: OptimizedSourceAccessOriginV18::Unresolved,
        });
        let mut removed_unreachable = 0usize;
        for (effect, binding) in input.iter().zip(&original_bindings) {
            budget.charge_work(1)?;
            match optimized.operation(effect.coordinate, budget)? {
                ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                    removed_unreachable = removed_unreachable
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                }
                ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                    return relation.source.missing(
                        "ordered memory footprint was rewritten without an exact occurrence",
                    );
                }
                ProductionOptimizedSourceOperationV18::Retained {
                    output: operation, ..
                } => {
                    let key = [
                        operation.block.block as usize,
                        operation.operation as usize,
                        effect.ordinal as usize,
                    ];
                    let index = private_array_partition_v1(
                        &output,
                        source_effect_key_v18,
                        key,
                        false,
                        &mut SourceCorrespondenceWorkV18(budget),
                    )?;
                    let actual = output
                        .get(index)
                        .filter(|row| row.coordinate == operation && row.ordinal == effect.ordinal)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "optimized source memory footprint is absent",
                        ))?;
                    budget.charge_work(6)?;
                    if actual.access != effect.access
                        || actual.static_space != effect.static_space
                        || actual.concrete_space != effect.concrete_space
                        || actual.atomic != effect.atomic
                        || actual.storage_operation != effect.storage_operation
                        || binding.instance.is_none()
                        || bindings[index].original.instance.is_some()
                    {
                        return relation.source.missing(
                            "optimized memory footprint metadata or original occurrence differs",
                        );
                    }
                    optimized_source_effect_pointer_v18(
                        relation, optimized, effect, actual, budget,
                    )?;
                    bindings[index] = OptimizedSourceMemoryBindingV18 {
                        original: *binding,
                        input: effect.coordinate,
                        input_access: effect.ordinal,
                        origin: OptimizedSourceAccessOriginV18::Unresolved,
                    };
                }
            }
        }
        // A complete output census also rejects synthesized, duplicated and
        // otherwise unattached effects. Scalar-only readers cannot waive it.
        for binding in &bindings {
            budget.charge_work(1)?;
            if binding.original.instance.is_none() {
                return relation
                    .source
                    .missing("optimized physical memory effect has no original source occurrence");
            }
        }
        if let Some(currentness) = currentness {
            optimized_source_access_ordinals_v18(relation, optimized, root,
                currentness, &input, &original_bindings, &output, &mut bindings, budget)?;
        }
        let scratch = argument_sum_v1(&[
            size_of::<Vec<SourceMemoryEffectV18>>(),
            size_of::<Vec<SourceMemoryBindingV18>>(),
            argument_product_v1(input.capacity(), size_of::<SourceMemoryEffectV18>())?,
            argument_product_v1(original_bindings.capacity(), size_of::<SourceMemoryBindingV18>())?,
        ])?;
        drop((input, original_bindings));
        budget.release_storage(scratch)?;
        Ok(OptimizedSourceEffectCensusV18 {
            original: relation,
            optimized,
            root,
            required: budget.storage(),
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            effects: output,
            bindings,
            removed_unreachable,
        })
    })())
}

impl OptimizedSourceEffectCensusV18<'_> {
    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.original.observe_custody(budget)?;
        if self.required > budget.storage() || self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
        {
            self.original.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn check(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        original.retain_query((|| {
            self.observe_custody(budget)?;
            optimized_source_endpoints_v18(original, self.optimized, budget)?;
            budget.charge_work(3)?;
            if !std::ptr::eq(original, self.original) || root != self.root
                || self.effects.len() != self.bindings.len()
            {
                return original.source.missing("optimized effect census changed original relation or root");
            }
            original.source.root(root, budget)?;
            Ok(())
        })())
    }

    // The constructor already joined every output effect to its exact input
    // occurrence. Reuse that one paid table instead of scanning source spans
    // again for each CFG event. Source access ordinals are checked by the final
    // ranked-effect census, not inferred from a matching statement.
    fn check_location(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        root: usize,
        location: &SourceEffectLocationV18,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        original.retain_query((|| {
            self.check(original, root, budget)?;
            let key = [location.physical.block.block as usize,
                location.physical.operation as usize, location.access as usize];
            let index = private_array_partition_v1(&self.effects, source_effect_key_v18,
                key, false, &mut SourceCorrespondenceWorkV18(budget))?;
            budget.charge_work(5)?;
            let effect = self.effects.get(index).filter(|row|
                row.coordinate == location.physical && row.ordinal == location.access)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("optimized effect-order occurrence is absent"))?;
            let binding = self.bindings.get(index).map(|row| &row.original)
                .filter(|row| row.instance == Some(location.site.instance))
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("optimized effect-order invocation differs"))?;
            let root_row = original.source.root_row(root)?;
            let span = binding.span.and_then(|row| root_row.coordinates.spans.rows.get(row))
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("optimized effect-order lacks an exact original span"))?;
            let source_matches = match span.source {
                InstanceSpanSourceV1::Statement(site) =>
                    site.semantic_function == location.site.function
                        && site.semantic_block == location.site.block
                        && Some(site.statement_ordinal) == location.site.statement,
                InstanceSpanSourceV1::Terminator(site) =>
                    site.semantic_function == location.site.function
                        && site.semantic_block == location.site.block
                        && location.site.statement.is_none(),
                InstanceSpanSourceV1::Synthetic(_) | InstanceSpanSourceV1::InvocationEntry(_) => false,
            };
            budget.charge_work(6)?;
            if span.instance.index() != location.site.instance || !source_matches
                || effect.coordinate.block.function
                    != optimized_source_root_function_v18(original, self.optimized, root, budget)?.coordinate
            {
                return original.source.missing("optimized effect-order changed its original source site");
            }
            Ok(())
        })())
    }
}

fn optimized_source_effect_pointer_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: &SourceMemoryEffectV18,
    output: &SourceMemoryEffectV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 as Usage;
    let operation =
        optimized_source_operation_row_v18(relation.inventory, input.coordinate, budget)?;
    let actual_operation = optimized_source_operation_row_v18(
        optimized.output_inventory(budget)?,
        output.coordinate,
        budget,
    )?;
    budget.charge_work(2)?;
    let (operand, pointer) =
        optimized_effect_pointer_role_v18(&operation.operation.kind, input.ordinal).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding(
                "memory effect has no exact original pointer role",
            ),
        )?;
    let (output_operand, output_pointer) =
        optimized_effect_pointer_role_v18(&actual_operation.operation.kind, output.ordinal).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding(
                "memory effect has no exact output pointer role",
            ),
        )?;
    budget.charge_work(5)?;
    if input.pointer != pointer || output.pointer != output_pointer {
        return relation
            .source
            .missing("memory effect differs from its declared pointer role");
    }
    let original_use = Usage::OperationOperand {
        operation: input.coordinate,
        operand,
    };
    let original_row = operation
        .operands
        .start
        .checked_add(operand as usize)
        .filter(|index| *index < operation.operands.end)
        .and_then(|index| relation.inventory.uses().get(index))
        .filter(|row| row.coordinate == original_use)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "memory effect original pointer use",
        ))?;
    let definition = relation
        .inventory
        .definitions()
        .get(original_row.definition)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "memory effect original pointer definition",
        ))?;
    if definition.value != Some(pointer) {
        return relation
            .source
            .missing("memory effect original pointer use changed value");
    }
    let (selected, value) = optimized_source_actual_operand_v18(
        relation,
        optimized,
        original_use,
        output.coordinate,
        budget,
    )?;
    budget.charge_work(2)?;
    if selected.coordinate
        != (Usage::OperationOperand {
            operation: output.coordinate,
            operand: output_operand,
        })
        || value != output.pointer
    {
        return relation
            .source
            .missing("optimized memory effect changed its exact pointer operand");
    }
    Ok(())
}

// Effect visitation order and operand visitation order are distinct (notably
// transpose Stage). Decode the closed physical role before consulting values.
fn optimized_effect_pointer_role_v18(kind: &OperationKind, effect: u32) -> Option<(u32, ValueId)> {
    use fe2o3_kernel_ir::{StorageOperationV1 as Storage, StorageProjectionV1 as Projection};
    match kind {
        OperationKind::Load { pointer, .. }
        | OperationKind::GuardedLoad { pointer, .. }
        | OperationKind::Store { pointer, .. }
        | OperationKind::GuardedStore { pointer, .. } => (effect == 0).then_some((0, *pointer)),
        OperationKind::Atomic(atomic) => (effect == 0).then_some((0, atomic.pointer)),
        OperationKind::MemoryIntrinsic(intrinsic) => match intrinsic {
            MemoryIntrinsicOperation::CopyNonOverlapping {
                source,
                destination,
                ..
            } => match effect {
                0 => Some((0, *source)),
                1 => Some((1, *destination)),
                _ => None,
            },
            MemoryIntrinsicOperation::PointerDistance { .. }
            | MemoryIntrinsicOperation::VolatileLoad { .. }
            | MemoryIntrinsicOperation::VolatileStore { .. } => None,
        },
        OperationKind::Storage(storage) => match storage {
            Storage::Project {
                base,
                step: Projection::Variant { .. },
            } => (effect == 0).then_some((0, *base)),
            Storage::Project { .. } => None,
            Storage::ReadValue { address, .. }
            | Storage::ReadDiscriminant { address, .. }
            | Storage::WriteValue { address, .. }
            | Storage::SetDiscriminant { address, .. } => (effect == 0).then_some((0, *address)),
            Storage::CopyObject {
                source,
                destination,
                ..
            } => match effect {
                0 => Some((0, *source)),
                1 => Some((1, *destination)),
                _ => None,
            },
        },
        OperationKind::Matrix(matrix) => match matrix.kind {
            MatrixOperationKind::LdsLoad { base, .. }
            | MatrixOperationKind::LdsStore { base, .. } => (effect == 0).then_some((0, base)),
            MatrixOperationKind::MultiplyAccumulate { .. }
            | MatrixOperationKind::ScaledMultiplyAccumulate { .. } => None,
        },
        OperationKind::Gfx950LdsTranspose(transpose) => match transpose.kind {
            Gfx950LdsTransposeOperationKindV1::Stage {
                storage,
                source_slice,
                ..
            } => match effect {
                0 => Some((1, source_slice)),
                1 => Some((0, storage)),
                _ => None,
            },
            Gfx950LdsTransposeOperationKindV1::Read { storage, .. } => {
                (effect == 0).then_some((0, storage))
            }
            Gfx950LdsTransposeOperationKindV1::Current { .. }
            | Gfx950LdsTransposeOperationKindV1::Publish { .. } => None,
        },
        _ => None,
    }
}
