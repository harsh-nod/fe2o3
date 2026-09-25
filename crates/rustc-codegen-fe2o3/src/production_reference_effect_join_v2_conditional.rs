//! Production continuation of the existing protected, bound-reference transaction.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, ConditionalTotalViewAnalysisV1,
};
use fe2o3_lower_mir_kernel::{
    ProductionConditionalRootInputV1, ProductionPreRankedKirOwnerV1,
    ProductionRankedAccessSourceV1, ProductionRankedExecutableEffectSourceV1,
};
use fe2o3_pliron::{ProductionConditionalOwnershipSiteV1 as Site, ProductionPlironSessionV1};

type Error = ProductionReferenceEffectJoinErrorV2;
#[cfg(test)]
use crate::production_ranked_projection_v1::conditional_retention_observation_v1 as observation;

pub(crate) struct ReferenceSourceV1<'a> {
    pub(crate) root: u32,
    pub(crate) rank: u8,
    pub(crate) references: &'a AuthenticatedReferenceEffectBindingsV1,
    pub(crate) access: Vec<ProductionRankedAccessSourceV1>,
    pub(crate) effects: Vec<ProductionRankedExecutableEffectSourceV1>,
    pub(crate) ranked_ir: String,
}

/// Distinct from ordinary clean evidence. The arena, source rows, protected
/// runtime and signed effect receipts remain in the original transaction.
/// The aggregate receipt is retained, not reconstructed from an inert report.
pub(crate) struct ConditionalReferenceRootV1 {
    input: ProductionConditionalRootInputV1,
    _runtime: FunctionalRefinementVerusRuntimeLeaseV1,
    _receipts: Vec<InertFunctionalRefinementReceiptSignatureV2>,
    proof: fe2o3_verifier::RetainedProductionConditionalFormulaV1,
    _cpu_bounds_require_host: Option<u32>,
}

pub(crate) enum ReferenceRootV1 {
    // Projection scratch must be dropped before adopting a conditional arena.
    // The private root-roster consumer resolves every pending request.
    Pending(CompilerOwnedReferenceEffectRequestV2),
    Ordinary {
        lowering: ProductionRankedKernelLoweringInputV1,
        receipts: Vec<InertFunctionalRefinementReceiptSignatureV2>,
    },
    Conditional(ConditionalReferenceRootV1),
}

impl ReferenceRootV1 {
    pub(crate) fn kernel(&self) -> &ProductionRankedKernelV1 {
        match self {
            Self::Pending(request) => request.kernel(),
            Self::Ordinary { lowering, .. } => lowering.kernel(),
            Self::Conditional(root) => root
                .input
                .pending
                .kernel()
                .expect("retained immutable conditional arena"),
        }
    }

    pub(crate) fn ordinary(&self) -> Option<&ProductionRankedKernelLoweringInputV1> {
        match self {
            Self::Ordinary { lowering, .. } => Some(lowering),
            Self::Conditional(_) | Self::Pending(_) => None,
        }
    }

    pub(crate) fn into_ordinary(
        self,
    ) -> Result<
        (
            ProductionRankedKernelLoweringInputV1,
            Vec<InertFunctionalRefinementReceiptSignatureV2>,
        ),
        crate::production_ranked_projection_v1::ProductionRankedVerificationErrorV1,
    > {
        match self {
            Self::Pending(_) => Err(crate::production_ranked_projection_v1::ProductionRankedVerificationErrorV1::RosterMetadata(
                "reference proof request was not consumed",
            )),
            Self::Ordinary { lowering, receipts } => Ok((lowering, receipts)),
            Self::Conditional(root) => Err(crate::production_ranked_projection_v1::ProductionRankedVerificationErrorV1::ConditionalFinalizerRequired {
                root: root.input.semantic_root,
            }),
        }
    }
}

impl ConditionalReferenceRootV1 {
    pub(crate) fn input(&self) -> &ProductionConditionalRootInputV1 {
        &self.input
    }

    #[cfg(test)]
    pub(crate) fn report(&self) -> fe2o3_verifier::ProductionConditionalFormulaReportV1 {
        self.proof.report()
    }

    pub(crate) fn retained_storage_v1(&self) -> Result<usize, Resource> {
        self.input
            .retained_storage_v1()?
            .checked_add(self.proof.retained_storage_v1())
            .ok_or(Resource::Arithmetic)
    }

    /// Only the private projection owner calls this with its original ledger.
    /// Moving the arena through the existing continuation replays source and
    /// every fixed-pipeline check; the receipt alone cannot substitute for them.
    pub(crate) fn replay_for_target_v1(
        self,
        source: &ProductionPreRankedKirOwnerV1,
        reference: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1,
        budget: &mut Budget<'_>,
        consume: impl FnOnce(
            &fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1<'_>,
            &fe2o3_verifier::ProductionConditionalFormulaExecutionV1,
            &mut Budget<'_>,
        ) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let Self {
            input,
            proof,
            _runtime,
            _receipts,
            _cpu_bounds_require_host,
        } = self;
        let root = input.semantic_root;
        let transferred = input.retained_storage_v1().map_err(failure)?;
        let floor = budget.storage();
        let account = budget.work_ledger_identity_v1();
        let replay = fe2o3_lower_mir_kernel::with_conditional_root_request_v1(
            source,
            input,
            budget,
            |request, budget| {
                conditional_source_v1::replay_source_bound_cpu_formula_v1(
                    &proof,
                    request,
                    reference,
                    root,
                    budget,
                    |execution, budget| {
                        #[cfg(test)]
                        observation::replay_callback(root, request, execution, budget);
                        consume(request, execution, budget)
                    },
                )
            },
        );
        // The lower continuation reserves the returned arena itself. Transfer
        // its prior reservation, retaining the new one, not both charges.
        if budget.work_ledger_identity_v1() != account || budget.storage() < floor {
            return Err(failure(Resource::Accounting));
        }
        match replay {
            Ok((result, input)) => {
                let expected = floor
                    .checked_add(transferred)
                    .ok_or_else(|| failure(Resource::Arithmetic))?;
                if budget.storage() != expected {
                    return Err(failure(Resource::Accounting));
                }
                budget.release_storage(transferred).map_err(failure)?;
                // The consumer result is nested inside CPU/formula replay.
                // Both errors follow all verifier/lower/account postchecks.
                result??;
                #[cfg(test)]
                observation::replay_accepted(root, &proof, budget);
                Ok(Self {
                    input,
                    proof,
                    _runtime,
                    _receipts,
                    _cpu_bounds_require_host,
                })
            }
            Err(error) => {
                // Lowering destroyed the arena. Its prior charge remains until
                // the enclosing original phase destroys the rest of the roster.
                Err(failure(error))
            }
        }
    }
}

fn failure(error: impl fmt::Display) -> Error {
    Error::ProofExecution(error.to_string())
}

/// No source spelling selects this route. Canonical coverage plus its checked
/// root association decides whether a conditional continuation is applicable.
pub(crate) fn continue_reference_v1<'a>(
    owner: &ProductionPreRankedKirOwnerV1,
    request: CompilerOwnedReferenceEffectRequestV2,
    source: ReferenceSourceV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<(ReferenceRootV1, ReferenceSourceV1<'a>), Error> {
    if !has_conditional_output(owner, source.root, budget)? {
        let (lowering, receipts) = request.prove_and_compile()?;
        return Ok((ReferenceRootV1::Ordinary { lowering, receipts }, source));
    }
    let [binding] = source.references.as_slice() else {
        return Err(Error::BindingCount(source.references.as_slice().len()));
    };
    let subjects = conditional_source_v1::subjects(binding)?;
    let timeout = request.proof_timeout_seconds;
    let bound = request.prove_and_bind()?;
    let site = selected_site(&bound.kernel, budget)?;
    let CompilerOwnedStagedReferenceEffectV2 {
        construction,
        signed_receipts,
        runtime,
        pending_cpu_bounds,
    } = bound.into_staged()?;
    let mut session =
        ProductionPlironSessionV1::new_ranked_v1(ProductionSessionLimitsV1::default())
            .map_err(|error| Error::Construction(format!("{error:?}")))?;
    let registered = session
        .register_construction(construction)
        .map_err(|error| Error::Construction(format!("{error:?}")))?;
    let (stage, root) = session
        .construct_registered(registered)
        .map_err(|error| Error::Construction(format!("{error:?}")))?;
    let pending = session
        .prepare_conditional_ranked_analysis_v1(stage, root, &[site])
        .map_err(|error| Error::Construction(format!("{error:?}")))?;
    let ReferenceSourceV1 {
        root,
        rank,
        references,
        access,
        effects,
        ranked_ir,
    } = source;
    let input = ProductionConditionalRootInputV1 {
        pending,
        semantic_root: root,
        launch_rank: rank,
        access_sources: access,
        executable_effect_sources: effects,
        ranked_ir,
        reference_subjects: subjects,
    };
    let account = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    let (result, input) = fe2o3_lower_mir_kernel::with_conditional_root_request_v1(
        owner,
        input,
        budget,
        |request, budget| {
            conditional_source_v1::retain_source_bound_cpu_formula_v1(
                &runtime, request, references, root, budget, timeout,
            )
        },
    )
    .map_err(failure)?;
    let proof = match result {
        Ok(proof) => proof,
        Err(error) => {
            let retained = input.retained_storage_v1().map_err(failure)?;
            // The arena and its source buffers must die before their charge.
            // Any separate reservation retained by the consumer is untouched.
            drop(input);
            let protected = floor
                .checked_add(retained)
                .ok_or_else(|| failure(Resource::Arithmetic))?;
            if budget.work_ledger_identity_v1() != account || budget.storage() < protected {
                return Err(failure(Resource::Accounting));
            }
            budget.release_storage(retained).map_err(failure)?;
            return Err(error);
        }
    };
    #[cfg(test)]
    observation::retained(root, &proof, budget);
    // Source rows stay with the conditional arena; the ordinary projection
    // fields remain empty, so no later V5 validator can accidentally use them.
    let source = ReferenceSourceV1 {
        root,
        rank,
        references,
        access: Vec::new(),
        effects: Vec::new(),
        ranked_ir: String::new(),
    };
    Ok((
        ReferenceRootV1::Conditional(ConditionalReferenceRootV1 {
            input,
            _runtime: runtime,
            _receipts: signed_receipts,
            proof,
            _cpu_bounds_require_host: pending_cpu_bounds,
        }),
        source,
    ))
}

fn has_conditional_output(
    owner: &ProductionPreRankedKirOwnerV1,
    root: u32,
    budget: &mut Budget<'_>,
) -> Result<bool, Error> {
    budget.charge_work(1).map_err(failure)?;
    if budget.storage() < owner.retained_analysis_storage_v1() {
        return Err(failure(Resource::Accounting));
    }
    let mut found = false;
    for kernel in &owner.executable().module().kernels {
        budget.charge_work(1).map_err(failure)?;
        let analysis = fe2o3_kernel_ir::derive_conditional_total_view_from_verified_v1(
            owner.executable().verified_module_ref_v1(),
            &kernel.id,
            budget,
        )
        .map_err(failure)?;
        let ConditionalTotalViewAnalysisV1::Established(facts) = analysis else {
            continue;
        };
        let binding = owner
            .bind_conditional_output_v1(facts, budget)
            .map_err(failure)?;
        if binding.association().correspondence_owner().index() == root {
            if found {
                return Err(Error::UnsupportedReference(
                    "ambiguous canonical conditional root",
                ));
            }
            found = true;
        }
    }
    Ok(found)
}

fn selected_site(
    kernel: &ProductionRankedKernelV1,
    budget: &mut Budget<'_>,
) -> Result<Site, Error> {
    let mut output = None;
    for block in kernel.blocks() {
        budget.charge_work(1).map_err(failure)?;
        for op in block.operations() {
            budget.charge_work(1).map_err(failure)?;
            if let ProductionRankedOperationV1::RequireEffectRefinement { contract, .. } = op {
                if output.replace(contract.view()).is_some() {
                    return Err(Error::UnsupportedReference("conditional output roster"));
                }
            }
        }
    }
    let output = output.ok_or(Error::UnsupportedReference("missing bound output"))?;
    let mut selected = None;
    for (b, block) in kernel.blocks().iter().enumerate() {
        budget.charge_work(1).map_err(failure)?;
        for (o, op) in block.operations().iter().enumerate() {
            budget.charge_work(1).map_err(failure)?;
            if let ProductionRankedOperationV1::OwnershipContract {
                view,
                coverage,
                partition,
            } = op
                && *view == output
            {
                if selected.is_some()
                    || *coverage != OwnershipCoverageAttr::TotalView
                    || *partition != OwnershipPartitionAttr::ExactSets
                {
                    return Err(Error::AmbiguousOwnership);
                }
                selected = Some(Site {
                    block: u32::try_from(b).map_err(|_| Error::WriteLocation)?,
                    operation: u32::try_from(o).map_err(|_| Error::WriteLocation)?,
                    view: *view,
                });
            }
        }
    }
    selected.ok_or(Error::AmbiguousOwnership)
}
