use super::*;

enum OriginalCopyV1 {
    Local(RuntimeSubmissionV1<RuntimeCopyV1>),
    Peer(RuntimeSubmissionV1<RuntimePeerCopyV1>),
}

impl OriginalCopyV1 {
    fn identity(&self) -> (RuntimeSubmissionIdV1, u64) {
        match self {
            Self::Local(s) => (s.id, s.backend_submission),
            Self::Peer(s) => (s.id, s.backend_submission),
        }
    }
}

/// Original copy observer, not a completed replica. Forgetting this value leaves
/// its Context-owned pending entry intact, blocks cleanup and fails stop before
/// backend destruction. It cannot be converted into an ordinary submission.
#[must_use = "retire the exact original copy before destroying its Context"]
pub struct RuntimeTrackedReplicaCopyV1 {
    reference: RuntimeReplicaReferenceV1,
    original: OriginalCopyV1,
}

impl fmt::Debug for RuntimeTrackedReplicaCopyV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeTrackedReplicaCopyV1")
            .field("submission", &self.original.identity().0)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct RuntimeReplicaCopyRetirementFailureV1<E> {
    pub copy: RuntimeTrackedReplicaCopyV1,
    pub error: RuntimeErrorV1<E>,
}

/// Every variant follows actual original backend release. A successful copy may
/// no longer be a current replica if either allocation was changed after poll.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeReplicaCopySettlementV1 {
    Current(RuntimeReplicaReferenceV1),
    SettledWithoutCurrentReplica(RuntimeCompletionStatusV1),
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    /// Starts one exact whole-allocation local or peer copy with a fixed metadata
    /// reservation. No pending source writer, dependency forwarding, alias,
    /// partial copy or replica chaining is accepted by this first profile.
    /// Native admission and actual transfer policy are unchanged.
    pub fn submit_tracked_replica_copy_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<RuntimeTrackedReplicaCopyV1, RuntimeErrorV1<B::Error>>
    where
        B: RuntimeAsyncCopyBackendV1,
    {
        self.require_graph_access(None)?;
        if source.access != RuntimeAccessV1::Read
            || destination.access != RuntimeAccessV1::Write
            || source.allocation == destination.allocation
            || source.byte_len != destination.byte_len
        {
            return Err(RuntimeValidationErrorV1::InvalidRange.into());
        }
        let source = self.replica_stamp_v1(source)?;
        let destination = self.replica_stamp_v1(destination)?;
        if self.unheld_stream_v1(stream)?.device != destination.record.device {
            return Err(RuntimeValidationErrorV1::WrongDevice.into());
        }
        let pending = ReplicaPendingV1 {
            source,
            destination,
            submission: None,
            writer: None,
            destination_epoch: None,
        };
        let reference = self
            .replicas
            .as_mut()
            .ok_or(RuntimeValidationErrorV1::Unsupported)?
            .reserve(self.context_generation, pending)?;
        // The table roots the attempt before any existing native/custody entry.
        let result = catch_unwind(AssertUnwindSafe(|| {
            if source.record.device == destination.record.device {
                self.copy_async(stream, source.region, destination.region, &[])
                    .map(OriginalCopyV1::Local)
            } else {
                self.peer_copy(stream, source.region, destination.region, &[])
                    .map(OriginalCopyV1::Peer)
            }
        }));
        let original = match result {
            Ok(Ok(original)) => original,
            Ok(Err(error)) => {
                if !self.terminal
                    && matches!(
                        &error,
                        RuntimeErrorV1::Validation(_) | RuntimeErrorV1::BackendRejected(_)
                    )
                {
                    self.replica_slot_after_effect_v1(reference).state = ReplicaStateV1::Vacant;
                } else {
                    self.quarantine_after_async_command_panic_v1();
                }
                return Err(error);
            }
            Err(payload) => {
                self.quarantine_after_async_command_panic_v1();
                std::panic::resume_unwind(payload);
            }
        };
        let bound = match &original {
            OriginalCopyV1::Local(s) => self.bind_replica_submission_v1(s, pending),
            OriginalCopyV1::Peer(s) => self.bind_replica_submission_v1(s, pending),
        };
        match bound {
            Ok(pending) => {
                self.replica_slot_after_effect_v1(reference).state =
                    ReplicaStateV1::Pending(pending);
                Ok(RuntimeTrackedReplicaCopyV1 {
                    reference,
                    original,
                })
            }
            Err(error) => {
                self.quarantine_after_async_command_panic_v1();
                Err(error.into())
            }
        }
    }

    pub(super) fn replica_slot_after_effect_v1(
        &mut self,
        reference: RuntimeReplicaReferenceV1,
    ) -> &mut storage::ReplicaSlotV1 {
        let Some(table) = self.replicas.as_mut() else {
            std::process::abort()
        };
        let Some(slot) = table.slots.get_mut(reference.slot) else {
            std::process::abort()
        };
        if reference.context != self.context_generation
            || slot.incarnation != reference.incarnation
            || !matches!(slot.state, ReplicaStateV1::Pending(_))
        {
            std::process::abort();
        }
        slot
    }

    fn retained_replica_copy_v1(
        &self,
        copy: &RuntimeTrackedReplicaCopyV1,
    ) -> Result<ReplicaPendingV1, RuntimeValidationErrorV1> {
        match &copy.original {
            OriginalCopyV1::Local(s) => {
                self.retained_replica_submission_v1(copy.reference, s, None)
            }
            OriginalCopyV1::Peer(s) => self.retained_replica_submission_v1(copy.reference, s, None),
        }
    }

    pub(super) fn retained_replica_submission_v1<A>(
        &self,
        reference: RuntimeReplicaReferenceV1,
        submission: &RuntimeSubmissionV1<A>,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<ReplicaPendingV1, RuntimeValidationErrorV1> {
        self.require_graph_access(access)?;
        if reference.context != self.context_generation {
            return Err(RuntimeValidationErrorV1::UnknownSubmission);
        }
        let pending = self
            .replicas
            .as_ref()
            .ok_or(RuntimeValidationErrorV1::Unsupported)?
            .pending(reference)?;
        if pending.submission != Some((submission.id, submission.backend_submission))
            || pending.writer.is_none()
            || pending.destination_epoch.is_none()
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        self.submission_record(submission)?;
        Ok(pending)
    }

    pub fn poll_tracked_replica_copy_v1(
        &mut self,
        copy: &mut RuntimeTrackedReplicaCopyV1,
    ) -> Result<RuntimePollV1, RuntimeErrorV1<B::Error>> {
        self.retained_replica_copy_v1(copy)?;
        match &mut copy.original {
            OriginalCopyV1::Local(s) => self.poll(s),
            OriginalCopyV1::Peer(s) => self.poll(s),
        }
    }

    /// Requires the exact original submission's terminal result and successful
    /// native release. Prevalidation decides the metadata outcome before release;
    /// no allocation, callback or fallible promotion follows that release.
    #[allow(clippy::result_large_err)]
    pub fn retire_tracked_replica_copy_v1(
        &mut self,
        copy: RuntimeTrackedReplicaCopyV1,
    ) -> Result<RuntimeReplicaCopySettlementV1, RuntimeReplicaCopyRetirementFailureV1<B::Error>>
    {
        let result = self.prepare_replica_retirement_v1(&copy);
        let (status, fact) = match result {
            Ok(prepared) => prepared,
            Err(error) => return Err(RuntimeReplicaCopyRetirementFailureV1 { copy, error }),
        };
        let release = match &copy.original {
            OriginalCopyV1::Local(s) => self.release_submission_ref(s, None),
            OriginalCopyV1::Peer(s) => self.release_submission_ref(s, None),
        };
        if let Err(error) = release {
            return Err(RuntimeReplicaCopyRetirementFailureV1 { copy, error });
        }
        let slot = self.replica_slot_after_effect_v1(copy.reference);
        slot.state = fact.map_or(ReplicaStateV1::Vacant, ReplicaStateV1::Settled);
        Ok(if fact.is_some() {
            RuntimeReplicaCopySettlementV1::Current(copy.reference)
        } else {
            RuntimeReplicaCopySettlementV1::SettledWithoutCurrentReplica(status)
        })
    }

    fn prepare_replica_retirement_v1(
        &self,
        copy: &RuntimeTrackedReplicaCopyV1,
    ) -> Result<(RuntimeCompletionStatusV1, Option<ReplicaFactV1>), RuntimeErrorV1<B::Error>> {
        match &copy.original {
            OriginalCopyV1::Local(s) => {
                self.prepare_replica_submission_retirement_v1(copy.reference, s, None)
            }
            OriginalCopyV1::Peer(s) => {
                self.prepare_replica_submission_retirement_v1(copy.reference, s, None)
            }
        }
    }

    pub(super) fn prepare_replica_submission_retirement_v1<A>(
        &self,
        reference: RuntimeReplicaReferenceV1,
        submission: &RuntimeSubmissionV1<A>,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<(RuntimeCompletionStatusV1, Option<ReplicaFactV1>), RuntimeErrorV1<B::Error>> {
        let pending = self.retained_replica_submission_v1(reference, submission, access)?;
        let record = self.submission_record(submission)?;
        if !record.quiescent || !record.status.is_terminal() {
            return Err(RuntimeValidationErrorV1::SubmissionPending.into());
        }
        let current = (|| {
            self.validate_replica_stamp_with_access_v1(pending.source, access)?;
            let destination =
                self.replica_stamp_with_access_v1(pending.destination.region, access)?;
            if destination.record != pending.destination.record
                || Some(destination.read.attempt_epoch) != pending.destination_epoch
                || destination.read.content_lineage != destination.read.attempt_epoch
            {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            Ok(ReplicaFactV1 {
                source: pending.source,
                destination,
            })
        })();
        Ok((
            record.status,
            (record.status == RuntimeCompletionStatusV1::Succeeded)
                .then(|| current.ok())
                .flatten(),
        ))
    }
}
