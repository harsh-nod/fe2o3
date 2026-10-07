use super::*;

impl KfdNativeXgmiRuntimeBackendV1 {
    pub(in crate::kfd_backend) fn admit_retained_batch(
        &mut self,
        requested: &[u64],
    ) -> Result<
        (Admission, Vec<u64>, Vec<Gfx942XgmiSdmaCopyRequestV1>),
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        self.require_live()?;
        if !self.events.is_empty()
            || !self.submissions.is_empty()
            || !self.directed_roots.is_empty()
            || requested.len() != self.active.len()
            || self
                .active
                .values()
                .any(|r| r.ticket.is_some() || r.sequence.is_some() || !r.dependencies.is_empty())
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "retained XGMI requires only the complete unpublished ordinary roster",
            ));
        }
        let selection = admit_with_sharing(
            requested,
            &self.active,
            &self.ready_by_direction,
            &self.in_flight_by_direction,
            &self.submissions,
            Vec::try_reserve_exact,
            |left, right, allocation| xgmi_directed::shared_read(self, left, right, allocation),
        )
        .map_err(|error| match error {
            AdmissionError::Corrupt => self.terminal_error("retained XGMI index corruption"),
            AdmissionError::Capacity => Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "retained XGMI ready-index storage",
            ),
            AdmissionError::Busy => Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "retained XGMI requires the complete directional roster",
            ),
            AdmissionError::Invalid => Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "invalid retained XGMI roster",
            ),
        })?;
        let valid = self
            .batch_custody_is_valid(requested, selection.admission)
            .map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "retained XGMI dependency-index storage",
                )
            })?;
        let admission = qualify_selection(selection, valid).map_err(|error| match error {
            AdmissionError::Busy => Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "retained XGMI requires disjoint mappings",
            ),
            _ => self.terminal_error("retained XGMI custody corruption"),
        })?;
        let mut ids = Vec::new();
        let mut requests = Vec::new();
        ids.try_reserve_exact(requested.len()).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "retained XGMI roster storage",
            )
        })?;
        requests.try_reserve_exact(requested.len()).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "retained XGMI request storage",
            )
        })?;
        ids.extend(self.ready_by_direction[admission.direction].iter().copied());
        if self.in_flight_by_direction[admission.direction].capacity() < ids.len() {
            return Err(self.terminal_error("retained XGMI lacks reserved in-flight slots"));
        }
        Ok((admission, ids, requests))
    }

    pub(in crate::kfd_backend) fn batch_custody_is_valid(
        &self,
        ids: &[u64],
        admission: Admission,
    ) -> Result<bool, DependencyScratchCapacity> {
        if self.completion_reservations != self.active.len()
            || self
                .submissions
                .capacity()
                .saturating_sub(self.submissions.len())
                < self.completion_reservations
            || (0..2).any(|direction| {
                self.active_by_direction[direction]
                    != self
                        .active
                        .values()
                        .filter(|record| record.direction == direction)
                        .count()
            })
            || self.active_stream_owners.len() != self.active.len()
            || (0..2).any(|direction| {
                // Aggregate admission checks FIFO uniqueness; ordered admission
                // checks its exact singleton before entering this shared guard.
                self.ready_by_direction[direction].len()
                    != self
                        .active
                        .values()
                        .filter(|record| record.direction == direction && record.ready_indexed)
                        .count()
                    || self.ready_by_direction[direction].iter().any(|id| {
                        self.active.get(id).is_none_or(|record| {
                            record.direction != direction || !record.ready_indexed
                        })
                    })
            })
        {
            return Ok(false);
        }
        if !valid_dependency_indexes(
            ids,
            &self.active,
            &self.submissions,
            &self.dependency_waiters,
            &self.dependency_retain_counts,
        )? {
            return Ok(false);
        }
        for (id, record) in &self.active {
            let Ok(depth) =
                next_xgmi_dependency_depth_v1(&self.dependency_depths, &record.dependencies)
            else {
                return Ok(false);
            };
            if self.active_stream_owners.get(&record.stream) != Some(id)
                || self.streams.get(&record.stream) != Some(&(1 - record.direction))
                || self.dependency_depths.get(id) != Some(&depth)
            {
                return Ok(false);
            }
        }
        for id in ids {
            let active = &self.active[id];
            if self.active_stream_owners.get(&active.stream) != Some(id)
                || self.streams.get(&active.stream) != Some(&(1 - admission.direction))
                || active.byte_len == 0
                || active.dependency_cursor > active.dependencies.len()
            {
                return Ok(false);
            }
            for (allocation, device, offset) in [
                (active.source, admission.direction, active.source_offset),
                (
                    active.destination,
                    1 - admission.direction,
                    active.destination_offset,
                ),
            ] {
                let Some(record) = self.allocations.get(&allocation) else {
                    return Ok(false);
                };
                if record.device != device
                    || offset
                        .checked_add(u64::from(active.byte_len))
                        .is_none_or(|end| end > record.byte_len)
                    || match &record.authority {
                        None => !admission.published,
                        Some(XgmiAllocationAuthorityV1::Unmapped(_)) => admission.published,
                        Some(XgmiAllocationAuthorityV1::Mapped(mapping)) => {
                            admission.published || !mapping.is_fully_mapped()
                        }
                        Some(XgmiAllocationAuthorityV1::QuarantinedMapped(_)) => true,
                    }
                {
                    return Ok(false);
                }
                let Some(owners) = self.active_allocation_owners.get(&allocation) else {
                    return Ok(false);
                };
                // Unpublished directed readers and later dependency-blocked
                // copies may retain this allocation outside the native frontier.
                if !valid_owner_roster(
                    *id,
                    allocation,
                    owners,
                    &self.active,
                    |left, right, allocation| {
                        xgmi_directed::shared_read(self, left, right, allocation)
                    },
                ) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    pub(in crate::kfd_backend) fn batch_quarantine(&mut self, direction: usize) {
        self.terminal = true;
        let (sessions, queues) = self
            .native
            .parts_mut()
            .unwrap_or_else(|_| std::process::abort());
        if let Some(queue) = queues[direction].as_mut() {
            let (source, destination) = Self::session_pair(sessions, direction);
            queue.quarantine_batch_v1(source, destination);
        }
    }

    pub(in crate::kfd_backend) fn restore_batch_pair(
        &mut self,
        id: u64,
        source: Gfx942XgmiMappedDeviceMemoryV1,
        destination: Gfx942XgmiMappedDeviceMemoryV1,
        quarantine: bool,
    ) {
        let active = self
            .active
            .get(&id)
            .unwrap_or_else(|| std::process::abort());
        let allocations = [active.source, active.destination];
        for (id, mapping) in allocations.into_iter().zip([source, destination]) {
            let record = self
                .allocations
                .get_mut(&id)
                .unwrap_or_else(|| std::process::abort());
            if record.authority.is_some() || (!quarantine && !mapping.is_fully_mapped()) {
                std::process::abort();
            }
            record.authority = Some(if quarantine {
                XgmiAllocationAuthorityV1::QuarantinedMapped(mapping)
            } else {
                XgmiAllocationAuthorityV1::Mapped(mapping)
            });
        }
    }

    pub(in crate::kfd_backend) fn restore_batch_requests(
        &mut self,
        ids: &[u64],
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
        quarantine: bool,
    ) {
        if ids.len() != requests.len() {
            std::process::abort();
        }
        for (id, request) in ids.iter().copied().zip(requests) {
            let (source, destination) = request.into_mappings();
            self.restore_batch_pair(id, source, destination, quarantine);
        }
    }

    pub(in crate::kfd_backend) fn commit_batch_status(
        &mut self,
        ids: &[u64],
        status: BackendPollV1,
    ) {
        for id in ids {
            let active = self
                .active
                .remove(id)
                .unwrap_or_else(|| std::process::abort());
            self.settle_submission(active, status);
        }
    }

    pub(in crate::kfd_backend) fn install_batch_tickets(
        &mut self,
        ids: &[u64],
        tickets: &[Gfx942SdmaCopyTicketV1],
        admission: Admission,
    ) {
        if ids.len() != tickets.len() {
            std::process::abort();
        }
        for (id, ticket) in ids.iter().copied().zip(tickets.iter().copied()) {
            let active = self
                .active
                .get_mut(&id)
                .unwrap_or_else(|| std::process::abort());
            if admission.published && active.ticket != Some(ticket) {
                std::process::abort();
            }
            active.ticket = Some(ticket);
            if !admission.published {
                insert_ordered_xgmi_id_v1(
                    &mut self.in_flight_by_direction[admission.direction],
                    id,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_admitted_peer_batch<const PROFILE: bool, const CURRENTNESS: bool>(
        &mut self,
        admission: Admission,
        ids: Vec<u64>,
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        deadline: Instant,
        timer: &mut CallTimer<PROFILE>,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let preparation_start = timer.start();
        let direction = admission.direction;
        let requests = self.prepare_batch_requests(admission, &ids, requests)?;
        timer.end(Phase::Preparation, preparation_start);
        let result = {
            let (sessions, queues) = self.native.parts_mut()?;
            let sessions = Self::session_pair(sessions, direction);
            let queue = queues[direction]
                .as_mut()
                .unwrap_or_else(|| std::process::abort());
            open_and_execute::<PROFILE, CURRENTNESS>(
                queue,
                sessions,
                admission.published,
                requests,
                tickets,
                deadline,
                timer,
            )
        };
        self.settle_batch_attempt(admission, ids, result, timer)
    }

    pub(in crate::kfd_backend) fn prepare_batch_requests(
        &mut self,
        admission: Admission,
        ids: &[u64],
        mut requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
    ) -> Result<Vec<Gfx942XgmiSdmaCopyRequestV1>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        let direction = admission.direction;
        self.ensure_queue(direction)?;
        if !admission.published {
            for id in ids {
                if self.ready_by_direction[direction].pop_front() != Some(*id) {
                    std::process::abort();
                }
                self.take_ready_membership_v1(*id);
            }
            for (index, id) in ids.iter().copied().enumerate() {
                let active = &self.active[&id];
                let (source_id, destination_id, source_offset, destination_offset, byte_len) = (
                    active.source,
                    active.destination,
                    active.source_offset,
                    active.destination_offset,
                    active.byte_len,
                );
                let source = match self.map_allocation(source_id, direction) {
                    Ok(source) => source,
                    Err(failure) => {
                        return self.fail_batch_preparation(
                            ids,
                            &ids[..index],
                            requests,
                            direction,
                            failure,
                        );
                    }
                };
                let destination = match self.map_allocation(destination_id, direction) {
                    Ok(destination) => destination,
                    Err(failure) => {
                        let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
                        let record = self
                            .allocations
                            .get_mut(&source_id)
                            .unwrap_or_else(|| std::process::abort());
                        if record.authority.is_some() {
                            std::process::abort();
                        }
                        record.authority = Some(if terminal {
                            XgmiAllocationAuthorityV1::QuarantinedMapped(source)
                        } else {
                            XgmiAllocationAuthorityV1::Mapped(source)
                        });
                        return self.fail_batch_preparation(
                            ids,
                            &ids[..index],
                            requests,
                            direction,
                            failure,
                        );
                    }
                };
                requests.push(Gfx942XgmiSdmaCopyRequestV1::new(
                    source,
                    source_offset,
                    destination,
                    destination_offset,
                    byte_len,
                ));
            }
        }
        Ok(requests)
    }

    pub(super) fn settle_batch_attempt<const PROFILE: bool>(
        &mut self,
        admission: Admission,
        ids: Vec<u64>,
        result: NativeAttempt,
        timer: &mut CallTimer<PROFILE>,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let direction = admission.direction;
        let (operation, closing) = match result {
            Ok(result) => result,
            Err((error, requests)) => {
                self.batch_quarantine(direction);
                if !admission.published {
                    self.restore_batch_requests(&ids, requests, true);
                }
                return Err(self.terminal_error(format!("XGMI aggregate opening: {error}")));
            }
        };
        let settlement_start = timer.start();
        let outcome = match operation {
            Operation::PublicationIndeterminate { error, tickets } => {
                self.install_batch_tickets(&ids, &tickets, admission);
                self.batch_quarantine(direction);
                Err(self.terminal_error(format!(
                    "XGMI aggregate publication indeterminate: {error}; closing: {closing:?}"
                )))
            }
            Operation::Retained { error, tickets } => {
                self.install_batch_tickets(&ids, &tickets, admission);
                if matches!(error, Gfx942SdmaErrorV1::Timeout) && closing.is_ok() {
                    return Ok(RuntimePeerCopyBatchPollV1::Pending);
                }
                self.batch_quarantine(direction);
                Err(self.terminal_error(format!(
                    "XGMI aggregate retained work: {error}; closing: {closing:?}"
                )))
            }
            Operation::Unpublished { error, requests } => {
                let terminal = closing.is_err();
                if terminal {
                    self.batch_quarantine(direction);
                }
                self.restore_batch_requests(&ids, requests, terminal);
                if terminal {
                    Err(self.terminal_error(format!(
                        "XGMI aggregate unpublished: {error}; closing: {closing:?}"
                    )))
                } else {
                    self.commit_batch_status(
                        &ids,
                        BackendPollV1::Failed {
                            code: COOPERATIVE_COPY_FAILURE_CODE_V1,
                        },
                    );
                    Err(Self::quiescent_error(
                        KfdRuntimeBackendErrorKindV1::Native,
                        format!("XGMI aggregate unpublished: {error}"),
                    ))
                }
            }
            Operation::Completed(completed) => {
                self.finish_batch_completions(&ids, completed, direction, closing.err())
            }
            Operation::Indeterminate { error, completed } => {
                self.finish_batch_completions(&ids, completed, direction, Some(error))
            }
        };
        timer.end(Phase::Settlement, settlement_start);
        outcome
    }

    pub(super) fn fail_batch_preparation<T>(
        &mut self,
        ids: &[u64],
        prepared: &[u64],
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
        direction: usize,
        failure: RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    ) -> Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
        if terminal {
            self.batch_quarantine(direction);
        }
        self.restore_batch_requests(prepared, requests, terminal);
        match failure {
            failure @ RuntimeBackendFailureV1::Terminal(_) => Err(failure),
            RuntimeBackendFailureV1::Rejected(error)
            | RuntimeBackendFailureV1::Quiescent(error) => {
                self.commit_batch_status(
                    ids,
                    BackendPollV1::Failed {
                        code: COOPERATIVE_COPY_FAILURE_CODE_V1,
                    },
                );
                Err(RuntimeBackendFailureV1::Quiescent(error))
            }
        }
    }

    pub(super) fn finish_batch_completions(
        &mut self,
        ids: &[u64],
        completed: Vec<Gfx942XgmiCompletedCopyV1>,
        direction: usize,
        error: Option<Gfx942SdmaErrorV1>,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if ids.len() != completed.len() {
            std::process::abort();
        }
        let terminal = error.is_some();
        if terminal {
            self.batch_quarantine(direction);
        }
        for (id, completed) in ids.iter().copied().zip(completed) {
            if self.active[&id].byte_len != completed.copy_bytes() {
                std::process::abort();
            }
            let (source, destination) = completed.into_mappings();
            self.restore_batch_pair(id, source, destination, terminal);
        }
        if let Some(error) = error {
            return Err(
                self.terminal_error(format!("XGMI aggregate completion currentness: {error}"))
            );
        }
        // All mapping authority is restored before any logical Success exists.
        self.commit_batch_status(ids, BackendPollV1::Succeeded);
        Ok(RuntimePeerCopyBatchPollV1::Succeeded)
    }
}
