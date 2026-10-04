// Source catalog construction after exact B0 source replay and N/B coordinates.
// The producer below is the ROOT live-only algorithm, without wire or lineage subjects.

/// Failure of source-derived catalog construction or checked output placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceOutputCatalogErrorV1 {
    /// The shared canonical ledger refused work, storage or arithmetic.
    Resource(AssertOriginResourceV1),
    /// The existing canonical catalog codec rejected the constructed rows.
    Codec(fe2o3_kernel_ir::KernelIrContractCatalogErrorV1),
    /// The borrowed actual graph inventory rejected a lookup.
    Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1),
    /// The source-derived input catalog failed exact graph binding.
    Binding(fe2o3_kernel_analysis::KernelIrContractCatalogBindingErrorV1),
    /// The independent checked transition rejected output catalog transport.
    Transport(fe2o3_kernel_analysis::KernelIrContractCatalogTransportErrorV1),
    /// A required source, coordinate, payload, or allocation rule failed.
    Invalid(&'static str),
}
impl fmt::Display for ProductionSourceOutputCatalogErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::Codec(error) => error.fmt(formatter),
            Self::Inventory(error) => error.fmt(formatter),
            Self::Binding(error) => error.fmt(formatter),
            Self::Transport(error) => error.fmt(formatter),
            Self::Invalid(reason) => write!(formatter, "source/output catalog rejected: {reason}"),
        }
    }
}
impl std::error::Error for ProductionSourceOutputCatalogErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

#[derive(Debug)]
struct SourceOutputCatalogsV1 {
    source: fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
    transported: fe2o3_kernel_analysis::TransportedKernelIrContractCatalogV1,
}

// The only production caller is the B0 constructor, after complete source replay.
// This helper additionally checks actual endpoint pointers; it accepts no inert
// correspondence roster, caller catalog, source digest, or substituted graph.
fn source_output_catalogs_after_replay_v1(
    source: &ProductionPreRankedKirOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(SourceOutputCatalogsV1, usize), ProductionSourceOutputCatalogErrorV1> {
    use ProductionSourceOutputCatalogErrorV1 as Error;
    let floor = budget.storage();
    let result = (|| {
        // Entry, two exact pointer checks, and wrapper size arithmetic.
        budget.charge_work(5).map_err(Error::Resource)?;
        if !std::ptr::eq(source.executable(), coordinates.input())
            || !std::ptr::eq(coordinates.output(), transition.input().owner())
        {
            return Err(Error::Invalid("catalog endpoint custody"));
        }
        let wrapper = std::mem::size_of::<SourceOutputCatalogsV1>()
            .checked_sub(std::mem::size_of::<
                fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
            >())
            .and_then(|n| {
                n.checked_sub(std::mem::size_of::<
                    fe2o3_kernel_analysis::TransportedKernelIrContractCatalogV1,
                >())
            })
            .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
        budget.reserve_storage(wrapper).map_err(Error::Resource)?;
        let (catalog, catalog_storage) = source_catalog_from_live_v1(
            source.semantic_ssa.source_semantic(),
            SourceCatalogCorrespondenceV1(&source.correspondence),
            transition.input(),
            budget,
        )?;
        budget
            .reserve_storage(catalog_storage.retained_storage())
            .map_err(Error::Resource)?;
        let (checked_catalog, checked_storage) =
            fe2o3_kernel_analysis::check_kernel_ir_contract_catalog_v1(
                transition.input(),
                &catalog,
                budget,
            )
            .map_err(Error::Binding)?;
        budget
            .reserve_storage(checked_storage.retained_storage())
            .map_err(Error::Resource)?;
        let (transported, transported_storage) =
            fe2o3_kernel_analysis::transport_kernel_ir_contract_catalog_v1(
                transition,
                &checked_catalog,
                budget,
            )
            .map_err(Error::Transport)?;
        budget
            .reserve_storage(transported_storage.retained_storage())
            .map_err(Error::Resource)?;
        #[allow(
            clippy::drop_non_drop,
            reason = "End the borrowed witness lifetime before releasing its ledger reservation"
        )]
        drop(checked_catalog);
        budget
            .release_storage(checked_storage.retained_storage())
            .map_err(Error::Resource)?;
        budget.charge_work(2).map_err(Error::Resource)?;
        let retained = wrapper
            .checked_add(catalog_storage.retained_storage())
            .and_then(|n| n.checked_add(transported_storage.retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        Ok((
            SourceOutputCatalogsV1 {
                source: catalog,
                transported,
            },
            retained,
        ))
    })();
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
    budget.release_storage(release).map_err(Error::Resource)?;
    result
}

fn source_catalog_error_v1(reason: &'static str) -> ProductionSourceOutputCatalogErrorV1 {
    ProductionSourceOutputCatalogErrorV1::Invalid(reason)
}
fn source_catalog_resource_v1(
    error: AssertOriginResourceV1,
) -> ProductionSourceOutputCatalogErrorV1 {
    ProductionSourceOutputCatalogErrorV1::Resource(error)
}

#[derive(Clone, Copy)]
struct SourceCatalogFunctionV1<'a> {
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    target: &'a str,
}

#[derive(Clone, Copy)]
struct SourceCatalogSpanV1 {
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    target_block: BlockId,
    first: u32,
    count: u32,
}

#[derive(Clone, Copy)]
struct SourceCatalogCorrespondenceV1<'a>(&'a SemanticKirCorrespondenceV1);
impl<'a> SourceCatalogCorrespondenceV1<'a> {
    fn function_count(
        self,
        _budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<usize, ProductionSourceOutputCatalogErrorV1> {
        Ok(self.0.lowered_functions().len())
    }

    fn visit_functions(
        self,
        budget: &mut AssertOriginBudgetV1<'_>,
        mut visit: impl FnMut(
            SourceCatalogFunctionV1<'a>,
            &mut AssertOriginBudgetV1<'_>,
        ) -> Result<(), ProductionSourceOutputCatalogErrorV1>,
    ) -> Result<(), ProductionSourceOutputCatalogErrorV1> {
        for row in self.0.lowered_functions() {
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            visit(
                SourceCatalogFunctionV1 {
                    owner: row.correspondence_owner(),
                    function: row.semantic_function(),
                    target: row.kernel_ir_function().as_str(),
                },
                budget,
            )?;
        }
        Ok(())
    }

    fn visit_spans(
        self,
        budget: &mut AssertOriginBudgetV1<'_>,
        mut visit: impl FnMut(
            SourceCatalogSpanV1,
            &mut AssertOriginBudgetV1<'_>,
        ) -> Result<(), ProductionSourceOutputCatalogErrorV1>,
    ) -> Result<(), ProductionSourceOutputCatalogErrorV1> {
        for row in self.0.terminator_operation_spans() {
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            visit(
                SourceCatalogSpanV1 {
                    owner: row.correspondence_owner(),
                    function: row.semantic_function(),
                    block: row.semantic_block(),
                    target_block: row.kernel_ir_block(),
                    first: row.first_operation_ordinal(),
                    count: row.operation_count(),
                },
                budget,
            )?;
        }
        Ok(())
    }
}

fn source_catalog_from_live_v1(
    semantic: &AdmittedInertSemanticMirV1,
    correspondence: SourceCatalogCorrespondenceV1<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
        fe2o3_kernel_ir::KernelIrContractCatalogStorageV1,
    ),
    ProductionSourceOutputCatalogErrorV1,
> {
    let floor = budget.storage();
    let result = (|| {
        use fe2o3_kernel_ir::{
            InertCanonicalKernelIrContractCatalogV1 as Catalog,
            KernelIrPipelineContractDefinitionV1 as Definition,
            KernelIrPipelineStorageBindingV1 as Binding,
        };
        let source_headers = 4 * std::mem::size_of::<Vec<()>>();
        budget
            .reserve_storage(source_headers)
            .map_err(source_catalog_resource_v1)?;
        let payload_count = semantic.callables().iter().try_fold(0_usize, |count, callable| {
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            let is_payload = matches!(callable, SemanticCallableDeclV1::CompilerIntrinsic { operation: SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead { .. } | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite { .. }, .. });
            count.checked_add(usize::from(is_payload)).ok_or_else(|| source_catalog_error_v1("pipeline payload count overflow"))
        })?;
        let mut payloads = source_catalog_vec_v1::<(u32, u32)>(payload_count, budget)?;
        for callable in semantic.callables() {
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            if let SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead { pipeline, element }
                    | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite { pipeline, element },
                ..
            } = callable
            {
                payloads.push((pipeline.index(), element.index()));
            }
        }
        source_catalog_sort_v1(&mut payloads, |row| *row, budget)?;
        let mut payload_write = 0_usize;
        for read in 0..payloads.len() {
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            let row = payloads[read];
            if payload_write != 0 && payloads[payload_write - 1].0 == row.0 {
                if payloads[payload_write - 1] != row {
                    return Err(source_catalog_error_v1(
                        "one pipeline type has conflicting payload identities",
                    ));
                }
            } else {
                payloads[payload_write] = row;
                payload_write += 1;
            }
        }
        payloads.truncate(payload_write);
        let create_count = semantic.callables().iter().try_fold(0_usize, |count, callable| {
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            count.checked_add(usize::from(matches!(callable, SemanticCallableDeclV1::CompilerIntrinsic { operation: SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { .. }, .. }))).ok_or_else(|| source_catalog_error_v1("pipeline create count overflow"))
        })?;
        let mut definitions = source_catalog_vec_v1::<Definition>(create_count, budget)?;
        for callable in semantic.callables() {
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            let SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
                        pipeline,
                        buffers,
                        elements,
                        prefetch_distance,
                        ..
                    },
                ..
            } = callable
            else {
                continue;
            };
            let payload = source_catalog_find_v1(&payloads, pipeline.index(), |row| row.0, budget)?
                .ok_or_else(|| {
                    source_catalog_error_v1("pipeline create lacks an exact payload contract")
                })?
                .1;
            let declaration = semantic
                .types()
                .get(payload as usize)
                .ok_or_else(|| source_catalog_error_v1("pipeline payload type is missing"))?;
            let source_size_bytes = declaration.layout().size_bytes().ok_or_else(|| {
                source_catalog_error_v1("pipeline payload layout is dynamically sized")
            })?;
            let packed_bits = source_size_bytes
                .checked_mul(8)
                .and_then(|bits| u16::try_from(bits).ok())
                .ok_or_else(|| source_catalog_error_v1("pipeline payload layout width overflow"))?;
            definitions.push(Definition {
                key: 0,
                semantic_pipeline_type: pipeline.index(),
                semantic_payload_type: payload,
                buffers: *buffers,
                elements: *elements,
                prefetch_distance: *prefetch_distance,
                packed_bits,
                source_size_bytes,
                source_alignment_bytes: declaration.layout().alignment_bytes(),
            });
        }
        source_catalog_sort_v1(&mut definitions, |row| row.semantic_pipeline_type, budget)?;
        let mut definition_write = 0_usize;
        for read in 0..definitions.len() {
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            let row = definitions[read];
            if definition_write != 0
                && definitions[definition_write - 1].semantic_pipeline_type
                    == row.semantic_pipeline_type
            {
                if definitions[definition_write - 1] != row {
                    return Err(source_catalog_error_v1(
                        "one pipeline type has conflicting creation geometry",
                    ));
                }
            } else {
                definitions[definition_write] = row;
                definition_write += 1;
            }
        }
        definitions.truncate(definition_write);
        if definitions.len() != payloads.len() {
            return Err(source_catalog_error_v1(
                "pipeline payload and creation catalogs differ",
            ));
        }
        for (index, (row, payload)) in definitions.iter_mut().zip(&payloads).enumerate() {
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            if row.semantic_pipeline_type != payload.0 {
                return Err(source_catalog_error_v1(
                    "pipeline catalog key differs from lowering key",
                ));
            }
            row.key = u32::try_from(index)
                .map_err(|_| source_catalog_error_v1("pipeline contract key overflow"))?;
        }
        let function_count = correspondence.function_count(budget)?;
        let mut functions =
            source_catalog_vec_v1::<SourceCatalogFunctionV1<'_>>(function_count, budget)?;
        correspondence.visit_functions(budget, |record, _budget| {
            functions.push(record);
            Ok(())
        })?;
        source_catalog_sort_v1(
            &mut functions,
            |record| (record.owner, record.function),
            budget,
        )?;
        for pair in functions.windows(2) {
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            if (pair[0].owner, pair[0].function) == (pair[1].owner, pair[1].function) {
                return Err(source_catalog_error_v1(
                    "pipeline source function has ambiguous correspondence",
                ));
            }
        }
        let mut binding_count = 0_usize;
        correspondence.visit_spans(budget, |span, _budget| {
            binding_count = binding_count
                .checked_add(usize::from(
                    source_pipeline_create_for_span_v1(semantic, &span)?.is_some(),
                ))
                .ok_or_else(|| source_catalog_error_v1("pipeline binding count overflow"))?;
            Ok(())
        })?;
        let mut bindings = source_catalog_vec_v1::<Binding>(binding_count, budget)?;
        correspondence.visit_spans(budget, |span, budget| {
            let Some(pipeline) = source_pipeline_create_for_span_v1(semantic, &span)? else {
                return Ok(());
            };
            let definition = source_catalog_find_v1(
                &definitions,
                pipeline,
                |row| row.semantic_pipeline_type,
                budget,
            )?
            .ok_or_else(|| {
                source_catalog_error_v1("emitted pipeline allocation lacks its catalog definition")
            })?;
            let source_function = source_catalog_find_v1(
                &functions,
                (span.owner, span.function),
                |record| (record.owner, record.function),
                budget,
            )?
            .ok_or_else(|| {
                source_catalog_error_v1("pipeline source function lacks correspondence")
            })?;
            let function = inventory
                .function_for_name(source_function.target, budget)
                .map_err(ProductionSourceOutputCatalogErrorV1::Inventory)?
                .ok_or_else(|| source_catalog_error_v1("pipeline target function is missing"))?;
            let block = inventory
                .block_for_id(function.coordinate, span.target_block, budget)
                .map_err(ProductionSourceOutputCatalogErrorV1::Inventory)?
                .ok_or_else(|| source_catalog_error_v1("pipeline allocation block is missing"))?;
            let first = span.first as usize;
            let end = first
                .checked_add(span.count as usize)
                .ok_or_else(|| source_catalog_error_v1("pipeline operation span overflow"))?;
            let operations = block.block.operations.get(first..end).ok_or_else(|| {
                source_catalog_error_v1("pipeline operation span is outside its exact block")
            })?;
            let mut allocation = None;
            for (relative, operation) in operations.iter().enumerate() {
                budget.charge_work(1).map_err(source_catalog_resource_v1)?;
                if matches!(operation.kind, OperationKind::WorkgroupMemory(_)) {
                    let [result] = operation.results.as_slice() else {
                        return Err(source_catalog_error_v1(
                            "pipeline allocation result arity changed",
                        ));
                    };
                    if allocation.replace((first + relative, result.id)).is_some() {
                        return Err(source_catalog_error_v1(
                            "pipeline create emitted multiple physical allocations",
                        ));
                    }
                }
            }
            let (operation, storage) = allocation.ok_or_else(|| {
                source_catalog_error_v1("pipeline create emitted no physical allocation")
            })?;
            bindings.push(Binding {
                function: function.coordinate.0,
                storage: storage.0,
                key: definition.key,
                block: block.coordinate.block,
                operation: u32::try_from(operation)
                    .map_err(|_| source_catalog_error_v1("pipeline operation ordinal overflow"))?,
            });
            Ok(())
        })?;
        source_catalog_sort_v1(&mut bindings, |row| *row, budget)?;
        let mut binding_write = 0_usize;
        for read in 0..bindings.len() {
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            let row = bindings[read];
            if binding_write != 0
                && (
                    bindings[binding_write - 1].function,
                    bindings[binding_write - 1].storage,
                ) == (row.function, row.storage)
            {
                if bindings[binding_write - 1] != row {
                    return Err(source_catalog_error_v1(
                        "source aliases disagree about one physical pipeline allocation",
                    ));
                }
            } else {
                bindings[binding_write] = row;
                binding_write += 1;
            }
        }
        bindings.truncate(binding_write);
        let (catalog, catalog_storage) = Catalog::from_rows_with_budget(
            *semantic.semantic_sha256().as_bytes(),
            &definitions,
            &bindings,
            budget,
        )
        .map_err(ProductionSourceOutputCatalogErrorV1::Codec)?;
        budget
            .reserve_storage(catalog_storage.retained_storage())
            .map_err(source_catalog_resource_v1)?;
        // The caller checks this catalog against this same borrowed inventory
        // and keeps that checked view alive through output transport.
        drop(payloads);
        drop(definitions);
        drop(bindings);
        drop(functions);
        let scratch = payload_count
            .checked_mul(std::mem::size_of::<(u32, u32)>())
            .and_then(|n| {
                n.checked_add(create_count.checked_mul(std::mem::size_of::<Definition>())?)
            })
            .and_then(|n| n.checked_add(binding_count.checked_mul(std::mem::size_of::<Binding>())?))
            .and_then(|n| {
                n.checked_add(
                    function_count
                        .checked_mul(std::mem::size_of::<SourceCatalogFunctionV1<'_>>())?,
                )
            })
            .and_then(|n| n.checked_add(source_headers))
            .ok_or_else(|| source_catalog_error_v1("source catalog scratch overflow"))?;
        budget
            .release_storage(scratch)
            .map_err(source_catalog_resource_v1)?;
        Ok((catalog, catalog_storage))
    })();
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or_else(|| source_catalog_error_v1("source catalog storage accounting"))?;
    budget
        .release_storage(release)
        .map_err(source_catalog_resource_v1)?;
    result
}

fn source_pipeline_create_for_span_v1(
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    span: &SourceCatalogSpanV1,
) -> Result<Option<u32>, ProductionSourceOutputCatalogErrorV1> {
    let function = semantic
        .functions()
        .get(span.function.index() as usize)
        .ok_or_else(|| source_catalog_error_v1("pipeline source function coordinate"))?;
    let block = function
        .blocks()
        .get(span.block.index() as usize)
        .ok_or_else(|| source_catalog_error_v1("pipeline source block coordinate"))?;
    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
        return Ok(None);
    };
    let callable = semantic
        .callables()
        .get(call.callee().index() as usize)
        .ok_or_else(|| source_catalog_error_v1("pipeline source callable coordinate"))?;
    Ok(match callable {
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { pipeline, .. },
            ..
        } => Some(pipeline.index()),
        _ => None,
    })
}

fn source_catalog_vec_v1<T>(
    count: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Vec<T>, ProductionSourceOutputCatalogErrorV1> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| source_catalog_error_v1("source catalog allocation size overflow"))?;
    budget
        .reserve_storage(bytes)
        .map_err(source_catalog_resource_v1)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count).map_err(|_| {
        source_catalog_resource_v1(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Allocation,
        )
    })?;
    Ok(rows)
}

fn source_catalog_find_v1<'a, T, K: Ord>(
    rows: &'a [T],
    sought: K,
    key: impl Fn(&T) -> K,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Option<&'a T>, ProductionSourceOutputCatalogErrorV1> {
    let mut low = 0;
    let mut high = rows.len();
    while low < high {
        budget.charge_work(1).map_err(source_catalog_resource_v1)?;
        let middle = low + (high - low) / 2;
        match key(&rows[middle]).cmp(&sought) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(Some(&rows[middle])),
        }
    }
    Ok(None)
}

fn source_catalog_sort_v1<T, K: Ord>(
    rows: &mut [T],
    key: impl Fn(&T) -> K,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputCatalogErrorV1> {
    fn sift<T, K: Ord>(
        rows: &mut [T],
        mut root: usize,
        end: usize,
        key: &impl Fn(&T) -> K,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputCatalogErrorV1> {
        loop {
            let left = root
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| source_catalog_error_v1("source catalog sort index overflow"))?;
            if left >= end {
                return Ok(());
            }
            let mut child = left;
            if left + 1 < end {
                budget.charge_work(1).map_err(source_catalog_resource_v1)?;
                if key(&rows[left]) < key(&rows[left + 1]) {
                    child = left + 1;
                }
            }
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            if key(&rows[root]) >= key(&rows[child]) {
                return Ok(());
            }
            budget.charge_work(1).map_err(source_catalog_resource_v1)?;
            rows.swap(root, child);
            root = child;
        }
    }
    for root in (0..rows.len() / 2).rev() {
        sift(rows, root, rows.len(), &key, budget)?;
    }
    for end in (1..rows.len()).rev() {
        budget.charge_work(1).map_err(source_catalog_resource_v1)?;
        rows.swap(0, end);
        sift(rows, 0, end, &key, budget)?;
    }
    Ok(())
}
