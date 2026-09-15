//! Inert source-to-final write requests. No proof is executed, re-scoped, or
//! attached here; the ordinary canonical write-contract gate stays closed.

use super::{
    AuthenticatedRankedVerificationRosterV1, FunctionalProofPhaseKindV1,
    ProductionRankedKernelRosterIdentityV1, RankedRootSemanticBindingRecordV1,
    validate_ranked_roster_semantic_bindings_v1,
};
use crate::reference_effect_v1::{
    AuthenticatedReferenceEffectBindingV1, AuthenticatedReferenceEffectBindingsV1,
};
use dialect_kernel::{DYNAMIC_EXTENT, SemanticNumericalContractV1, SemanticTypedExpressionV1};
use fe2o3_kernel_analysis::{CanonicalRankedViewPreflightV1, preflight_canonical_ranked_view_v1};
use fe2o3_kernel_ir::{
    Function, FunctionId, FunctionRole, LaunchExtent, Module, Type, ValueId,
    VerifiedCanonicalKernelIrV13,
};
use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
use fe2o3_pliron::{
    ContextIdentity, ProductionEffectRefinementContractV2, ProductionRankedKernelV1,
    ProductionRankedOperationV1, ProductionReferenceOutputSiteV2, ProductionReferenceProofV2,
};
use std::collections::BTreeSet;

#[cfg(test)]
mod tests;
mod typed_expression;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum FinalWriteContractRequestErrorV1 {
    SourceProof(String),
    SourceOwner(String),
    SourceExport(String),
    CanonicalPreflight(String),
    SubjectChanged,
    RootBijection,
    ReferenceBinding,
    ParameterIdentity,
    OutputBijection,
    UnsupportedDynamicExtent,
    UnsupportedPartialOrFrame,
    UnsupportedLoad,
    UnsupportedNumericalPolicy,
    UnboundScalarSymbol(u32),
    InvalidTypedExpression,
    ExpressionLimit,
}
type Failure = FinalWriteContractRequestErrorV1;

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "final write request rejected: {self:?}")
    }
}
impl std::error::Error for Failure {}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FinalWriteParameterBindingV1 {
    source_argument: u32,
    source_parameter: ValueId,
    final_parameter: ValueId,
    final_ordinal: u32,
    scalar_type: Type,
}
impl FinalWriteParameterBindingV1 {
    pub(crate) fn source_argument(&self) -> u32 {
        self.source_argument
    }
    pub(crate) fn source_parameter(&self) -> ValueId {
        self.source_parameter
    }
    pub(crate) fn final_parameter(&self) -> ValueId {
        self.final_parameter
    }
    pub(crate) fn final_ordinal(&self) -> u32 {
        self.final_ordinal
    }
    pub(crate) fn parameter_type(&self) -> &Type {
        &self.scalar_type
    }
}

#[derive(Debug)]
pub(crate) struct InertFinalWriteContractRequestV1 {
    final_write_ordinal: usize,
    source_effect: ProductionEffectRefinementContractV2,
    source_view: ProductionRankedOperationV1,
    source_ownership: ProductionRankedOperationV1,
    source_proof_request: ProductionReferenceProofV2,
    reference_rhs: SemanticTypedExpressionV1,
    numerical: SemanticNumericalContractV1,
}
impl InertFinalWriteContractRequestV1 {
    pub(crate) fn final_write_ordinal(&self) -> usize {
        self.final_write_ordinal
    }
    pub(crate) fn source_effect(&self) -> &ProductionEffectRefinementContractV2 {
        &self.source_effect
    }
    pub(crate) fn source_view(&self) -> &ProductionRankedOperationV1 {
        &self.source_view
    }
    pub(crate) fn source_ownership(&self) -> &ProductionRankedOperationV1 {
        &self.source_ownership
    }
    /// Source per-effect request, never a receipt for the final write.
    pub(crate) fn source_proof_request(&self) -> ProductionReferenceProofV2 {
        self.source_proof_request
    }
    pub(crate) fn reference_rhs(&self) -> &SemanticTypedExpressionV1 {
        &self.reference_rhs
    }
    pub(crate) fn numerical_contract(&self) -> SemanticNumericalContractV1 {
        self.numerical
    }
}

#[derive(Debug)]
pub(crate) struct InertFinalWriteContractRootV1 {
    source_context: ContextIdentity,
    source_epoch: u64,
    source_graph: [u8; 32],
    // Retain the original coordinate/domain DAG, not a parallel proof model.
    source_kernel: ProductionRankedKernelV1,
    reference: AuthenticatedReferenceEffectBindingV1,
    parameters: Box<[FinalWriteParameterBindingV1]>,
    preflight: CanonicalRankedViewPreflightV1,
    requests: Box<[InertFinalWriteContractRequestV1]>,
}
impl InertFinalWriteContractRootV1 {
    pub(crate) fn source_context(&self) -> ContextIdentity {
        self.source_context
    }
    pub(crate) fn source_epoch(&self) -> u64 {
        self.source_epoch
    }
    pub(crate) fn source_graph(&self) -> &[u8; 32] {
        &self.source_graph
    }
    pub(crate) fn source_kernel(&self) -> &ProductionRankedKernelV1 {
        &self.source_kernel
    }
    pub(crate) fn reference(&self) -> &AuthenticatedReferenceEffectBindingV1 {
        &self.reference
    }
    pub(crate) fn parameters(&self) -> &[FinalWriteParameterBindingV1] {
        &self.parameters
    }
    pub(crate) fn preflight(&self) -> &CanonicalRankedViewPreflightV1 {
        &self.preflight
    }
    pub(crate) fn requests(&self) -> &[InertFinalWriteContractRequestV1] {
        &self.requests
    }
}

#[derive(Debug)]
pub(crate) struct InertFinalWriteContractRequestsV1 {
    source_canonical: Box<[u8]>,
    final_canonical: Box<[u8]>,
    final_epoch: u64,
    source_roster: ProductionRankedKernelRosterIdentityV1,
    roots: Box<[InertFinalWriteContractRootV1]>,
}
impl InertFinalWriteContractRequestsV1 {
    pub(crate) fn roots(&self) -> &[InertFinalWriteContractRootV1] {
        &self.roots
    }
    pub(crate) fn require_exact(
        &self,
        source: &VerifiedCanonicalKernelIrV13,
        final_graph: &VerifiedCanonicalKernelIrV13,
        epoch: u64,
        source_roster: ProductionRankedKernelRosterIdentityV1,
    ) -> Result<(), Failure> {
        if self.source_canonical.as_ref() != source.canonical_bytes()
            || self.final_canonical.as_ref() != final_graph.canonical_bytes()
            || self.final_epoch != epoch
            || self.source_roster != source_roster
        {
            return Err(Failure::SubjectChanged);
        }
        Ok(())
    }
    pub(crate) const fn grants_graph_verification_authority(&self) -> bool {
        false
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Preflight only, before source proof custody moves into the prepared phase.
/// A successful result would still need final effect/ownership proofs and live
/// graph attachment before the mandatory common scheduler can admit writes.
pub(crate) fn prepare_final_write_contract_requests_v1(
    roster: &AuthenticatedRankedVerificationRosterV1,
    semantic_kir: &ProductionSemanticKirOwnerV1,
    references: &AuthenticatedReferenceEffectBindingsV1,
    final_canonical: &VerifiedCanonicalKernelIrV13,
    final_epoch: u64,
) -> Result<InertFinalWriteContractRequestsV1, Failure> {
    roster
        .require_functional_phase(FunctionalProofPhaseKindV1::Source)
        .map_err(|e| Failure::SourceProof(e.to_string()))?;
    semantic_kir
        .verify_equivalence()
        .map_err(|e| Failure::SourceOwner(e.to_string()))?;
    let source = semantic_kir
        .canonical_kernel_ir_v13()
        .ok_or(Failure::SubjectChanged)?;
    let final_module = fe2o3_kernel_ir::decode_module_v13(final_canonical.canonical_bytes())
        .map_err(|e| Failure::CanonicalPreflight(e.to_string()))?;
    require_kernel_bijection(roster, semantic_kir.module())?;
    require_kernel_bijection(roster, &final_module)?;
    if references.as_slice().len() != roster.roots().len() {
        return Err(Failure::RootBijection);
    }
    let metadata = roster
        .roots()
        .iter()
        .map(|root| {
            Ok(RankedRootSemanticBindingRecordV1 {
                export_symbol: root.export_symbol(),
                semantic_root: root.semantic_root(),
                semantic_root_identity: root.semantic_root_identity(),
                kernel_binding: *root.kernel_binding(),
                function_name: std::str::from_utf8(root.export_symbol())
                    .map_err(|_| Failure::RootBijection)?,
            })
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    validate_ranked_roster_semantic_bindings_v1(semantic_kir.semantic(), &metadata)
        .map_err(|e| Failure::SourceOwner(e.to_string()))?;
    let mut consumed_references = BTreeSet::new();
    let mut roots = Vec::with_capacity(roster.roots().len());
    for root in roster.roots() {
        let reference_index = unique_index(
            references
                .as_slice()
                .iter()
                .map(|r| r.logical_kernel_name == root.logical_name()),
        )
        .ok_or(Failure::RootBijection)?;
        if !consumed_references.insert(reference_index) {
            return Err(Failure::RootBijection);
        }
        let reference = &references.as_slice()[reference_index];
        if reference.effect_ir_sha256 == [0; 32]
            || reference.effect_ir_sha256 != reference.effect_ir.canonical_sha256_v1()
            || reference.observable_output_writes.is_empty()
        {
            return Err(Failure::ReferenceBinding);
        }
        let lowering = semantic_kir
            .ranked_lowering_for_root(root.semantic_root())
            .ok_or(Failure::RootBijection)?;
        let evidence = root.verification().middle_end_evidence();
        let replay = fe2o3_pliron::ProductionMiddleEndEvidenceV5::try_new(
            semantic_kir.semantic(),
            lowering,
            evidence.ranked_ir(),
        )
        .map_err(|e| Failure::SourceOwner(e.to_string()))?;
        if replay.canonical_bytes() != evidence.canonical_bytes() {
            return Err(Failure::SubjectChanged);
        }
        let export = lowering
            .export_live_source_reference_contracts_v1()
            .map_err(|e| Failure::SourceExport(e.to_string()))?;
        let function_id = FunctionId::new(
            std::str::from_utf8(root.export_symbol()).map_err(|_| Failure::RootBijection)?,
        );
        let preflight =
            preflight_canonical_ranked_view_v1(final_canonical, final_epoch, &function_id)
                .map_err(|e| Failure::CanonicalPreflight(e.to_string()))?;
        let parameters = bind_parameters(semantic_kir, preflight.function())?;
        let final_arguments = preflight
            .writes()
            .iter()
            .enumerate()
            .map(|(ordinal, write)| {
                let parameter = parameters
                    .get(write.parameter_ordinal())
                    .ok_or(Failure::ParameterIdentity)?;
                if Some(parameter.final_parameter) != preflight.write_parameter(ordinal) {
                    return Err(Failure::ParameterIdentity);
                }
                reference
                    .effect_ir
                    .reference_argument_for_kernel_argument_v1(parameter.source_argument)
                    .map_err(|_| Failure::ParameterIdentity)
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        let reference_sites = reference
            .observable_output_writes
            .iter()
            .map(|o| ProductionReferenceOutputSiteV2::new(o.argument, o.block, o.statement))
            .collect::<Vec<_>>();
        let source_sites = export
            .outputs()
            .iter()
            .map(|o| o.effect().reference_output_site())
            .collect::<Vec<_>>();
        let output_bindings = bind_output_sites(&final_arguments, &reference_sites, &source_sites)?;
        let mut requests = Vec::with_capacity(export.outputs().len());
        for (ordinal, (write, &(_, effect_index))) in
            preflight.writes().iter().zip(&output_bindings).enumerate()
        {
            let facts = &export.outputs()[effect_index];
            let binding = facts.proof_request().binding();
            if binding.safe_reference_identity().as_bytes() != &reference.reference.function_sha256
                || binding.safe_reference_mir_hash().as_bytes()
                    != &reference.reference.rustc_mir_body_sha256
                || binding.kernel_subject_identity().as_bytes() != &reference.kernel.function_sha256
                || binding.kernel_mir_hash().as_bytes() != &reference.kernel.rustc_mir_body_sha256
            {
                return Err(Failure::ReferenceBinding);
            }
            let numerical = typed_expression::numerical(facts.numerical_contract())?;
            let reference_rhs = typed_expression::reference_rhs(
                facts.reference_rhs(),
                facts.numerical_contract(),
                &parameters,
            )?;
            let gpu_rhs = preflight
                .write_rhs(ordinal)
                .ok_or(Failure::OutputBijection)?;
            if numerical != preflight.numerical_contract()
                || reference_rhs.scalar() != gpu_rhs.scalar()
            {
                return Err(Failure::UnsupportedNumericalPolicy);
            }
            // No RHS equality is asserted here. In particular the final GPU
            // expression is never substituted for the independent CPU root.
            require_total_static_write(preflight.write_view_shape(ordinal), write.predicate())?;
            requests.push(InertFinalWriteContractRequestV1 {
                final_write_ordinal: ordinal,
                source_effect: facts.effect().clone(),
                source_view: facts.view_definition().clone(),
                source_ownership: facts.ownership_contract().clone(),
                source_proof_request: facts.proof_request(),
                reference_rhs,
                numerical,
            });
        }
        let kernel = final_module
            .kernels
            .iter()
            .find(|k| k.entry == function_id)
            .ok_or(Failure::RootBijection)?;
        if kernel
            .domain
            .extents()
            .any(|e| matches!(e, LaunchExtent::Dynamic))
        {
            return Err(Failure::UnsupportedDynamicExtent);
        }
        roots.push(InertFinalWriteContractRootV1 {
            source_context: export.source_context_identity(),
            source_epoch: export.source_mutation_epoch(),
            source_graph: *export.live_graph_sha256(),
            source_kernel: export.source_kernel().clone(),
            reference: reference.clone(),
            parameters: parameters.into_boxed_slice(),
            preflight,
            requests: requests.into_boxed_slice(),
        });
    }
    if consumed_references.len() != references.as_slice().len() {
        return Err(Failure::RootBijection);
    }
    Ok(InertFinalWriteContractRequestsV1 {
        source_canonical: source.canonical_bytes().into(),
        final_canonical: final_canonical.canonical_bytes().into(),
        final_epoch,
        source_roster: roster.canonical_roster_identity(),
        roots: roots.into_boxed_slice(),
    })
}

fn require_kernel_bijection(
    roster: &AuthenticatedRankedVerificationRosterV1,
    module: &Module,
) -> Result<(), Failure> {
    if module.kernels.len() != roster.roots().len() {
        return Err(Failure::RootBijection);
    }
    let mut used = BTreeSet::new();
    for root in roster.roots() {
        let index = unique_index(
            module
                .kernels
                .iter()
                .map(|k| k.id.as_str().as_bytes() == root.export_symbol()),
        )
        .ok_or(Failure::RootBijection)?;
        let kernel = &module.kernels[index];
        if !used.insert(index)
            || kernel.entry.as_str().as_bytes() != root.export_symbol()
            || kernel.domain.rank() != root.source_rank()
        {
            return Err(Failure::RootBijection);
        }
    }
    Ok(())
}

fn unique_index(matches: impl IntoIterator<Item = bool>) -> Option<usize> {
    let mut found = None;
    for (index, matches) in matches.into_iter().enumerate() {
        if matches {
            if found.is_some() {
                return None;
            }
            found = Some(index);
        }
    }
    found
}

fn bind_output_sites(
    final_arguments: &[u32],
    references: &[ProductionReferenceOutputSiteV2],
    source: &[ProductionReferenceOutputSiteV2],
) -> Result<Vec<(usize, usize)>, Failure> {
    if final_arguments.is_empty()
        || final_arguments.len() != references.len()
        || references.len() != source.len()
    {
        return Err(Failure::OutputBijection);
    }
    let mut outputs = BTreeSet::new();
    let mut effects = BTreeSet::new();
    final_arguments
        .iter()
        .map(|argument| {
            let output = unique_index(references.iter().map(|site| site.argument() == *argument))
                .ok_or(Failure::OutputBijection)?;
            let effect = unique_index(source.iter().map(|site| *site == references[output]))
                .ok_or(Failure::OutputBijection)?;
            if !outputs.insert(output) || !effects.insert(effect) {
                return Err(Failure::OutputBijection);
            }
            Ok((output, effect))
        })
        .collect()
}

fn bind_parameters(
    owner: &ProductionSemanticKirOwnerV1,
    final_function: &Function,
) -> Result<Vec<FinalWriteParameterBindingV1>, Failure> {
    let source = owner
        .module()
        .function(&final_function.id)
        .ok_or(Failure::ParameterIdentity)?;
    if source.role != FunctionRole::KernelEntry
        || final_function.role != source.role
        || source.signature != final_function.signature
    {
        return Err(Failure::ParameterIdentity);
    }
    let source_parameters = &source
        .body
        .as_ref()
        .ok_or(Failure::ParameterIdentity)?
        .parameters;
    let final_parameters = &final_function
        .body
        .as_ref()
        .ok_or(Failure::ParameterIdentity)?
        .parameters;
    if source_parameters.len() != source.signature.parameters.len()
        || final_parameters.len() != source_parameters.len()
    {
        return Err(Failure::ParameterIdentity);
    }
    let mut arguments = BTreeSet::new();
    let mut final_ids = BTreeSet::new();
    source_parameters
        .iter()
        .zip(final_parameters)
        .enumerate()
        .map(|(ordinal, (&source_parameter, &final_parameter))| {
            let source_argument = owner
                .source_argument_for_kernel_parameter(&source.id, source_parameter)
                .ok_or(Failure::ParameterIdentity)?;
            if !arguments.insert(source_argument) || !final_ids.insert(final_parameter) {
                return Err(Failure::ParameterIdentity);
            }
            Ok(FinalWriteParameterBindingV1 {
                source_argument,
                source_parameter,
                final_parameter,
                final_ordinal: u32::try_from(ordinal).map_err(|_| Failure::ParameterIdentity)?,
                scalar_type: source.signature.parameters[ordinal].clone(),
            })
        })
        .collect()
}

fn require_total_static_write(
    shape: Option<&[u64]>,
    predicate: Option<ValueId>,
) -> Result<(), Failure> {
    let shape = shape.ok_or(Failure::OutputBijection)?;
    if shape.is_empty()
        || shape
            .iter()
            .any(|extent| *extent == DYNAMIC_EXTENT || *extent == 0)
    {
        return Err(Failure::UnsupportedDynamicExtent);
    }
    if predicate.is_some() {
        return Err(Failure::UnsupportedPartialOrFrame);
    }
    Ok(())
}
