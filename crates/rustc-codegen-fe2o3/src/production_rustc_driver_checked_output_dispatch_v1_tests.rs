//! Test-only dispatch over the authenticated materialized source policy.
use super::*;
use crate::production_pipeline::{
    ProductionPipelineError, RankedVerifiedProductionCompilation,
    checked_output_policy4_v1::CheckedOutputTargetProductionCompilation,
    erased_checked_output_policy4_v1::ErasedCheckedOutputTargetProductionCompilationV1,
};
use fe2o3_kernel_ir::{FormalMemoryObligations, VerifiedCanonicalKernelIrModuleV12};
use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1;
use fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Route {
    DirectRawEmpty,
    SilentUnitLocal,
}

impl Route {
    fn from_policy(policy: ProductionHelperSourcePolicyV1) -> Self {
        match policy {
            ProductionHelperSourcePolicyV1::RawEmpty => Self::DirectRawEmpty,
            ProductionHelperSourcePolicyV1::UnitLocal => Self::SilentUnitLocal,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RouteObservation {
    route: Route,
    original_helpers: usize,
    original_helper_calls: usize,
    original_private_accesses: usize,
    original_digest: Option<[u8; 32]>,
    erased_digest: Option<[u8; 32]>,
}

pub(super) enum Stage {
    Direct(Box<CheckedOutputTargetProductionCompilation>),
    Erased(Box<ErasedCheckedOutputTargetProductionCompilationV1>),
}

impl Stage {
    pub(super) fn lower(
        ranked: RankedVerifiedProductionCompilation,
    ) -> Result<Self, ProductionPipelineError> {
        match Route::from_policy(ranked.checked_output_source_policy_v1()) {
            Route::DirectRawEmpty => ranked
                .lower_checked_output_policy4_v1()
                .map(Box::new)
                .map(Self::Direct),
            Route::SilentUnitLocal => ranked
                .lower_silent_unit_checked_output_policy4_v1()
                .map(Box::new)
                .map(Self::Erased),
        }
    }

    pub(super) fn output(&self) -> &VerifiedCanonicalKernelIrModuleV12 {
        match self {
            Self::Direct(stage) => {
                assert!(!stage.output().grants_artifact_or_launch_authority());
                stage.output().output()
            }
            Self::Erased(stage) => {
                assert!(!stage.output().grants_artifact_or_launch_authority());
                stage.output().output()
            }
        }
    }

    pub(super) fn checked_output(&self) -> &CheckedCanonicalKernelIrOwnerPolicy4V1 {
        match self {
            Self::Direct(stage) => stage.output().checked_output(),
            Self::Erased(stage) => stage.output().checked_output(),
        }
    }

    pub(super) fn kernels(&self) -> &[FormalMemoryObligations] {
        match self {
            Self::Direct(stage) => stage.output().kernels(),
            Self::Erased(stage) => stage.output().kernels(),
        }
    }

    pub(super) fn semantic(&self) -> &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1 {
        match self {
            Self::Direct(stage) => stage.output().source_semantic_kir().semantic().semantic(),
            Self::Erased(stage) => stage
                .output()
                .original_source()
                .semantic_ssa()
                .source_semantic(),
        }
    }

    pub(super) fn observe_route(&self) -> RouteObservation {
        let (route, original, original_digest, erased_digest) = match self {
            Self::Direct(stage) => (
                Route::DirectRawEmpty,
                stage.output().source_semantic_kir().module(),
                None,
                None,
            ),
            Self::Erased(stage) => (
                Route::SilentUnitLocal,
                stage.output().original_source().executable().module(),
                Some(
                    *stage
                        .output()
                        .original_source()
                        .executable()
                        .canonical()
                        .identity()
                        .digest(),
                ),
                Some(*stage.output().erased().canonical().identity().digest()),
            ),
        };
        let helpers: std::collections::BTreeSet<_> = original
            .functions
            .iter()
            .filter(|f| f.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
            .map(|f| &f.id)
            .collect();
        let mut original_helper_calls = 0;
        let mut original_private_accesses = 0;
        for function in &original.functions {
            for operation in function
                .body
                .as_ref()
                .into_iter()
                .flat_map(|body| &body.blocks)
                .flat_map(|block| &block.operations)
            {
                match &operation.kind {
                    OperationKind::Call { callee, .. } => {
                        original_helper_calls += usize::from(helpers.contains(callee));
                    }
                    OperationKind::Load { access, .. }
                    | OperationKind::Store { access, .. }
                    | OperationKind::GuardedLoad { access, .. }
                    | OperationKind::GuardedStore { access, .. }
                        if helpers.contains(&function.id)
                            && access.address_space == fe2o3_kernel_ir::AddressSpace::Private =>
                    {
                        original_private_accesses += 1;
                    }
                    _ => {}
                }
            }
        }
        RouteObservation {
            route,
            original_helpers: helpers.len(),
            original_helper_calls,
            original_private_accesses,
            original_digest,
            erased_digest,
        }
    }

    pub(super) fn retained_storage_floor_v1(&self) -> usize {
        match self {
            Self::Direct(stage) => stage.retained_storage_floor_v1(),
            Self::Erased(stage) => stage.retained_storage_floor_v1(),
        }
    }

    /// Calls the actual owning producer. An unexpected prepared success is
    /// dropped and returned as Ok, which the unsigned parent must reject.
    pub(super) fn probe_native_source_lineage_v1(
        self,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), ProductionPipelineError> {
        match self {
            Self::Direct(stage) => stage.prepare_native_source_lineage_v1(budget).map(drop),
            Self::Erased(stage) => stage.prepare_native_source_lineage_v1(budget).map(drop),
        }
    }

    pub(super) fn into_worker_handoff_extraction_v1(
        self,
    ) -> Result<
        (
            fe2o3_compiler_ffi::CompilerModuleHandoffV2,
            fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
        ),
        ProductionPipelineError,
    > {
        match self {
            Self::Direct(stage) => stage.into_worker_handoff_extraction_v1(),
            Self::Erased(stage) => stage.into_worker_handoff_extraction_v1(),
        }
    }
}

pub(super) fn check_private_helper_route(
    observation: &Observation,
    require_local: bool,
) -> Result<Route, &'static str> {
    let actual = observation
        .source_route
        .as_ref()
        .ok_or("source route observation unavailable")?;
    if require_local && actual.route != Route::SilentUnitLocal {
        return Err("retained helper did not enter UnitLocal erasure");
    }
    match actual.route {
        Route::DirectRawEmpty => {
            if actual.original_digest.is_some() || actual.erased_digest.is_some() {
                return Err("direct source falsely names checked N/E erasure");
            }
        }
        Route::SilentUnitLocal => {
            if actual.original_helpers == 0
                || actual.original_helper_calls == 0
                || actual.original_private_accesses == 0
            {
                return Err("UnitLocal source did not retain private helper effects");
            }
            let (Some(original), Some(erased)) = (actual.original_digest, actual.erased_digest)
            else {
                return Err("UnitLocal source lacks observed N/E identities");
            };
            if original == erased || original == [0; 32] || erased == [0; 32] {
                return Err("UnitLocal N/E identities are not distinct");
            }
            if observation.internal_helpers != 0 || observation.helper_calls != 0 {
                return Err("silent private helper remains in actual O");
            }
        }
    }
    Ok(actual.route)
}

#[test]
fn qualification_route_exhaustively_classifies_source_policy() {
    assert_eq!(
        Route::from_policy(ProductionHelperSourcePolicyV1::RawEmpty),
        Route::DirectRawEmpty
    );
    assert_eq!(
        Route::from_policy(ProductionHelperSourcePolicyV1::UnitLocal),
        Route::SilentUnitLocal
    );
}

#[test]
fn inert_route_observation_does_not_relabel_frontend_erasure() {
    // This test checks only observation framing, not source or proof admission.
    let mut observation: Observation = serde_json::from_value(serde_json::json!({
        "roots":[], "internal_helpers":0, "helper_calls":0, "reads":0, "writes":0,
        "global_reads":0, "global_writes":0, "private_reads":0, "private_writes":0,
        "other_reads":0, "other_writes":0, "formal_accesses":0, "policy":4,
        "output_digest":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
        "llvm_bytes":0, "descriptor_roots":0, "missing_proof_refused":false
    }))
    .unwrap();
    assert!(check_private_helper_route(&observation, false).is_err());
    observation.source_route = Some(RouteObservation {
        route: Route::DirectRawEmpty,
        original_helpers: 0,
        original_helper_calls: 0,
        original_private_accesses: 0,
        original_digest: None,
        erased_digest: None,
    });
    assert_eq!(
        check_private_helper_route(&observation, false),
        Ok(Route::DirectRawEmpty)
    );
    assert!(check_private_helper_route(&observation, true).is_err());
    let local = observation.source_route.as_mut().unwrap();
    local.route = Route::SilentUnitLocal;
    assert!(check_private_helper_route(&observation, true).is_err());
    let local = observation.source_route.as_mut().unwrap();
    local.original_helpers = 1;
    local.original_helper_calls = 1;
    local.original_private_accesses = 2;
    local.original_digest = Some([1; 32]);
    local.erased_digest = Some([2; 32]);
    assert_eq!(
        check_private_helper_route(&observation, true),
        Ok(Route::SilentUnitLocal)
    );
    observation.helper_calls = 1;
    assert!(check_private_helper_route(&observation, true).is_err());
    observation.helper_calls = 0;
    observation.source_route.as_mut().unwrap().erased_digest = Some([1; 32]);
    assert!(check_private_helper_route(&observation, true).is_err());
}
