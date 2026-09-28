// Backend-private policies over genuine nominal handoffs. The shared target
// continuation owns all text custody; neither implementation accepts a graph,
// report, digest or caller-selected completion predicate as authority.
use fe2o3_kernel_ir::{
    CanonicalClosedScalarFormalScopeV18 as ClosedFormal,
    CanonicalScalarCfgFormalScopeV18 as CfgFormal, ExplicitLaunchExtent, FormalIndexWidth,
    FormalMemoryObligationAnalysis, KernelId, VerifiedCanonicalKernelIrModuleV18 as CanonicalOwner,
};

trait TargetFormalScopeV29 {
    const RESIDUAL: &'static str;
    fn derive(
        &mut self,
        kernel: &KernelId,
        launch: ExplicitLaunchExtent,
        width: FormalIndexWidth,
    ) -> Result<FormalMemoryObligationAnalysis, Error>;
}

mod target_handoff_sealed {
    pub trait Sealed {}
}

pub(crate) trait TargetOutputHandoffV29: target_handoff_sealed::Sealed {
    fn check_original(
        &self,
        source: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<(), SourceError>;
    fn owner(&self, budget: &Budget<'_>) -> Result<&CanonicalOwner, SourceError>;
    fn observe_retained_storage(
        &self,
        required: usize,
        budget: &Budget<'_>,
    ) -> Result<(), SourceError>;
    fn formal(owner: &CanonicalOwner, budget: &mut Budget<'_>) -> Result<(), Error>;
    fn formal_headers() -> Result<usize, Resource>;
}

macro_rules! target_output_policy_v29 {
    ($handoff:ident, $scope:ident, $formal_error:ty, $variant:ident, $residual:literal) => {
        impl TargetFormalScopeV29 for $scope<'_> {
            const RESIDUAL: &'static str = $residual;
            fn derive(
                &mut self,
                kernel: &KernelId,
                launch: ExplicitLaunchExtent,
                width: FormalIndexWidth,
            ) -> Result<FormalMemoryObligationAnalysis, Error> {
                self.derive(kernel, launch, width).map_err(Error::$variant)
            }
        }

        impl target_handoff_sealed::Sealed for $handoff<'_, '_> {}
        impl TargetOutputHandoffV29 for $handoff<'_, '_> {
            fn check_original(
                &self,
                source: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
                budget: &mut Budget<'_>,
            ) -> Result<(), SourceError> {
                self.check_original_source(source, budget)
            }
            fn owner(&self, budget: &Budget<'_>) -> Result<&CanonicalOwner, SourceError> {
                self.output(budget).map(|output| output.owner())
            }
            fn observe_retained_storage(
                &self,
                required: usize,
                budget: &Budget<'_>,
            ) -> Result<(), SourceError> {
                self.observe_retained_storage_v18(required, budget)
            }
            fn formal(owner: &CanonicalOwner, budget: &mut Budget<'_>) -> Result<(), Error> {
                let scope = $scope::new(owner).map_err(Error::$variant)?;
                formal(scope, owner, budget)
            }
            fn formal_headers() -> Result<usize, Resource> {
                sum(&[
                    size_of::<Result<FormalMemoryObligationAnalysis, $formal_error>>(),
                    size_of::<$scope<'_>>(),
                    size_of::<Result<$scope<'_>, $formal_error>>(),
                ])
            }
        }
    };
}

target_output_policy_v29!(
    Handoff,
    ClosedFormal,
    fe2o3_kernel_ir::CanonicalClosedScalarFormalErrorV18,
    Formal,
    "closed scalar formal obligations remain"
);
target_output_policy_v29!(
    CfgHandoff,
    CfgFormal,
    fe2o3_kernel_ir::CanonicalScalarCfgFormalErrorV18,
    ScalarCfgFormal,
    "scalar CFG formal obligations remain"
);
