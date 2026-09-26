//! Exact pending inputs for explicitly success-gated copy and typed-launch profiles.

use super::*;
use crate::context::peer_custody::ScalarPeerDependencyV1;
use fe2o3_runtime_model::{
    ContextAllocationReadV1, ContextProducerReadReferenceV1, ContextProducerReadStatusV1,
    ContextProducerReadV1, ContextReadQuiescenceEvidenceV1, ContextWriterKeyV1,
    ContextWriterKindV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::context) struct SubmissionProducerReaderMarkerV1 {
    pub(in crate::context) first: ContextProducerReadReferenceV1,
    pub(in crate::context) count: usize,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ProducerReadDomainV1 {
    DirectedPeer,
    Launch,
}

struct ProducerInputV1 {
    source: ContextReadSourceV1,
    dependency: ScalarPeerDependencyV1,
    request: ContextProducerReadV1,
}

pub(super) struct RetainedProducerReadV1 {
    domain: ProducerReadDomainV1,
    inputs: Vec<ProducerInputV1>,
    requests: Vec<ContextProducerReadV1>,
    references: Vec<ContextProducerReadReferenceV1>,
    pub(super) marker: Option<SubmissionProducerReaderMarkerV1>,
}

impl RetainedProducerReadV1 {
    pub(super) fn sources(&self) -> impl Iterator<Item = &ContextReadSourceV1> {
        self.inputs.iter().map(|input| &input.source)
    }
}

pub(super) struct PreparedProducerReadsV1 {
    root: RetainedProducerReadV1,
    output: Vec<Option<ContextProducerReadReferenceV1>>,
}

#[cfg(test)]
pub(super) enum MixedInputFaultV1 {
    RejectPending,
    FinalizationPanic(Box<dyn core::any::Any + Send>),
}

#[cfg(test)]
impl ContextVersionsV1 {
    pub(in crate::context) fn reject_mixed_input_for_test_v1(&mut self) {
        self.mixed_input_fault = Some(MixedInputFaultV1::RejectPending);
    }

    pub(in crate::context) fn panic_mixed_finalization_for_test_v1(
        &mut self,
        payload: Box<dyn core::any::Any + Send>,
    ) {
        self.mixed_input_fault = Some(MixedInputFaultV1::FinalizationPanic(payload));
    }

    pub(in crate::context) fn mixed_input_roots_for_test_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> [(usize, usize, bool); 2] {
        let reads = self
            .submission_readers
            .get(&id)
            .map_or((0, 0, false), |root| {
                (
                    root.sources.len(),
                    root.references.len(),
                    root.marker.is_some(),
                )
            });
        let producers = self
            .producer_readers
            .get(&id)
            .map_or((0, 0, false), |root| {
                (
                    root.inputs.len(),
                    root.references.len(),
                    root.marker.is_some(),
                )
            });
        [reads, producers]
    }

    pub(super) fn assert_mixed_markers_for_test_v1(&self, id: RuntimeSubmissionIdV1) {
        for (requests, references, marked) in self.mixed_input_roots_for_test_v1(id) {
            assert_eq!(
                requests, references,
                "complete input references before backend entry"
            );
            assert_eq!(
                marked,
                requests != 0,
                "complete input markers before backend entry"
            );
        }
    }

    pub(in crate::context) fn remove_producer_read_root_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) {
        self.producer_readers.remove(&id);
    }

    pub(in crate::context) fn corrupt_producer_read_reference_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        index: usize,
    ) {
        self.producer_readers.get_mut(&id).unwrap().references[index].incarnation += 1;
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    fn prepare_pending_input_v1(
        &mut self,
        source: ContextReadSourceV1,
        dependencies: &[ScalarPeerDependencyV1],
        launch: Option<&ProducerLaunchRootV1>,
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
        let Some(writer) = state.pending_writer else {
            return Ok(None);
        };
        let result = self
            .versions
            .as_ref()
            .expect("configured journal")
            .retained_writer(writer);
        if !matches!(
            self.journal_result_v1(result)?,
            fe2o3_runtime_model::ContextWriterStateV1::Pending { .. }
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
        if let Some(consumer) = launch {
            let producer = self
                .producer_launches
                .get(&dependency.submission)
                .ok_or(RuntimeValidationErrorV1::ContextReserved)?;
            // Leases cover the allocation; native coverage must cover every original Read alias.
            let mut found = false;
            for binding in consumer
                .bindings
                .iter()
                .filter(|binding| binding.region.allocation == source.region.allocation)
            {
                if binding.record != source.record
                    || binding.region.access != RuntimeAccessV1::Read
                    || !producer.covers_input_v1(*binding)
                {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                found = true;
            }
            if !found {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
            }
            let result = self.validate_pending_producer_launch_roots_v1(dependency.submission);
            self.journal_result_v1(result)?;
        } else {
            let producer = self
                .scalar_peer_copies
                .get(&dependency.submission)
                .filter(|producer| producer.directed.is_some())
                .ok_or(RuntimeValidationErrorV1::ContextReserved)?;
            let covered = producer.destination;
            if covered.region.allocation != source.region.allocation
                || covered.record != source.record
                || source.region.byte_offset < covered.region.byte_offset
                || source
                    .region
                    .byte_offset
                    .checked_add(source.region.byte_len)
                    .zip(
                        covered
                            .region
                            .byte_offset
                            .checked_add(covered.region.byte_len),
                    )
                    .is_none_or(|(end, producer_end)| end > producer_end)
            {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            let result = self.validate_pending_peer_copy_roots_v1(dependency.submission);
            self.journal_result_v1(result)?;
        }
        if self.submissions[&dependency.submission].journal_writer != Some(writer) {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        let request = ContextProducerReadV1 {
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
        };
        let result = self
            .versions
            .as_ref()
            .expect("configured journal")
            .journal
            .validate_producer_read(&request);
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

    fn prepare_producer_batch_v1(
        &mut self,
        inputs: Vec<ProducerInputV1>,
        domain: ProducerReadDomainV1,
    ) -> Result<Option<PreparedProducerReadsV1>, RuntimeValidationErrorV1> {
        if inputs.is_empty() {
            return Ok(None);
        }
        let result = self
            .versions
            .as_ref()
            .expect("configured journal")
            .journal
            .validate_producer_read_capacity(inputs.len());
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
        requests
            .try_reserve_exact(inputs.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        references
            .try_reserve_exact(inputs.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        output
            .try_reserve_exact(inputs.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        requests.extend(inputs.iter().map(|input| input.request));
        output.resize(inputs.len(), None);
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
                marker: None,
            },
            output,
        }))
    }

    pub(super) fn prepare_producer_read_v1(
        &mut self,
        peer: Option<&PreparedPeerSubmissionV1>,
    ) -> Result<Option<PreparedProducerReadsV1>, RuntimeValidationErrorV1> {
        self.guard_journal_unwind_v1(|context| {
            let Some(root) = peer
                .and_then(|peer| peer.scalar.as_ref())
                .filter(|root| root.directed.is_some())
            else {
                return Ok(None);
            };
            let mut inputs = Vec::new();
            inputs
                .try_reserve_exact(1)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            if let Some(input) =
                context.prepare_pending_input_v1(root.source, &root.dependencies, None)?
            {
                inputs.push(input);
            }
            context.prepare_producer_batch_v1(inputs, ProducerReadDomainV1::DirectedPeer)
        })
    }

    pub(super) fn prepare_launch_inputs_v1(
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
                match context.prepare_pending_input_v1(*source, &root.dependencies, Some(root))? {
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

    pub(super) fn begin_launch_inputs_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        reads: Option<PreparedSubmissionReadersV1>,
        producers: Option<PreparedProducerReadsV1>,
    ) -> Result<
        (
            Option<SubmissionReaderMarkerV1>,
            Option<SubmissionProducerReaderMarkerV1>,
        ),
        RuntimeValidationErrorV1,
    > {
        self.guard_journal_unwind_v1(|context| {
            let result = (|| {
                let launch = context.producer_launches.contains_key(&id);
                let versions = context
                    .versions
                    .as_mut()
                    .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
                if !launch
                    || versions.submission_readers.contains_key(&id)
                    || versions.producer_readers.contains_key(&id)
                    || producers.as_ref().is_some_and(|prepared| {
                        prepared.root.domain != ProducerReadDomainV1::Launch
                    })
                    || versions
                        .submission_writers
                        .get(&id)
                        .is_some_and(|root| root.domain != SubmissionWriterDomainV1::Ordinary)
                {
                    return Err(ContextVersionJournalErrorV1::InvalidState);
                }
                assert!(
                    reads.is_none()
                        || versions.submission_readers.len()
                            < versions.submission_readers.capacity(),
                    "preallocated reader root"
                );
                assert!(
                    producers.is_none()
                        || versions.producer_readers.len() < versions.producer_readers.capacity(),
                    "preallocated producer root"
                );
                // Retain both original rosters before the atomic journal operation.
                let mut read_output = if let Some(mut prepared) = reads {
                    prepared.root.domain = SubmissionWriterDomainV1::Ordinary;
                    versions.submission_readers.insert(id, prepared.root);
                    prepared.output
                } else {
                    Vec::new()
                };
                let mut producer_output = if let Some(prepared) = producers {
                    versions.producer_readers.insert(id, prepared.root);
                    prepared.output
                } else {
                    Vec::new()
                };
                let mut read_root = versions.submission_readers.get_mut(&id);
                let mut producer_root = versions.producer_readers.get_mut(&id);
                #[cfg(test)]
                let fault = versions.mixed_input_fault.take();
                #[cfg(test)]
                if matches!(fault.as_ref(), Some(MixedInputFaultV1::RejectPending)) {
                    producer_root
                        .as_mut()
                        .expect("fault fixture producer")
                        .requests
                        .last_mut()
                        .expect("fault fixture pending input")
                        .read
                        .byte_len = 0;
                }
                let consumer = ContextWriterKeyV1 {
                    context_generation: id.context_generation,
                    local: id.local,
                    kind: ContextWriterKindV1::Submission,
                };
                versions.journal.acquire_mixed_reads(
                    consumer,
                    read_root
                        .as_ref()
                        .map_or(&[], |root| root.requests.as_slice()),
                    &mut read_output,
                    producer_root
                        .as_ref()
                        .map_or(&[], |root| root.requests.as_slice()),
                    &mut producer_output,
                )?;
                #[cfg(test)]
                if let Some(MixedInputFaultV1::FinalizationPanic(payload)) = fault {
                    std::panic::resume_unwind(payload);
                }
                if let Some(root) = read_root.as_mut() {
                    assert!(
                        root.references.capacity() >= read_output.len(),
                        "preallocated references"
                    );
                    for reference in read_output {
                        root.references
                            .push(reference.expect("complete read roster"));
                    }
                }
                if let Some(root) = producer_root.as_mut() {
                    assert!(
                        root.references.capacity() >= producer_output.len(),
                        "preallocated producer references"
                    );
                    for reference in producer_output {
                        root.references
                            .push(reference.expect("complete producer reservations"));
                    }
                }
                let read_marker = read_root.as_ref().map(|root| SubmissionReaderMarkerV1 {
                    first: root.references[0],
                    count: root.references.len(),
                });
                let producer_marker =
                    producer_root
                        .as_ref()
                        .map(|root| SubmissionProducerReaderMarkerV1 {
                            first: root.references[0],
                            count: root.references.len(),
                        });
                // Publish neither marker until both complete reference arrays are retained.
                if let Some(root) = read_root {
                    root.marker = read_marker;
                }
                if let Some(root) = producer_root {
                    root.marker = producer_marker;
                }
                Ok((read_marker, producer_marker))
            })();
            context.journal_result_v1(result)
        })
    }

    pub(super) fn begin_producer_read_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        prepared: Option<PreparedProducerReadsV1>,
    ) -> Result<Option<SubmissionProducerReaderMarkerV1>, RuntimeValidationErrorV1> {
        let Some(mut prepared) = prepared else {
            return Ok(None);
        };
        self.guard_journal_unwind_v1(|context| {
            let result = (|| {
                let launch = context.producer_launches.contains_key(&id);
                let versions = context
                    .versions
                    .as_mut()
                    .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
                if versions.producer_readers.contains_key(&id)
                    || launch != (prepared.root.domain == ProducerReadDomainV1::Launch)
                    || !launch && versions.submission_readers.contains_key(&id)
                    || versions
                        .submission_writers
                        .get(&id)
                        .is_some_and(|root| root.domain != SubmissionWriterDomainV1::Ordinary)
                    || !launch && !versions.submission_writers.contains_key(&id)
                {
                    return Err(ContextVersionJournalErrorV1::InvalidReference);
                }
                assert!(
                    versions.producer_readers.len() < versions.producer_readers.capacity(),
                    "preallocated producer root"
                );
                versions.producer_readers.insert(id, prepared.root);
                let root = versions
                    .producer_readers
                    .get_mut(&id)
                    .expect("retained producer inputs");
                let consumer = ContextWriterKeyV1 {
                    context_generation: id.context_generation,
                    local: id.local,
                    kind: ContextWriterKindV1::Submission,
                };
                versions.journal.acquire_producer_reads(
                    consumer,
                    &root.requests,
                    &mut prepared.output,
                )?;
                assert!(
                    root.references.capacity() >= prepared.output.len(),
                    "preallocated producer references"
                );
                for reference in prepared.output {
                    root.references
                        .push(reference.expect("complete producer reservations"));
                }
                let marker = SubmissionProducerReaderMarkerV1 {
                    first: root.references[0],
                    count: root.references.len(),
                };
                root.marker = Some(marker);
                Ok(Some(marker))
            })();
            context.journal_result_v1(result)
        })
    }

    pub(super) fn validate_producer_read_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<Option<ContextProducerReadStatusV1>, ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        let record = self.submissions.get(&id);
        let expected = record.and_then(|record| record.journal_producer_read);
        let absent = if expected.is_some() {
            Err(E::InvalidReference)
        } else {
            Ok(None)
        };
        let Some(versions) = &self.versions else {
            return absent;
        };
        let Some(root) = versions.producer_readers.get(&id) else {
            return absent;
        };
        let marker = root.marker.ok_or(E::InvalidReference)?;
        let launch = root.domain == ProducerReadDomainV1::Launch;
        let consumer = ContextWriterKeyV1 {
            context_generation: id.context_generation,
            local: id.local,
            kind: ContextWriterKindV1::Submission,
        };
        if marker.count == 0
            || marker.count != root.inputs.len()
            || marker.count != root.references.len()
            || marker.count != root.requests.len()
            || marker.first != root.references[0]
            || !launch && versions.submission_readers.contains_key(&id)
            || record.is_some_and(|record| {
                record.producer_launch != launch
                    || !launch && (!record.directed_peer_copy || record.journal_read.is_some())
                    || expected != Some(marker)
                    || record.journal_writer
                        != versions.submission_writers.get(&id).map(|root| root.writer)
            })
            || versions
                .submission_writers
                .get(&id)
                .is_some_and(|root| root.domain != SubmissionWriterDomainV1::Ordinary)
            || !launch && !versions.submission_writers.contains_key(&id)
        {
            return Err(E::InvalidReference);
        }
        let mut aggregate = ContextProducerReadStatusV1::Success;
        for (index, ((input, request), reference)) in root
            .inputs
            .iter()
            .zip(&root.requests)
            .zip(&root.references)
            .enumerate()
        {
            let source = input.source;
            let read = request.read;
            let bound = if launch {
                self.producer_launches.get(&id).is_some_and(|launch| {
                    launch.dependencies_held
                        && launch.dependencies.contains(&input.dependency)
                        && launch.sources.iter().any(|original| {
                            original.region == source.region && original.record == source.record
                        })
                })
            } else {
                self.scalar_peer_copies.get(&id).is_some_and(|peer| {
                    peer.directed.is_some()
                        && peer.dependencies_held
                        && peer.dependencies.contains(&input.dependency)
                        && peer.source.region == source.region
                        && peer.source.record == source.record
                })
            };
            if !bound
                || *request != input.request
                || reference.consumer != consumer
                || marker.first.incarnation.checked_add(index as u64) != Some(reference.incarnation)
                || request.producer.key
                    != (ContextWriterKeyV1 {
                        context_generation: input.dependency.submission.context_generation,
                        local: input.dependency.submission.local,
                        kind: ContextWriterKindV1::Submission,
                    })
                || input.dependency.submission.local >= id.local
                || index > 0
                    && root.inputs[index - 1].source.region.allocation >= source.region.allocation
                || self.allocations.get(&source.region.allocation) != Some(&source.record)
                || !self
                    .backend_allocations
                    .contains(&source.record.backend_allocation)
                || !self.allocation_admission.has_expected_credit(
                    source.region.allocation,
                    source.record.device,
                    source.record.byte_len,
                )
                || versions.validate_live(source.region.allocation, &source.record)?
                    != read.allocation
                || read.device
                    != enrollment(
                        source.region.allocation,
                        source.record.device,
                        source.record.byte_len,
                    )
                    .device
                || read.byte_extent != source.record.byte_len
                || read.byte_offset != source.region.byte_offset
                || read.byte_len != source.region.byte_len
                || versions.journal.lookup_producer_read(*reference)? != *request
            {
                return Err(E::InvalidReference);
            }
            // Resolved reservations outlive their writer slot; never revalidate admission.
            let status = versions.journal.producer_read_status(*reference)?;
            aggregate = match (aggregate, status) {
                (ContextProducerReadStatusV1::Unknown, _)
                | (_, ContextProducerReadStatusV1::Unknown) => ContextProducerReadStatusV1::Unknown,
                (ContextProducerReadStatusV1::NoEffect, _)
                | (_, ContextProducerReadStatusV1::NoEffect) => {
                    ContextProducerReadStatusV1::NoEffect
                }
                (ContextProducerReadStatusV1::Pending, _)
                | (_, ContextProducerReadStatusV1::Pending) => ContextProducerReadStatusV1::Pending,
                _ => ContextProducerReadStatusV1::Success,
            };
        }
        Ok(Some(aggregate))
    }

    pub(in crate::context) fn directed_input_status_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<Option<ContextProducerReadStatusV1>, RuntimeValidationErrorV1> {
        let result = self.validate_producer_read_v1(id);
        self.journal_result_v1(result)
    }

    #[allow(clippy::question_mark)] // Explicit early exits are shared with Verus.
    pub(in crate::context) fn release_submission_inputs_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_ordinary_submission_v1(id)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_submission_readers_v1(id, SubmissionWriterDomainV1::Ordinary)?;
            self.validate_producer_read_v1(id)
        }));
        let producer = match result {
            Ok(result) => self.journal_result_v1(result)?.is_some(),
            Err(payload) => {
                self.quarantine_after_async_command_panic_v1();
                core::mem::forget(payload);
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
            }
        };
        // Validate both complete rosters before releasing either class of input.
        completion_input_release_body!(completion_journal_rust_syntax, self, id, producer, [], [])
    }

    #[allow(clippy::question_mark)] // The effect/retirement body is shared with Verus.
    fn release_validated_submission_producer_readers_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let versions = self.versions.as_mut().expect("validated producer inputs");
            #[cfg(test)]
            versions.completion_boundary_for_test_v1(
                id,
                completion_faults::CompletionJournalStageV1::Producer,
                completion_faults::CompletionJournalPointV1::BeforeEffect,
            )?;
            completion_selected_reader_release_body!(completion_journal_rust_syntax,
            versions.producer_readers, self.submissions, versions.journal,
            id, journal_producer_read, release_producer_reads, [
                #[cfg(test)]
                versions.completion_boundary_for_test_v1(
                    id,
                    completion_faults::CompletionJournalStageV1::Producer,
                    completion_faults::CompletionJournalPointV1::AfterEffect,
                )?;
            ])
        }));
        match result {
            Ok(result) => self.journal_result_v1(result),
            Err(payload) => {
                self.quarantine_after_async_command_panic_v1();
                core::mem::forget(payload);
                Err(RuntimeValidationErrorV1::InvalidBackendDescription)
            }
        }
    }
}
