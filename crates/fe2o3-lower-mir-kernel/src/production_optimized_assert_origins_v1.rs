// Distinct optimized placement custody. Legacy import origin meanings stay frozen.

use fe2o3_kernel_analysis::{
    CanonicalKirBlockPlacementV1, CanonicalKirEdgeControlV1, CanonicalKirEdgePlacementV1,
    CanonicalKirOutputUseV1, CanonicalKirTransitionErrorV1, CheckedCanonicalKirControlIndexV1,
};

/// Actual optimized control outcome, never an assertion-discharge certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticKirOptimizedAssertOutcomeV1 {
    /// The output still tests the original expected polarity at runtime.
    Conditional {
        /// Actual output condition occurrence and the definition it consumes.
        condition: CanonicalKirOutputUseV1,
        /// Retained edge taken when the source assertion succeeds.
        success: CanonicalKirEdgePlacementV1,
        /// Distinct retained edge taken when the source assertion fails.
        failure: CanonicalKirEdgePlacementV1,
    },
    /// The fixed checker independently selected the source success edge.
    SelectedSuccess {
        /// Executable retained edge or consumed block-merge connector.
        success: CanonicalKirEdgePlacementV1,
        /// Omitted, independently nonexecutable failure edge.
        failure: CanonicalKirEdgePlacementV1,
    },
    /// The fixed checker selected failure; this does not discharge the assertion.
    SelectedFailure {
        /// Omitted, independently nonexecutable success edge.
        success: CanonicalKirEdgePlacementV1,
        /// Executable retained edge or consumed connector leading to failure.
        failure: CanonicalKirEdgePlacementV1,
    },
    /// No checked execution reaches the original assertion. Physical unreachable
    /// output may remain; its actual placement is retained rather than fabricated.
    RemovedUnreachable {
        /// Optional physical output placement, even though execution cannot reach it.
        block: Option<CanonicalKirBlockPlacementV1>,
        /// Condition occurrence if the unreachable conditional remains in output.
        condition: Option<CanonicalKirOutputUseV1>,
        /// Actual placement or omission of the original success edge.
        success: CanonicalKirEdgePlacementV1,
        /// Absent only when a source rule already elided the original assertion.
        failure: Option<CanonicalKirEdgePlacementV1>,
    },
    /// Historical source-rule elision, not a new optimizer proof or Boolean use.
    SourceRuleElision {
        /// Executable successor retained or consumed by checked block merging.
        success: CanonicalKirEdgePlacementV1,
    },
}

/// Exact original edge-argument occurrence and its optional actual output.
/// None may mean a removed parameter, merge connector, or dead control; consult
/// the enclosing edge/control outcome rather than inferring a safety result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKirOptimizedAssertArgumentV1 {
    /// Historical argument occurrence in the imported executable.
    pub input: fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1,
    /// Exact surviving output occurrence, if any.
    pub output: Option<fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1>,
}

/// One physical assertion; multiple source/root aliases can share this record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKirOptimizedAssertBindingV1 {
    physical: usize,
    import: SemanticKirAssertConditionBindingV1,
    outcome: SemanticKirOptimizedAssertOutcomeV1,
    first_argument: usize,
    argument_count: usize,
}
impl SemanticKirOptimizedAssertBindingV1 {
    /// Source condition polarity required for assertion success.
    pub const fn expected(self) -> bool {
        self.import.expected()
    }
    /// Original semantic MIR success block, not an optimized block coordinate.
    pub const fn semantic_success(self) -> SemanticBlockIdV1 {
        self.import.semantic_success()
    }
    /// Checked control placement in the actual optimized executable.
    pub const fn outcome(self) -> SemanticKirOptimizedAssertOutcomeV1 {
        self.outcome
    }
    /// Historical input binding only; these coordinates never describe output.
    pub const fn import_binding(self) -> SemanticKirAssertConditionBindingV1 {
        self.import
    }
}

/// Owned, sealed output placement data produced only from the actual checked
/// transition and sealed input aliases. The final compiler owner must retain
/// this beside the actual checked output executable; no method pairs arbitrary
/// graph borrows with these records. This is callback-compatible owned data.
#[derive(Debug)]
pub struct SemanticKirOptimizedAssertOriginOwnerV1 {
    input: fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV12,
    output: fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV12,
    aliases: Vec<AssertOriginAliasV1>,
    bindings: Vec<SemanticKirOptimizedAssertBindingV1>,
    arguments: Vec<SemanticKirOptimizedAssertArgumentV1>,
}
impl SemanticKirOptimizedAssertOriginOwnerV1 {
    /// Identity of the exact sealed input graph used by the transition.
    pub const fn input_identity(&self) -> fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV12 {
        self.input
    }
    /// Identity of the actual independently checked successor graph.
    pub const fn output_identity(&self) -> fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV12 {
        self.output
    }
    /// Number of source/root aliases, including aliases of shared helpers.
    pub fn source_site_count(&self) -> usize {
        self.aliases.len()
    }
    /// Number of physical assertions represented by those aliases.
    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }
    /// These placement records grant no proof, artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Looks up the unchanged source/root alias and returns actual output placement.
    pub fn assert_condition(
        &self,
        correspondence_owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
        semantic_block: SemanticBlockIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<&SemanticKirOptimizedAssertBindingV1, SemanticKirOptimizedAssertOriginErrorV1> {
        let site =
            SemanticKirAssertSiteV1::new(correspondence_owner, semantic_function, semantic_block);
        let index = assert_origin_find_v1(&self.aliases, budget, |row, _| Ok(row.site.cmp(&site)))?
            .ok_or(SemanticKirAssertOriginErrorV1::MissingBinding { site })?;
        budget.charge_work(1)?;
        self.bindings.get(self.aliases[index].binding).ok_or(
            SemanticKirOptimizedAssertOriginErrorV1::Invalid("alias binding ordinal"),
        )
    }
    /// Requires the exact borrowed physical row from this owner, not a copied or
    /// same-shaped foreign binding. Each argument visit remains caller-metered.
    pub fn arguments_for(
        &self,
        binding: &SemanticKirOptimizedAssertBindingV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<&[SemanticKirOptimizedAssertArgumentV1], SemanticKirOptimizedAssertOriginErrorV1>
    {
        // Binding fields are private, but a binding from another owner is possible.
        budget.charge_work(1)?;
        if !self
            .bindings
            .get(binding.physical)
            .is_some_and(|actual| std::ptr::eq(actual, binding))
        {
            return Err(SemanticKirOptimizedAssertOriginErrorV1::Invalid(
                "foreign assertion binding",
            ));
        }
        let end = binding
            .first_argument
            .checked_add(binding.argument_count)
            .ok_or(AssertOriginResourceV1::Arithmetic)?;
        self.arguments.get(binding.first_argument..end).ok_or(
            SemanticKirOptimizedAssertOriginErrorV1::Invalid("argument range"),
        )
    }
}

/// Separate transfer receipt for the owned origin records and their inline header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKirOptimizedAssertOriginStorageV1(usize);
impl SemanticKirOptimizedAssertOriginStorageV1 {
    /// Logical bytes to reserve while this origin owner remains live.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Rejection while transporting or querying optimized source assertion origins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticKirOptimizedAssertOriginErrorV1 {
    /// Work, storage, arithmetic or allocation failure on the shared ledger.
    Resource(AssertOriginResourceV1),
    /// Original sealed source/root binding could not be resolved.
    Source(SemanticKirAssertOriginErrorV1),
    /// The checked transition index rejected a control or occurrence query.
    Transition(CanonicalKirTransitionErrorV1),
    /// Source origins belong to a different executable owner.
    InputOwner,
    /// A required placement, polarity, census or binding-custody rule failed.
    Invalid(&'static str),
}
impl From<AssertOriginResourceV1> for SemanticKirOptimizedAssertOriginErrorV1 {
    fn from(error: AssertOriginResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl From<SemanticKirAssertOriginErrorV1> for SemanticKirOptimizedAssertOriginErrorV1 {
    fn from(error: SemanticKirAssertOriginErrorV1) -> Self {
        Self::Source(error)
    }
}
impl From<CanonicalKirTransitionErrorV1> for SemanticKirOptimizedAssertOriginErrorV1 {
    fn from(error: CanonicalKirTransitionErrorV1) -> Self {
        Self::Transition(error)
    }
}
impl fmt::Display for SemanticKirOptimizedAssertOriginErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
            Self::Transition(error) => error.fmt(formatter),
            Self::InputOwner => formatter.write_str("optimized assertion input owner mismatch"),
            Self::Invalid(rule) => {
                write!(formatter, "optimized assertion placement rejected: {rule}")
            }
        }
    }
}
impl std::error::Error for SemanticKirOptimizedAssertOriginErrorV1 {}

/// Transports sealed physical assertions once and preserves their source/root
/// aliases. Does not reinterpret legacy source elision, discharge a failed
/// assertion, or reattach input coordinates to output. Work is linear in physical
/// bindings, aliases and edge arguments plus metered control-index queries.
/// Caller reserves the sealed input owner and the separate checked control index.
/// All returned exits restore the incoming floor; success transfers owned data.
pub fn transport_semantic_kir_assert_origins_v1(
    input: SemanticKirAssertOriginsV1<'_>,
    control: &CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        SemanticKirOptimizedAssertOriginOwnerV1,
        SemanticKirOptimizedAssertOriginStorageV1,
    ),
    SemanticKirOptimizedAssertOriginErrorV1,
> {
    let floor = budget.storage();
    let result = build_optimized_assert_origins_v1(input, control, budget);
    let retained = budget
        .storage()
        .checked_sub(floor)
        .ok_or(AssertOriginResourceV1::Accounting)?;
    budget.release_storage(retained)?;
    result.map(|owner| (owner, SemanticKirOptimizedAssertOriginStorageV1(retained)))
}

fn build_optimized_assert_origins_v1(
    input: SemanticKirAssertOriginsV1<'_>,
    control: &CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SemanticKirOptimizedAssertOriginOwnerV1, SemanticKirOptimizedAssertOriginErrorV1> {
    use SemanticKirOptimizedAssertOriginErrorV1 as Failure;
    budget.charge_work(1)?;
    if !std::ptr::eq(input.executable(), control.input().owner()) {
        return Err(Failure::InputOwner);
    }
    let mut argument_count = 0_usize;
    for binding in &input.origins.bindings {
        budget.charge_work(1)?;
        let (success, failure) = optimized_assert_input_edges_v1(*binding);
        for edge in [Some(success), failure].into_iter().flatten() {
            budget.charge_work(1)?;
            argument_count = argument_count
                .checked_add(control.input_edge_arguments(edge, budget)?.len())
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
        }
    }
    let retained = std::mem::size_of::<SemanticKirOptimizedAssertOriginOwnerV1>()
        .checked_add(
            input
                .origins
                .bindings
                .len()
                .checked_mul(std::mem::size_of::<SemanticKirOptimizedAssertBindingV1>())
                .ok_or(AssertOriginResourceV1::Arithmetic)?,
        )
        .and_then(|n| {
            n.checked_add(
                input
                    .origins
                    .aliases
                    .len()
                    .checked_mul(std::mem::size_of::<AssertOriginAliasV1>())?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                argument_count
                    .checked_mul(std::mem::size_of::<SemanticKirOptimizedAssertArgumentV1>())?,
            )
        })
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    budget.reserve_storage(retained)?;
    budget.charge_work(3)?;
    let mut result = SemanticKirOptimizedAssertOriginOwnerV1 {
        input: control.input().identity(),
        output: control.output().identity(),
        aliases: Vec::new(),
        bindings: Vec::new(),
        arguments: Vec::new(),
    };
    result
        .aliases
        .try_reserve_exact(input.origins.aliases.len())
        .map_err(|_| AssertOriginResourceV1::Allocation)?;
    result
        .bindings
        .try_reserve_exact(input.origins.bindings.len())
        .map_err(|_| AssertOriginResourceV1::Allocation)?;
    result
        .arguments
        .try_reserve_exact(argument_count)
        .map_err(|_| AssertOriginResourceV1::Allocation)?;
    for binding in &input.origins.bindings {
        budget.charge_work(1)?;
        let first_argument = result.arguments.len();
        let (success, failure) = optimized_assert_input_edges_v1(*binding);
        for edge in [Some(success), failure].into_iter().flatten() {
            budget.charge_work(1)?;
            let placement = control.edge(edge, budget)?.placement;
            for argument in control.input_edge_arguments(edge, budget)? {
                budget.charge_work(1)?;
                let output = control.edge_argument(argument.coordinate, budget)?;
                if let Some(output) = output
                    && !matches!(placement, CanonicalKirEdgePlacementV1::Retained(retained) if output.edge == retained)
                {
                    return Err(Failure::Invalid("edge argument placement"));
                }
                if result.arguments.len() >= argument_count {
                    return Err(Failure::Invalid("argument census"));
                }
                result.arguments.push(SemanticKirOptimizedAssertArgumentV1 {
                    input: argument.coordinate,
                    output,
                });
            }
        }
        let outcome = optimized_assert_outcome_v1(*binding, control, budget)?;
        if result.bindings.len() >= input.origins.bindings.len() {
            return Err(Failure::Invalid("binding census"));
        }
        result.bindings.push(SemanticKirOptimizedAssertBindingV1 {
            physical: result.bindings.len(),
            import: *binding,
            outcome,
            first_argument,
            argument_count: result
                .arguments
                .len()
                .checked_sub(first_argument)
                .ok_or(AssertOriginResourceV1::Accounting)?,
        });
    }
    budget.charge_work(input.origins.aliases.len())?;
    for alias in &input.origins.aliases {
        if alias.binding >= result.bindings.len() {
            return Err(Failure::Invalid("source alias binding"));
        }
        result.aliases.push(*alias);
    }
    if result.arguments.len() != argument_count {
        return Err(Failure::Invalid("argument census coverage"));
    }
    Ok(result)
}

fn optimized_assert_input_edges_v1(
    binding: SemanticKirAssertConditionBindingV1,
) -> (
    fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    Option<fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1>,
) {
    match binding.outcome() {
        SemanticKirAssertConditionOutcomeV1::Emitted {
            success_edge,
            failure_edge,
            ..
        } => (success_edge, Some(failure_edge)),
        SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge } => {
            (success_edge, None)
        }
    }
}

fn optimized_assert_outcome_v1(
    binding: SemanticKirAssertConditionBindingV1,
    control: &CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SemanticKirOptimizedAssertOutcomeV1, SemanticKirOptimizedAssertOriginErrorV1> {
    use CanonicalKirEdgePlacementV1 as Placement;
    use SemanticKirOptimizedAssertOriginErrorV1 as Failure;
    use SemanticKirOptimizedAssertOutcomeV1 as Outcome;
    let (success_edge, failure_edge) = optimized_assert_input_edges_v1(binding);
    let block = control.block(success_edge.source, budget)?;
    let success = control.edge(success_edge, budget)?;
    match binding.outcome() {
        SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { .. } => {
            if !block.reachable {
                return Ok(Outcome::RemovedUnreachable {
                    block: block.placement,
                    condition: None,
                    success: success.placement,
                    failure: None,
                });
            }
            if !success.executable || matches!(success.placement, Placement::Omitted) {
                return Err(Failure::Invalid("source elision success transport"));
            }
            Ok(Outcome::SourceRuleElision {
                success: success.placement,
            })
        }
        SemanticKirAssertConditionOutcomeV1::Emitted { condition_use, .. } => {
            let failure_edge = failure_edge.ok_or(Failure::Invalid("missing failure edge"))?;
            let failure = control.edge(failure_edge, budget)?;
            let condition = control.operand(condition_use, budget)?;
            if !block.reachable {
                return Ok(Outcome::RemovedUnreachable {
                    block: block.placement,
                    condition,
                    success: success.placement,
                    failure: Some(failure.placement),
                });
            }
            if let Some(condition) = condition {
                let (Placement::Retained(success), Placement::Retained(failure)) =
                    (success.placement, failure.placement)
                else {
                    return Err(Failure::Invalid("conditional edge placement"));
                };
                if success.source != failure.source
                    || success.successor != u32::from(!binding.expected())
                    || failure.successor != u32::from(binding.expected())
                    || condition.coordinate
                        != (fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::TerminatorOperand {
                            block: success.source,
                            operand: 0,
                        })
                {
                    return Err(Failure::Invalid(
                        "conditional polarity or operand occurrence",
                    ));
                }
                return Ok(Outcome::Conditional {
                    condition,
                    success: Placement::Retained(success),
                    failure: Placement::Retained(failure),
                });
            }
            let selected = block
                .selected_successor
                .ok_or(Failure::Invalid("missing checked Boolean selection"))?;
            let selected_success = selected == success_edge;
            if !selected_success && selected != failure_edge {
                return Err(Failure::Invalid("selected assertion edge"));
            }
            let (chosen, other): (CanonicalKirEdgeControlV1, CanonicalKirEdgeControlV1) =
                if selected_success {
                    (success, failure)
                } else {
                    (failure, success)
                };
            if !chosen.executable
                || matches!(chosen.placement, Placement::Omitted)
                || other.executable
                || !matches!(other.placement, Placement::Omitted)
            {
                return Err(Failure::Invalid("selected control transport"));
            }
            Ok(if selected_success {
                Outcome::SelectedSuccess {
                    success: success.placement,
                    failure: failure.placement,
                }
            } else {
                Outcome::SelectedFailure {
                    success: success.placement,
                    failure: failure.placement,
                }
            })
        }
    }
}
