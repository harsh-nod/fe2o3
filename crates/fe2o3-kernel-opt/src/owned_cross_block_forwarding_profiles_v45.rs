//! Closed admitted owners for the shared must-value worklist.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirControlFlowViewV1 as Flow, CanonicalKirFunctionCoordinateV1 as Function,
    ControlFlowLimits, VerifiedCanonicalKernelIrModuleV18 as Owner18,
    with_canonical_kir_control_flow_v1, with_canonical_kir_control_flow_v18,
};
pub(super) trait Profile: Sized {
    fn with_flow<'g, 'w, T>(
        &'g self,
        function: Function,
        limits: ControlFlowLimits,
        budget: &mut Budget<'w>,
        run: impl for<'s> FnOnce(&mut Flow<'s, 'g, Self>, &mut Budget<'w>) -> Result<T>,
    ) -> Result<T>;
}
macro_rules! profile {
    ($owner:ty, $flow:ident) => {
        impl Profile for $owner {
            fn with_flow<'g, 'w, T>(
                &'g self,
                function: Function,
                limits: ControlFlowLimits,
                budget: &mut Budget<'w>,
                run: impl for<'s> FnOnce(&mut Flow<'s, 'g, Self>, &mut Budget<'w>) -> Result<T>,
            ) -> Result<T> {
                $flow(self, function, limits, budget, run)
            }
        }
    };
}
profile!(Owner, with_canonical_kir_control_flow_v1);
profile!(Owner18, with_canonical_kir_control_flow_v18);
