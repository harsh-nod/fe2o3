use super::*;

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(super) fn prepare_pending_input_v1(
        &mut self,
        source: ContextReadSourceV1,
        dependencies: &[ScalarPeerDependencyV1],
        launch: Option<&ProducerLaunchRootV1>,
        copy: Option<&SameDeviceCopyRootV1>,
        compute_peer: Option<&ScalarPeerCopyRootV1>,
        segmented_peer: Option<&SegmentedPeerCopyRootV1>,
    ) -> Result<Option<ProducerInputV1>, RuntimeValidationErrorV1> {
        if self.allocations.get(&source.region.allocation) != Some(&source.record)
            || !self
                .backend_allocations
                .contains(&source.record.backend_allocation)
            || !self.allocation_admission.has_expected_credit(
                source.region.allocation,
                source.record.device,
                source.record.byte_len,
            )
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        let Some(versions) = &self.versions else {
            return Ok(None);
        };
        let result = versions
            .validate_live(source.region.allocation, &source.record)
            .and_then(|allocation| {
                versions
                    .journal
                    .lookup_allocation(allocation)
                    .map(|state| (allocation, state))
            });
        let (allocation, state) = self.journal_result_v1(result)?;
        let result = self
            .versions
            .as_ref()
            .expect("configured journal")
            .journal
            .latest_writer(allocation);
        let Some(writer) = self.journal_result_v1(result)? else {
            return Ok(None);
        };
        let result = self
            .versions
            .as_ref()
            .expect("configured journal")
            .journal
            .queued_writer_status(writer);
        let queued = match self.journal_result_v1(result)? {
            None => false,
            Some(ContextQueuedWriterStatusV1::Waiting | ContextQueuedWriterStatusV1::Ready) => true,
            Some(_) => return Err(RuntimeValidationErrorV1::ContextReserved),
        };
        let versions = self.versions.as_ref().expect("configured journal");
        let result = if queued {
            versions.journal.lookup_writer(writer)
        } else {
            versions.retained_writer(writer)
        };
        let retained = self.journal_result_v1(result)?;
        if !matches!(
            (queued, retained),
            (
                false,
                fe2o3_runtime_model::ContextWriterStateV1::Pending { .. }
            ) | (true, fe2o3_runtime_model::ContextWriterStateV1::Reserved)
        ) {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        let dependency = dependencies
            .iter()
            .find(|dependency| {
                writer.key.kind == ContextWriterKindV1::Submission
                    && writer.key.context_generation == dependency.submission.context_generation
                    && writer.key.local == dependency.submission.local
            })
            .copied()
            .ok_or(RuntimeValidationErrorV1::ContextReserved)?;
        // The frame-forwarding contracts cover scalar/list copies and their
        // readbacks, not a new pending compute-consumer profile.
        if launch.is_some()
            && (self
                .scalar_peer_copies
                .get(&dependency.submission)
                .and_then(|peer| peer.compute.as_ref())
                .is_some_and(|input| input.is_segmented_frame_v1())
                || self
                    .segmented_peer_copies
                    .get(&dependency.submission)
                    .is_some_and(|peer| peer.is_frame_source_v1()))
        {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        let queued_launch = (launch.is_some() || segmented_peer.is_some())
            && self.producer_launches.contains_key(&dependency.submission);
        let queued_frame_input = (copy.is_some() || launch.is_some())
            && (self
                .scalar_peer_copies
                .get(&dependency.submission)
                .is_some_and(|producer| producer.preserves_destination_frame_v1(source))
                || self
                    .segmented_peer_copies
                    .get(&dependency.submission)
                    .is_some_and(|producer| producer.preserves_destination_frame_v1(source)))
            || compute_peer
                .and_then(|peer| peer.compute.as_ref())
                .is_some_and(|input| {
                    self.segmented_peer_copies
                        .get(&dependency.submission)
                        .is_some_and(|producer| input.matches_segmented_frame_v1(producer, source))
                })
            || segmented_peer.is_some_and(|peer| {
                self.segmented_peer_copies
                    .get(&dependency.submission)
                    .is_some_and(|producer| peer.matches_source_frame_v1(producer, source))
            });
        if queued && !queued_launch && !queued_frame_input {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        if let Some(consumer) = launch {
            // Each original Read alias needs copied coverage or an authenticated preserved frame.
            let mut found = false;
            for binding in consumer
                .bindings
                .iter()
                .filter(|binding| binding.region.allocation == source.region.allocation)
            {
                if binding.record != source.record
                    || binding.region.access != RuntimeAccessV1::Read
                    || !self
                        .producer_launches
                        .get(&dependency.submission)
                        .map(|producer| producer.covers_input_v1(*binding))
                        .or_else(|| {
                            self.scalar_peer_copies
                                .get(&dependency.submission)
                                .map(|producer| {
                                    producer.covers_input_v1(*binding)
                                        || producer.preserves_destination_frame_v1(*binding)
                                })
                        })
                        .or_else(|| {
                            self.segmented_peer_copies
                                .get(&dependency.submission)
                                .map(|producer| producer.preserves_destination_frame_v1(*binding))
                        })
                        .unwrap_or(false)
                {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                found = true;
            }
            if !found {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
            }
            let result = if self.producer_launches.contains_key(&dependency.submission) {
                self.validate_pending_producer_launch_roots_v1(dependency.submission)
            } else {
                self.validate_pending_peer_copy_roots_v1(dependency.submission)
            };
            self.journal_result_v1(result)?;
        } else if let Some(peer) = segmented_peer {
            if Some(dependency) != peer.source_dependency_v1()
                || source.region != peer.source.region
                || source.record != peer.source.record
            {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            let result = if peer.is_frame_source_v1() {
                if self
                    .segmented_peer_copies
                    .get(&dependency.submission)
                    .is_none_or(|producer| !peer.matches_source_frame_v1(producer, source))
                {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                self.validate_pending_segmented_peer_roots_v1(dependency.submission)
            } else {
                if self
                    .producer_launches
                    .get(&dependency.submission)
                    .is_none_or(|producer| !producer.covers_input_v1(source))
                {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                self.validate_pending_producer_launch_roots_v1(dependency.submission)
            };
            self.journal_result_v1(result)?;
        } else if let Some(peer) = compute_peer {
            let compute = peer
                .compute
                .as_ref()
                .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
            if dependency != compute.producer
                || source.region != peer.source.region
                || source.record != peer.source.record
            {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            let result = if compute.is_segmented_frame_v1() {
                if self
                    .segmented_peer_copies
                    .get(&dependency.submission)
                    .is_none_or(|producer| !compute.matches_segmented_frame_v1(producer, source))
                {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                self.validate_pending_segmented_peer_roots_v1(dependency.submission)
            } else {
                if self
                    .producer_launches
                    .get(&dependency.submission)
                    .is_none_or(|producer| !producer.covers_input_v1(source))
                {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                self.validate_pending_producer_launch_roots_v1(dependency.submission)
            };
            self.journal_result_v1(result)?;
        } else if let Some(copy) = copy {
            let covered = self
                .scalar_peer_copies
                .get(&dependency.submission)
                .is_some_and(|producer| {
                    producer.covers_input_v1(source)
                        || producer.preserves_destination_frame_v1(source)
                })
                || self
                    .segmented_peer_copies
                    .get(&dependency.submission)
                    .is_some_and(|producer| producer.preserves_destination_frame_v1(source));
            if dependency != copy.producer
                || source.region != copy.source.region
                || source.record != copy.source.record
                || !covered
            {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            let result = self.validate_pending_peer_copy_roots_v1(dependency.submission);
            self.journal_result_v1(result)?;
        } else {
            let producer = self
                .scalar_peer_copies
                .get(&dependency.submission)
                .filter(|producer| producer.directed.is_some())
                .ok_or(RuntimeValidationErrorV1::ContextReserved)?;
            if !producer.covers_input_v1(source) {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            let result = self.validate_pending_peer_copy_roots_v1(dependency.submission);
            self.journal_result_v1(result)?;
        }
        if self.submissions[&dependency.submission].journal_writer != Some(writer) {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        let request = if queued {
            ProducerReadRequestV1::Queued(ContextQueuedProducerReadV1 {
                allocation: fe2o3_runtime_model::ContextAllocationWriteV1 {
                    allocation,
                    device: state.device,
                    byte_extent: state.byte_extent,
                },
                byte_offset: source.region.byte_offset,
                byte_len: source.region.byte_len,
                producer: writer,
            })
        } else {
            ProducerReadRequestV1::Active(ContextProducerReadV1 {
                read: ContextAllocationReadV1 {
                    allocation,
                    device: state.device,
                    byte_extent: state.byte_extent,
                    byte_offset: source.region.byte_offset,
                    byte_len: source.region.byte_len,
                    attempt_epoch: state.attempt_epoch,
                    content_lineage: state.content_lineage,
                },
                producer: writer,
            })
        };
        let journal = &self.versions.as_ref().expect("configured journal").journal;
        let result = match request {
            ProducerReadRequestV1::Active(request) => journal.validate_producer_read(&request),
            ProducerReadRequestV1::Queued(request) => {
                journal.validate_queued_producer_read(&request)
            }
        };
        match result {
            Err(ContextVersionJournalErrorV1::AllocationBusy) => {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            result => self.journal_result_v1(result)?,
        }
        Ok(Some(ProducerInputV1 {
            source,
            dependency,
            request,
        }))
    }

    pub(super) fn prepare_producer_batch_v1(
        &mut self,
        inputs: Vec<ProducerInputV1>,
        domain: ProducerReadDomainV1,
    ) -> Result<Option<PreparedProducerReadsV1>, RuntimeValidationErrorV1> {
        if inputs.is_empty() {
            return Ok(None);
        }
        let journal = &self.versions.as_ref().expect("configured journal").journal;
        let active_count = inputs
            .iter()
            .filter(|input| matches!(input.request, ProducerReadRequestV1::Active(_)))
            .count();
        let queued_count = inputs.len() - active_count;
        let result = if journal.remaining_read_slots() < inputs.len() {
            Err(ContextVersionJournalErrorV1::MemberCapacity)
        } else {
            journal
                .validate_producer_read_capacity(active_count)
                .and_then(|()| journal.validate_queued_producer_read_capacity(queued_count))
        };
        match result {
            Err(
                ContextVersionJournalErrorV1::MemberCapacity
                | ContextVersionJournalErrorV1::EpochExhausted,
            ) => return Err(RuntimeValidationErrorV1::Capacity),
            result => self.journal_result_v1(result)?,
        }
        let mut requests = Vec::new();
        let mut references = Vec::new();
        let mut output = Vec::new();
        let mut queued_requests = Vec::new();
        let mut queued_references = Vec::new();
        let mut queued_output = Vec::new();
        requests
            .try_reserve_exact(active_count)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        references
            .try_reserve_exact(active_count)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        output
            .try_reserve_exact(active_count)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        queued_requests
            .try_reserve_exact(queued_count)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        queued_references
            .try_reserve_exact(queued_count)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        queued_output
            .try_reserve_exact(queued_count)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        for input in &inputs {
            match input.request {
                ProducerReadRequestV1::Active(request) => requests.push(request),
                ProducerReadRequestV1::Queued(request) => queued_requests.push(request),
            }
        }
        output.resize(active_count, None);
        queued_output.resize(queued_count, None);
        self.versions
            .as_mut()
            .expect("configured journal")
            .producer_readers
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        Ok(Some(PreparedProducerReadsV1 {
            root: RetainedProducerReadV1 {
                domain,
                inputs,
                requests,
                references,
                queued_requests,
                queued_references,
                marker: None,
            },
            output,
            queued_output,
        }))
    }

    pub(in crate::context::versions) fn prepare_producer_read_v1(
        &mut self,
        peer: Option<&PreparedPeerSubmissionV1>,
    ) -> Result<Option<PreparedProducerReadsV1>, RuntimeValidationErrorV1> {
        self.guard_journal_unwind_v1(|context| {
            let Some(root) = peer
                .and_then(|peer| peer.scalar.as_ref())
                .filter(|root| root.directed.is_some() || root.compute.is_some())
            else {
                return Ok(None);
            };
            let mut inputs = Vec::new();
            inputs
                .try_reserve_exact(1)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            let compute = root.compute.is_some();
            if let Some(input) = context.prepare_pending_input_v1(
                root.source,
                &root.dependencies,
                None,
                None,
                compute.then_some(root),
                None,
            )? {
                inputs.push(input);
            } else if compute {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            context.prepare_producer_batch_v1(
                inputs,
                if compute {
                    ProducerReadDomainV1::ComputePeer
                } else {
                    ProducerReadDomainV1::DirectedPeer
                },
            )
        })
    }

    pub(in crate::context::versions) fn prepare_launch_inputs_v1(
        &mut self,
        root: &ProducerLaunchRootV1,
        sources: &[ContextReadSourceV1],
    ) -> Result<
        (
            Option<PreparedSubmissionReadersV1>,
            Option<PreparedProducerReadsV1>,
        ),
        RuntimeValidationErrorV1,
    > {
        self.guard_journal_unwind_v1(|context| {
            let versions = context
                .versions
                .as_ref()
                .ok_or(RuntimeValidationErrorV1::Unsupported)?;
            if versions.journal.remaining_read_slots() < sources.len() {
                return Err(RuntimeValidationErrorV1::Capacity);
            }
            let mut stable = Vec::new();
            let mut pending = Vec::new();
            stable
                .try_reserve_exact(sources.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            pending
                .try_reserve_exact(sources.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            let mut previous = None;
            for source in sources {
                if previous.is_some_and(|previous| previous >= source.region.allocation) {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
                }
                previous = Some(source.region.allocation);
                match context.prepare_pending_input_v1(
                    *source,
                    &root.dependencies,
                    Some(root),
                    None,
                    None,
                    None,
                )? {
                    Some(input) => pending.push(input),
                    None => stable.push(*source),
                }
            }
            let reads = context.prepare_submission_readers_v1(&stable)?;
            let producers =
                context.prepare_producer_batch_v1(pending, ProducerReadDomainV1::Launch)?;
            Ok((reads, producers))
        })
    }
}
