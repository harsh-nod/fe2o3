/// Inert borrowed inputs to the scoped canonical memory analysis. Constructing
/// this record establishes no correspondence or proof. The private consumer
/// validates every field against one exact source/output view before use.
pub struct ProductionCanonicalMemoryAnalysisCandidateV1<'a> {
    /// Source launch-roster root, not a caller-defined graph identity.
    pub selected_root: SemanticFunctionIdV1,
    /// Selected admitted source body, including an authenticated wrapper body.
    pub selected_function: SemanticFunctionIdV1,
    /// Inert memory projection whose mandatory reports are checked again.
    pub lowering: &'a ProductionRankedKernelLoweringInputV1,
    /// Exact source sites claimed for the projection's ordinary accesses.
    pub access_sources: &'a [ProductionRankedAccessSourceV1],
    /// Generated-effect claims; unsupported families remain refusal.
    pub executable_effect_sources: &'a [ProductionRankedExecutableEffectSourceV1],
    /// Inert control annotations recorded by the existing projection emitter.
    pub control: &'a ProductionProjectionControlCandidateV1,
}

/// Complete scoped Store-value transport and conditional memory-path checks,
/// not whole-program or external-reference proof. Only the checked batch can
/// construct this result. No ranked root, lowering input, serializable receipt,
/// or legacy verification conversion is exposed.
pub struct ProductionScopedCanonicalStoreAnalysisV1<'scope, 'source, 'output> {
    view: &'scope ProductionSourceOutputOccurrencesV1<'source, 'output>,
    candidate: &'scope ProductionCanonicalMemoryAnalysisCandidateV1<'scope>,
    control: &'scope ProductionConditionalMemoryControlCoverageV1<'scope>,
    control_ordinal: usize,
    storage_floor: usize,
    output_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    accesses: &'scope [SourceOutputCanonicalAccessBindingV1],
}

struct SourceOutputCanonicalRootRangeV1 {
    accesses: std::ops::Range<usize>,
    output_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
}

struct SourceOutputCanonicalAccessBindingV1 {
    source: ProductionRankedAccessSourceV1,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    store: Option<usize>,
}

#[derive(Clone, Copy)]
struct SourceOutputCanonicalDefinitionV1 {
    value: ProductionRankedValueIdV1,
    coordinate: (u32, u32),
}

/// A borrowed checked association with one actual O memory operation. This is
/// not a standalone formal-memory discharge or an editable access program.
pub struct ProductionCanonicalMemoryAccessV1<'scope> {
    binding: &'scope SourceOutputCanonicalAccessBindingV1,
    store: Option<ProductionSourceOutputStoreValueV1<'scope>>,
}

impl ProductionCanonicalMemoryAccessV1<'_> {
    /// Source and memory-projection coordinates checked for this access.
    pub const fn source(&self) -> &ProductionRankedAccessSourceV1 {
        &self.binding.source
    }

    /// The actual O memory operation, with ordinary physical effect ordinal zero.
    pub const fn operation(&self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
        self.binding.operation
    }

    /// Source-rooted operand relation for a Store, or None for a Load.
    pub const fn store_value(&self) -> Option<&ProductionSourceOutputStoreValueV1<'_>> {
        self.store.as_ref()
    }
}

impl ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_> {
    /// Exact admitted source root in the complete ordered batch.
    pub const fn selected_root(&self) -> SemanticFunctionIdV1 {
        self.candidate.selected_root
    }

    /// Exact selected source body used by this root's occurrence queries.
    pub const fn selected_function(&self) -> SemanticFunctionIdV1 {
        self.candidate.selected_function
    }

    /// The actual immutable checked O, never a caller-supplied digest.
    pub fn output(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.view.output()
    }

    /// Inspect one fully consumed access association on the same ledger.
    pub fn access(
        &self,
        ordinal: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<ProductionCanonicalMemoryAccessV1<'_>>, ProductionSourceOutputErrorV1> {
        self.require_live_v1(budget)?;
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        let Some(binding) = self.accesses.get(ordinal) else {
            return Ok(None);
        };
        let store = binding
            .store
            .map(|row| self.view.source_output_store_value_at_v1(row, budget))
            .transpose()?;
        Ok(Some(ProductionCanonicalMemoryAccessV1 { binding, store }))
    }

    /// Size of the closed bidirectional ordinary memory-access partition.
    pub const fn access_count(&self) -> usize {
        self.accesses.len()
    }

    /// Conditional memory-path coverage, never termination/progress evidence.
    pub const fn conditional_control(&self) -> &ProductionConditionalMemoryControlCoverageV1<'_> {
        self.control
    }

    pub(crate) const fn view(&self) -> &ProductionSourceOutputOccurrencesV1<'_, '_> {
        self.view
    }

    pub(crate) fn output_function_v1(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1, ProductionSourceOutputErrorV1>
    {
        self.require_live_v1(budget)?;
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        Ok(self.output_function)
    }

    pub(crate) fn require_live_v1(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        self.view.source_output_store_value_floor_v1(budget)?;
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        if budget.storage() < self.storage_floor {
            return Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Accounting,
            ));
        }
        self.control.require_candidate_at_v1(
            self.view,
            self.candidate,
            self.control_ordinal,
            budget,
        )
    }
}

fn source_output_canonical_reports_v1(
    lowering: &ProductionRankedKernelLoweringInputV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    // Twelve borrowed finding slices, their checked sum, and fixed report joins.
    // This pays inspection only; production report engines keep their own domain.
    budget.charge_work(52).map_err(Error::Resource)?;
    let report = lowering.production_pipeline_report();
    let counts = [
        report
            .target_contract()
            .map_or(0, |report| report.findings().len()),
        lowering.tensor_layout_report().findings().len(),
        lowering.bounds_report().findings().len(),
        lowering.atomic_report().findings().len(),
        lowering.race_report().findings().len(),
        lowering.ownership_report().findings().len(),
        lowering.barrier_report().findings().len(),
        lowering.pipeline_protocol_report().findings().len(),
        lowering.workgroup_report().findings().len(),
        lowering.semantic_report().findings().len(),
        lowering.semantic_report().progress().findings().len(),
        lowering
            .semantic_report()
            .effect_refinement()
            .findings()
            .len(),
    ];
    let visits = counts
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .and_then(|count| count.checked_mul(3))
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    budget.charge_work(visits).map_err(Error::Resource)?;
    if !mandatory_generic_checks_are_clean(lowering) {
        return Err(Error::Invalid(
            "canonical memory projection has nonclean mandatory reports",
        ));
    }
    Ok(())
}

fn source_output_canonical_definitions_v1(
    lowering: &ProductionRankedKernelLoweringInputV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Vec<SourceOutputCanonicalDefinitionV1>, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget
        .reserve_storage(std::mem::size_of::<Vec<SourceOutputCanonicalDefinitionV1>>())
        .map_err(Error::Resource)?;
    let mut rows = Vec::new();
    for (block, body) in lowering.kernel().blocks().iter().enumerate() {
        budget.charge_work(1).map_err(Error::Resource)?;
        for (operation, value) in body.operations().iter().enumerate() {
            budget.charge_work(2).map_err(Error::Resource)?;
            let (ProductionRankedOperationV1::ViewInSpace { result, .. }
            | ProductionRankedOperationV1::IndexConstant { result, .. }) = value
            else {
                continue;
            };
            budget.charge_work(2).map_err(Error::Resource)?;
            let coordinate = (
                u32::try_from(block)
                    .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                u32::try_from(operation)
                    .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
            );
            assert_origin_push_v1(
                &mut rows,
                SourceOutputCanonicalDefinitionV1 {
                    value: *result,
                    coordinate,
                },
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
    }
    source_output_ranked_sort_unique_v1(&mut rows, budget, |a, b| a.value.cmp(&b.value))?;
    Ok(rows)
}

fn source_output_canonical_definition_v1(
    rows: &[SourceOutputCanonicalDefinitionV1],
    value: ProductionRankedValueV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(u32, u32), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(1).map_err(Error::Resource)?;
    let ProductionRankedValueV1::Local(value) = value else {
        return Err(Error::Invalid(
            "canonical private operand is not a local definition",
        ));
    };
    let row = assert_origin_find_v1(rows, budget, |row, budget| {
        budget.charge_work(1)?;
        Ok(row.value.cmp(&value))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid(
        "canonical private operand definition absent",
    ))?;
    budget.charge_work(1).map_err(Error::Resource)?;
    Ok(rows[row].coordinate)
}

impl<'source, 'output> ProductionSourceOutputOccurrencesV1<'source, 'output> {
    /// Consumes a complete source-root roster as scoped memory-analysis inputs.
    /// The callback receives no legacy ranked root/receipt and cannot retain the
    /// borrowed access partition. Formal/reference/final attachment obligations
    /// remain separate. The same caller ledger covers all new scratch and drops
    /// it before restoring the entry floor, including refusal and unwind.
    pub fn with_canonical_store_analysis_v1<T>(
        &self,
        candidates: &[ProductionCanonicalMemoryAnalysisCandidateV1<'_>],
        control: &ProductionConditionalMemoryControlCoverageV1<'_>,
        budget: &mut AssertOriginBudgetV1<'_>,
        body: impl for<'scope> FnOnce(
            &[ProductionScopedCanonicalStoreAnalysisV1<'scope, 'source, 'output>],
            &mut AssertOriginBudgetV1<'_>,
        ) -> Result<T, ProductionSourceOutputErrorV1>,
    ) -> Result<T, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        self.source_output_store_value_floor_v1(budget)?;
        source_output_global_scratch_scope_v1(budget, |budget| {
            budget.charge_work(2).map_err(Error::Resource)?;
            if candidates.is_empty() || candidates.len() != self.source.source_launch.roots().len()
            {
                return Err(Error::Invalid(
                    "canonical memory analysis root roster differs",
                ));
            }
            let header = std::mem::size_of::<Vec<SourceOutputCanonicalAccessBindingV1>>()
                + std::mem::size_of::<Vec<SourceOutputCanonicalRootRangeV1>>()
                + std::mem::size_of::<Vec<(u32, u32, usize)>>()
                + std::mem::size_of::<Vec<usize>>()
                + std::mem::size_of::<Vec<ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>>>();
            budget.reserve_storage(header).map_err(Error::Resource)?;
            let mut accesses = Vec::new();
            let mut ranges = Vec::new();
            let mut roots = Vec::new();
            let mut stores = Vec::new();
            for (ordinal, (candidate, root)) in candidates
                .iter()
                .zip(self.source.source_launch.roots())
                .enumerate()
            {
                budget.charge_work(3).map_err(Error::Resource)?;
                if candidate.selected_root != root.selected_root() {
                    return Err(Error::Invalid(
                        "canonical memory analysis root order differs",
                    ));
                }
                budget.charge_work(13).map_err(Error::Resource)?;
                let layout = root.layout();
                let expected_layout = ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: layout.grid_identity(),
                    global_extents: layout.global_extents(),
                    workgroup_extents: layout.workgroup_extents(),
                    subgroup_size: layout.subgroup_size(),
                    full_physical_workgroups: layout.full_physical_workgroups(),
                };
                if candidate
                    .lowering
                    .kernel()
                    .blocks()
                    .first()
                    .and_then(|block| block.operations().first())
                    != Some(&expected_layout)
                {
                    return Err(Error::Invalid(
                        "canonical memory analysis source execution layout differs",
                    ));
                }
                let output_function =
                    control.output_function_at_v1(self, candidate, ordinal, budget)?;
                source_output_canonical_reports_v1(candidate.lowering, budget)?;
                self.check_ranked_synchronization_tensor_contracts(
                    candidate.selected_root,
                    candidate.selected_function,
                    candidate.lowering,
                    budget,
                )?;
                self.check_ranked_global_allocation_values_inner_v1(
                    candidate.selected_root,
                    candidate.selected_function,
                    candidate.lowering,
                    candidate.access_sources,
                    SourceOutputRankedValueModeV1::CanonicalSourceUse,
                    budget,
                )?;
                self.check_ranked_output_effect_census(
                    candidate.selected_root,
                    candidate.selected_function,
                    candidate.lowering,
                    candidate.access_sources,
                    candidate.executable_effect_sources,
                    budget,
                )?;
                let start = accesses.len();
                self.source_output_canonical_accesses_v1(
                    candidate,
                    &mut accesses,
                    &mut stores,
                    budget,
                )?;
                assert_origin_push_v1(
                    &mut ranges,
                    SourceOutputCanonicalRootRangeV1 {
                        accesses: start..accesses.len(),
                        output_function,
                    },
                    budget,
                )
                .map_err(Error::SourceOrigin)?;
                assert_origin_push_v1(
                    &mut roots,
                    (
                        candidate.selected_root.index(),
                        candidate.selected_function.index(),
                        ordinal,
                    ),
                    budget,
                )
                .map_err(Error::SourceOrigin)?;
            }
            source_output_ranked_sort_unique_v1(&mut roots, budget, |a, b| {
                (a.0, a.1).cmp(&(b.0, b.1))
            })?;
            source_output_ranked_sort_unique_v1(&mut stores, budget, usize::cmp)?;
            for (ordinal, row) in self.store_values.rows.iter().enumerate() {
                budget.charge_work(2).map_err(Error::Resource)?;
                let root = assert_origin_find_v1(&roots, budget, |root, budget| {
                    budget.charge_work(2)?;
                    Ok((root.0, root.1).cmp(&(row.key[0], row.key[1])))
                })
                .map_err(Error::SourceOrigin)?;
                if root.is_none() {
                    continue;
                }
                let claim = assert_origin_find_v1(&stores, budget, |claim, budget| {
                    budget.charge_work(1)?;
                    Ok(claim.cmp(&ordinal))
                })
                .map_err(Error::SourceOrigin)?;
                budget.charge_work(3).map_err(Error::Resource)?;
                if claim.is_some() != (row.output.is_some() && row.executable)
                    || row.output.is_some() && !row.executable
                {
                    return Err(Error::Invalid(
                        "canonical Store partition is incomplete or nonexecutable",
                    ));
                }
            }
            let mut completed = Vec::new();
            assert_origin_reserve_v1(&mut completed, candidates.len(), budget)
                .map_err(Error::SourceOrigin)?;
            let storage_floor = budget.storage();
            for (control_ordinal, (candidate, range)) in candidates.iter().zip(&ranges).enumerate()
            {
                budget.charge_work(1).map_err(Error::Resource)?;
                assert_origin_push_v1(
                    &mut completed,
                    ProductionScopedCanonicalStoreAnalysisV1 {
                        view: self,
                        candidate,
                        control,
                        control_ordinal,
                        storage_floor,
                        output_function: range.output_function,
                        accesses: &accesses[range.accesses.clone()],
                    },
                    budget,
                )
                .map_err(Error::SourceOrigin)?;
            }
            body(&completed, budget)
        })
    }

    fn source_output_canonical_accesses_v1(
        &self,
        candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        accesses: &mut Vec<SourceOutputCanonicalAccessBindingV1>,
        stores: &mut Vec<usize>,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand;
        // Keep definition scratch on the caller scope: returned access rows are
        // also retained there, never released by a nested scratch-floor rollback.
        let mut definitions = None;
        for source in candidate.access_sources {
            budget.charge_work(4).map_err(Error::Resource)?;
            let operation = candidate
                .lowering
                .kernel()
                .blocks()
                .get(source.ranked_block() as usize)
                .and_then(|block| block.operations().get(source.ranked_operation() as usize))
                .ok_or(Error::Invalid("canonical memory ranked access absent"))?;
            let kind = source_output_census_ranked_kind_v1(operation)?
                .ok_or(Error::Invalid("canonical memory source is not an access"))?;
            let physical = match self.global_access(
                candidate.selected_root,
                candidate.selected_function,
                source.semantic_block(),
                source.semantic_statement(),
                source.semantic_access_ordinal(),
                budget,
            )? {
                ProductionSourceOutputGlobalAccessV1::Retained {
                    operation,
                    executable: true,
                    ..
                } => operation,
                ProductionSourceOutputGlobalAccessV1::Unsupported(
                    ProductionSourceOutputGlobalUnsupportedV1::PrivateMemory,
                ) => {
                    budget.charge_work(3).map_err(Error::Resource)?;
                    let ProductionRankedOperationV1::Access { view, indices, .. } = operation
                    else {
                        return Err(Error::Invalid(
                            "canonical private write requires an ordinary Access",
                        ));
                    };
                    let [index] = indices.as_slice() else {
                        return Err(Error::Invalid("canonical private write requires one index"));
                    };
                    if definitions.is_none() {
                        definitions = Some(source_output_canonical_definitions_v1(
                            candidate.lowering,
                            budget,
                        )?);
                    }
                    let rows = definitions
                        .as_deref()
                        .ok_or(Error::Invalid("canonical private definitions absent"))?;
                    let view_coordinate =
                        source_output_canonical_definition_v1(rows, *view, budget)?;
                    let index_coordinate =
                        source_output_canonical_definition_v1(rows, *index, budget)?;
                    let statement = source
                        .semantic_statement()
                        .ok_or(Error::Invalid("canonical private statement absent"))?;
                    self.check_ranked_private_array_write_allocation_index(
                        candidate.selected_root,
                        candidate.selected_function,
                        fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                            block: fe2o3_mir_model::SsaBlockIdV1::new(source.semantic_block()),
                            statement,
                        },
                        fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination,
                        candidate.lowering.kernel().function_name(),
                        candidate.lowering.kernel().blocks(),
                        (source.ranked_block(), source.ranked_operation()),
                        view_coordinate,
                        index_coordinate,
                        budget,
                    )?;
                    self.census_private_source_v1(
                        candidate.selected_root,
                        candidate.selected_function,
                        *source,
                        budget,
                    )?
                }
                _ => {
                    return Err(Error::Invalid(
                        "canonical memory access is unsupported or nonexecutable",
                    ));
                }
            };
            budget.charge_work(1).map_err(Error::Resource)?;
            let store = if kind == dialect_kernel::AccessKindAttr::Write {
                let relation = self
                    .source_output_store_value_for_output_v1(
                        candidate.selected_root,
                        candidate.selected_function,
                        OperationOperand {
                            operation: physical,
                            operand: 1,
                        },
                        budget,
                    )?
                    .ok_or(Error::Invalid(
                        "canonical memory write lacks source operand relation",
                    ))?;
                budget.charge_work(7).map_err(Error::Resource)?;
                let captured = relation.source();
                if captured.correspondence_owner() != candidate.selected_root
                    || captured.semantic_function() != candidate.selected_function
                    || captured.source_statement().0.index() != source.semantic_block()
                    || Some(captured.source_statement().1) != source.semantic_statement()
                    || !relation.executable()
                {
                    return Err(Error::Invalid(
                        "canonical memory write source operand changed",
                    ));
                }
                assert_origin_push_v1(stores, relation.ordinal, budget)
                    .map_err(Error::SourceOrigin)?;
                Some(relation.ordinal)
            } else {
                None
            };
            assert_origin_push_v1(
                accesses,
                SourceOutputCanonicalAccessBindingV1 {
                    source: *source,
                    operation: physical,
                    store,
                },
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
        Ok(())
    }
}
