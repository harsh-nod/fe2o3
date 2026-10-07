//! Per-member copied results cannot release the common arena backing.

use super::*;

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeChargedResultV1<T> {
    /// Delivers only this member's original result gate after its actual copied
    /// readback and decoder. The arena can still own all common native DATA and
    /// original source residuals; no scalar receipt or graph version is minted.
    pub fn take_arena1024_result_v1<
        'scope,
        P: fe2o3_runtime::RuntimeGfx942RegistryCompletionCarrierV1,
    >(
        &mut self,
        scope: &fe2o3_runtime::RuntimeGfx942Arena1024ScopeV1<'scope, '_, P>,
        ticket: &fe2o3_runtime::RuntimeGfx942Arena1024TicketV1<'scope>,
    ) -> Result<Option<ChargedTypedResultV1<T>>, Error> {
        self.take_completed_matching_v1(|gate| {
            scope.result_matches_owner_v1(ticket, gate).unwrap_or(false)
        })
    }
}
