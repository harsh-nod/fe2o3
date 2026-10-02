//! Named XGMI queue operations over two existing compute-owned VMs.

use super::model_pair_loan::{self, Failure};
use super::*;
use crate::sdma::{Gfx942NativeXgmiSdmaQueueCreationRootV1, Gfx942NativeXgmiSdmaQueueV1};
use crate::topology::Gfx942XgmiRouteV1;

#[path = "compute_xgmi/persistent.rs"]
mod persistent;
#[path = "compute_xgmi/transfer.rs"]
mod transfer;

/// Caller-owned custody spanning native creation and both model retakes.
///
/// Terminal custody grants no retry or cleanup authority. Keep this root and
/// both compute sessions alive until process teardown when it is occupied.
#[must_use = "occupied compute-XGMI creation custody requires process teardown"]
pub struct Gfx942ComputeXgmiQueueCreationRootV1 {
    armed: bool,
    native: Gfx942NativeXgmiSdmaQueueCreationRootV1,
    returned: Option<Gfx942NativeXgmiSdmaQueueV1>,
}

impl Gfx942ComputeXgmiQueueCreationRootV1 {
    pub const fn new() -> Self {
        Self {
            armed: false,
            native: Gfx942NativeXgmiSdmaQueueCreationRootV1::new(),
            returned: None,
        }
    }

    pub const fn is_vacant(&self) -> bool {
        !self.armed && self.native.is_vacant() && self.returned.is_none()
    }
}

impl Default for Gfx942ComputeXgmiQueueCreationRootV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Gfx942ComputeXgmiQueueCreationRootV1 {
    fn drop(&mut self) {
        if !self.is_vacant() {
            std::process::abort();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Attachment {
    source: QueueKeyV1,
    destination: QueueKeyV1,
    route: Gfx942XgmiRouteV1,
    native_queue_id: u32,
}

/// A directional peer queue attached to two existing compute-owned VMs.
///
/// Each compute owner retains an exact private attachment certificate. It
/// cannot be destroyed or acquire another peer queue until explicit retirement.
/// Recycled PUBLIC DATA retains its synchronous full-extent adapter. Persistent
/// PUBLIC owners also support begin, one-shot sample, and finish without a GPU
/// completion wait. Native syscalls remain synchronous and both endpoints must
/// be quiescent for each operation; this grants no general concurrent-compute
/// guarantee. Any indeterminate transfer retains its data in this owner.
#[must_use = "the peer queue must be explicitly destroyed before releasing either compute session"]
pub struct Gfx942ComputeXgmiQueueV1 {
    attachment: Attachment,
    queue: Gfx942NativeXgmiSdmaQueueV1,
    transfer: Option<transfer::TransferRoot>,
    persistent_transfer: Option<Box<persistent::TransferRoot>>,
}

impl Drop for Gfx942ComputeXgmiQueueV1 {
    fn drop(&mut self) {
        if self.transfer.is_some() || self.persistent_transfer.is_some() {
            std::process::abort();
        }
    }
}

impl Gfx942ComputeXgmiQueueV1 {
    pub const fn route(&self) -> Gfx942XgmiRouteV1 {
        self.queue.route()
    }

    pub fn observation(&self) -> Option<crate::sdma::Gfx942SdmaQueueObservationV1> {
        self.queue.observation()
    }

    /// Audit-only occupancy, including pending, ready, and terminal transfers.
    /// This does not grant completion, cancellation, or resource-release authority.
    pub const fn has_persistent_data_transfer_v1(&self) -> bool {
        self.persistent_transfer.is_some()
    }
}

struct Sessions<'a> {
    source: &'a mut ComputeAqlQueueSessionV1,
    destination: &'a mut ComputeAqlQueueSessionV1,
}

impl Sessions<'_> {
    fn endpoint(&mut self, endpoint: usize) -> &mut ComputeAqlQueueSessionV1 {
        match endpoint {
            0 => self.source,
            1 => self.destination,
            _ => unreachable!("exact two-session loan"),
        }
    }

    fn memories(&mut self) -> (&mut SharedGttMemorySessionV1, &mut SharedGttMemorySessionV1) {
        (
            &mut self
                .source
                .engine
                .as_mut()
                .expect("preflighted source")
                .backend
                .session,
            &mut self
                .destination
                .engine
                .as_mut()
                .expect("preflighted destination")
                .backend
                .session,
        )
    }
}

impl model_pair_loan::Context for Sessions<'_> {
    type Loan = LiveQueueModelFoundationLoanV1;
    type Error = ComputeAqlQueueSessionErrorV1;

    fn open(&mut self, endpoint: usize) -> Result<Self::Loan, Self::Error> {
        self.endpoint(endpoint)
            .restore_model_ownership_for_live_mutation()
    }

    fn retake(&mut self, endpoint: usize, loan: Self::Loan) -> Result<(), Self::Error> {
        self.endpoint(endpoint)
            .retake_model_ownership_after_live_mutation(loan)
    }

    fn poison_endpoint(&mut self, endpoint: usize) {
        self.endpoint(endpoint).poison_terminal();
    }

    fn poison_process(&mut self) {
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}

fn route_matches(source: u32, destination: u32, route: Gfx942XgmiRouteV1) -> bool {
    source != destination
        && source == route.source_gpu_id()
        && destination == route.destination_gpu_id()
}

fn attachment_matches(
    source: &ComputeAqlQueueSessionV1,
    destination: &ComputeAqlQueueSessionV1,
    attachment: Attachment,
) -> bool {
    source.xgmi_attachment == Some(attachment)
        && destination.xgmi_attachment == Some(attachment)
        && source.compute_lane_session == attachment.source
        && destination.compute_lane_session == attachment.destination
}

fn preflight(
    source: &ComputeAqlQueueSessionV1,
    destination: &ComputeAqlQueueSessionV1,
    route: Gfx942XgmiRouteV1,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    let source_memory = &source
        .engine
        .as_ref()
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "missing XGMI source compute engine",
        ))?
        .backend
        .session;
    let destination_memory = &destination
        .engine
        .as_ref()
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "missing XGMI destination compute engine",
        ))?
        .backend
        .session;
    if source.key.vm == destination.key.vm
        || !route_matches(source_memory.gpu_id(), destination_memory.gpu_id(), route)
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute-XGMI route mismatch",
        ));
    }
    source.require_compute_xgmi_endpoint_v1()?;
    destination.require_compute_xgmi_endpoint_v1()
}

fn session_error(failure: Failure<ComputeAqlQueueSessionErrorV1>) -> ComputeAqlQueueSessionErrorV1 {
    if failure.terminal {
        terminal_creation("compute-XGMI paired model custody", failure.error)
    } else {
        failure.error
    }
}

impl ComputeAqlQueueSessionV1 {
    pub(super) fn require_no_xgmi_attachment_v1(
        &self,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.xgmi_attachment.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "native XGMI attachment must be retired first",
            ));
        }
        Ok(())
    }

    fn require_compute_xgmi_endpoint_v1(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_no_sdma_recycle_v1()?;
        self.require_no_sdma_owner_transition_v1()?;
        let engine = self
            .engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing compute-XGMI endpoint engine",
            ))?;
        if !retained_device_queue_is_active_v1(
            self.terminal_poisoned,
            engine.authority_poisoned,
            engine.phase(self.key),
        ) || !engine.backend.foundation_in_engine
            || !self.unpublished_dispatch.is_clear()
            || self.has_any_persistent_compute_attachment_v1()
            || self.sdma_allocation.is_some()
            || self.sdma_pool_trim.is_some()
            || self.auxiliary_release.is_some()
            || !auxiliary_compute_lanes_are_quiescent_v1(&self.auxiliary_compute_lanes)
            || !auxiliary_compute_lane_quiescence_from_facts_v1(
                self.completion_owner.ensure_releasable().is_ok(),
                self.dispatch
                    .as_ref()
                    .map(|dispatch| dispatch.ensure_releasable().is_ok()),
                self.detached_data_count,
                self.detached_dispatch_generation,
                self.detached_data_identities.len(),
                self.detached_next_insertion_index,
            )
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "compute-XGMI endpoint must be active and quiescent",
            ));
        }
        self.dependency_owner
            .ensure_idle()
            .map_err(map_dependency_target_use_error_v1)?;
        for owner in self.sdma.iter().chain(self.striped_sdma.iter()) {
            owner.preflight_retained_sdma_release_v1(self.key, self.observation.queue_id)?;
        }
        Ok(())
    }

    fn compute_xgmi_queue_id_collides_v1(&self, candidate: u32) -> bool {
        queue_id_collides_with_session_owned_roster_v1(
            candidate,
            self.observation.queue_id,
            self.auxiliary_compute_lanes
                .iter()
                .filter_map(|slot| slot.state.as_ref())
                .any(|lane| lane.observation.queue_id == candidate),
            self.sdma
                .iter()
                .chain(self.striped_sdma.iter())
                .any(|owner| owner.contains_confirmed_queue_id(candidate)),
        )
    }

    /// Creates a directional XGMI queue in these existing compute-owned VMs.
    ///
    /// Both compute and ordinary SDMA ledgers must be quiescent. This operation
    /// does not open another VM or grant peer-mapped compute-buffer authority.
    /// Both owners retain an exact attachment certificate until explicit peer
    /// retirement; ordinary compute operations remain available. Terminal errors
    /// leave `root` occupied; neither root nor sessions may then be released.
    pub fn create_native_xgmi_queue_with_peer_v1(
        &mut self,
        peer: &mut Self,
        route: Gfx942XgmiRouteV1,
        root: &mut Gfx942ComputeXgmiQueueCreationRootV1,
    ) -> Result<Gfx942ComputeXgmiQueueV1, ComputeAqlQueueSessionErrorV1> {
        if !root.is_vacant() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "compute-XGMI creation root is occupied",
            ));
        }
        self.require_no_xgmi_attachment_v1()?;
        peer.require_no_xgmi_attachment_v1()?;
        preflight(self, peer, route)?;
        root.armed = true;
        let result = model_pair_loan::execute(
            &mut Sessions {
                source: &mut *self,
                destination: &mut *peer,
            },
            |sessions| {
                let (source, destination) = sessions.memories();
                match Gfx942NativeXgmiSdmaQueueV1::create(
                    source,
                    destination,
                    route,
                    &mut root.native,
                ) {
                    Ok(queue) => root.returned = Some(queue),
                    Err(failure) => {
                        let terminal = failure.is_terminal();
                        return Err(Failure {
                            error: failure.into_error().into(),
                            terminal,
                        });
                    }
                }
                // Root the complete owner before inspecting outputs or retaking either foundation.
                let observation = root
                    .returned
                    .as_ref()
                    .and_then(Gfx942NativeXgmiSdmaQueueV1::observation);
                if observation.is_none_or(|observation| {
                    sessions
                        .source
                        .compute_xgmi_queue_id_collides_v1(observation.queue_id)
                        || sessions
                            .destination
                            .compute_xgmi_queue_id_collides_v1(observation.queue_id)
                }) {
                    return Err(Failure {
                        error: ComputeAqlQueueSessionErrorV1::Contract(
                            "compute-XGMI native queue ID collision",
                        ),
                        terminal: true,
                    });
                }
                Ok(())
            },
        );
        match result {
            Ok(()) => {
                let queue = root
                    .returned
                    .take()
                    .unwrap_or_else(|| std::process::abort());
                let attachment = Attachment {
                    source: self.compute_lane_session,
                    destination: peer.compute_lane_session,
                    route,
                    native_queue_id: queue
                        .observation()
                        .unwrap_or_else(|| std::process::abort())
                        .queue_id,
                };
                self.xgmi_attachment = Some(attachment);
                peer.xgmi_attachment = Some(attachment);
                root.armed = false;
                Ok(Gfx942ComputeXgmiQueueV1 {
                    attachment,
                    queue,
                    transfer: None,
                    persistent_transfer: None,
                })
            }
            Err(failure) => {
                if !failure.terminal {
                    root.armed = false;
                }
                Err(session_error(failure))
            }
        }
    }

    /// Retires an exact native peer attachment before either compute VM closes.
    /// Both certificates remain installed through every terminal failure.
    pub fn destroy_native_xgmi_queue_with_peer_v1(
        &mut self,
        peer: &mut Self,
        queue: &mut Gfx942ComputeXgmiQueueV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if queue.transfer.is_some()
            || queue.persistent_transfer.is_some()
            || !attachment_matches(self, peer, queue.attachment)
            || queue.queue.route() != queue.attachment.route
            || queue
                .queue
                .observation()
                .is_none_or(|observation| observation.queue_id != queue.attachment.native_queue_id)
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "compute-XGMI attachment mismatch",
            ));
        }
        preflight(self, peer, queue.attachment.route)?;
        model_pair_loan::execute(
            &mut Sessions {
                source: &mut *self,
                destination: &mut *peer,
            },
            |sessions| {
                let (source, destination) = sessions.memories();
                queue
                    .queue
                    .destroy_and_release(source, destination)
                    .map_err(|error| Failure {
                        error: error.into(),
                        terminal: queue.queue.has_terminal_retirement_v1(),
                    })
            },
        )
        .map_err(session_error)?;
        self.xgmi_attachment = None;
        peer.xgmi_attachment = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{persistent_compute_cancellation_test_session, test_queue_key};
    use super::*;

    fn attachment() -> Attachment {
        Attachment {
            source: test_queue_key(11, 1),
            destination: test_queue_key(12, 1),
            route: crate::topology::tests::admitted_xgmi_routes()[0],
            native_queue_id: 99,
        }
    }

    #[test]
    fn compute_xgmi_route_preflight_rejects_reversal_same_gpu_and_foreign_endpoints() {
        for route in crate::topology::tests::admitted_xgmi_routes() {
            let (source, destination) = (route.source_gpu_id(), route.destination_gpu_id());
            assert!(route_matches(source, destination, route));
            for (source, destination) in [
                (destination, source),
                (source, source),
                (0, destination),
                (source, 0),
            ] {
                assert!(!route_matches(source, destination, route));
            }
        }
    }

    #[test]
    fn compute_xgmi_exact_attachment_rejects_stale_foreign_and_partial_rosters() {
        let exact = attachment();
        for case in 0..7 {
            let mut source = persistent_compute_cancellation_test_session(exact.source, None, None);
            let mut destination =
                persistent_compute_cancellation_test_session(exact.destination, None, None);
            source.xgmi_attachment = Some(exact);
            destination.xgmi_attachment = Some(exact);
            match case {
                0 => {}
                1 => source.xgmi_attachment = None,
                2 => destination.xgmi_attachment = None,
                3 => source.compute_lane_session = test_queue_key(11, 2),
                4 => destination.compute_lane_session = test_queue_key(13, 1),
                5 => source.xgmi_attachment.as_mut().unwrap().native_queue_id += 1,
                6 => {
                    destination.xgmi_attachment.as_mut().unwrap().route =
                        crate::topology::tests::admitted_xgmi_routes()[1]
                }
                _ => unreachable!(),
            }
            assert_eq!(attachment_matches(&source, &destination, exact), case == 0);
            assert!(!source.terminal_poisoned && !destination.terminal_poisoned);
            source.xgmi_attachment = None;
            destination.xgmi_attachment = None;
        }
    }

    #[test]
    fn compute_xgmi_attachment_blocks_both_teardown_paths_and_recreation_before_engine_access() {
        let exact = attachment();
        let mut source = persistent_compute_cancellation_test_session(exact.source, None, None);
        let mut destination =
            persistent_compute_cancellation_test_session(exact.destination, None, None);
        source.xgmi_attachment = Some(exact);
        destination.xgmi_attachment = Some(exact);
        for endpoint in [&mut source, &mut destination] {
            assert!(matches!(
                endpoint.destroy_queue_and_event(QueueDestroyModeV1::Release),
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "native XGMI attachment must be retired first"
                ))
            ));
            assert!(matches!(
                endpoint.supports_retained_primary_release_v1(),
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "native XGMI attachment must be retired first"
                ))
            ));
            assert!(
                endpoint
                    .active_compute_queue_ids_for_sdma_creation_v1()
                    .is_err()
            );
            assert_eq!(endpoint.xgmi_attachment, Some(exact));
            assert!(!endpoint.terminal_poisoned);
        }
        let mut root = Gfx942ComputeXgmiQueueCreationRootV1::new();
        assert!(
            source
                .create_native_xgmi_queue_with_peer_v1(&mut destination, exact.route, &mut root)
                .is_err()
        );
        assert!(root.is_vacant());
        assert_eq!(source.xgmi_attachment, Some(exact));
        assert_eq!(destination.xgmi_attachment, Some(exact));
        source.xgmi_attachment = None;
        destination.xgmi_attachment = None;
    }

    #[test]
    fn compute_xgmi_creation_root_reentry_is_inert() {
        let exact = attachment();
        let mut source = persistent_compute_cancellation_test_session(exact.source, None, None);
        let mut destination =
            persistent_compute_cancellation_test_session(exact.destination, None, None);
        let mut root = Gfx942ComputeXgmiQueueCreationRootV1::new();
        root.armed = true;
        assert!(
            source
                .create_native_xgmi_queue_with_peer_v1(&mut destination, exact.route, &mut root)
                .is_err()
        );
        assert!(root.armed && root.native.is_vacant() && root.returned.is_none());
        assert!(!source.terminal_poisoned && !destination.terminal_poisoned);
        root.armed = false;
    }
}
