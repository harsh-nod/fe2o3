//! Additive, exact-route scalar peer-copy backend contract.

use super::{BackendMemoryRegionV1, BackendPollV1, RuntimeBackendFailureV1, RuntimeBackendV1};

/// The complete directed route of one scalar peer copy, using backend handles.
///
/// These fields are untrusted requests, not execution authority. The backend
/// validates the device/allocation bindings and destination-owned stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendDirectedPeerRouteV1 {
    pub stream: u64,
    pub source_device: u64,
    pub destination_device: u64,
    pub source: BackendMemoryRegionV1,
    pub destination: BackendMemoryRegionV1,
}

/// An event and its exact expected producer, in caller-supplied dependency order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendDirectedPeerDependencyV1 {
    pub event: u64,
    pub producer_submission: u64,
}

/// Submission request for the success-gated directed scalar profile.
#[derive(Clone, Copy, Debug)]
pub struct BackendDirectedScalarPeerCopyV1<'a> {
    pub route: BackendDirectedPeerRouteV1,
    pub dependencies: &'a [BackendDirectedPeerDependencyV1],
}

/// Exact retained operation identity for one bounded progress quantum.
///
/// Events need not remain live after admission. The producer roster must still
/// match the original order exactly, even after a producer is released.
#[derive(Clone, Copy, Debug)]
pub struct BackendDirectedScalarProgressV1<'a> {
    pub submission: u64,
    pub route: BackendDirectedPeerRouteV1,
    pub producer_submissions: &'a [u64],
}

/// Optional success-gated scalar peer-copy SPI with exact directed provenance.
///
/// Admission authenticates every event-to-producer binding, rejects duplicate
/// producers (including event aliases), and retains the route and ordered
/// producer roster until successful submission release. Every dependency must
/// have been admitted through this same profile. The backend may publish the
/// consumer only after every producer has conclusively succeeded; failure,
/// cancellation and mere quiescence are not successful inputs. It retains all
/// possibly reachable resources under the ordinary backend failure contract.
///
/// Progress validates the complete retained identity before native action and
/// performs at most one bounded publication, observation or failed-dependent
/// settlement. It never waits, sleeps, recursively polls or spawns a task. A
/// backend must document its finite selection and publication bounds; native
/// calls are not thereby given a hard wall-clock bound. A terminal result
/// describes only the requested submission, never a dependency or FIFO blocker.
///
/// Ordinary progress APIs may also advance these operations. Native success
/// gating is not host-journal reconciliation: a Context adapter must reconcile
/// retained producer outcomes before committing a consumer's logical version.
/// This SPI alone does not enable Context pending reads, refine a formal model,
/// extend ordered copies, or change any Runtime Worker wire contract. There is
/// deliberately no blanket implementation or inferred capability-bit opt-in.
pub trait RuntimeDirectedScalarPeerCopyBackendV1: RuntimeBackendV1 {
    fn submit_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarPeerCopyV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;

    fn progress_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarProgressV1<'_>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>>;
}
