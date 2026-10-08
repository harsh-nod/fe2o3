//! Copied result delivery is independent of the common native DATA debit.

use super::*;

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeChargedResultV1<T> {
    /// Original N16 copied-result gate only. This does not retire the common
    /// DATA owner, its Context hold, or any of the sixteen source residuals.
    pub fn take_registry16_result_v1<
        'scope,
        P: fe2o3_runtime::RuntimeGfx942RegistryCompletionCarrierV1,
    >(
        &mut self,
        scope: &fe2o3_runtime::RuntimeGfx942Registry16ScopeV1<'scope, '_, P>,
        ticket: &fe2o3_runtime::RuntimeGfx942Registry16TicketV1<'scope>,
    ) -> Result<Option<ChargedTypedResultV1<T>>, Error> {
        self.take_completed_matching_v1(|gate| {
            scope.result_matches_owner_v1(ticket, gate).unwrap_or(false)
        })
    }

    /// Takes the original member output after actual copy and its original
    /// decoder. Common native backing and source-byte credits can remain live in
    /// the scope: this operation neither retires DATA nor yields a graph version.
    pub fn take_registry4_result_v1<
        'scope,
        P: fe2o3_runtime::RuntimeGfx942RegistryCompletionCarrierV1,
    >(
        &mut self,
        scope: &fe2o3_runtime::RuntimeGfx942Registry4ScopeV1<'scope, '_, P>,
        ticket: &fe2o3_runtime::RuntimeGfx942Registry4TicketV1<'scope>,
    ) -> Result<Option<ChargedTypedResultV1<T>>, Error> {
        self.take_completed_matching_v1(|gate| {
            scope.result_matches_owner_v1(ticket, gate).unwrap_or(false)
        })
    }
}
