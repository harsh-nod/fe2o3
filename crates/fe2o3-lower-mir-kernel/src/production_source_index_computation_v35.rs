// Preserve the checked source reader while its original loan and emission
// owners still coexist. The canonical value is a locator, not its semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceIndexReaderRowV35 {
    instance: usize,
    block: u32,
    function: SemanticFunctionIdV1,
    callee: fe2o3_mir_model::semantic_mir_v1::SemanticCallableIdV1,
    argument: SsaValueV1,
    result: SsaValueV1,
    reference_type: SemanticTypeIdV1,
    witness_type: SemanticTypeIdV1,
    index_space: SemanticDisjointIndexSpaceV1,
    disjoint: bool,
    loan: usize,
    loan_site: SourceReferenceSiteV29,
    origin: usize,
    origin_instance: ProductionCallInstanceIdV1,
    origin_local: SemanticLocalIdV1,
    origin_generation: u32,
    emitted_index: ValueId,
}

fn source_index_reader_kind_v35(callable: Option<&SemanticCallableDeclV1>) -> bool {
    matches!(
        callable,
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ThreadIndexGet { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { .. },
            ..
        })
    )
}

fn source_global_invocation_intrinsic_v35(
    intrinsic: &IntrinsicOperation,
    scalar: ProductionSemanticScalarTypeV2,
) -> Option<NormalizedScalarExpressionV1> {
    (intrinsic == &IntrinsicOperation::global_id_1d()
        && scalar
            == (ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 64,
            }))
    .then_some(NormalizedScalarExpressionV1::GlobalInvocation1d { scalar })
}

fn source_index_reader_argument_v35(
    instances: &ExecutionInstancesV29<'_>,
    function: SemanticFunctionIdV1,
    block: u32,
    operand: &SemanticOperandV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
        return Err(source_index_witness_error_v29());
    };
    if !place.projections().is_empty() {
        return Err(source_index_witness_error_v29());
    }
    let occurrences = instances
        .owner()
        .occurrences_v1()
        .and_then(|rows| rows.function(function))
        .ok_or_else(source_index_witness_error_v29)?;
    let events = occurrences.events();
    let key = [block, u32::MAX];
    let (mut lo, mut hi) = (0, events.len());
    while lo < hi {
        budget.charge_work(1)?;
        let middle = lo + (hi - lo) / 2;
        if original_entry_site_key_v20(events[middle].site()) < key {
            lo = middle + 1;
        } else {
            hi = middle;
        }
    }
    let mut selected = None;
    for event in &events[lo..] {
        budget.charge_work(5)?;
        if original_entry_site_key_v20(event.site()) != key {
            break;
        }
        if event.operand() != EntryOperandV20::CallArgument(0)
            || event.role() != EntryRoleV20::BaseUse
        {
            continue;
        }
        let Some(EntryEventV20::Use { variable, value }) = event.resolved() else {
            return Err(source_index_witness_error_v29());
        };
        if variable.get() != place.local().index()
            || !event.is_promoted()
            || !event.is_reachable()
            || selected.replace(value).is_some()
        {
            return Err(source_index_witness_error_v29());
        }
    }
    selected.ok_or_else(source_index_witness_error_v29)
}

fn source_index_reader_headers_v35() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<Vec<SourceIndexReaderRowV35>>()?,
        h::<SourceIndexReaderRowV35>()?,
        h::<&SourceIndexReaderRowV35>()?,
        h::<&SourceReferenceEmissionV29<'_, '_>>()?,
        h::<SourceIndexWitnessBorrowV29>()?,
        h::<&SourceReferenceLoanV29>()?,
        h::<&SourceReferenceOriginV29>()?,
        h::<&SemanticDirectCallV1>()?,
        h::<&SemanticOperandV1>()?,
        h::<&SemanticSourceReferenceBindingV29>()?,
        h::<(SemanticTypeIdV1, SemanticDisjointIndexSpaceV1, bool)>()?,
        h::<Option<SsaValueV1>>()?,
        h::<SsaValueV1>()?,
        h::<ValueId>()?,
        h::<[u32; 2]>()?,
        h::<(usize, u32)>()?,
        h::<ProductionSourceIndexReadV35<'_, '_>>()?,
        h::<Option<ProductionSourceIndexReadV35<'_, '_>>>()?,
        h::<&ProductionSourceIndexReadV35<'_, '_>>()?,
        h::<fe2o3_mir_model::SsaEdgeIdV1>()?,
        h::<&SsaArgumentV1>()?,
        h::<ProductionSemanticExpressionV2>()?,
        h::<ProductionSemanticScalarTypeV2>()?,
        h::<&AdmittedInertSemanticMirV1>()?,
        h::<ProductionCallInstanceIdV1>()?,
        h::<&production_call_instances_v1::ProductionCallInstanceV1<'_>>()?,
        h::<SemanticFunctionIdV1>()?,
        h::<SemanticBlockIdV1>()?,
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticCallDestinationV1>()?,
        h::<&SemanticPlaceV1>()?,
        h::<bool>()?,
        h::<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>()?,
        h::<&[SsaArgumentV1]>()?,
        h::<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>>()?,
        h::<
            std::iter::Enumerate<
                std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
            >,
        >()?,
        h::<std::iter::Enumerate<std::slice::Iter<'_, Option<usize>>>>()?,
        h::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>>()?,
        argument_product_v1(8, h::<usize>()?)?,
    ])
}

fn retain_source_index_readers_v35(
    pending: &PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SourceIndexReaderRowV35>, ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    budget.charge_work(1)?;
    if !std::ptr::eq(references.plan.instances, instances) {
        return Err(source_index_witness_error_v29());
    }
    let source = instances.owner().source_semantic();
    let mut count = 0;
    for (ordinal, selected) in pending.active_instances.rows.iter().enumerate() {
        budget.charge_work(3)?;
        if selected.is_none() {
            continue;
        }
        let instance = instances
            .id_at(ordinal)
            .ok_or_else(source_index_witness_error_v29)?;
        let original = instances
            .instance(instance)
            .ok_or_else(source_index_witness_error_v29)?;
        for (block, declaration) in original.declaration().blocks().iter().enumerate() {
            budget.charge_work(3)?;
            let block = SemanticBlockIdV1::from_index(
                u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            if instances.block_reachable(instance, block) != Some(true) {
                continue;
            }
            if let SemanticTerminatorKindV1::Call(call) = declaration.terminator().kind()
                && source_index_reader_kind_v35(
                    source.callables().get(call.callee().index() as usize),
                )
            {
                count = argument_sum_v1(&[count, 1])?;
            }
        }
    }
    let mut rows = emission_vec_v1(count, budget)?;
    for (ordinal, selected) in pending.active_instances.rows.iter().enumerate() {
        budget.charge_work(4)?;
        let Some(selected) = *selected else {
            continue;
        };
        let instance = instances
            .id_at(ordinal)
            .ok_or_else(source_index_witness_error_v29)?;
        let original = instances
            .instance(instance)
            .ok_or_else(source_index_witness_error_v29)?;
        let archive = pending
            .sidecars
            .rows
            .get(selected)
            .and_then(|row| row.execution_observation.as_ref())
            .ok_or_else(source_index_witness_error_v29)?;
        archive.check_original_v29(instances, instance, budget)?;
        for (block, declaration) in original.declaration().blocks().iter().enumerate() {
            budget.charge_work(3)?;
            let block = SemanticBlockIdV1::from_index(
                u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            if instances.block_reachable(instance, block) != Some(true) {
                continue;
            }
            let SemanticTerminatorKindV1::Call(call) = declaration.terminator().kind() else {
                continue;
            };
            if !source_index_reader_kind_v35(source.callables().get(call.callee().index() as usize))
            {
                continue;
            }
            let expected = source_index_reader_contract_v29(
                original.declaration(),
                source.callables(),
                block,
                call,
                budget,
            )?;
            let operand = &call.arguments()[0];
            let argument = source_index_reader_argument_v35(
                instances,
                original.function(),
                block.index(),
                operand,
                budget,
            )?;
            let SemanticValueBindingV1::SourceReference(binding) =
                archive.lookup_original_v29(instances, instance, argument, budget)?
            else {
                return Err(source_index_witness_error_v29());
            };
            let emitted_index =
                references.index_reader_value_v29(binding, operand.ty(), expected, budget)?;
            budget.charge_work(8)?;
            let destination = call
                .destination()
                .ok_or_else(source_index_witness_error_v29)?;
            let edge = fe2o3_mir_model::SsaEdgeIdV1::new(SsaBlockIdV1::new(block.index()), 0);
            let [result] = original
                .ssa()
                .plan()
                .edge_definitions(edge)
                .ok_or_else(source_index_witness_error_v29)?
            else {
                return Err(source_index_witness_error_v29());
            };
            if call.unwind() != SemanticUnwindActionV1::Unreachable
                || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
                || !destination.place().projections().is_empty()
                || result.variable().get() != destination.place().local().index()
                || !matches!(archive.lookup_original_v29(instances, instance, result.value(), budget)?,
                    SemanticValueBindingV1::Value { id, ty } if *id == emitted_index && *ty == Type::INDEX)
            {
                return Err(source_index_witness_error_v29());
            }
            budget.charge_work(12)?;
            let SourceReferenceBindingOriginV29::SingleLoan(loan) = binding.origin else {
                return Err(source_index_witness_error_v29());
            };
            let record = references
                .plan
                .loans
                .get(loan)
                .ok_or_else(source_index_witness_error_v29)?;
            let proof = references
                .index_witnesses
                .get(loan)
                .and_then(std::cell::Cell::get)
                .ok_or_else(source_index_witness_error_v29)?;
            let origin = references
                .plan
                .origins
                .get(record.origin)
                .ok_or_else(source_index_witness_error_v29)?;
            if !origin.projections.is_empty() || rows.len() == rows.capacity() {
                return Err(source_index_witness_error_v29());
            }
            rows.push(SourceIndexReaderRowV35 {
                instance: ordinal,
                block: block.index(),
                function: original.function(),
                callee: call.callee(),
                argument,
                result: result.value(),
                reference_type: operand.ty(),
                witness_type: expected.0,
                index_space: expected.1,
                disjoint: expected.2,
                loan,
                loan_site: proof.site,
                origin: record.origin,
                origin_instance: origin.instance,
                origin_local: origin.local,
                origin_generation: origin.generation,
                emitted_index,
            });
        }
    }
    if rows.len() != count {
        return Err(source_index_witness_error_v29());
    }
    Ok(rows)
}

/// Borrowed authenticated reader and its original canonical endpoint. This
/// joins source issuance to an explicit launch law; it is not launch authority.
pub struct ProductionSourceIndexReadV35<'a, 'source> {
    owner: &'a ProductionSourceCorrespondenceV18<'source>,
    row: &'a SourceIndexReaderRowV35,
    definition: usize,
    scalar: ProductionSemanticScalarTypeV2,
}

impl ProductionSourceIndexReadV35<'_, '_> {
    pub fn original_definition(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            Ok(self.definition)
        })())
    }

    pub fn expression(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(2)?;
            if self.row.index_space != SemanticDisjointIndexSpaceV1::Index1d {
                return self
                    .owner
                    .source
                    .missing("source index reader space has no interpreted launch law");
            }
            Ok(ProductionSemanticExpressionV2::GlobalInvocation1d {
                scalar: self.scalar,
            })
        })())
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    pub fn index_reader_computation_v35(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceIndexReadV35<'_, '_>>> {
        self.retain_query((|| {
            self.query(budget)?;
            let (function, _) = self.source.instance(root, instance, budget)?;
            let source = self.source.source_ssa(budget)?;
            let declaration = source
                .source_semantic()
                .functions()
                .get(function.index() as usize)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source index reader function is absent",
                ))?;
            budget.charge_work(6)?;
            let Some(SemanticTerminatorKindV1::Call(call)) = declaration
                .blocks()
                .get(block.index() as usize)
                .map(|block| block.terminator().kind())
            else {
                return Ok(None);
            };
            if !source_index_reader_kind_v35(
                source
                    .source_semantic()
                    .callables()
                    .get(call.callee().index() as usize),
            ) {
                return Ok(None);
            }
            if !self.source.instance_active(root, instance, budget)? {
                return self
                    .source
                    .missing("source index reader instance is inactive");
            }
            let owner = self.source.root_row(root)?;
            let results =
                owner
                    .rvalue_results
                    .as_ref()
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source index reader roster is absent",
                    ))?;
            budget.charge_work(7)?;
            if results.source.semantic != *source.source_semantic_sha256()
                || results.source.ssa != source.identity()
                || results.source.root != owner.coordinates.root
                || results.ledger != budget.work_ledger_identity_v1()
                || budget.storage() < results.storage
            {
                return self.source.missing("source index reader owner differs");
            }
            budget.charge_work(argument_product_v1(
                results.index_readers.len().checked_ilog2().unwrap_or(0) as usize + 2,
                16,
            )?)?;
            let key = (instance, block.index());
            let ordinal = results
                .index_readers
                .binary_search_by_key(&key, |row| (row.instance, row.block))
                .map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "source index reader is not retained",
                    )
                })?;
            let row = &results.index_readers[ordinal];
            let expected = source_index_reader_contract_v29(
                declaration,
                source.source_semantic().callables(),
                block,
                call,
                budget,
            )
            .map_err(source_emission_error_v18)?;
            budget.charge_work(6)?;
            if row.function != function
                || row.callee != call.callee()
                || row.reference_type != call.arguments()[0].ty()
                || (row.witness_type, row.index_space, row.disjoint) != expected
                || call.unwind() != SemanticUnwindActionV1::Unreachable
            {
                return self
                    .source
                    .missing("source index reader original contract differs");
            }
            let ty = call
                .destination()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source index reader destination is absent",
                ))?
                .place()
                .ty();
            let scalar = kir_semantic_scalar_v1(
                &lower_scalar_type(source.source_semantic().types(), ty)
                    .map_err(source_emission_error_v18)?,
            )
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source index reader scalar type is unsupported",
            ))?;
            if !matches!(
                scalar,
                ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 32 | 64
                }
            ) {
                return self
                    .source
                    .missing("source index reader scalar type is unsupported");
            }
            let canonical_function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(owner.function_ordinal)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            let definition = self
                .inventory
                .definition_index_for_value(canonical_function, row.emitted_index, budget)
                .map_err(|error| {
                    ProductionSourceOwnedViewErrorV18::from(
                        fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
                    )
                })?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source index reader emitted value is absent",
                ))?;
            budget.charge_work(1)?;
            if self.inventory.definitions()[definition].ty != &Type::INDEX {
                return self
                    .source
                    .missing("source index reader emitted value type differs");
            }
            if self.ssa_scalar_definition_v30(root, instance, row.result, budget)?
                != Some(definition)
            {
                return self
                    .source
                    .missing("source index reader call-return locator differs");
            }
            Ok(Some(ProductionSourceIndexReadV35 {
                owner: self,
                row,
                definition,
                scalar,
            }))
        })())
    }
}
