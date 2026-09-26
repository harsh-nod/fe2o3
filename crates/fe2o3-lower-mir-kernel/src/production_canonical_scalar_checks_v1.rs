/// Fresh borrowed original-to-final lineage. No constructor or proof grants.
/// Original coordinates never designate final occurrences without this relation.
pub struct ProductionCanonicalScalarLineageV1<'a> {
    original: &'a CanonicalKirInventoryV1<'a>,
    output: &'a CanonicalKirInventoryV1<'a>,
    rows: &'a CsLineageV1,
    guard: &'a CrGuardV1,
}
impl ProductionCanonicalScalarLineageV1<'_> {
    fn get<'a, T>(
        &self,
        rows: &'a [T],
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&'a T> {
        self.guard.query(budget)?;
        rows.get(ordinal)
            .ok_or_else(|| self.guard.missing("scalar lineage ordinal").into())
    }
    /// Exact N function for a final function ordinal, not ordinal equality.
    pub fn function_origin(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<CsFunctionV1> {
        Ok(*self.get(&self.rows.functions, ordinal, budget)?)
    }
    /// Exact surviving source operation or first actual constant synthesis site.
    pub fn operation_origin(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<ProductionCanonicalScalarOperationOriginV1> {
        Ok(*self.get(&self.rows.operations, ordinal, budget)?)
    }
    /// Complete original definition count, including definitions with no output.
    pub fn original_definition_count(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<usize> {
        self.guard.query(budget)?;
        Ok(self.original.definitions().len())
    }
    /// Count this original definition's actual sparse descendants; zero is valid.
    /// Each inspected sparse row is charged, with no Cartesian table allocated.
    pub fn definition_descendant_count(
        &self,
        original: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<usize> {
        self.get(self.original.definitions(), original, budget)?;
        budget
            .charge_work(self.rows.definitions.len())
            .map_err(|error| self.guard.fail(CrQueryFailureV1::Resource(error)))?;
        Ok(self
            .rows
            .definitions
            .iter()
            .filter(|r| r.original == original)
            .count())
    }
    /// One descendant in final inventory order; value substitution grants no
    /// permission from other source operations that coalesce at that definition.
    pub fn definition_descendant(
        &self,
        original: usize,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<ProductionCanonicalScalarDefinitionDescendantV1> {
        let source = self
            .get(self.original.definitions(), original, budget)?
            .coordinate;
        let mut found = 0;
        for row in &self.rows.definitions {
            budget
                .charge_work(1)
                .map_err(|error| self.guard.fail(CrQueryFailureV1::Resource(error)))?;
            if row.original != original {
                continue;
            }
            if found == ordinal {
                return Ok(ProductionCanonicalScalarDefinitionDescendantV1 {
                    original: source,
                    output: self.output.definitions()[row.output].coordinate,
                    kind: if row.retained {
                        fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1::Retained
                    } else {
                        fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1::Substituted
                    },
                });
            }
            found += 1;
        }
        Err(self
            .guard
            .missing("scalar definition descendant ordinal")
            .into())
    }
    /// Complete ordered original block chain for one final block. Borrowing a
    /// slice is not permission for unmetered iteration or source discharge.
    pub fn block_segments(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&[ProductionCanonicalScalarBlockSegmentV1]> {
        let range = self.get(&self.rows.blocks, ordinal, budget)?;
        Ok(&self.rows.segments[range.clone()])
    }
    /// Every original block, including physically omitted and retained dead ones.
    pub fn original_block_control(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<ProductionCanonicalScalarBlockControlV1> {
        Ok(*self.get(&self.rows.block_controls, ordinal, budget)?)
    }
    /// Every original successor occurrence, not a destination-keyed summary.
    pub fn original_edge_control(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<ProductionCanonicalScalarEdgeControlV1> {
        Ok(*self.get(&self.rows.edge_controls, ordinal, budget)?)
    }
    /// Exact N operand occurrence for one final operand. Its actual final
    /// definition is available in final_inventory(), not inferred from N bytes.
    pub fn use_origin(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<CsUseV1> {
        Ok(*self.get(&self.rows.uses, ordinal, budget)?)
    }
    /// Exact N successor occurrence for one final successor.
    pub fn edge_origin(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<CsEdgeV1> {
        Ok(*self.get(&self.rows.edges, ordinal, budget)?)
    }
    /// Exact N edge/argument occurrence, preserving order and duplicate payloads.
    pub fn edge_argument_origin(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<CsEdgeArgumentV1> {
        Ok(*self.get(&self.rows.arguments, ordinal, budget)?)
    }
}

/// Original source custody, actual final lineage and fresh final fixed-nine
/// reports. Original span ranges are explicitly not final operation ranges.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalScalarSourcePoliciesV1;
/// fn copy(x: &ProductionCanonicalScalarSourcePoliciesV1<'_, '_, '_>) { let _ = (*x).clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalScalarSourcePoliciesV1;
/// fn promote(x: &ProductionCanonicalScalarSourcePoliciesV1<'_, '_, '_>) { let _ = x.into_verified_ranked(); }
/// ```
pub struct ProductionCanonicalScalarSourcePoliciesV1<'s, 'm, 'g> {
    source: &'s ProductionCanonicalRankedMetadataV1<'m>,
    lineage: ProductionCanonicalScalarLineageV1<'s>,
    policies: &'s fe2o3_pliron::CheckedCanonicalRankedPoliciesV1<'s, 'g>,
}
impl ProductionCanonicalScalarSourcePoliciesV1<'_, '_, '_> {
    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<()> {
        self.lineage.guard.query(budget)?;
        self.source.guard.query(budget)?;
        self.policies.function_count(budget)?;
        Ok(())
    }
    /// Complete original source, including all zero-operation and elided spans.
    pub fn original_metadata(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&ProductionCanonicalRankedMetadataV1<'_>> {
        self.check(budget)?;
        Ok(self.source)
    }
    /// Actual final owner inventory; never the same-byte original or a donor.
    pub fn final_inventory(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&CanonicalKirInventoryV1<'_>> {
        self.check(budget)?;
        Ok(self.lineage.output)
    }
    /// Actual checked adjacent-source composition, with no public constructor.
    pub fn lineage(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&ProductionCanonicalScalarLineageV1<'_>> {
        self.check(budget)?;
        Ok(&self.lineage)
    }
    /// The same real canonical policy pipeline, recomputed on the final graph.
    pub fn policies(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&fe2o3_pliron::CheckedCanonicalRankedPoliciesV1<'_, '_>> {
        self.check(budget)?;
        Ok(self.policies)
    }
    /// All full compiler obligations remain unresolved by this staged service.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// No target binding, external proof, publication or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn cs_final_rows_v1<'p>(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    final_inventory: &CanonicalKirInventoryV1<'_>,
    lineage: &CsLineageV1,
    projection: &'p CrProjectionV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<Vec<CrInertRowV1<'p>>> {
    cr_check_projection_v1(source, projection, budget)?;
    lineage.shape(final_inventory, budget)?;
    budget.charge_work(3)?;
    if lineage.block_controls.len() != source.inventory.blocks().len()
        || lineage.edge_controls.len() != source.inventory.edges().len()
        || lineage.functions.len() != source.inventory.functions().len()
    {
        return Err(cs_invalid_v1("complete original occurrence rosters"));
    }
    budget.reserve_storage(std::mem::size_of::<Vec<CrInertRowV1<'_>>>())?;
    let mut rows = cs_vec_v1(projection.keys.len(), budget)?;
    let first_function = argument_sum_v1(&[1, source.contracts.launches.len()])?;
    for (subject, kind, range) in &projection.keys[..first_function] {
        cs_push_v1(
            &mut rows,
            CrInertRowV1 {
                subject: *subject,
                kind: *kind,
                facts: &projection.facts[range.clone()],
            },
            budget,
        )?;
    }
    for (output, input) in lineage.functions.iter().enumerate() {
        let original = cs_function_v1(source.inventory, *input, budget)?;
        let (subject, kind, range) = &projection.keys[first_function + original];
        budget.charge_work(2)?;
        if *subject != CrSubjectV1::Function(original) || *kind != CrKindV1::Memory {
            return Err(cs_invalid_v1("original function projection custody"));
        }
        cs_push_v1(
            &mut rows,
            CrInertRowV1 {
                subject: CrSubjectV1::Function(output),
                kind: *kind,
                facts: &projection.facts[range.clone()],
            },
            budget,
        )?;
    }
    Ok(rows)
}

fn cs_check_subjects_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    report_owner: &CsGraphV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    budget.charge_work(3)?;
    if !std::ptr::eq(output.owner(), owner.output())
        || !std::ptr::eq(source.owner, &owner.original)
        || !std::ptr::eq(report_owner, owner.output())
    {
        return Err(ProductionCanonicalScalarSourceErrorV1::InputCustody);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cs_policy_callback_v1<'w, T>(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    rows: &CsLineageV1,
    policies: &fe2o3_pliron::CheckedCanonicalRankedPoliciesV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'w>,
    callback: impl for<'s, 'm, 'g> FnOnce(
        &ProductionCanonicalScalarSourcePoliciesV1<'s, 'm, 'g>,
        &mut ArgumentBudgetV1<'w>,
    ) -> CsResultV1<T>,
) -> Result<CsResultV1<T>, fe2o3_pliron::CanonicalRankedPolicyFailureV1> {
    use fe2o3_pliron::CanonicalRankedPolicyFailureV1 as Failure;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let headers = argument_sum_v1(&[
        std::mem::size_of::<ProductionCanonicalScalarSourcePoliciesV1<'_, '_, '_>>(),
        std::mem::size_of::<CrGuardV1>(),
        std::mem::size_of::<std::thread::Result<CsResultV1<T>>>(),
        std::mem::size_of::<CsResultV1<T>>(),
    ])?;
    budget.reserve_storage(headers)?;
    let paid = budget.storage();
    let guard = CrGuardV1::new(budget);
    let returned = catch_unwind(AssertUnwindSafe(|| {
        cs_check_subjects_v1(owner, source, output, policies.owner(budget)?, budget)?;
        let view = ProductionCanonicalScalarSourcePoliciesV1 {
            source,
            lineage: ProductionCanonicalScalarLineageV1 {
                original: source.inventory,
                output,
                rows,
                guard: &guard,
            },
            policies,
        };
        callback(&view, budget)
    }));
    let same = |budget: &ArgumentBudgetV1<'_>| {
        slot == std::ptr::from_ref(budget) as usize && ledger == budget.work_ledger_identity_v1()
    };
    let accounting = same(budget) && budget.storage() == paid;
    let custody = guard
        .check(budget)
        .and_then(|()| source.guard.check(budget));
    let failure = if !accounting {
        Some(Failure::Resource(ArgumentResourceV1::Accounting))
    } else {
        policies.function_count(budget).err()
    };
    let result = if let Some(error) = failure {
        let rejected = catch_unwind(AssertUnwindSafe(|| drop(returned)));
        drop(rejected);
        Err(error)
    } else if let Err(error) = custody {
        let rejected = catch_unwind(AssertUnwindSafe(|| drop(returned)));
        drop(rejected);
        Ok(Err(error.into()))
    } else {
        match returned {
            Ok(value) => Ok(value),
            Err(payload) => {
                drop(payload);
                Err(Failure::Panicked)
            }
        }
    };
    if same(budget) && budget.storage() >= paid {
        budget.release_storage(headers)?;
    }
    result
}

fn cs_with_final_view_v1<'w, T>(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    lineage: &CsLineageV1,
    budget: &mut ArgumentBudgetV1<'w>,
    callback: impl FnOnce(
        &CanonicalKirInventoryV1<'_>,
        &mut fe2o3_kernel_analysis::CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
        &mut ArgumentBudgetV1<'w>,
    ) -> CsResultV1<T>,
) -> CsResultV1<T> {
    let projection = cr_build_projection_v1(source, budget)?;
    fe2o3_pliron::with_canonical_analysis_scope_v1(owner.output(), budget, |scope| {
        scope.with_inventory_v1(|output, budget| {
            cs_scope_v1(budget, |budget| {
                let rows = cs_final_rows_v1(source, output, lineage, &projection, budget)?;
                budget.reserve_storage(std::mem::size_of::<CrInertV1<'_, '_>>())?;
                let inert = CrInertV1::new(owner.output(), &rows);
                let (candidate, receipt) =
                    fe2o3_kernel_analysis::build_canonical_ranked_candidate_v1(
                        output, &inert, budget,
                    )?;
                budget.reserve_storage(receipt.retained_storage())?;
                fe2o3_kernel_analysis::with_checked_canonical_ranked_view_v1(
                    output,
                    &inert,
                    &candidate,
                    budget,
                    |checked, budget| {
                        let actual = checked.inventory(budget)?.owner();
                        if !std::ptr::eq(actual, owner.output()) {
                            return Err(ProductionCanonicalScalarSourceErrorV1::InputCustody);
                        }
                        // The selected consumer makes its exact-floor query before
                        // reserving callback or assertion scratch.
                        callback(output, checked, budget)
                    },
                )
            })
        })
    })
}

impl ProductionCanonicalScalarFixedPointOwnerV1 {
    /// Replay original source and every real adjacent history, transport exact
    /// occurrences, then run the unchanged fixed nine on actual final output.
    /// Prepay retained_storage_floor_v1() plus siblings; entry storage is restored
    /// on all exits. The callback must preserve its exact entry floor and ledger.
    /// Returned owned callback values require caller prepayment before entry.
    pub fn with_policy_checks_v1<'w, T>(
        &self,
        budget: &mut ArgumentBudgetV1<'w>,
        callback: impl for<'s, 'm, 'g> FnOnce(
            &ProductionCanonicalScalarSourcePoliciesV1<'s, 'm, 'g>,
            &mut ArgumentBudgetV1<'w>,
        ) -> CsResultV1<T>,
    ) -> CsResultV1<T> {
        cs_scope_v1(budget, |budget| {
            budget.charge_work(1)?;
            if budget.storage() < self.retained {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.original
                .with_canonical_ranked_metadata_v1(budget, |source, budget| {
                    Ok(cs_scope_v1(budget, |budget| {
                        cr_policy_source_profile_v1(source, budget)?;
                        self.history
                            .replay_against(self.original.executable(), budget)?;
                        let lineage = cs_lineage_v1(self, source.inventory, budget)?;
                        cs_with_final_view_v1(
                            self,
                            source,
                            &lineage,
                            budget,
                            |output, checked, budget| {
                                fe2o3_pliron::with_canonical_ranked_policy_checks_v1(
                                    checked,
                                    budget,
                                    |policies, budget| {
                                        cs_policy_callback_v1(
                                            self, source, output, &lineage, policies, budget,
                                            callback,
                                        )
                                    },
                                )
                                .map_err(ProductionCanonicalScalarSourceErrorV1::FinalPolicy)?
                            },
                        )
                    }))
                })?
        })
    }
}
