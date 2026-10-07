//! Stream lifetime exclusion without a submission or publication receipt.

use super::*;

pub(crate) struct ContextUnpublishedHoldV1 {
    stream: RuntimeStreamIdV1,
    local: u64,
    access: Option<ContextGraphReservationV1>,
}

impl ContextUnpublishedHoldV1 {
    pub(super) fn graph_access(&self) -> Option<ContextGraphReservationV1> {
        self.access
    }
    pub(crate) fn is_graph_scoped_v1(&self) -> bool {
        self.access.is_some()
    }
    pub(super) fn identity(&self) -> u64 {
        self.local
    }
    pub(crate) fn stream(&self) -> RuntimeStreamIdV1 {
        self.stream
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    #[cfg(test)]
    pub(crate) fn unpublished_identity_for_test_v1(
        &self,
        stream: RuntimeStreamIdV1,
    ) -> Option<Option<u64>> {
        self.streams.get(&stream).map(|record| record.unpublished)
    }

    pub(super) fn validate_unpublished_hold_v1(
        &self,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_graph_access(hold.access)?;
        if self
            .streams
            .get(&hold.stream)
            .is_some_and(|record| record.unpublished == Some(hold.local))
        {
            Ok(())
        } else {
            Err(RuntimeValidationErrorV1::ContextReserved)
        }
    }
    #[cfg(test)]
    pub(crate) fn hold_unpublished_stream_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
    ) -> Result<ContextUnpublishedHoldV1, RuntimeValidationErrorV1> {
        self.hold_unpublished_stream_with_access_v1(stream, None)
    }

    pub(crate) fn require_unpublished_open_access_v1(
        &self,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_graph_access(access)?;
        if access.is_some() && self.graph_issue_closed {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        Ok(())
    }

    pub(crate) fn hold_unpublished_stream_with_access_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<ContextUnpublishedHoldV1, RuntimeValidationErrorV1> {
        self.require_unpublished_open_access_v1(access)?;
        self.require_stream_unheld_v1(stream)?;
        // Polling a deferred submission can publish it. Do not claim exclusive
        // unpublished custody while an older operation may still make progress.
        if self
            .submissions
            .values()
            .any(|record| record.stream == stream && !record.quiescent)
        {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        // Use the existing stream cell and checked identity allocator; no new
        // registry growth is allowed between installation and driver rooting.
        let local = self.next_id()?;
        self.streams
            .get_mut(&stream)
            .expect("validated stream")
            .unpublished = Some(local);
        Ok(ContextUnpublishedHoldV1 {
            stream,
            local,
            access,
        })
    }

    pub(crate) fn release_unpublished_hold_v1(
        &mut self,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_graph_access(hold.access)?;
        let record = self
            .streams
            .get_mut(&hold.stream())
            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
        if record.unpublished != Some(hold.local) || record.generated.is_some() {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        record.unpublished = None;
        Ok(())
    }

    pub(super) fn require_stream_unheld_v1(
        &self,
        stream: RuntimeStreamIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.unheld_stream_v1(stream).map(|_| ())
    }

    pub(super) fn unheld_stream_v1(
        &self,
        stream: RuntimeStreamIdV1,
    ) -> Result<&StreamRecordV1, RuntimeValidationErrorV1> {
        let record = self
            .streams
            .get(&stream)
            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
        if record.unpublished.is_some() {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        Ok(record)
    }

    pub(super) fn has_unpublished_holds_v1(&self) -> bool {
        self.streams
            .values()
            .any(|record| record.unpublished.is_some())
    }
}
