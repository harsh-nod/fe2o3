//! Exact directed provenance around the existing scalar owner and progress driver.

#![forbid(unsafe_code)]

use super::*;
use crate::{
    BackendDirectedPeerDependencyV1, BackendDirectedPeerRouteV1, BackendDirectedScalarPeerCopyV1,
    BackendDirectedScalarProgressV1, RuntimeDirectedScalarPeerCopyBackendV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[cfg(test)]
mod tests;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Root {
    route: BackendDirectedPeerRouteV1,
    source_extent: u64,
    destination_extent: u64,
    dependencies: Vec<BackendDirectedPeerDependencyV1>,
    admitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Error {
    Unknown,
    Unsupported,
    Invalid,
    Capacity,
    Corrupt,
}

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

struct View<'a> {
    devices: [u64; 2],
    streams: &'a HashMap<u64, usize>,
    allocations: &'a HashMap<u64, XgmiRuntimeAllocationV1>,
    events: &'a HashMap<u64, EventRecordV1>,
    active: &'a HashMap<u64, XgmiRuntimeSubmissionV1>,
    completed: &'a HashMap<u64, SubmissionRecordV1>,
    roots: &'a HashMap<u64, Root>,
}

impl View<'_> {
    fn devices(&self, route: BackendDirectedPeerRouteV1) -> Result<(usize, usize), Error> {
        let source = self
            .devices
            .iter()
            .position(|device| *device == route.source_device)
            .ok_or(Error::Unknown)?;
        let destination = self
            .devices
            .iter()
            .position(|device| *device == route.destination_device)
            .ok_or(Error::Unknown)?;
        if route.stream == 0
            || route.source_device == 0
            || route.destination_device == 0
            || source == destination
            || route.source.allocation == 0
            || route.destination.allocation == 0
            || route.source.allocation == route.destination.allocation
        {
            return Err(Error::Invalid);
        }
        Ok((source, destination))
    }

    fn route(
        &self,
        route: BackendDirectedPeerRouteV1,
        source_extent: u64,
        destination_extent: u64,
        live: bool,
    ) -> Result<usize, Error> {
        let (source, destination) = self.devices(route)?;
        if live {
            let source_record = self
                .allocations
                .get(&route.source.allocation)
                .ok_or(Error::Unknown)?;
            let destination_record = self
                .allocations
                .get(&route.destination.allocation)
                .ok_or(Error::Unknown)?;
            let stream_device = self.streams.get(&route.stream).ok_or(Error::Unknown)?;
            if *stream_device != destination
                || source_record.device != source
                || destination_record.device != destination
                || source_record.byte_len != source_extent
                || destination_record.byte_len != destination_extent
            {
                return Err(Error::Invalid);
            }
        }
        admit_xgmi_peer_copy_v1(XgmiPeerCopyAdmissionV1 {
            stream_device: destination,
            source_device: source,
            destination_device: destination,
            source_offset: route.source.byte_offset,
            source_len: route.source.byte_len,
            source_allocation_len: source_extent,
            source_access: route.source.access,
            destination_offset: route.destination.byte_offset,
            destination_len: route.destination.byte_len,
            destination_allocation_len: destination_extent,
            destination_access: route.destination.access,
        })
        .map_err(|_| Error::Invalid)
    }

    fn retained(&self, id: u64) -> Result<&Root, Error> {
        let active = self.active.get(&id);
        let completed = self.completed.get(&id);
        if active.is_some() && completed.is_some() {
            return Err(Error::Corrupt);
        }
        let Some(root) = self.roots.get(&id) else {
            return Err(if active.is_some() || completed.is_some() {
                Error::Unsupported
            } else {
                Error::Unknown
            });
        };
        if id == 0 || !root.admitted {
            return Err(Error::Corrupt);
        }
        validate_roster(&root.dependencies, id).map_err(|_| Error::Corrupt)?;
        if active.is_some() == completed.is_some() {
            return Err(Error::Corrupt);
        }
        let direction = self
            .route(
                root.route,
                root.source_extent,
                root.destination_extent,
                active.is_some(),
            )
            .map_err(|_| Error::Corrupt)?;
        if let Some(active) = active
            && (active.id != id
                || active.stream != root.route.stream
                || active.direction != direction
                || active.source != root.route.source.allocation
                || active.destination != root.route.destination.allocation
                || active.source_offset != root.route.source.byte_offset
                || active.destination_offset != root.route.destination.byte_offset
                || u64::from(active.byte_len) != root.route.source.byte_len
                || active.sequence.is_some()
                || !active.dependencies.iter().copied().eq(root
                    .dependencies
                    .iter()
                    .map(|entry| entry.producer_submission)))
        {
            return Err(Error::Corrupt);
        }
        if let Some(completed) = completed
            && (completed.stream != root.route.stream || completed.status == BackendPollV1::Pending)
        {
            return Err(Error::Corrupt);
        }
        Ok(root)
    }

    fn prepare(
        &self,
        id: u64,
        request: BackendDirectedScalarPeerCopyV1<'_>,
    ) -> Result<Root, Error> {
        if id == 0
            || self.roots.contains_key(&id)
            || self.active.contains_key(&id)
            || self.completed.contains_key(&id)
        {
            return Err(Error::Corrupt);
        }
        validate_roster(request.dependencies, id)?;
        let source_extent = self
            .allocations
            .get(&request.route.source.allocation)
            .ok_or(Error::Unknown)?
            .byte_len;
        let destination_extent = self
            .allocations
            .get(&request.route.destination.allocation)
            .ok_or(Error::Unknown)?
            .byte_len;
        self.route(request.route, source_extent, destination_extent, true)?;
        for dependency in request.dependencies {
            let producer = self.events.get(&dependency.event).ok_or(Error::Unknown)?;
            if producer.submission != dependency.producer_submission {
                return Err(Error::Invalid);
            }
            // A completed legacy record does not distinguish scalar from ordered.
            // Absence from this profile is unsupported, never implicit authority.
            self.retained(dependency.producer_submission)
                .map_err(|error| {
                    if error == Error::Unknown {
                        Error::Corrupt
                    } else {
                        error
                    }
                })?;
        }
        if self.roots.len() >= MAX_RUNTIME_SUBMISSIONS_V1 {
            return Err(Error::Capacity);
        }
        let mut dependencies = Vec::new();
        dependencies
            .try_reserve_exact(request.dependencies.len())
            .map_err(|_| Error::Capacity)?;
        dependencies.extend_from_slice(request.dependencies);
        Ok(Root {
            route: request.route,
            source_extent,
            destination_extent,
            dependencies,
            admitted: false,
        })
    }

    fn matches(&self, request: BackendDirectedScalarProgressV1<'_>) -> Result<(), Error> {
        let root = self.retained(request.submission)?;
        if root.route != request.route
            || !root
                .dependencies
                .iter()
                .map(|entry| entry.producer_submission)
                .eq(request.producer_submissions.iter().copied())
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}

fn validate_roster(dependencies: &[BackendDirectedPeerDependencyV1], id: u64) -> Result<(), Error> {
    if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
        return Err(Error::Capacity);
    }
    let mut sorted = [0; MAX_RUNTIME_DEPENDENCIES_V1];
    for (index, dependency) in dependencies.iter().enumerate() {
        if dependency.event == 0
            || dependency.producer_submission == 0
            || dependency.producer_submission >= id
        {
            return Err(Error::Invalid);
        }
        sorted[index] = dependency.producer_submission;
    }
    let sorted = &mut sorted[..dependencies.len()];
    sorted.sort_unstable();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::Invalid);
    }
    Ok(())
}

// The native leaves alone are replaceable in CPU fixtures. Admission, provisional
// rooting, exact progress matching and failure retention use this same driver.
trait Driver {
    fn require_live(&self) -> Result<(), Failure>;
    fn view(&self) -> View<'_>;
    fn roots_mut(&mut self) -> &mut HashMap<u64, Root>;
    fn next_handle(&self) -> u64;
    fn seal(&mut self);
    fn submit_scalar(
        &mut self,
        route: BackendDirectedPeerRouteV1,
        events: &[u64],
    ) -> Result<u64, Failure>;
    fn progress_scalar(&mut self, submission: u64) -> Result<BackendPollV1, Failure>;
    fn retained_for_release(&self, submission: u64) -> bool;
    fn depth_retained(&self, submission: u64) -> bool;
    fn remove_completion(&mut self, submission: u64);
}

fn failure(driver: &mut impl Driver, error: Error) -> Failure {
    let (kind, detail) = match error {
        Error::Unknown => (
            KfdRuntimeBackendErrorKindV1::UnknownHandle,
            "unknown directed scalar resource",
        ),
        Error::Unsupported => (
            KfdRuntimeBackendErrorKindV1::Unsupported,
            "submission is not in the directed scalar profile",
        ),
        Error::Invalid => (
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
            "directed scalar route or producer roster mismatch",
        ),
        Error::Capacity => (
            KfdRuntimeBackendErrorKindV1::Capacity,
            "directed scalar custody capacity",
        ),
        Error::Corrupt => {
            driver.seal();
            return RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Terminal,
                "directed scalar retained custody is inconsistent",
            ));
        }
    };
    KfdNativeXgmiRuntimeBackendV1::rejected(kind, detail)
}

fn submit(
    driver: &mut impl Driver,
    request: BackendDirectedScalarPeerCopyV1<'_>,
) -> Result<u64, Failure> {
    driver.require_live()?;
    let expected = driver.next_handle();
    let root = driver
        .view()
        .prepare(expected, request)
        .map_err(|error| failure(driver, error))?;
    driver
        .roots_mut()
        .try_reserve(1)
        .map_err(|_| failure(driver, Error::Capacity))?;
    let mut events = [0; MAX_RUNTIME_DEPENDENCIES_V1];
    for (slot, dependency) in events.iter_mut().zip(request.dependencies) {
        *slot = dependency.event;
    }
    driver.roots_mut().insert(expected, root);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        driver.submit_scalar(request.route, &events[..request.dependencies.len()])
    }));
    match outcome {
        Ok(Ok(id)) => {
            if id != expected {
                return Err(failure(driver, Error::Corrupt));
            }
            let Some(root) = driver.roots_mut().get_mut(&expected) else {
                return Err(failure(driver, Error::Corrupt));
            };
            root.admitted = true;
            driver
                .view()
                .retained(id)
                .map(|_| ())
                .map_err(|error| failure(driver, error))?;
            Ok(id)
        }
        Ok(Err(error @ RuntimeBackendFailureV1::Rejected(_))) => {
            // The shared scalar admission never acquires custody on Rejected.
            // Keep the original error, with all pre-existing roots untouched.
            driver.roots_mut().remove(&expected);
            Err(error)
        }
        Ok(Err(
            RuntimeBackendFailureV1::Quiescent(error) | RuntimeBackendFailureV1::Terminal(error),
        )) => {
            driver.seal();
            // Shared scalar admission has no quiescent-error return today. If
            // that changes, an unreturned handle needs an explicit settlement
            // protocol; quiescence alone cannot discard this provisional root.
            Err(RuntimeBackendFailureV1::Terminal(error))
        }
        Err(payload) => {
            driver.seal();
            resume_unwind(payload)
        }
    }
}

fn progress(
    driver: &mut impl Driver,
    request: BackendDirectedScalarProgressV1<'_>,
) -> Result<BackendPollV1, Failure> {
    driver.require_live()?;
    driver
        .view()
        .matches(request)
        .map_err(|error| failure(driver, error))?;
    let result = catch_unwind(AssertUnwindSafe(|| {
        driver.progress_scalar(request.submission)
    }));
    match result {
        Ok(Err(
            RuntimeBackendFailureV1::Quiescent(error) | RuntimeBackendFailureV1::Terminal(error),
        )) => {
            // Neither native scalar leaf has a quiescent-error path. An
            // unexpected one is not an authenticated completion of this root.
            driver.seal();
            Err(RuntimeBackendFailureV1::Terminal(error))
        }
        Ok(result) => result,
        Err(payload) => {
            driver.seal();
            resume_unwind(payload)
        }
    }
}

pub(super) fn release_submission(
    backend: &mut KfdNativeXgmiRuntimeBackendV1,
    id: u64,
) -> Result<(), Failure> {
    release(backend, id)
}

fn release(driver: &mut impl Driver, id: u64) -> Result<(), Failure> {
    driver.require_live()?;
    if driver.view().active.contains_key(&id) || driver.retained_for_release(id) {
        return Err(KfdNativeXgmiRuntimeBackendV1::rejected(
            KfdRuntimeBackendErrorKindV1::Busy,
            "XGMI submission remains retained",
        ));
    }
    if !driver.view().completed.contains_key(&id) {
        return Err(KfdNativeXgmiRuntimeBackendV1::rejected(
            KfdRuntimeBackendErrorKindV1::UnknownHandle,
            "unknown XGMI submission",
        ));
    }
    if !driver.depth_retained(id) {
        driver.seal();
        return Err(RuntimeBackendFailureV1::Terminal(
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Terminal,
                "XGMI submission lost dependency-depth custody",
            ),
        ));
    }
    if driver.view().roots.contains_key(&id) {
        driver
            .view()
            .retained(id)
            .map(|_| ())
            .map_err(|error| failure(driver, error))?;
    }
    driver.remove_completion(id);
    Ok(())
}

fn remove_completion(
    completed: &mut HashMap<u64, SubmissionRecordV1>,
    depths: &mut HashMap<u64, usize>,
    roots: &mut HashMap<u64, Root>,
    id: u64,
) {
    completed.remove(&id);
    depths.remove(&id);
    roots.remove(&id);
}

impl Driver for KfdNativeXgmiRuntimeBackendV1 {
    fn require_live(&self) -> Result<(), Failure> {
        self.require_live()
    }

    fn view(&self) -> View<'_> {
        View {
            devices: self
                .descriptions
                .each_ref()
                .map(|device| device.backend_device),
            streams: &self.streams,
            allocations: &self.allocations,
            events: &self.events,
            active: &self.active,
            completed: &self.submissions,
            roots: &self.directed_roots,
        }
    }

    fn roots_mut(&mut self) -> &mut HashMap<u64, Root> {
        &mut self.directed_roots
    }

    fn next_handle(&self) -> u64 {
        self.next_handle
    }

    fn seal(&mut self) {
        self.terminal = true;
    }

    fn submit_scalar(
        &mut self,
        route: BackendDirectedPeerRouteV1,
        events: &[u64],
    ) -> Result<u64, Failure> {
        self.peer_copy_v1(route.stream, route.source, route.destination, events)
    }

    fn progress_scalar(&mut self, submission: u64) -> Result<BackendPollV1, Failure> {
        self.progress_peer_copy_v1(submission)
    }

    fn retained_for_release(&self, submission: u64) -> bool {
        self.event_submission_retain_counts
            .contains_key(&submission)
            || self.dependency_retain_counts.contains_key(&submission)
    }

    fn depth_retained(&self, submission: u64) -> bool {
        self.dependency_depths.contains_key(&submission)
    }

    fn remove_completion(&mut self, submission: u64) {
        remove_completion(
            &mut self.submissions,
            &mut self.dependency_depths,
            &mut self.directed_roots,
            submission,
        );
    }
}

impl RuntimeDirectedScalarPeerCopyBackendV1 for KfdNativeXgmiRuntimeBackendV1 {
    fn submit_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarPeerCopyV1<'_>,
    ) -> Result<u64, Failure> {
        submit(self, request)
    }

    /// One existing scalar quantum: at most 256 levels, 256 dependencies per
    /// level and one publication of at most 63 copies, observation or failure.
    fn progress_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarProgressV1<'_>,
    ) -> Result<BackendPollV1, Failure> {
        progress(self, request)
    }
}
