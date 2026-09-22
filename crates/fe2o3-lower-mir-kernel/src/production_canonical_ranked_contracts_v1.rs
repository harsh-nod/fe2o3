/// Ordered source launch bound to one actual graph kernel, before target binding.
pub struct ProductionCanonicalRankedLaunchV1<'a> {
    kernel: &'a fe2o3_kernel_analysis::CanonicalKirKernelRefV1<'a>,
    source: &'a crate::ProductionSourceLaunchRootV1,
}
impl ProductionCanonicalRankedLaunchV1<'_> {
    /// Actual kernel in the shared inventory.
    pub const fn kernel(&self) -> &fe2o3_kernel_analysis::CanonicalKirKernelRefV1<'_> {
        self.kernel
    }
    /// Complete source layout, including grid identity and physical-workgroup fact.
    pub const fn source(&self) -> &crate::ProductionSourceLaunchRootV1 {
        self.source
    }
}
/// One retained assertion occurrence; no assertion predicate is assumed true.
pub struct ProductionCanonicalRankedAssertionV1 {
    span: usize,
    binding: SemanticKirAssertConditionBindingV1,
}
impl ProductionCanonicalRankedAssertionV1 {
    /// Index of the typed source terminator span.
    pub const fn span(&self) -> usize {
        self.span
    }
    /// Exact emitted condition/success/failure, or explicit source-rule elision.
    pub const fn binding(&self) -> SemanticKirAssertConditionBindingV1 {
        self.binding
    }
}
struct CrContractsV1<'a> {
    launches: Vec<ProductionCanonicalRankedLaunchV1<'a>>,
    assertions: Vec<ProductionCanonicalRankedAssertionV1>,
    catalog: fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
}
fn cr_build_contracts_v1<'a>(
    owner: &'a ProductionPreRankedKirOwnerV1,
    inventory: &'a CanonicalKirInventoryV1<'a>,
    rows: &CrSourceRowsV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<CrContractsV1<'a>> {
    let mut launches = cr_vec_v1(owner.source_launch.roots().len(), budget)?;
    for (kernel, source) in inventory.kernels().iter().zip(owner.source_launch.roots()) {
        cr_push_v1(
            &mut launches,
            ProductionCanonicalRankedLaunchV1 { kernel, source },
            budget,
        )?;
    }
    let mut assertions = cr_vec_v1(owner.assert_origins().source_site_count(), budget)?;
    for (span, row) in rows.spans.iter().enumerate() {
        budget.charge_work(1)?;
        if let ProductionCanonicalRankedSourceSiteV1::Terminator {
            span: source,
            source: term,
        } = row.site
        {
            if matches!(term.kind(), SemanticTerminatorKindV1::Assert { .. }) {
                let binding = owner
                    .assert_origins()
                    .assert_condition(
                        source.correspondence_owner,
                        source.semantic_function,
                        source.semantic_block,
                        budget,
                    )
                    .map_err(ProductionSemanticKirErrorV1::from)?;
                cr_push_v1(
                    &mut assertions,
                    ProductionCanonicalRankedAssertionV1 { span, binding },
                    budget,
                )?;
            }
        }
    }
    let (catalog, storage) = source_catalog_from_live_v1(
        owner.semantic_ssa.source_semantic(),
        SourceCatalogCorrespondenceV1(&owner.correspondence),
        inventory,
        budget,
    )?;
    budget.reserve_storage(storage.retained_storage())?;
    Ok(CrContractsV1 {
        launches,
        assertions,
        catalog,
    })
}
fn cr_check_contracts_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    rows: &CrSourceRowsV1<'_>,
    contracts: &CrContractsV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<()> {
    let semantic = owner.semantic_ssa.source_semantic();
    budget.charge_work(4)?;
    if contracts.launches.len() != semantic.roots().len()
        || inventory.kernels().len() != semantic.roots().len()
        || owner.source_launch.roots().len() != semantic.roots().len()
        || owner.source_launch.semantic_sha256() != semantic.semantic_sha256().as_bytes()
    {
        return Err(cr_invalid_v1("ordered source launch roster"));
    }
    for (ordinal, root) in semantic.roots().iter().enumerate() {
        budget.charge_work(160)?;
        let row = &contracts.launches[ordinal];
        let source = &owner.source_launch.roots()[ordinal];
        let kernel = &inventory.kernels()[ordinal];
        let function = &semantic.functions()[root.index() as usize];
        let entry = function
            .kernel_entry()
            .ok_or_else(|| cr_invalid_v1("source launch entry"))?;
        let layout = crate::ProductionSourceExecutionLayoutV1::try_from_source(
            semantic.target().architecture(),
            function,
            source.source_launch(),
        )
        .map_err(|_| cr_invalid_v1("source launch layout"))?;
        budget.charge_work(argument_sum_v1(&[
            kernel.kernel.id.as_str().len(),
            entry.export_symbol().as_bytes().len(),
        ])?)?;
        if !std::ptr::eq(row.source, source)
            || !std::ptr::eq(row.kernel, kernel)
            || source.selected_root() != *root
            || source.semantic_root_identity() != function.identity()
            || source.kernel_binding() != *entry.kernel_binding_identity().as_bytes()
            || source.layout() != layout
            || kernel.ordinal as usize != ordinal
            || kernel.kernel.id.as_str().as_bytes() != entry.export_symbol().as_bytes()
        {
            return Err(cr_invalid_v1("source launch identity or layout"));
        }
    }
    let mut next = 0;
    for (span_index, row) in rows.spans.iter().enumerate() {
        budget.charge_work(1)?;
        let ProductionCanonicalRankedSourceSiteV1::Terminator { span, source } = row.site else {
            continue;
        };
        let SemanticTerminatorKindV1::Assert {
            expected, target, ..
        } = source.kind()
        else {
            continue;
        };
        let actual = contracts
            .assertions
            .get(next)
            .ok_or_else(|| cr_invalid_v1("missing assertion"))?;
        let binding = owner
            .assert_origins()
            .assert_condition(
                span.correspondence_owner,
                span.semantic_function,
                span.semantic_block,
                budget,
            )
            .map_err(ProductionSemanticKirErrorV1::from)?;
        budget.charge_work(6)?;
        if actual.span != span_index
            || actual.binding != binding
            || binding.block() != row.block
            || binding.expected() != *expected
            || binding.semantic_success() != target.target()
        {
            return Err(cr_invalid_v1("assertion source/graph binding"));
        }
        next += 1;
    }
    if next != contracts.assertions.len() || next != owner.assert_origins().source_site_count() {
        return Err(cr_invalid_v1("extra assertion or missing source alias"));
    }
    // Genuine nonempty definitions and bindings use the existing independent
    // actual-graph catalog checker. Never substitute an empty catalog.
    cr_check_catalog_source_v1(owner, inventory, rows, &contracts.catalog, budget)?;
    let floor = budget.storage();
    let (checked, storage) = fe2o3_kernel_analysis::check_kernel_ir_contract_catalog_v1(
        inventory,
        &contracts.catalog,
        budget,
    )
    .map_err(ProductionSourceOutputCatalogErrorV1::Binding)?;
    budget.reserve_storage(storage.retained_storage())?;
    drop(checked);
    budget.release_storage(budget.storage() - floor)?;
    Ok(())
}

fn cr_check_catalog_source_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    rows: &CrSourceRowsV1<'_>,
    catalog: &fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<()> {
    let semantic = owner.semantic_ssa.source_semantic();
    if catalog.semantic_source() != semantic.semantic_sha256().as_bytes() {
        return Err(cr_invalid_v1("catalog source identity"));
    }
    let floor = budget.storage();
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of::<Vec<(bool, bool)>>(),
        std::mem::size_of::<Vec<bool>>(),
    ])?)?;
    let mut seen = cr_vec_v1::<(bool, bool)>(catalog.definitions().len(), budget)?;
    budget.charge_work(catalog.definitions().len())?;
    seen.resize(catalog.definitions().len(), (false, false));
    let mut bindings = cr_vec_v1::<bool>(catalog.bindings().len(), budget)?;
    budget.charge_work(catalog.bindings().len())?;
    bindings.resize(catalog.bindings().len(), false);
    for callable in semantic.callables() {
        budget.charge_work(1)?;
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable else {
            continue;
        };
        let pipeline = match operation {
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { pipeline, .. }
            | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead { pipeline, .. }
            | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite { pipeline, .. } => {
                pipeline.index()
            }
            _ => continue,
        };
        let index = assert_origin_find_v1(catalog.definitions(), budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.semantic_pipeline_type.cmp(&pipeline))
        })
        .map_err(call_index_error_v1)?
        .ok_or_else(|| cr_invalid_v1("missing source pipeline definition"))?;
        let definition = &catalog.definitions()[index];
        budget.charge_work(8)?;
        if definition.key as usize != index {
            return Err(cr_invalid_v1("pipeline key order"));
        }
        match operation {
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
                buffers,
                elements,
                prefetch_distance,
                ..
            } => {
                if definition.buffers != *buffers
                    || definition.elements != *elements
                    || definition.prefetch_distance != *prefetch_distance
                {
                    return Err(cr_invalid_v1("source pipeline geometry"));
                }
                seen[index].0 = true;
            }
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead { element, .. }
            | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite { element, .. } => {
                let layout = semantic.types()[element.index() as usize].layout();
                let bytes = layout
                    .size_bytes()
                    .ok_or_else(|| cr_invalid_v1("dynamic pipeline payload"))?;
                if definition.semantic_payload_type != element.index()
                    || definition.source_size_bytes != bytes
                    || definition.source_alignment_bytes != layout.alignment_bytes()
                    || u64::from(definition.packed_bits)
                        != bytes.checked_mul(8).ok_or(ArgumentResourceV1::Arithmetic)?
                {
                    return Err(cr_invalid_v1("source pipeline payload"));
                }
                seen[index].1 = true;
            }
            _ => unreachable!("selected only source pipeline declaration families"),
        }
    }
    for span in &rows.spans {
        let ProductionCanonicalRankedSourceSiteV1::Terminator { source, .. } = span.site else {
            continue;
        };
        budget.charge_work(1)?;
        let SemanticTerminatorKindV1::Call(call) = source.kind() else {
            continue;
        };
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { pipeline, .. },
            ..
        } = &semantic.callables()[call.callee().index() as usize]
        else {
            continue;
        };
        let mut found = false;
        for operation in span.operations.clone() {
            budget.charge_work(1)?;
            let actual = &inventory.operations()[operation];
            if !matches!(actual.operation.kind, OperationKind::WorkgroupMemory(_)) {
                continue;
            }
            if found {
                return Err(cr_invalid_v1("multiple source pipeline allocations"));
            }
            found = true;
            let [value] = actual.operation.results.as_slice() else {
                return Err(cr_invalid_v1("pipeline result arity"));
            };
            let key = (actual.coordinate.block.function.0, value.id.0);
            let index = assert_origin_find_v1(catalog.bindings(), budget, |row, budget| {
                budget.charge_work(2)?;
                Ok((row.function, row.storage).cmp(&key))
            })
            .map_err(call_index_error_v1)?
            .ok_or_else(|| cr_invalid_v1("missing source pipeline allocation"))?;
            let binding = catalog.bindings()[index];
            let definition = catalog
                .definitions()
                .get(binding.key as usize)
                .ok_or_else(|| cr_invalid_v1("pipeline binding key"))?;
            budget.charge_work(4)?;
            if binding.block != actual.coordinate.block.block
                || binding.operation != actual.coordinate.operation
                || definition.semantic_pipeline_type != pipeline.index()
            {
                return Err(cr_invalid_v1("source pipeline allocation identity"));
            }
            bindings[index] = true;
        }
        if !found {
            return Err(cr_invalid_v1("unemitted source pipeline allocation"));
        }
    }
    budget.charge_work(argument_sum_v1(&[seen.len(), bindings.len()])?)?;
    if seen.iter().any(|flags| *flags != (true, true)) || bindings.iter().any(|seen| !seen) {
        return Err(cr_invalid_v1("extra source catalog definition or binding"));
    }
    drop(bindings);
    drop(seen);
    budget.release_storage(budget.storage() - floor)?;
    Ok(())
}
