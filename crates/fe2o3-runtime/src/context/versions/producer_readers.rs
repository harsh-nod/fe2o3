//! Exact pending inputs for explicitly success-gated copy and typed-launch profiles.

use super::*;
use crate::context::peer_custody::ScalarPeerDependencyV1;
use fe2o3_runtime_model::{
    ContextAllocationReadV1, ContextProducerReadReferenceV1, ContextProducerReadStatusV1,
    ContextProducerReadV1, ContextQueuedProducerReadReferenceV1, ContextQueuedProducerReadV1,
    ContextQueuedWriterStatusV1, ContextReadQuiescenceEvidenceV1, ContextWriterKeyV1,
    ContextWriterKindV1,
};

include!("producer_input_preflight_body.rs");
include!("producer_input_fold_body.rs");
include!("producer_journal_observer_bodies.rs");

#[cfg(test)]
#[path = "producer_input_preflight_tests.rs"]
mod preflight_tests;

#[cfg(test)]
#[path = "producer_input_fold_tests.rs"]
mod fold_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::context) struct SubmissionProducerReaderMarkerV1 {
    first: FirstProducerReadV1,
    pub(in crate::context) count: usize,
    active: Option<(ContextProducerReadReferenceV1, usize)>,
    queued: Option<(ContextQueuedProducerReadReferenceV1, usize)>,
}

#[cfg(test)]
impl SubmissionProducerReaderMarkerV1 {
    pub(in crate::context) fn active_first_for_test(
        self,
    ) -> Option<ContextProducerReadReferenceV1> {
        self.active.map(|(first, _)| first)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ProducerReadDomainV1 {
    DirectedPeer,
    Launch,
    Copy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FirstProducerReadV1 {
    consumer: ContextWriterKeyV1,
    reference: ProducerReadReferenceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProducerReadReferenceV1 {
    Active(ContextProducerReadReferenceV1),
    Queued(ContextQueuedProducerReadReferenceV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProducerReadRequestV1 {
    Active(ContextProducerReadV1),
    Queued(ContextQueuedProducerReadV1),
}

struct ProducerInputV1 {
    source: ContextReadSourceV1,
    dependency: ScalarPeerDependencyV1,
    request: ProducerReadRequestV1,
}

pub(super) struct RetainedProducerReadV1 {
    domain: ProducerReadDomainV1,
    inputs: Vec<ProducerInputV1>,
    requests: Vec<ContextProducerReadV1>,
    references: Vec<ContextProducerReadReferenceV1>,
    queued_requests: Vec<ContextQueuedProducerReadV1>,
    queued_references: Vec<ContextQueuedProducerReadReferenceV1>,
    pub(super) marker: Option<SubmissionProducerReaderMarkerV1>,
}

impl RetainedProducerReadV1 {
    fn first_reference(&self) -> Option<FirstProducerReadV1> {
        producer_input_first_reference_body!(completion_journal_rust_syntax, self)
    }

    fn complete_marker(&self) -> SubmissionProducerReaderMarkerV1 {
        SubmissionProducerReaderMarkerV1 {
            first: self
                .first_reference()
                .expect("complete typed producer references"),
            count: self.inputs.len(),
            active: self
                .references
                .first()
                .copied()
                .map(|first| (first, self.references.len())),
            queued: self
                .queued_references
                .first()
                .copied()
                .map(|first| (first, self.queued_references.len())),
        }
    }
    pub(super) fn sources(&self) -> impl Iterator<Item = &ContextReadSourceV1> {
        self.inputs.iter().map(|input| &input.source)
    }
}

struct ProducerInputRootV1<'a> {
    versions: &'a ContextVersionsV1,
    root: &'a RetainedProducerReadV1,
    consumer: ContextWriterKeyV1,
    launch: bool,
}

// This adapter borrows retained Context storage. Each reached credit predicate
// calls the existing account implementation afresh; no account snapshot is kept.
struct ProducerInputObservationsV1<'a, B: RuntimeBackendV1> {
    context: &'a RuntimeContextV1<B>,
    versions: &'a ContextVersionsV1,
    root: &'a RetainedProducerReadV1,
    id: RuntimeSubmissionIdV1,
    consumer: ContextWriterKeyV1,
    launch: bool,
}

fn producer_dependency_contains_v1(
    dependencies: &[ScalarPeerDependencyV1],
    dependency: &ScalarPeerDependencyV1,
) -> bool {
    producer_dependency_contains_body!(
        completion_journal_rust_syntax,
        dependencies,
        dependency,
        index,
        []
    )
}

fn producer_source_pair_contains_v1(
    sources: &[ContextReadSourceV1],
    source: &ContextReadSourceV1,
) -> bool {
    producer_source_pair_contains_body!(completion_journal_rust_syntax, sources, source, index, [])
}

impl<B: RuntimeBackendV1> ProducerInputObservationsV1<'_, B> {
    fn input_count(&self) -> usize {
        self.root.inputs.len()
    }

    fn active_count(&self) -> usize {
        self.root.references.len()
    }

    fn queued_count(&self) -> usize {
        self.root.queued_references.len()
    }

    fn observe_active_lookup(
        &mut self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<ContextProducerReadV1, ContextVersionJournalErrorV1> {
        producer_observe_active_lookup_body_v1!(self, reference)
    }

    fn observe_active_status(
        &mut self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1> {
        producer_observe_active_status_body_v1!(self, reference)
    }

    fn observe_queued_lookup(
        &mut self,
        reference: ContextQueuedProducerReadReferenceV1,
    ) -> Result<ContextQueuedProducerReadV1, ContextVersionJournalErrorV1> {
        producer_observe_queued_lookup_body_v1!(self, reference)
    }

    fn observe_queued_status(
        &mut self,
        reference: ContextQueuedProducerReadReferenceV1,
    ) -> Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1> {
        producer_observe_queued_status_body_v1!(self, reference)
    }

    fn observe_live(
        &mut self,
        id: RuntimeAllocationIdV1,
        record: &AllocationRecordV1,
    ) -> Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1> {
        self.versions.validate_live(id, record)
    }

    fn observe_expected_credit(
        &mut self,
        id: RuntimeAllocationIdV1,
        device: RuntimeDeviceIdV1,
        byte_len: u64,
    ) -> bool {
        self.context
            .allocation_admission
            .has_expected_credit(id, device, byte_len)
    }

    fn validate(
        &mut self,
        index: usize,
        active_index: &mut usize,
        queued_index: &mut usize,
    ) -> Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1> {
        if self.root.domain == ProducerReadDomainV1::Copy {
            return self.validate_copy_input_v1(index, active_index, queued_index);
        }
        let context = self.context;
        let root = self.root;
        let id = self.id;
        let consumer = self.consumer;
        let launch = self.launch;
        producer_input_validate_body!(
            completion_journal_rust_syntax,
            context,
            root,
            id,
            consumer,
            launch,
            index,
            active_index,
            queued_index,
            self
        )
    }

    fn validate_copy_input_v1(
        &mut self,
        index: usize,
        active_index: &mut usize,
        queued_index: &mut usize,
    ) -> Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        if index != 0 || *active_index != 0 || *queued_index != 0 {
            return Err(E::InvalidReference);
        }
        let input = &self.root.inputs[0];
        let ProducerReadRequestV1::Active(request) = input.request else {
            return Err(E::InvalidReference);
        };
        let reference = self.root.references[0];
        let source = input.source;
        let copy = self
            .context
            .same_device_copies
            .get(&self.id)
            .ok_or(E::InvalidReference)?;
        if self.root.requests[0] != request
            || reference.consumer != self.consumer
            || !copy.dependencies_held
            || copy.producer != input.dependency
            || !producer_dependency_contains_v1(&copy.dependencies, &input.dependency)
            || copy.source.region != source.region
            || copy.source.record != source.record
            || request.producer.key
                != (ContextWriterKeyV1 {
                    context_generation: input.dependency.submission.context_generation,
                    local: input.dependency.submission.local,
                    kind: ContextWriterKindV1::Submission,
                })
            || input.dependency.submission.local >= self.id.local
            || self.context.allocations.get(&source.region.allocation) != Some(&source.record)
            || !self
                .context
                .backend_allocations
                .contains(&source.record.backend_allocation)
            || !self.observe_expected_credit(
                source.region.allocation,
                source.record.device,
                source.record.byte_len,
            )
            || self.observe_live(source.region.allocation, &source.record)?
                != request.read.allocation
            || request.read.device
                != enrollment(
                    source.region.allocation,
                    source.record.device,
                    source.record.byte_len,
                )
                .device
            || request.read.byte_extent != source.record.byte_len
            || request.read.byte_offset != source.region.byte_offset
            || request.read.byte_len != source.region.byte_len
            || self.observe_active_lookup(reference)? != request
        {
            return Err(E::InvalidReference);
        }
        *active_index = 1;
        self.observe_active_status(reference)
    }

    fn reconcile(&mut self) -> Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1> {
        let invalid_reference = ContextVersionJournalErrorV1::InvalidReference;
        producer_input_fold_body!(
            completion_journal_rust_syntax,
            self,
            invalid_reference,
            (index, aggregate, active_index, queued_index, input_count),
            [],
            [],
            []
        )
    }
}

pub(super) struct PreparedProducerReadsV1 {
    root: RetainedProducerReadV1,
    output: Vec<Option<ContextProducerReadReferenceV1>>,
    queued_output: Vec<Option<ContextQueuedProducerReadReferenceV1>>,
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
                    root.references.len() + root.queued_references.len(),
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

    pub(in crate::context) fn corrupt_queued_read_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        corruption: usize,
    ) {
        let root = self.producer_readers.get_mut(&id).unwrap();
        match corruption {
            0 => root.queued_references[0].incarnation += 1,
            1 => root.queued_requests[0].producer.key.local += 1,
            2 => root.marker.as_mut().unwrap().queued.as_mut().unwrap().1 += 1,
            3 => root.queued_references.clear(),
            _ => panic!("unknown queued read corruption"),
        }
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    fn prepare_pending_input_v1(
        &mut self,
        source: ContextReadSourceV1,
        dependencies: &[ScalarPeerDependencyV1],
        launch: Option<&ProducerLaunchRootV1>,
        copy: Option<&SameDeviceCopyRootV1>,
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
        if queued
            && (launch.is_none() || !self.producer_launches.contains_key(&dependency.submission))
        {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        if let Some(consumer) = launch {
            // Leases cover the allocation; native coverage must cover every original Read alias.
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
                                .map(|producer| producer.covers_input_v1(*binding))
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
        } else if let Some(copy) = copy {
            let producer = self
                .scalar_peer_copies
                .get(&dependency.submission)
                .filter(|producer| producer.directed.is_none())
                .ok_or(RuntimeValidationErrorV1::ContextReserved)?;
            if dependency != copy.producer
                || source.region != copy.source.region
                || source.record != copy.source.record
                || !producer.covers_input_v1(source)
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

    fn prepare_producer_batch_v1(
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
                context.prepare_pending_input_v1(root.source, &root.dependencies, None, None)?
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
                match context.prepare_pending_input_v1(
                    *source,
                    &root.dependencies,
                    Some(root),
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
                let (mut producer_output, mut queued_output) = if let Some(prepared) = producers {
                    versions.producer_readers.insert(id, prepared.root);
                    (prepared.output, prepared.queued_output)
                } else {
                    (Vec::new(), Vec::new())
                };
                let mut read_root = versions.submission_readers.get_mut(&id);
                let mut producer_root = versions.producer_readers.get_mut(&id);
                #[cfg(test)]
                let fault = versions.mixed_input_fault.take();
                #[cfg(test)]
                if matches!(fault.as_ref(), Some(MixedInputFaultV1::RejectPending)) {
                    let root = producer_root.as_mut().expect("fault fixture producer");
                    match root
                        .inputs
                        .last()
                        .expect("fault fixture pending input")
                        .request
                    {
                        ProducerReadRequestV1::Active(_) => {
                            root.requests.last_mut().unwrap().read.byte_len = 0
                        }
                        ProducerReadRequestV1::Queued(_) => {
                            root.queued_requests.last_mut().unwrap().byte_len = 0
                        }
                    }
                }
                let consumer = ContextWriterKeyV1 {
                    context_generation: id.context_generation,
                    local: id.local,
                    kind: ContextWriterKindV1::Submission,
                };
                versions.journal.acquire_mixed_reads_with_queued(
                    consumer,
                    (
                        read_root
                            .as_ref()
                            .map_or(&[], |root| root.requests.as_slice()),
                        &mut read_output,
                    ),
                    (
                        producer_root
                            .as_ref()
                            .map_or(&[], |root| root.requests.as_slice()),
                        &mut producer_output,
                    ),
                    (
                        producer_root
                            .as_ref()
                            .map_or(&[], |root| root.queued_requests.as_slice()),
                        &mut queued_output,
                    ),
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
                    assert!(
                        root.queued_references.capacity() >= queued_output.len(),
                        "preallocated queued references"
                    );
                    for reference in queued_output {
                        root.queued_references
                            .push(reference.expect("complete queued reservations"));
                    }
                }
                let read_marker = read_root.as_ref().map(|root| SubmissionReaderMarkerV1 {
                    first: root.references[0],
                    count: root.references.len(),
                });
                let producer_marker = producer_root.as_ref().map(|root| root.complete_marker());
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
                let copy = context.same_device_copies.contains_key(&id);
                let versions = context
                    .versions
                    .as_mut()
                    .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
                if versions.producer_readers.contains_key(&id)
                    || launch != (prepared.root.domain == ProducerReadDomainV1::Launch)
                    || copy != (prepared.root.domain == ProducerReadDomainV1::Copy)
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
                let marker = root.complete_marker();
                root.marker = Some(marker);
                Ok(Some(marker))
            })();
            context.journal_result_v1(result)
        })
    }

    fn producer_input_root_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<Option<ProducerInputRootV1<'_>>, ContextVersionJournalErrorV1> {
        if self
            .versions
            .as_ref()
            .and_then(|versions| versions.producer_readers.get(&id))
            .is_some_and(|root| root.domain == ProducerReadDomainV1::Copy)
        {
            return self.copy_producer_input_root_v1(id).map(Some);
        }
        if self
            .submissions
            .get(&id)
            .is_some_and(|record| record.same_device_copy && !record.quiescent)
            || self
                .same_device_copies
                .get(&id)
                .is_some_and(|copy| copy.dependencies_held)
        {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        producer_input_preflight_body!(completion_journal_rust_syntax, self, id)
    }

    fn copy_producer_input_root_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<ProducerInputRootV1<'_>, ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        self.validate_same_device_copy_custody_v1(id)
            .map_err(|_| E::InvalidReference)?;
        let versions = self.versions.as_ref().ok_or(E::InvalidReference)?;
        let root = versions
            .producer_readers
            .get(&id)
            .ok_or(E::InvalidReference)?;
        let marker = root.marker.ok_or(E::InvalidReference)?;
        let consumer = ContextWriterKeyV1 {
            context_generation: id.context_generation,
            local: id.local,
            kind: ContextWriterKindV1::Submission,
        };
        let writer = versions
            .submission_writers
            .get(&id)
            .ok_or(E::InvalidReference)?;
        if root.domain != ProducerReadDomainV1::Copy
            || root.inputs.len() != 1
            || root.references.len() != 1
            || root.requests.len() != 1
            || !root.queued_requests.is_empty()
            || !root.queued_references.is_empty()
            || !matches!(root.inputs[0].request, ProducerReadRequestV1::Active(_))
            || marker != root.complete_marker()
            || marker.first.consumer != consumer
            || versions.submission_readers.contains_key(&id)
            || writer.domain != SubmissionWriterDomainV1::Ordinary
            || self.submissions.get(&id).is_some_and(|record| {
                !record.same_device_copy
                    || record.producer_launch
                    || record.scalar_peer_copy
                    || record.directed_peer_copy
                    || record.journal_read.is_some()
                    || record.journal_producer_read != Some(marker)
                    || record.journal_writer != Some(writer.writer)
            })
        {
            return Err(E::InvalidReference);
        }
        Ok(ProducerInputRootV1 {
            versions,
            root,
            consumer,
            launch: false,
        })
    }

    pub(super) fn prepare_copy_inputs_v1(
        &mut self,
        root: &SameDeviceCopyRootV1,
    ) -> Result<Option<PreparedProducerReadsV1>, RuntimeValidationErrorV1> {
        self.guard_journal_unwind_v1(|context| {
            let mut inputs = Vec::new();
            inputs
                .try_reserve_exact(1)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            let input = context
                .prepare_pending_input_v1(root.source, &root.dependencies, None, Some(root))?
                .ok_or(RuntimeValidationErrorV1::ContextReserved)?;
            inputs.push(input);
            context.prepare_producer_batch_v1(inputs, ProducerReadDomainV1::Copy)
        })
    }

    pub(super) fn validate_producer_read_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<Option<ContextProducerReadStatusV1>, ContextVersionJournalErrorV1> {
        let Some(ProducerInputRootV1 {
            versions,
            root,
            consumer,
            launch,
        }) = self.producer_input_root_v1(id)?
        else {
            return Ok(None);
        };
        ProducerInputObservationsV1 {
            context: self,
            versions,
            root,
            id,
            consumer,
            launch,
        }
        .reconcile()
        .map(Some)
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
            let root = versions
                .producer_readers
                .get(&id)
                .expect("validated producer inputs");
            if !root.queued_references.is_empty() {
                let consumer = root
                    .marker
                    .expect("validated producer marker")
                    .first
                    .consumer;
                let evidence = ContextReadQuiescenceEvidenceV1 { consumer };
                if !root.references.is_empty() {
                    versions.journal.release_producer_reads(
                        consumer,
                        &root.references,
                        &evidence,
                    )?;
                }
                // Each family preflights independently. A failure after the active
                // release retains the Context root and quarantines the remainder.
                #[cfg(test)]
                versions.completion_boundary_for_test_v1(
                    id,
                    completion_faults::CompletionJournalStageV1::Producer,
                    completion_faults::CompletionJournalPointV1::BetweenProducerClasses,
                )?;
                let root = versions
                    .producer_readers
                    .get(&id)
                    .expect("retained producer inputs");
                versions.journal.release_queued_producer_reads(
                    consumer,
                    &root.queued_references,
                    &evidence,
                )?;
                #[cfg(test)]
                versions.completion_boundary_for_test_v1(
                    id,
                    completion_faults::CompletionJournalStageV1::Producer,
                    completion_faults::CompletionJournalPointV1::AfterEffect,
                )?;
                versions.producer_readers.remove(&id);
                if let Some(record) = self.submissions.get_mut(&id) {
                    record.journal_producer_read = None;
                }
                return Ok(());
            }
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
