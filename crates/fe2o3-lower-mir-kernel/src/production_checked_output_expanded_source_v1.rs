// Included in the existing source-admission module. All grants remain private;
// public rows are inert provenance, not an arithmetic or trap permission table.
type ExpandedSiteV1 = Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>;

#[derive(Clone, Copy)]
enum GeneralOutputSubjectV1<'a> {
    Owned(&'a fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1),
    ActualPair {
        output: &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'a>,
        audit: &'a [u8],
        required: usize,
    },
}
impl<'a> GeneralOutputSubjectV1<'a> {
    fn output(self) -> &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        match self {
            Self::Owned(v) => v.owner(),
            Self::ActualPair { output, .. } => output,
        }
    }
    fn rows(self) -> fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'a> {
        match self {
            Self::Owned(v) => v.occurrences().candidate(),
            Self::ActualPair { rows, .. } => rows,
        }
    }
    fn audit_bytes(self) -> &'a [u8] {
        match self {
            Self::Owned(v) => v.native_input_audit_bytes(),
            Self::ActualPair { audit, .. } => audit,
        }
    }
}

// Shared source frontend for an independently decoded B/C pair. This private
// adapter replays original Direct N or Erased N/E and runs the SAME source,
// assertion, lifetime, catalog, native and formal checks as the owning API.
// Numeric prepaid backing is non-authoritative; exact rows/owners are checked.
#[allow(clippy::too_many_arguments)]
fn check_decoded_source_output_pair_v1(
    source: GeneralSourceContextV1<'_>,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    audit: &[u8],
    required: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    charge(budget, 3)?;
    let source_floor = match source {
        GeneralSourceContextV1::Direct(v) => v
            .pre_ranked_retained_analysis_storage_v1()
            .ok_or_else(|| refused("decoded source", "connected Direct source"))?,
        GeneralSourceContextV1::Erased(v) => v.retained_storage_floor_v1(),
    };
    if required < source_floor || budget.storage() < required {
        return Err(E::Resource(AssertOriginResourceV1::Accounting));
    }
    erased_general_scratch_v1(budget, |budget| {
        check_actual_source_output_v1(
            source,
            bound,
            GeneralOutputSubjectV1::ActualPair {
                output,
                rows,
                audit,
                required,
            },
            budget,
        )
    })
}

pub(super) struct CheckedPromotedSites<'s, 'g> {
    source: GeneralSourceContextV1<'s>,
    output: &'s CanonicalKirInventoryV1<'g>,
    output_sites: &'s [ExpandedSiteV1],
    traps: &'s [bool],
}
impl<'s, 'g> CheckedPromotedSites<'s, 'g> {
    fn source(&self) -> GeneralSourceContextV1<'s> {
        self.source
    }
    pub(super) fn output(&self) -> &'s CanonicalKirInventoryV1<'g> {
        self.output
    }
    pub(super) fn statements(&self) -> &'s [ExpandedSiteV1] {
        self.output_sites
    }
    pub(super) fn traps(&self) -> &'s [bool] {
        self.traps
    }
}

/// Inert final occurrence. Eliminated and merged occurrences remain in the
/// complete adjacent history; a surviving CSE operation keeps only its own site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionExpandedSourceOriginV1 {
    output: CanonicalKirOperationCoordinateV1,
    original_u: Option<CanonicalKirOperationCoordinateV1>,
    source_statement: ExpandedSiteV1,
}
impl ProductionExpandedSourceOriginV1 {
    /// Exact final operation coordinate.
    pub const fn output(self) -> CanonicalKirOperationCoordinateV1 {
        self.output
    }
    /// Historical U occurrence, absent for synthesized constants.
    pub const fn original_u(self) -> Option<CanonicalKirOperationCoordinateV1> {
        self.original_u
    }
    /// Inert source statement inherited only from the retained operation.
    pub const fn original_source_statement(self) -> ExpandedSiteV1 {
        self.source_statement
    }
    /// An inert origin never authorizes an operation or executable artifact.
    pub const fn grants_authority(self) -> bool {
        false
    }
}

// This view can only be constructed after complete actual-pair checks below.
// Inert coordinates are insufficient to construct one outside this module.
struct CheckedExpandedOriginsV1<'a, 'u, 'o> {
    seed: &'a CheckedPromotedSites<'a, 'u>,
    output: &'a CanonicalKirInventoryV1<'o>,
    rows: &'a [ProductionExpandedSourceOriginV1],
}
impl CheckedExpandedOriginsV1<'_, '_, '_> {
    fn original_operation(
        &self,
        output: &CanonicalKirInventoryV1<'_>,
        ordinal: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> R<Option<usize>> {
        charge(budget, 9)?;
        if !std::ptr::eq(self.output, output) || self.rows.len() != output.operations().len() {
            return Err(refused(
                "expanded source",
                "actual final inventory and complete rows",
            ));
        }
        let row = self
            .rows
            .get(ordinal)
            .ok_or_else(|| refused("expanded source", "final ordinal"))?;
        if row.output != output.operations()[ordinal].coordinate {
            return Err(refused("expanded source", "exact final occurrence"));
        }
        row.original_u
            .map(|coordinate| {
                let old = operation_ordinal(self.seed.output(), coordinate)?;
                if self.seed.output().operations()[old].coordinate != coordinate
                    || self.seed.statements()[old] != row.source_statement
                {
                    return Err(refused(
                        "expanded source",
                        "checked original source occurrence",
                    ));
                }
                Ok(old)
            })
            .transpose()
    }
    fn trap(&self, ordinal: usize, budget: &mut AssertOriginBudgetV1<'_>) -> R<bool> {
        let Some(old) = self.original_operation(self.output, ordinal, budget)? else {
            return Ok(false);
        };
        charge(budget, 5)?;
        if !self.seed.traps()[old] {
            return Ok(false);
        }
        let before = self.seed.output().operations()[old].operation;
        let after = self.output.operations()[ordinal].operation;
        // Result IDs may legitimately be renumbered, but a trap has none. Its
        // exact callee and empty arguments are rechecked after all substitutions.
        if let (
            OperationKind::Call {
                callee: a,
                arguments: aa,
            },
            OperationKind::Call {
                callee: b,
                arguments: ba,
            },
        ) = (&before.kind, &after.kind)
        {
            charge(
                budget,
                a.as_str()
                    .len()
                    .checked_add(b.as_str().len())
                    .ok_or_else(arithmetic)?,
            )?;
            return Ok(a == b
                && aa.is_empty()
                && ba.is_empty()
                && before.results.is_empty()
                && after.results.is_empty());
        }
        Err(refused(
            "expanded source",
            "retained exact source trap form",
        ))
    }
}

// Each pair is independently checked on the actual owners and candidate row
// backing. This generic bridge is also usable with decoded immutable rows; it
// requires neither an optimizer call nor a fabricated producer witness.
fn expanded_transport_pair_v1(
    input: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    candidate: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    previous: &[ProductionExpandedSourceOriginV1],
    next: &mut Vec<ProductionExpandedSourceOriginV1>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    erased_general_scratch_v1(budget, |budget| {
        charge(budget, 7)?;
        let (before, receipt) =
            CanonicalKirInventoryV1::derive(input, budget).map_err(inventory_error)?;
        budget
            .reserve_storage(receipt.retained_storage())
            .map_err(E::Resource)?;
        let (after, receipt) =
            CanonicalKirInventoryV1::derive(output, budget).map_err(inventory_error)?;
        budget
            .reserve_storage(receipt.retained_storage())
            .map_err(E::Resource)?;
        let (pair, receipt) = fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
            &before, &after, candidate, budget,
        )
        .map_err(transition_error)?;
        budget
            .reserve_storage(receipt.retained_storage())
            .map_err(E::Resource)?;
        if previous.len() != before.operations().len()
            || !next.is_empty()
            || next.capacity() < after.operations().len()
        {
            return Err(refused("expanded source", "complete paid adjacent state"));
        }
        charge(
            budget,
            input
                .canonical()
                .canonical_bytes()
                .len()
                .checked_add(output.canonical().canonical_bytes().len())
                .ok_or_else(arithmetic)?,
        )?;
        if input.module().kernels != output.module().kernels {
            return Err(refused(
                "expanded source",
                "unchanged complete bound root ABI and launch roster",
            ));
        }
        for (row, actual) in pair.rows().operations.iter().zip(after.operations()) {
            charge(budget, 12)?;
            if row.output != actual.coordinate || next.len() == next.capacity() {
                return Err(refused(
                    "expanded source",
                    "exact complete output occurrence",
                ));
            }
            let (original_u, source_statement) = match row.origin {
                CanonicalKirOperationOriginV1::Retained(original) => {
                    let old = operation_ordinal(&before, original)?;
                    if previous[old].output != original {
                        return Err(refused("expanded source", "adjacent retained operation"));
                    }
                    (previous[old].original_u, previous[old].source_statement)
                }
                // A synthesized constant never inherits an operation grant,
                // including when its value came from a checked Add or trap path.
                CanonicalKirOperationOriginV1::ConstantFrom(_) => (None, None),
            };
            next.push(ProductionExpandedSourceOriginV1 {
                output: row.output,
                original_u,
                source_statement,
            });
        }
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
fn check_expanded_source_v1(
    seed: CheckedPromotedSites<'_, '_>,
    intermediate: &CanonicalKirInventoryV1<'_>,
    final_input: &CanonicalKirInventoryV1<'_>,
    refinement: &fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1<'_>,
    forwarding: &fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV1<'_>,
    unroll: &fe2o3_kernel_analysis::CheckedCanonicalKirLoopUnrollPairV1<'_, '_, '_>,
    core: &fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1,
    retained: &mut Vec<ProductionExpandedSourceOriginV1>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    charge(budget, 8)?;
    if !std::ptr::eq(seed.output().owner(), unroll.output())
        || seed.statements().len() != seed.output().operations().len()
        || seed.traps().len() != seed.output().operations().len()
        || !retained.is_empty()
    {
        return Err(refused("expanded source", "actual source-owned U seed"));
    }
    let mut maximum = seed.output().operations().len();
    for round in core.rounds() {
        charge(budget, 4)?;
        maximum = maximum
            .max(round.integer().occurrences().candidate().operations.len())
            .max(round.scalar().occurrences().candidate().operations.len());
    }
    let mut previous = scratch::<ProductionExpandedSourceOriginV1>(maximum, budget)?;
    let mut next = scratch::<ProductionExpandedSourceOriginV1>(maximum, budget)?;
    for (ordinal, row) in seed.output().operations().iter().enumerate() {
        charge(budget, 4)?;
        previous.push(ProductionExpandedSourceOriginV1 {
            output: row.coordinate,
            original_u: Some(row.coordinate),
            source_statement: seed.statements()[ordinal],
        });
    }
    let mut actual = seed.output().owner();
    for round in core.rounds() {
        for (output, rows) in [
            (
                round.integer().owner(),
                round.integer().occurrences().candidate(),
            ),
            (
                round.scalar().owner(),
                round.scalar().occurrences().candidate(),
            ),
        ] {
            expanded_transport_pair_v1(actual, output, rows, &previous, &mut next, budget)?;
            charge(
                budget,
                previous.len().checked_add(1).ok_or_else(arithmetic)?,
            )?;
            previous.clear();
            std::mem::swap(&mut previous, &mut next);
            actual = output;
        }
    }
    if !std::ptr::eq(actual, core.output()) || retained.capacity() < previous.len() {
        return Err(refused(
            "expanded source",
            "complete final history and prepaid rows",
        ));
    }
    let (output, receipt) =
        CanonicalKirInventoryV1::derive(actual, budget).map_err(inventory_error)?;
    budget
        .reserve_storage(receipt.retained_storage())
        .map_err(E::Resource)?;
    budget
        .reserve_storage(std::mem::size_of::<CheckedExpandedOriginsV1<'_, '_, '_>>())
        .map_err(E::Resource)?;
    let transport = CheckedExpandedOriginsV1 {
        seed: &seed,
        output: &output,
        rows: &previous,
    };
    let mut sites = scratch::<ExpandedSiteV1>(previous.len(), budget)?;
    for row in &previous {
        charge(budget, 2)?;
        sites.push(row.source_statement);
    }
    let private = private_memory::check(&output, seed.source().limits().max_operations, budget)?;
    private_memory::source_lifetimes_from_sites(
        seed.source().semantic(),
        &private,
        &sites,
        budget,
    )?;
    let division = unsigned_division::check(&output, seed.source().semantic().target(), budget)?;
    let helpers = scalar_helpers::check(&output, budget)?;
    census::native_with_expanded_source_v1(
        &output,
        &private,
        &division,
        &helpers,
        &transport,
        refinement,
        forwarding,
        intermediate,
        final_input,
        unroll,
        budget,
    )?;
    let kernels =
        derive_checked_output_guarded_obligations_v1(actual, seed.source().limits().max_operations)
            .map_err(E::Formal)?;
    census::formal(&output, &private, &kernels, budget)?;
    if kernels.len() != actual.module().kernels.len() {
        return Err(E::Formal(
            crate::ProductionFormalMemoryErrorV1::ObligationMismatch,
        ));
    }
    charge(
        budget,
        previous.len().checked_mul(2).ok_or_else(arithmetic)?,
    )?;
    // The only caller-row mutation follows every fallible check and payment.
    // Capacity was checked above, so this cannot allocate or leave a prefix on
    // work denial. Failed decoded checks preserve the old caller-owned slots.
    retained.extend_from_slice(&previous);
    Ok(kernels)
}
