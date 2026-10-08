//! Indexed original result gates after whole-cohort runtime settlement.

use super::*;

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeChargedResultV1<T> {
    /// Takes this original member's output after the entire ordered Batch3 has
    /// retired and all three decoders returned successfully. This creates no
    /// per-member native receipt. Foreign member indices/tickets retain output.
    pub fn take_cohort3_completed_v1<'scope, P>(
        &mut self,
        scope: &fe2o3_runtime::RuntimeGfx942GeneratedScopeV1<
            'scope,
            '_,
            fe2o3_runtime::KfdRuntimeBackendV1,
            fe2o3_runtime::RuntimeGfx942GeneratedCohort3V1<P>,
        >,
        ticket: &fe2o3_runtime::RuntimeGfx942ScopedCohort3TicketV1<'scope>,
        member: usize,
    ) -> Result<Option<ChargedTypedResultV1<T>>, Error>
    where
        P: fe2o3_runtime::RuntimeGfx942GeneratedCompletionCarrierV1,
    {
        self.take_completed_matching_v1(|gate| {
            scope
                .cohort3_completion_matches_owner_v1(ticket, member, gate)
                .unwrap_or(false)
        })
    }
}
