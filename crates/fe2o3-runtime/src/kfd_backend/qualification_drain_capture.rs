//! Bounded copy-only qualification observations, never execution authority.

use super::*;
use fe2o3_kfd::Gfx942R66NativeObservationFailureV1;

const MAX_COPIES: usize = 8;
const MAX_DEPENDENCIES: usize = 8;
const MAX_PUBLICATIONS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdDrainCaptureCopyPhaseV1 {
    Ready,
    DirectionalPublished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDrainCaptureCopyObservationV1 {
    pub submission: u64,
    pub stream: u64,
    pub source: u64,
    pub destination: u64,
    pub source_offset: u64,
    pub destination_offset: u64,
    pub byte_len: u64,
    pub phase: KfdDrainCaptureCopyPhaseV1,
    pub native_receipt: Option<[u8; 32]>,
    pub runtime_membership: Option<[u8; 32]>,
    pub native_packets: usize,
    dependencies: [u64; MAX_DEPENDENCIES],
    dependency_count: usize,
}

impl KfdDrainCaptureCopyObservationV1 {
    const EMPTY: Self = Self {
        submission: 0,
        stream: 0,
        source: 0,
        destination: 0,
        source_offset: 0,
        destination_offset: 0,
        byte_len: 0,
        phase: KfdDrainCaptureCopyPhaseV1::Ready,
        native_receipt: None,
        runtime_membership: None,
        native_packets: 0,
        dependencies: [0; MAX_DEPENDENCIES],
        dependency_count: 0,
    };

    pub fn dependencies(&self) -> &[u64] {
        &self.dependencies[..self.dependency_count]
    }
}

/// A fixed-storage snapshot. Native retention does not imply unfinished GPU work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDrainCaptureCustodyObservationV1 {
    copies: [KfdDrainCaptureCopyObservationV1; MAX_COPIES],
    copy_count: usize,
    native_retained_copies: usize,
    publication_ids: [u64; MAX_PUBLICATIONS],
    publication_count: usize,
}

impl KfdDrainCaptureCustodyObservationV1 {
    pub fn copies(&self) -> &[KfdDrainCaptureCopyObservationV1] {
        &self.copies[..self.copy_count]
    }

    /// Complete indexed async-publication history for this bounded profile.
    /// Synchronous copies and physical completion are not represented by it.
    pub fn publication_ids(&self) -> &[u64] {
        &self.publication_ids[..self.publication_count]
    }

    pub const fn native_retained_copies(&self) -> usize {
        self.native_retained_copies
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdDrainCaptureObservationFailureV1 {
    Authority,
    Terminal,
    Capacity,
    ComputeState,
    PublicationHistory,
    PublicationIndex,
    Submission,
    Stream,
    Allocation,
    Custody,
    Storage,
    Geometry,
    Dependencies,
    Phase,
    NativeQueueUnavailable,
    NativeCounts(Gfx942R66NativeObservationFailureV1),
    NativeCountMismatch,
    RetainedCopy(KfdR66RetainedCustodyObservationFailureV1),
}

impl fmt::Display for KfdDrainCaptureObservationFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for KfdDrainCaptureObservationFailureV1 {}

#[derive(Debug)]
pub(super) struct PublicationHistoryV1 {
    ids: [u64; MAX_PUBLICATIONS],
    count: usize,
    overflow: bool,
}

impl PublicationHistoryV1 {
    pub(super) const fn new() -> Self {
        Self {
            ids: [0; MAX_PUBLICATIONS],
            count: 0,
            overflow: false,
        }
    }

    pub(super) fn record(&mut self, submission: u64) {
        if self.count == MAX_PUBLICATIONS {
            self.overflow = true;
            return;
        }
        self.ids[self.count] = submission;
        self.count += 1;
    }

    fn valid(&self) -> bool {
        !self.overflow
            && self.ids[..self.count]
                .iter()
                .enumerate()
                .all(|(index, id)| *id != 0 && !self.ids[..index].contains(id))
    }
}

impl KfdRuntimeBackendV1 {
    /// Opens the frozen copy-only qualification profile without a caller authorizer.
    /// All compute, atomic and collective launch requests remain denied.
    pub fn open_copy_only_qualification_v1(
        device_unique_id: u64,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let mut backend = Self::open_default_with_gate(
            device_unique_id,
            KfdRuntimeLaunchGateV1::CopyOnlyQualification,
        )?;
        backend.drain_capture_publications = Some(PublicationHistoryV1::new());
        Ok(backend)
    }

    pub(super) fn drain_capture_runtime_roster_v1(
        &self,
    ) -> Result<KfdDrainCaptureCustodyObservationV1, KfdDrainCaptureObservationFailureV1> {
        use KfdDrainCaptureObservationFailureV1 as Failure;
        if !matches!(
            self.launch_gate,
            KfdRuntimeLaunchGateV1::CopyOnlyQualification
        ) {
            return Err(Failure::Authority);
        }
        if self.terminal
            || self.queue_retired
            || self.terminal_memory.is_some()
            || self.terminal_sdma_custody.is_some()
        {
            return Err(Failure::Terminal);
        }
        if self.any_compute_active_v1()
            || self.allocations.has_generated()
            || !self.pending_compute.is_empty()
            || !self.compute_pipeline.is_empty()
            || !self.modules.is_empty()
            || !self.kernels.is_empty()
            || self.resident_data.is_some()
            || self.recycled_dispatch.is_some()
            || self.retained_persistent_dispatch.is_some()
            || self
                .auxiliary_compute_lanes
                .iter()
                .any(|lane| lane.active.is_some() || !lane.pipeline.is_empty())
        {
            return Err(Failure::ComputeState);
        }
        if self.active_sdma.len() > MAX_COPIES
            || self.published_sdma_submissions.len() > 1
            || self.submissions.len() > MAX_PUBLICATIONS
            || self.allocations.len() > MAX_COPIES
            || self.streams.len() > 2
            || self.events.len() > MAX_COPIES
            || self.active_sdma_streams.len() > 2
            || self.allocation_custody.len() > MAX_COPIES
            || self
                .active_sdma_streams
                .values()
                .any(|ids| ids.len() > MAX_COPIES)
            || self
                .allocation_custody
                .values()
                .any(|custody| custody.owners.len() > MAX_COPIES)
        {
            return Err(Failure::Capacity);
        }
        let history = self
            .drain_capture_publications
            .as_ref()
            .filter(|history| history.valid())
            .ok_or(Failure::PublicationHistory)?;
        let mut observation = KfdDrainCaptureCustodyObservationV1 {
            copies: [KfdDrainCaptureCopyObservationV1::EMPTY; MAX_COPIES],
            copy_count: 0,
            native_retained_copies: self.published_sdma_submissions.len(),
            publication_ids: history.ids,
            publication_count: history.count,
        };
        for (&id, copy) in &self.active_sdma {
            // Active copies own pending state; this table contains terminal records.
            if id == 0 || id != copy.id || self.submissions.contains_key(&id) {
                return Err(Failure::Submission);
            }
            if !self.streams.contains_key(&copy.stream)
                || self
                    .active_sdma_streams
                    .get(&copy.stream)
                    .is_none_or(|ids| ids.iter().filter(|actual| **actual == id).count() != 1)
            {
                return Err(Failure::Stream);
            }
            if copy.source == copy.destination
                || copy.byte_len == 0
                || copy.completed_bytes != 0
                || copy.dependencies.len() > MAX_DEPENDENCIES
                || copy.dependency_cursor > copy.dependencies.len()
            {
                return Err(Failure::Geometry);
            }
            let owner = RuntimeAllocationCustodyOwnerV1 {
                submission: id,
                stream: copy.stream,
                kind: RuntimeAllocationCustodyKindV1::Sdma,
            };
            for (allocation, offset) in [
                (copy.source, copy.source_offset),
                (copy.destination, copy.destination_offset),
            ] {
                let record = self
                    .allocations
                    .get(&allocation)
                    .ok_or(Failure::Allocation)?;
                if self.streams.get(&copy.stream) != Some(&record.device) {
                    return Err(Failure::Allocation);
                }
                if offset
                    .checked_add(copy.byte_len)
                    .is_none_or(|end| end > record.bytes.len() as u64)
                {
                    return Err(Failure::Geometry);
                }
                if !self.allocation_retains_exact_owner_v1(allocation, owner) {
                    return Err(Failure::Custody);
                }
            }
            let source_kind = self.allocations[&copy.source].kind;
            let destination_kind = self.allocations[&copy.destination].kind;
            if !matches!(
                (source_kind, destination_kind),
                (
                    RuntimeMemoryKindV1::HostVisible,
                    RuntimeMemoryKindV1::DeviceLocal
                ) | (
                    RuntimeMemoryKindV1::DeviceLocal,
                    RuntimeMemoryKindV1::HostVisible
                )
            ) {
                return Err(Failure::Geometry);
            }
            for (index, dependency) in copy.dependencies.iter().enumerate() {
                if *dependency == id
                    || copy.dependencies[..index].contains(dependency)
                    || !(self.active_sdma.contains_key(dependency)
                        || self
                            .submissions
                            .get(dependency)
                            .is_some_and(|record| record.status == BackendPollV1::Succeeded))
                {
                    return Err(Failure::Dependencies);
                }
            }
            if copy
                .prior_stream_submission
                .is_some_and(|prior| !copy.dependencies.contains(&prior))
            {
                return Err(Failure::Dependencies);
            }
            let indexed = self.published_sdma_submissions.binary_search(&id).is_ok();
            let phase = match copy.phase {
                ActiveSdmaPhaseV1::Ready
                    if !indexed
                        && copy.window_bytes == 0
                        && !history.ids[..history.count].contains(&id) =>
                {
                    KfdDrainCaptureCopyPhaseV1::Ready
                }
                ActiveSdmaPhaseV1::DirectionalPublished(_)
                    if indexed
                        && copy.window_bytes == copy.byte_len
                        && history.ids[..history.count].contains(&id) =>
                {
                    KfdDrainCaptureCopyPhaseV1::DirectionalPublished
                }
                _ => return Err(Failure::Phase),
            };
            let mut row = KfdDrainCaptureCopyObservationV1 {
                submission: id,
                stream: copy.stream,
                source: copy.source,
                destination: copy.destination,
                source_offset: copy.source_offset,
                destination_offset: copy.destination_offset,
                byte_len: copy.byte_len,
                phase,
                ..KfdDrainCaptureCopyObservationV1::EMPTY
            };
            row.dependencies[..copy.dependencies.len()].copy_from_slice(&copy.dependencies);
            row.dependency_count = copy.dependencies.len();
            observation.copies[observation.copy_count] = row;
            observation.copy_count += 1;
        }
        observation.copies[..observation.copy_count].sort_unstable_by_key(|copy| copy.submission);
        if self
            .published_sdma_submissions
            .iter()
            .any(|id| !self.active_sdma.contains_key(id))
        {
            return Err(Failure::PublicationIndex);
        }
        for (&stream, ids) in &self.active_sdma_streams {
            if ids.is_empty()
                || ids.len() > MAX_COPIES
                || ids.iter().any(|id| {
                    self.active_sdma
                        .get(id)
                        .is_none_or(|copy| copy.stream != stream)
                })
            {
                return Err(Failure::Stream);
            }
        }
        for (&allocation, custody) in &self.allocation_custody {
            if !self.allocations.contains_key(&allocation)
                || custody.owners.is_empty()
                || custody.owners.len() > MAX_COPIES
                || custody.owner_counts != [0, custody.owners.len()]
                || custody.owners.iter().enumerate().any(|(index, owner)| {
                    owner.kind != RuntimeAllocationCustodyKindV1::Sdma
                        || custody
                            .owners
                            .iter()
                            .take(index)
                            .any(|prior| prior.submission == owner.submission)
                        || !self.active_sdma.get(&owner.submission).is_some_and(|copy| {
                            copy.stream == owner.stream
                                && (copy.source == allocation || copy.destination == allocation)
                        })
                })
            {
                return Err(Failure::Custody);
            }
        }
        for record in self.submissions.values() {
            if record.status != BackendPollV1::Succeeded {
                return Err(Failure::Submission);
            }
        }
        for (&allocation, record) in self.allocations.ordinary_iter() {
            match record.sdma_storage {
                KfdRuntimeSdmaStorageV1::ComputeInFlight(_)
                | KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous) => {
                    return Err(Failure::Storage);
                }
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(id))
                    if !self.active_sdma.get(&id).is_some_and(|copy| {
                        (copy.source == allocation || copy.destination == allocation)
                            && matches!(copy.phase, ActiveSdmaPhaseV1::DirectionalPublished(_))
                    }) =>
                {
                    return Err(Failure::Storage);
                }
                _ => {}
            }
        }
        Ok(observation)
    }

    /// Observes a warmed directional queue without polling, allocation or mutation.
    /// This profile accepts at most eight active copies and one retained native
    /// directional publication, with complete windows and no compute ownership.
    /// Queued rows are runtime ownership facts, never native-publication evidence.
    pub fn diagnose_drain_capture_custody_v1(
        &self,
    ) -> Result<KfdDrainCaptureCustodyObservationV1, KfdDrainCaptureObservationFailureV1> {
        use KfdDrainCaptureObservationFailureV1 as Failure;
        let mut observation = self.drain_capture_runtime_roster_v1()?;
        let queue = self.queue.as_ref().ok_or(Failure::NativeQueueUnavailable)?;
        let native_counts = queue
            .diagnose_r66_retained_counts_v1()
            .map_err(Failure::NativeCounts)?;
        if native_counts != (0, observation.native_retained_copies) {
            return Err(Failure::NativeCountMismatch);
        }
        for row in &mut observation.copies[..observation.copy_count] {
            if row.phase == KfdDrainCaptureCopyPhaseV1::DirectionalPublished {
                let copy = &self.active_sdma[&row.submission];
                let native = self
                    .r66_copy_observation_v1(row.submission, copy)
                    .map_err(Failure::RetainedCopy)?;
                row.native_receipt = native.copy;
                row.runtime_membership = native.copy_membership;
                row.native_packets = native.copy_packets;
            }
        }
        Ok(observation)
    }
}

#[cfg(test)]
mod tests {
    use super::super::drain_capture::tests::counted;
    use super::*;

    struct QueuedFixture {
        backend: KfdRuntimeBackendV1,
        stream: u64,
        source: u64,
        destination: u64,
    }

    impl QueuedFixture {
        fn new() -> Self {
            let mut backend = KfdRuntimeBackendV1::mock();
            backend.launch_gate = KfdRuntimeLaunchGateV1::CopyOnlyQualification;
            backend.drain_capture_publications = Some(PublicationHistoryV1::new());
            let stream = backend.create_stream_v1(7).unwrap();
            let source = backend
                .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 64, 8)
                .unwrap();
            let destination = backend
                .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
                .unwrap();
            let mut fixture = Self {
                backend,
                stream,
                source,
                destination,
            };
            fixture.add(101);
            fixture
        }

        fn add(&mut self, id: u64) {
            let allocations = [self.source, self.destination];
            let reserved = self
                .backend
                .reserve_allocation_custody_v1(&allocations)
                .unwrap();
            self.backend.retain_allocation_custody_v1(
                &allocations,
                RuntimeAllocationCustodyOwnerV1 {
                    submission: id,
                    stream: self.stream,
                    kind: RuntimeAllocationCustodyKindV1::Sdma,
                },
                reserved,
            );
            self.backend
                .active_sdma_streams
                .entry(self.stream)
                .or_default()
                .push_back(id);
            self.backend.active_sdma.insert(
                id,
                ActiveSdmaCopyV1 {
                    id,
                    stream: self.stream,
                    prior_stream_submission: None,
                    source: self.source,
                    destination: self.destination,
                    source_offset: 3,
                    destination_offset: 5,
                    byte_len: 17,
                    completed_bytes: 0,
                    window_bytes: 0,
                    window_requests: None,
                    dependencies: Vec::new(),
                    dependency_cursor: 0,
                    dependency_depth: 0,
                    phase: ActiveSdmaPhaseV1::Ready,
                },
            );
        }
    }

    impl Drop for QueuedFixture {
        fn drop(&mut self) {
            // Only synthetic records exist; no native authority is fabricated here.
            assert!(!self.backend.native_available && self.backend.queue.is_none());
            self.backend.active_sdma.clear();
            self.backend.active_sdma_streams.clear();
            self.backend.published_sdma_submissions.clear();
            self.backend.allocation_custody.clear();
            for record in self.backend.allocations.values_mut() {
                record.sdma_storage = KfdRuntimeSdmaStorageV1::Synthetic;
            }
        }
    }

    #[test]
    fn publication_history_is_bounded_complete_and_never_silently_truncated() {
        let mut history = PublicationHistoryV1::new();
        for id in 1..=MAX_PUBLICATIONS as u64 {
            history.record(id);
            assert!(history.valid());
        }
        let full = history.ids;
        history.record(33);
        assert!(!history.valid());
        assert_eq!(history.ids, full);
        assert_eq!(history.count, MAX_PUBLICATIONS);
        for ids in [[0, 1], [1, 1]] {
            let mut invalid = PublicationHistoryV1::new();
            for id in ids {
                invalid.record(id);
            }
            assert!(!invalid.valid());
        }
    }

    #[test]
    fn indexed_publication_history_never_rewinds_when_active_index_retires() {
        let mut fixture = QueuedFixture::new();
        fixture.backend.reserve_published_sdma_index_v1().unwrap();
        fixture.backend.index_published_sdma_v1(101);
        fixture.backend.record_drain_capture_publication_v1(101);
        assert_eq!(
            fixture
                .backend
                .drain_capture_publications
                .as_ref()
                .unwrap()
                .ids[0],
            101
        );
        assert_eq!(
            fixture.backend.drain_capture_runtime_roster_v1(),
            Err(KfdDrainCaptureObservationFailureV1::Phase)
        );
        fixture.backend.unindex_published_sdma_v1(101);
        assert_eq!(
            fixture
                .backend
                .drain_capture_publications
                .as_ref()
                .unwrap()
                .count,
            1
        );
        assert_eq!(
            fixture.backend.drain_capture_runtime_roster_v1(),
            Err(KfdDrainCaptureObservationFailureV1::Phase)
        );
    }

    #[test]
    fn queued_observations_are_immutable_allocation_free_and_not_native_evidence() {
        let fixture = QueuedFixture::new();
        let expected = fixture.backend.drain_capture_runtime_roster_v1().unwrap();
        assert_eq!(expected.copies().len(), 1);
        assert_eq!(expected.copies()[0].submission, 101);
        assert_eq!(expected.copies()[0].source_offset, 3);
        assert_eq!(expected.copies()[0].destination_offset, 5);
        assert_eq!(expected.copies()[0].byte_len, 17);
        assert_eq!(
            expected.copies()[0].phase,
            KfdDrainCaptureCopyPhaseV1::Ready
        );
        assert_eq!(expected.copies()[0].native_receipt, None);
        assert_eq!(expected.copies()[0].runtime_membership, None);
        assert!(expected.publication_ids().is_empty());
        let next_handle = fixture.backend.next_handle;
        for _ in 0..16 {
            let (actual, allocations) =
                counted(|| fixture.backend.drain_capture_runtime_roster_v1());
            assert_eq!(actual.unwrap(), expected);
            assert_eq!(allocations, 0);
            let (public, allocations) =
                counted(|| fixture.backend.diagnose_drain_capture_custody_v1());
            assert_eq!(
                public,
                Err(KfdDrainCaptureObservationFailureV1::NativeQueueUnavailable)
            );
            assert_eq!(allocations, 0);
        }
        assert_eq!(fixture.backend.next_handle, next_handle);
    }

    #[test]
    fn complete_queued_roster_is_sorted_and_capacity_is_checked_before_extraction() {
        let mut fixture = QueuedFixture::new();
        for id in (102..=108).rev() {
            fixture.add(id);
        }
        let observation = fixture.backend.drain_capture_runtime_roster_v1().unwrap();
        assert_eq!(observation.copies().len(), MAX_COPIES);
        for (index, row) in observation.copies().iter().enumerate() {
            assert_eq!(row.submission, 101 + index as u64);
        }
        fixture.add(109);
        assert_eq!(
            fixture.backend.drain_capture_runtime_roster_v1(),
            Err(KfdDrainCaptureObservationFailureV1::Capacity)
        );
    }

    #[test]
    fn ready_dependencies_resolve_active_owners_or_successful_terminal_records() {
        let mut fixture = QueuedFixture::new();
        fixture.add(102);
        fixture
            .backend
            .active_sdma
            .get_mut(&102)
            .unwrap()
            .dependencies
            .push(101);
        assert!(fixture.backend.submissions.is_empty());
        assert!(fixture.backend.drain_capture_runtime_roster_v1().is_ok());
        fixture.backend.submissions.insert(
            100,
            SubmissionRecordV1 {
                stream: fixture.stream,
                status: BackendPollV1::Succeeded,
                profile_dispatch_published: false,
            },
        );
        fixture
            .backend
            .active_sdma
            .get_mut(&101)
            .unwrap()
            .dependencies
            .push(100);
        assert!(fixture.backend.drain_capture_runtime_roster_v1().is_ok());
        fixture.backend.submissions.get_mut(&100).unwrap().status = BackendPollV1::Pending;
        assert_eq!(
            fixture.backend.drain_capture_runtime_roster_v1(),
            Err(KfdDrainCaptureObservationFailureV1::Dependencies)
        );
    }

    #[test]
    fn publication_history_hooks_exclude_poll_and_wait_index_restoration() {
        let source = include_str!("../kfd_backend.rs");
        let source = source.split("#[cfg(test)]\nmod tests").next().unwrap();
        assert_eq!(
            source
                .matches("self.record_drain_capture_publication_v1(active.id);")
                .count(),
            2
        );
        for (start, end) in [
            (
                "    fn publish_directional_sdma_window_v1(",
                "    fn publish_same_device_sdma_window_v1(",
            ),
            (
                "    fn publish_same_device_sdma_window_v1(",
                "    fn progress_unpublished_sdma_copy_v1(",
            ),
        ] {
            let body = source
                .split_once(start)
                .unwrap()
                .1
                .split_once(end)
                .unwrap()
                .0;
            assert_eq!(
                body.matches("self.record_drain_capture_publication_v1(active.id);")
                    .count(),
                1
            );
        }
    }

    #[test]
    fn queued_roster_rejects_one_coordinate_and_unaccounted_ownership_mutations() {
        use KfdDrainCaptureObservationFailureV1 as Failure;
        for case in 0..17 {
            let mut fixture = QueuedFixture::new();
            let expected = match case {
                0 => {
                    fixture.backend.active_sdma.get_mut(&101).unwrap().id = 102;
                    Failure::Submission
                }
                1 => {
                    fixture.backend.submissions.insert(
                        101,
                        SubmissionRecordV1 {
                            stream: fixture.stream,
                            status: BackendPollV1::Succeeded,
                            profile_dispatch_published: false,
                        },
                    );
                    Failure::Submission
                }
                2 => {
                    fixture.backend.streams.remove(&fixture.stream);
                    Failure::Stream
                }
                3 => {
                    fixture
                        .backend
                        .active_sdma
                        .get_mut(&101)
                        .unwrap()
                        .source_offset = u64::MAX;
                    Failure::Geometry
                }
                4 => {
                    fixture.backend.active_sdma.get_mut(&101).unwrap().byte_len = 0;
                    Failure::Geometry
                }
                5 => {
                    fixture
                        .backend
                        .allocations
                        .get_mut(&fixture.destination)
                        .unwrap()
                        .device = 8;
                    Failure::Allocation
                }
                6 => {
                    fixture.backend.allocation_custody.remove(&fixture.source);
                    Failure::Custody
                }
                7 => {
                    fixture
                        .backend
                        .active_sdma
                        .get_mut(&101)
                        .unwrap()
                        .dependencies
                        .push(999);
                    Failure::Dependencies
                }
                8 => {
                    fixture
                        .backend
                        .active_sdma
                        .get_mut(&101)
                        .unwrap()
                        .dependencies
                        .push(101);
                    Failure::Dependencies
                }
                9 => {
                    fixture
                        .backend
                        .active_sdma
                        .get_mut(&101)
                        .unwrap()
                        .prior_stream_submission = Some(999);
                    Failure::Dependencies
                }
                10 => {
                    fixture
                        .backend
                        .active_sdma_streams
                        .get_mut(&fixture.stream)
                        .unwrap()
                        .push_back(999);
                    Failure::Stream
                }
                11 => {
                    fixture
                        .backend
                        .allocation_custody
                        .get_mut(&fixture.source)
                        .unwrap()
                        .owner_counts[1] = 2;
                    Failure::Custody
                }
                12 => {
                    fixture.backend.submissions.insert(
                        999,
                        SubmissionRecordV1 {
                            stream: fixture.stream,
                            status: BackendPollV1::Pending,
                            profile_dispatch_published: false,
                        },
                    );
                    Failure::Submission
                }
                13 => {
                    fixture
                        .backend
                        .allocations
                        .get_mut(&fixture.destination)
                        .unwrap()
                        .sdma_storage =
                        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(101));
                    Failure::Storage
                }
                14 => {
                    fixture.backend.published_sdma_submissions.push(999);
                    Failure::PublicationIndex
                }
                15 => {
                    fixture
                        .backend
                        .active_sdma_streams
                        .get_mut(&fixture.stream)
                        .unwrap()
                        .extend([999; MAX_COPIES]);
                    Failure::Capacity
                }
                16 => {
                    let custody = fixture
                        .backend
                        .allocation_custody
                        .get_mut(&fixture.source)
                        .unwrap();
                    let owner = custody.owners[0];
                    custody.owners.extend([owner; MAX_COPIES]);
                    Failure::Capacity
                }
                _ => unreachable!(),
            };
            assert_eq!(
                fixture.backend.drain_capture_runtime_roster_v1(),
                Err(expected),
                "case {case}"
            );
        }
    }

    #[test]
    fn caller_authority_and_missing_history_cannot_enter_copy_only_observation() {
        for backend in [
            KfdRuntimeBackendV1::mock(),
            KfdRuntimeBackendV1::mock_with_semantic_authority_v1(),
        ] {
            assert_eq!(
                backend.diagnose_drain_capture_custody_v1(),
                Err(KfdDrainCaptureObservationFailureV1::Authority)
            );
        }
        let mut fixture = QueuedFixture::new();
        assert!(!fixture.backend.launch_gate.advertises_atomics_v1());
        assert!(!fixture.backend.launch_gate.advertises_collectives_v1());
        fixture.backend.drain_capture_publications = None;
        assert_eq!(
            fixture.backend.drain_capture_runtime_roster_v1(),
            Err(KfdDrainCaptureObservationFailureV1::PublicationHistory)
        );
    }
}
