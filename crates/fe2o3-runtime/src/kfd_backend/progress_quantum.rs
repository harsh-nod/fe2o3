use super::*;

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn cooperative_progress_quantum_active_v1(&self) -> bool {
        self.cooperative_progress_quantum.is_some()
    }

    pub(super) fn cooperative_progress_quantum_spent_v1(&self) -> bool {
        self.cooperative_progress_quantum == Some(true)
    }

    pub(super) fn take_cooperative_progress_leaf_v1(&mut self) -> bool {
        match self.cooperative_progress_quantum.as_mut() {
            None => true,
            Some(spent) if !*spent => {
                *spent = true;
                true
            }
            Some(_) => false,
        }
    }

    /// Attempts at most one cooperative Read/Write leaf, including native peer
    /// publication, sampling or retirement. Metadata traversal remains bounded by
    /// admission capacities. Child-native flushes and syscalls have no new bound.
    pub(super) fn progress_stream_quantum_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.cooperative_progress_quantum.is_some() {
            return Err(self.directed_corruption_v1());
        }
        self.cooperative_progress_quantum = Some(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.drive_stream_v1(stream)
        }));
        self.cooperative_progress_quantum = None;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Shares ordering and failure attribution between strict flush and progress.
    pub(super) fn drive_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let route = Self::route(&self.streams, stream, "unknown multi-device KFD stream")?;
        if let Some(submission) = self.deferred_stream_head_v1(stream) {
            self.progress_deferred_compute_v1(submission)?;
            return Ok(());
        }
        self.flush_peer_launch_roots_v1(stream)?;
        if self.cooperative_progress_quantum_spent_v1() {
            return Ok(());
        }
        if let Some(local) = self.children[route.child]
            .pending_compute_streams
            .get(&route.local)
            .and_then(|ids| ids.front())
            .copied()
        {
            let result = self.service_native_peer_prefix_v1(
                RoutedHandleV1 {
                    child: route.child,
                    local,
                },
                true,
            );
            if let Err(error) = result {
                if !self.terminal {
                    self.retire_flushed_peer_launches_v1(stream)?;
                }
                return Err(error);
            }
        }
        if self.cooperative_progress_quantum_spent_v1() {
            return Ok(());
        }
        if let Some(submission) =
            self.cooperative_stream_tails
                .get(&stream)
                .copied()
                .filter(|submission| {
                    matches!(self.submissions.get(submission),
                    Some(RoutedSubmissionV1::CooperativeCopy(copy)) if !copy.is_quiescent())
                })
        {
            loop {
                let progress_before = self.cooperative_progress_generation;
                let status = if matches!(self.submissions.get(&submission),
                    Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.directed.is_some())
                {
                    self.progress_retained_directed_peer_v1(submission)?
                } else {
                    self.progress_cooperative_copy(submission)?
                };
                match status {
                    BackendPollV1::Succeeded => break,
                    BackendPollV1::Failed { .. } => {
                        return Err(KfdRuntimeBackendV1::quiescent_error(
                            KfdRuntimeBackendErrorKindV1::Native,
                            "multi-device cooperative flush ended in quiescent failure",
                        ));
                    }
                    BackendPollV1::Pending
                        if self.cooperative_progress_quantum_active_v1()
                            || self.cooperative_progress_generation == progress_before =>
                    {
                        return Ok(());
                    }
                    BackendPollV1::Pending => {}
                }
            }
        }
        if self.cooperative_progress_quantum_spent_v1()
            || self.compute_xgmi_child_occupied_v1(route.child)
        {
            return Ok(());
        }
        let result = self.children[route.child].flush_stream_v1(route.local);
        let result = self.latch(result);
        if !self.terminal {
            self.retire_flushed_peer_launches_v1(stream)?;
        }
        result
    }
}
