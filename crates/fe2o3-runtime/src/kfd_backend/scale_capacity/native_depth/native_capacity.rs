//! Probe the native epoch table independently of runtime allocation custody.

use super::*;
use fe2o3_kfd::{ComputeAqlQueueSessionErrorV1, Gfx942DispatchBindingErrorV1};

pub(super) fn probe(backend: &mut KfdRuntimeBackendV1, index: usize) -> bool {
    let lane = backend.native_compute_lanes[index].unwrap();
    let queue = backend.queue.as_mut().unwrap();
    let result = queue
        .with_compute_lane_v1(lane, |selected| {
            selected.submit_fixed_dispatch_classified_v1::<1>()
        })
        .unwrap();
    match result {
        Err(Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::DispatchEpochCapacity { maximum: 1024 },
            ),
        )) => true,
        Ok(batch) => {
            // An erroneous publication still owns a real native receipt. Drain
            // it independently, then fail qualification after ordinary cleanup.
            drain_unexpected(queue, lane, batch);
            false
        }
        Err(error) => {
            eprintln!("unexpected native capacity failure on lane {index}: {error:?}");
            false
        }
    }
}

// Recycle failures return original linear custody without another allocation.
#[allow(clippy::result_large_err)]
fn drain_unexpected(
    queue: &mut ComputeAqlQueueSessionV1,
    lane: ComputeAqlQueueLaneV1,
    batch: Gfx942DispatchBatchV1<1>,
) {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut batch = Some(batch);
    let mut completed = loop {
        assert!(
            Instant::now() < deadline,
            "unexpected native receipt did not complete"
        );
        match queue
            .with_compute_lane_v1(lane, |selected| {
                selected.poll_fixed_dispatch(batch.take().unwrap())
            })
            .unwrap()
            .unwrap()
        {
            Gfx942DispatchPollV1::Pending(pending) => batch = Some(pending),
            Gfx942DispatchPollV1::Ready(completed) => break Some(completed),
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    loop {
        assert!(
            Instant::now() < deadline,
            "unexpected native receipt did not recycle"
        );
        match queue
            .with_compute_lane_v1(lane, |selected| {
                selected.recycle_fixed_dispatch(completed.take().unwrap())
            })
            .unwrap()
        {
            Ok(_) => return,
            Err(failure) => {
                let (error, retained) = failure.into_parts();
                completed = Some(retained.expect("terminal unexpected native receipt recycle"));
                assert!(
                    Instant::now() < deadline,
                    "unexpected receipt recycle: {error:?}"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}
