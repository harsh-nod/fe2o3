use fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1 as CsaEdgePlaceV1;

/// Diagnostic location of an actual checked adjacent-history event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCanonicalScalarAssertionStepV1 {
    round: u16,
    integer: bool,
}
impl ProductionCanonicalScalarAssertionStepV1 {
    /// Actual fixed-point round ordinal.
    pub const fn round(self) -> u16 {
        self.round
    }
    /// Whether the event occurred in that round's integer substage.
    pub const fn is_integer(self) -> bool {
        self.integer
    }
}

/// Diagnostic disposition; only the enclosing borrowed facade retains evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionCanonicalScalarAssertionDispositionV1 {
    /// An actual final conditional remains attached to a complete trap incoming row.
    Retained,
    /// The original materializer emitted an independently proved success route.
    SourceElided,
    /// An actual checked adjacent transition selected the asserted-success edge.
    HistoryElided,
    /// Checked unreachable control removed this assertion's physical route.
    UnreachableRemoved,
}

#[derive(Clone, Copy)]
struct CsaConditionV1 {
    used: CsUseV1,
    definition: CsDefinitionV1,
    failure: CsEdgeV1,
}
#[derive(Clone, Copy)]
struct CsaStateV1 {
    condition: Option<CsaConditionV1>,
    success: CsaEdgePlaceV1,
    selected: Option<ProductionCanonicalScalarAssertionStepV1>,
    removed: Option<ProductionCanonicalScalarAssertionStepV1>,
}

/// One complete original source alias transported through every actual substage.
/// Coordinates and proof classification are diagnostic, not detached capabilities.
pub struct ProductionCanonicalScalarAssertionV1 {
    span: usize,
    binding: SemanticKirAssertConditionBindingV1,
    proof: fe2o3_mir_model::SemanticAssertionProofKindV1,
    block_origin: usize,
    success_origin: usize,
    failure_origin: Option<usize>,
    state: CsaStateV1,
}
impl ProductionCanonicalScalarAssertionV1 {
    /// Original root-qualified source span ordinal.
    pub const fn span(&self) -> usize {
        self.span
    }
    /// Exact N binding, never relabeled as a final F binding.
    pub const fn original_binding(&self) -> SemanticKirAssertConditionBindingV1 {
        self.binding
    }
    /// Classification of this callback's fresh source proof.
    pub const fn proof_kind(&self) -> fe2o3_mir_model::SemanticAssertionProofKindV1 {
        self.proof
    }
    /// Final physical disposition after the complete checked history.
    pub const fn disposition(&self) -> ProductionCanonicalScalarAssertionDispositionV1 {
        if self.state.removed.is_some() {
            ProductionCanonicalScalarAssertionDispositionV1::UnreachableRemoved
        } else if self.state.condition.is_some() {
            ProductionCanonicalScalarAssertionDispositionV1::Retained
        } else if self.state.selected.is_some() {
            ProductionCanonicalScalarAssertionDispositionV1::HistoryElided
        } else {
            ProductionCanonicalScalarAssertionDispositionV1::SourceElided
        }
    }
    /// Exact surviving F selector use and definition, if a conditional remains.
    pub fn condition(&self) -> Option<(CsUseV1, CsDefinitionV1)> {
        self.state.condition.map(|row| (row.used, row.definition))
    }
    /// Exact F failure successor occurrence, if retained.
    pub fn failure_edge(&self) -> Option<CsEdgeV1> {
        self.state.condition.map(|row| row.failure)
    }
    /// A retained edge, an actual ordered merge connector, or checked omission.
    pub const fn success_placement(&self) -> CsaEdgePlaceV1 {
        self.state.success
    }
    /// The actual first checked asserted-success selection, not inferred omission.
    pub const fn selection(&self) -> Option<ProductionCanonicalScalarAssertionStepV1> {
        self.state.selected
    }
    /// The actual checked unreachable-removal event, when present.
    pub const fn removal(&self) -> Option<ProductionCanonicalScalarAssertionStepV1> {
        self.state.removed
    }
    fn next_row(&self) -> Self {
        Self {
            span: self.span,
            binding: self.binding,
            proof: self.proof,
            block_origin: self.block_origin,
            success_origin: self.success_origin,
            failure_origin: self.failure_origin,
            state: self.state,
        }
    }
}

struct CsaTransportV1 {
    rows: Vec<ProductionCanonicalScalarAssertionV1>,
    storage: usize,
}
impl CsaTransportV1 {
    fn original(
        source: &ProductionCanonicalRankedMetadataV1<'_>,
        coverage: &Coverage<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self> {
        let mut result = csa_allocate_v1(source.contracts.assertions.len(), budget)?;
        for assertion in &source.contracts.assertions {
            budget.charge_work(2)?;
            coverage.span(source, assertion.span, budget)?;
            let proved = coverage
                .rows
                .get(assertion.span)
                .and_then(Option::as_ref)
                .ok_or_else(|| binding(Some(assertion.span), "missing fresh assertion alias"))?;
            if proved.binding != assertion.binding {
                return Err(binding(Some(assertion.span), "fresh alias binding differs").into());
            }
            let (condition, success, failure_origin) = match assertion.binding.outcome() {
                SemanticKirAssertConditionOutcomeV1::Emitted {
                    condition_use,
                    definition,
                    success_edge,
                    failure_edge,
                } => (
                    Some(CsaConditionV1 {
                        used: condition_use,
                        definition,
                        failure: failure_edge,
                    }),
                    success_edge,
                    Some(cs_edge_v1(source.inventory, failure_edge, budget)?),
                ),
                SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge } => {
                    (None, success_edge, None)
                }
            };
            let row = ProductionCanonicalScalarAssertionV1 {
                span: assertion.span,
                binding: assertion.binding,
                proof: proved.proof,
                block_origin: cs_block_v1(source.inventory, assertion.binding.block(), budget)?,
                success_origin: cs_edge_v1(source.inventory, success, budget)?,
                failure_origin,
                state: CsaStateV1 {
                    condition,
                    success: CsaEdgePlaceV1::Retained(success),
                    selected: None,
                    removed: None,
                },
            };
            cs_push_v1(&mut result.rows, row, budget)?;
        }
        Ok(result)
    }
}

fn csa_condition_v1(
    inventory: &CanonicalKirInventoryV1<'_>,
    condition: CsaConditionV1,
    success: CsEdgeV1,
    expected: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    let CsUseV1::TerminatorOperand { block, operand: 0 } = condition.used else {
        return Err(cs_invalid_v1("assertion selector occurrence"));
    };
    let body = &inventory.blocks()[cs_block_v1(inventory, block, budget)?];
    let used = &inventory.uses()[cs_use_v1(inventory, condition.used, budget)?];
    let definition =
        &inventory.definitions()[cs_definition_v1(inventory, condition.definition, budget)?];
    let success_row = &inventory.edges()[cs_edge_v1(inventory, success, budget)?];
    let failure_row = &inventory.edges()[cs_edge_v1(inventory, condition.failure, budget)?];
    budget.charge_work(9)?;
    if !matches!(body.terminator, Terminator::ConditionalBranch { .. })
        || body.edges.len() != 2
        || success.source != block
        || condition.failure.source != block
        || success.successor != u32::from(!expected)
        || condition.failure.successor != u32::from(expected)
        || inventory.definitions()[used.definition].coordinate != condition.definition
        || definition.value != Some(used.value)
        || *definition.ty != Type::BOOL
        || !failure_row.arguments.is_empty()
        || success_row.target == failure_row.target
    {
        return Err(cs_invalid_v1(
            "actual assertion selector or ordered successors",
        ));
    }
    Ok(())
}

fn csa_payloads_v1(
    output: &CanonicalKirInventoryV1<'_>,
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    input: CsEdgeV1,
    placement: CsaEdgePlaceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    let mut mapped = 0usize;
    for argument in control.input_edge_arguments(input, budget)? {
        budget.charge_work(1)?;
        if let Some(after) = control.edge_argument(argument.coordinate, budget)? {
            let CsaEdgePlaceV1::Retained(edge) = placement else {
                return Err(cs_invalid_v1("removed connector retained edge payload"));
            };
            let ordinal = cs_argument_v1(output, after, budget)?;
            if after.edge != edge || rows.edge_arguments[ordinal].input != argument.coordinate {
                return Err(cs_invalid_v1("assertion payload occurrence transport"));
            }
            mapped = argument_sum_v1(&[mapped, 1])?;
        }
    }
    if let CsaEdgePlaceV1::Retained(edge) = placement {
        let actual = &output.edges()[cs_edge_v1(output, edge, budget)?];
        budget.charge_work(1)?;
        if mapped != actual.bindings.len() {
            return Err(cs_invalid_v1("incomplete assertion payload transport"));
        }
        for argument in &output.edge_arguments()[actual.bindings.clone()] {
            budget.charge_work(1)?;
            let ordinal = cs_argument_v1(output, argument.coordinate, budget)?;
            if rows.edge_arguments[ordinal].input.edge != input {
                return Err(cs_invalid_v1("foreign assertion payload origin"));
            }
        }
    } else if mapped != 0 {
        return Err(cs_invalid_v1("omitted edge has transported payload"));
    }
    Ok(())
}

impl CsPairObserverV1 for CsaTransportV1 {
    type Next = Self;
    #[allow(clippy::too_many_arguments)]
    fn pair(
        &self,
        input: &CanonicalKirInventoryV1<'_>,
        output: &CanonicalKirInventoryV1<'_>,
        control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
        rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
        previous: &CsLineageV1,
        next_lineage: &CsLineageV1,
        round: u16,
        integer: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self> {
        budget.charge_work(2)?;
        if !std::ptr::eq(control.input(), input) || !std::ptr::eq(control.output(), output) {
            return Err(ProductionCanonicalScalarSourceErrorV1::InputCustody);
        }
        let step = ProductionCanonicalScalarAssertionStepV1 { round, integer };
        let mut result = csa_allocate_v1(self.rows.len(), budget)?;
        for row in &self.rows {
            budget.charge_work(7)?;
            let fail = |reason| csa_transport_error_v1(row.span, step, reason);
            if (row.state.condition.is_some() && row.state.selected.is_some())
                || (row.state.removed.is_some()
                    && (row.state.condition.is_some()
                        || row.state.success != CsaEdgePlaceV1::Omitted))
                || (row.state.selected.is_some()
                    && matches!(
                        row.binding.outcome(),
                        SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { .. }
                    ))
            {
                return Err(fail("inconsistent assertion disposition"));
            }
            let old_success = previous
                .edge_controls
                .get(row.success_origin)
                .ok_or_else(|| fail("original success roster"))?;
            let next_success = next_lineage
                .edge_controls
                .get(row.success_origin)
                .ok_or_else(|| fail("next success roster"))?;
            let old_block = previous
                .block_controls
                .get(row.block_origin)
                .ok_or_else(|| fail("original block roster"))?;
            let next_block = next_lineage
                .block_controls
                .get(row.block_origin)
                .ok_or_else(|| fail("next block roster"))?;
            if old_success.placement != row.state.success
                || old_block.original != row.binding.block()
            {
                return Err(fail("assertion sidecar disagrees with original lineage"));
            }
            let mut after = row.next_row();
            after.state.success = next_success.placement;
            if row.state.removed.is_some() {
                if next_success.placement != CsaEdgePlaceV1::Omitted
                    || next_block.reachable
                    || row.state.condition.is_some()
                {
                    return Err(fail("removed source occurrence reappeared"));
                }
            } else if let Some(condition) = row.state.condition {
                let CsaEdgePlaceV1::Retained(success) = row.state.success else {
                    return Err(fail("conditional has no success edge"));
                };
                csa_condition_v1(input, condition, success, row.binding.expected(), budget)?;
                let failure_origin = row
                    .failure_origin
                    .ok_or_else(|| fail("missing original failure occurrence"))?;
                if previous.edge_controls[failure_origin].placement
                    != CsaEdgePlaceV1::Retained(condition.failure)
                {
                    return Err(fail("conditional failure lineage"));
                }
                let block = control.block(success.source, budget)?;
                let success_step = control.edge(success, budget)?;
                let failure_step = control.edge(condition.failure, budget)?;
                csa_payloads_v1(
                    output,
                    control,
                    rows,
                    success,
                    success_step.placement,
                    budget,
                )?;
                csa_payloads_v1(
                    output,
                    control,
                    rows,
                    condition.failure,
                    failure_step.placement,
                    budget,
                )?;
                if block.placement.is_none() {
                    if block.reachable
                        || next_block.reachable
                        || next_success.placement != CsaEdgePlaceV1::Omitted
                        || failure_step.placement != CsaEdgePlaceV1::Omitted
                    {
                        return Err(fail("removed assertion was reachable"));
                    }
                    after.state.condition = None;
                    after.state.removed = Some(step);
                } else if let (
                    CsaEdgePlaceV1::Retained(success),
                    CsaEdgePlaceV1::Retained(failure),
                ) = (success_step.placement, failure_step.placement)
                {
                    let transported = control
                        .operand(condition.used, budget)?
                        .ok_or_else(|| fail("retained selector has no actual output use"))?;
                    let condition = CsaConditionV1 {
                        used: transported.coordinate,
                        definition: transported.definition,
                        failure,
                    };
                    csa_condition_v1(output, condition, success, row.binding.expected(), budget)?;
                    after.state.condition = Some(condition);
                    if next_success.placement != CsaEdgePlaceV1::Retained(success)
                        || next_lineage.edge_controls[failure_origin].placement
                            != CsaEdgePlaceV1::Retained(failure)
                    {
                        return Err(fail("retained selector differs from composed lineage"));
                    }
                } else {
                    if block.selected_successor != Some(success)
                        || failure_step.placement != CsaEdgePlaceV1::Omitted
                        || failure_step.executable
                        || matches!(success_step.placement, CsaEdgePlaceV1::Omitted)
                        || after.state.selected.is_some()
                    {
                        return Err(fail("no actual checked asserted-success selection"));
                    }
                    after.state.condition = None;
                    after.state.selected = Some(step);
                }
            } else {
                match row.state.success {
                    CsaEdgePlaceV1::Retained(edge) => {
                        let block = control.block(edge.source, budget)?;
                        let edge_step = control.edge(edge, budget)?;
                        csa_payloads_v1(output, control, rows, edge, edge_step.placement, budget)?;
                        if next_success.placement == CsaEdgePlaceV1::Omitted {
                            if block.reachable || edge_step.executable || next_block.reachable {
                                return Err(fail("proved success route removed while executable"));
                            }
                            after.state.removed = Some(step);
                        }
                    }
                    CsaEdgePlaceV1::InternalConnector(placement) => {
                        let block = control.block(placement.output, budget)?;
                        if next_success.placement == CsaEdgePlaceV1::Omitted {
                            if block.reachable || next_block.reachable {
                                return Err(fail("internalized assertion removed while reachable"));
                            }
                            after.state.removed = Some(step);
                        } else if !matches!(
                            next_success.placement,
                            CsaEdgePlaceV1::InternalConnector(_)
                        ) {
                            return Err(fail("consumed connector became a new edge"));
                        }
                    }
                    CsaEdgePlaceV1::Omitted => {
                        return Err(fail("omission without a checked removal event"));
                    }
                }
            }
            cs_push_v1(&mut result.rows, after, budget)?;
        }
        Ok(result)
    }
    fn retained(next: &Self) -> usize {
        next.storage
    }
    fn replace(&mut self, next: Self) -> usize {
        let previous = std::mem::replace(self, next);
        let retired = previous.storage;
        drop(previous);
        retired
    }
}

#[cfg(test)]
pub(super) fn read_test_scalar_assertion_sidecar_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
    fault: u8,
) -> CsResultV1<()> {
    // Hostile unauthenticated sidecar rows, never a source-admission fixture.
    csa_scope_v1(budget, |budget| {
        owner
            .original
            .with_checked_canonical_ranked_source_v1(budget, |view, budget| {
                Ok(csa_with_source_v1(
                    view,
                    budget,
                    |source, coverage, budget| {
                        let mut rows = CsaTransportV1::original(source, coverage, budget)?;
                        let row = rows
                            .rows
                            .first_mut()
                            .expect("nonempty original assertion fixture");
                        match fault {
                            0 => row.state.success = CsaEdgePlaceV1::Omitted,
                            1 => {
                                row.state.removed = Some(ProductionCanonicalScalarAssertionStepV1 {
                                    round: 0,
                                    integer: true,
                                })
                            }
                            2 => {
                                row.state.selected =
                                    Some(ProductionCanonicalScalarAssertionStepV1 {
                                        round: 0,
                                        integer: true,
                                    })
                            }
                            _ => panic!("unknown sidecar fault"),
                        }
                        let lineage = cs_lineage_with_observer_v1(
                            owner,
                            source.inventory,
                            &mut rows,
                            budget,
                        )?;
                        drop(lineage);
                        Ok(())
                    },
                ))
            })?
    })
}
