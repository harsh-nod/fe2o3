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
    fn formal(&self, owner: &CanonicalOwner, budget: &mut Budget<'_>) -> Result<(), Error>;
    fn formal_headers() -> Result<usize, Resource>;
}

// Closed mixed handoffs add the original ABI and fresh contract operations.
// Worker preparation remains generic over the exact retained handoff type.
pub(crate) trait MixedTargetOutputHandoffV29: TargetOutputHandoffV29 {
    fn check_original_argument_abi_v26(
        &self,
        abi: fe2o3_lower_mir_kernel::ProductionKernelArgumentAbiInputV18<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), SourceError>;
    fn emit_mixed_contract_v26(
        &self,
        root: usize,
        abi: fe2o3_lower_mir_kernel::ProductionKernelArgumentAbiInputV18<'_>,
        table: &fe2o3_kernel_descriptor::DeviceDescriptorTableV3<'_>,
        ordinal: usize,
        output: &mut [u8],
        budget: &mut Budget<'_>,
    ) -> Result<usize, SourceError>;
}

macro_rules! mixed_target_contract_v29 {
    ([$($generics:tt)*] $handoff:ty) => {
        impl $($generics)* MixedTargetOutputHandoffV29 for $handoff {
            fn check_original_argument_abi_v26(&self,
                abi: fe2o3_lower_mir_kernel::ProductionKernelArgumentAbiInputV18<'_>,
                budget: &mut Budget<'_>) -> Result<(), SourceError>
            { self.check_original_argument_abi_v26(abi, budget) }
            fn emit_mixed_contract_v26(&self, root: usize,
                abi: fe2o3_lower_mir_kernel::ProductionKernelArgumentAbiInputV18<'_>,
                table: &fe2o3_kernel_descriptor::DeviceDescriptorTableV3<'_>,
                ordinal: usize, output: &mut [u8], budget: &mut Budget<'_>) -> Result<usize, SourceError>
            { self.emit_mixed_contract_v26(root, abi, table, ordinal, output, budget) }
        }
    };
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
            fn formal(&self, owner: &CanonicalOwner, budget: &mut Budget<'_>) -> Result<(), Error> {
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

impl target_handoff_sealed::Sealed for BoundHandoff<'_, '_> {}
impl TargetOutputHandoffV29 for BoundHandoff<'_, '_> {
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
    fn formal(&self, owner: &CanonicalOwner, budget: &mut Budget<'_>) -> Result<(), Error> {
        // This nominal handoff already consumed complete fresh paired reports,
        // then the actual ranked/native checks, before adopting this same owner.
        // It is constructed only with the private binding context in this lane.
        self.check_reported_owner_v19(owner, budget)?;
        Ok(())
    }
    fn formal_headers() -> Result<usize, Resource> {
        Ok(size_of::<Result<(), SourceError>>())
    }
}
