//! Builds KFD as a normal dependency, including real process-global poisoning.
#![cfg(all(
    feature = "cpu-runtime-fixtures",
    target_os = "linux",
    target_arch = "x86_64"
))]

use fe2o3_kfd::{
    ComputeAqlQueueSessionErrorV1, CpuFixedDispatchFixtureV1 as Fixture,
    CpuFixedDispatchLaneV1 as Lane, Gfx942CompletedDispatchBatchV1, Gfx942CompletionErrorV1,
    Gfx942DispatchBatchV1, Gfx942DispatchPollV1, Gfx942FixedDispatchSubmissionFailureV1,
};

fn complete(
    lane: &mut Lane<'_>,
    batch: Gfx942DispatchBatchV1<1>,
) -> Gfx942CompletedDispatchBatchV1<1> {
    lane.complete_signal(&batch).unwrap();
    match lane.poll(batch).unwrap() {
        Gfx942DispatchPollV1::Ready(completed) => completed,
        Gfx942DispatchPollV1::Pending(_) => panic!("completed CPU signal must be observed"),
    }
}

#[test]
fn cpu_fixture_pending_ready_recycle_and_clean_refusal() {
    for auxiliary in [false, true] {
        let mut fixture = Fixture::new().unwrap();
        fixture.ensure_clean().unwrap();
        let lane = if auxiliary {
            fixture.auxiliary_lane()
        } else {
            fixture.primary_lane()
        };
        let untouched = fixture
            .snapshots()
            .into_iter()
            .nth(usize::from(!auxiliary))
            .unwrap();
        let batch = fixture
            .with_lane(lane, |lane| {
                let available = lane.available_signals();
                let batch = lane.submit().unwrap();
                let identity = lane.identity(&batch).unwrap();
                let batch = match lane.poll(batch).unwrap() {
                    Gfx942DispatchPollV1::Pending(batch) => batch,
                    Gfx942DispatchPollV1::Ready(_) => {
                        panic!("unobserved CPU signal must remain pending")
                    }
                };
                assert_eq!(lane.identity(&batch).unwrap(), identity);
                assert_eq!(lane.live_epochs(), 1);
                assert_eq!(lane.available_signals(), available - 1);
                assert_eq!((lane.observed_signals(), lane.reset_signals()), (1, 0));
                batch
            })
            .unwrap();
        assert!(fixture.ensure_clean().is_err());
        let completed = fixture
            .with_lane(lane, |lane| complete(lane, batch))
            .unwrap();
        assert!(fixture.ensure_clean().is_err());
        fixture
            .with_lane(lane, |lane| {
                assert_eq!(lane.live_epochs(), 1);
                assert_eq!(lane.recycle(completed).unwrap().packet_count(), 1);
                assert_eq!(lane.live_epochs(), 0);
                assert_eq!((lane.observed_signals(), lane.reset_signals()), (2, 1));
            })
            .unwrap();
        assert_eq!(fixture.snapshots()[usize::from(!auxiliary)], untouched);
        fixture.ensure_clean().unwrap();
    }
}

#[test]
fn cpu_fixture_out_of_order_receipts_and_republication_with_live_neighbors() {
    for auxiliary in [false, true] {
        let mut fixture = Fixture::new().unwrap();
        let lane = if auxiliary {
            fixture.auxiliary_lane()
        } else {
            fixture.primary_lane()
        };
        let untouched = fixture
            .snapshots()
            .into_iter()
            .nth(usize::from(!auxiliary))
            .unwrap();
        fixture
            .with_lane(lane, |lane| {
                let capacity = lane.available_signals();
                let a = lane.submit().unwrap();
                let b = lane.submit().unwrap();
                let c = lane.submit().unwrap();
                let ids = [&a, &b, &c].map(|batch| lane.identity(batch).unwrap());
                assert_ne!(ids[0], ids[1]);
                assert_ne!(ids[0], ids[2]);
                assert_ne!(ids[1], ids[2]);
                let completed = complete(lane, b);
                lane.recycle(completed).unwrap();
                let reused = lane.submit().unwrap();
                assert!(
                    ids.iter()
                        .all(|old| *old != lane.identity(&reused).unwrap())
                );
                assert_eq!(lane.identity(&a).unwrap(), ids[0]);
                assert_eq!(lane.identity(&c).unwrap(), ids[2]);
                for (index, batch) in [c, reused, a].into_iter().enumerate() {
                    assert_eq!(lane.live_epochs(), 3 - index);
                    let completed = complete(lane, batch);
                    lane.recycle(completed).unwrap();
                    assert_eq!(lane.available_signals(), capacity - 2 + index);
                }
                assert_eq!(lane.next_generation(), 5);
            })
            .unwrap();
        assert_eq!(fixture.snapshots()[usize::from(!auxiliary)], untouched);
        fixture.ensure_clean().unwrap();
    }
}

#[test]
fn cpu_fixture_foreign_lane_and_receipt_reject_without_mutation() {
    let mut first = Fixture::new().unwrap();
    let mut second = Fixture::new().unwrap();
    let a = first.auxiliary_lane();
    let b = second.auxiliary_lane();
    let batch = first.with_lane(a, |lane| lane.submit().unwrap()).unwrap();
    let before_a = first.snapshots();
    let before_b = second.snapshots();
    assert!(
        second
            .with_lane(a, |_| panic!("foreign lane entered callback"))
            .is_err()
    );
    second
        .with_lane(b, |lane| {
            assert!(lane.identity(&batch).is_err());
            assert!(lane.complete_signal(&batch).is_err());
        })
        .unwrap();
    assert_eq!(first.snapshots(), before_a);
    assert_eq!(second.snapshots(), before_b);
    first
        .with_lane(a, |lane| {
            let completed = complete(lane, batch);
            lane.recycle(completed).unwrap();
        })
        .unwrap();
    first.ensure_clean().unwrap();
    second.ensure_clean().unwrap();
}

#[test]
fn cpu_fixture_signal_capacity_retry_then_genuine_drain_and_reuse() {
    for auxiliary in [false, true] {
        let mut fixture = Fixture::new().unwrap();
        let untouched = fixture
            .snapshots()
            .into_iter()
            .nth(usize::from(!auxiliary))
            .unwrap();
        let lane = if auxiliary {
            fixture.auxiliary_lane()
        } else {
            fixture.primary_lane()
        };
        fixture
            .with_lane(lane, |lane| lane.saturate_signals().unwrap())
            .unwrap();
        assert!(fixture.ensure_clean().is_err());
        fixture
            .with_lane(lane, |lane| {
                assert_eq!(lane.available_signals(), 0);
                for _ in 0..3 {
                    assert!(matches!(
                        lane.submit(),
                        Err(Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_))
                    ));
                    assert_eq!(lane.live_epochs(), 0);
                    assert_eq!(lane.available_signals(), 0);
                    assert_eq!((lane.observed_signals(), lane.reset_signals()), (0, 0));
                }
                assert_eq!(lane.next_generation(), 4);
                lane.drain_saturation().unwrap();
                let capacity = lane.available_signals();
                assert_eq!(capacity, 8192);
                assert_eq!(
                    (lane.observed_signals(), lane.reset_signals()),
                    (capacity, capacity)
                );
                let batch = lane.submit().unwrap();
                let completed = complete(lane, batch);
                lane.recycle(completed).unwrap();
                assert_eq!(lane.next_generation(), 5);
            })
            .unwrap();
        assert_eq!(fixture.snapshots()[usize::from(!auxiliary)], untouched);
        fixture.ensure_clean().unwrap();
    }
}

#[test]
fn cpu_fixture_pin_refusal_retains_completed_receipt_until_release() {
    for auxiliary in [false, true] {
        let mut fixture = Fixture::new().unwrap();
        let untouched = fixture
            .snapshots()
            .into_iter()
            .nth(usize::from(!auxiliary))
            .unwrap();
        let lane = if auxiliary {
            fixture.auxiliary_lane()
        } else {
            fixture.primary_lane()
        };
        fixture
            .with_lane(lane, |lane| {
                let capacity = lane.available_signals();
                let batch = lane.submit_pinned().unwrap();
                let identity = lane.identity(&batch).unwrap();
                let completed = complete(lane, batch);
                assert!(identity.matches_completed(&completed));
                let before = lane.snapshot();
                let failure = lane.recycle(completed).unwrap_err();
                let (error, completed) = failure.into_parts();
                assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::Completion(
                        Gfx942CompletionErrorV1::SignalPinned {
                            event_pins: 1,
                            native_reader_pins: 0,
                            ..
                        }
                    )
                ));
                let completed = completed.expect("pin retry must retain the completed receipt");
                assert!(identity.matches_completed(&completed));
                assert_eq!(lane.snapshot(), before);
                assert_eq!(lane.live_epochs(), 1);
                assert_eq!(lane.available_signals(), capacity - 1);
                assert_eq!(lane.reset_signals(), 0);
                lane.release_pin(identity).unwrap();
                assert!(lane.release_pin(identity).is_err());
                lane.recycle(completed).unwrap();
                assert_eq!(lane.live_epochs(), 0);
                assert_eq!(lane.reset_signals(), 1);
            })
            .unwrap();
        assert_eq!(fixture.snapshots()[usize::from(!auxiliary)], untouched);
        fixture.ensure_clean().unwrap();
    }
}

#[test]
fn cpu_fixture_outer_unwind_keeps_deposited_receipt_and_poison() {
    const CHILD: &str = "FE2O3_CPU_FIXTURE_UNWIND_CHILD";
    if let Some(auxiliary) = std::env::var_os(CHILD) {
        let mut fixture = Fixture::new().unwrap();
        let index = usize::from(auxiliary == "1");
        let before = fixture.snapshots();
        let lane = if auxiliary == "1" {
            fixture.auxiliary_lane()
        } else {
            fixture.primary_lane()
        };
        let mut cell = None;
        let mut identity = None;
        let mut deposited = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = fixture.with_lane(lane, |lane| {
                cell = Some(lane.submit().unwrap());
                identity = Some(lane.identity(cell.as_ref().unwrap()).unwrap());
                deposited = Some(lane.snapshot());
                panic!("outer callback after receipt deposit");
            });
        }));
        assert_eq!(
            result.unwrap_err().downcast_ref::<&str>(),
            Some(&"outer callback after receipt deposit")
        );
        assert!(identity.unwrap().matches_published(cell.as_ref().unwrap()));
        assert!(fixture.is_terminal());
        assert!(fixture.ensure_clean().is_err());
        let restored = fixture.snapshots();
        assert!(restored[index].same_custody(deposited.as_ref().unwrap()));
        assert!(restored[1 - index].same_custody(&before[1 - index]));
        assert_ne!(restored[index], *deposited.as_ref().unwrap());
        assert!(
            fixture
                .with_lane(lane, |_| panic!("terminal fixture callback"))
                .is_err()
        );
        // No fake retirement or in-process recovery after terminal custody.
        std::mem::forget(cell);
        std::mem::forget(fixture);
        println!("CPU fixture unwind child assertions complete");
        return;
    }
    for auxiliary in ["0", "1"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "cpu_fixture_outer_unwind_keeps_deposited_receipt_and_poison",
                "--nocapture",
            ])
            .env(CHILD, auxiliary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains("CPU fixture unwind child assertions complete")
        );
    }
}

#[test]
fn cpu_fixture_same_callback_terminal_reentry_cannot_mutate_owners() {
    const CHILD: &str = "FE2O3_CPU_FIXTURE_TERMINAL_CHILD";
    if let Some(auxiliary) = std::env::var_os(CHILD) {
        let mut fixture = Fixture::new().unwrap();
        let lane = if auxiliary == "1" {
            fixture.auxiliary_lane()
        } else {
            fixture.primary_lane()
        };
        fixture
            .with_lane(lane, |lane| {
                let mut receipts: Vec<_> = (0..64).map(|_| lane.submit().unwrap()).collect();
                let completed = complete(lane, receipts.pop().unwrap());
                let identity = lane.identity(&receipts[0]).unwrap();
                assert!(matches!(
                    lane.submit(),
                    Err(Gfx942FixedDispatchSubmissionFailureV1::Terminal(_))
                ));
                let before = lane.snapshot();
                assert!(lane.submit().is_err());
                assert!(lane.submit_pinned().is_err());
                assert!(lane.identity(&receipts[0]).is_err());
                assert!(lane.complete_signal(&receipts[0]).is_err());
                assert!(lane.release_pin(identity).is_err());
                assert!(lane.saturate_signals().is_err());
                assert!(lane.drain_saturation().is_err());
                assert!(lane.poll(receipts.pop().unwrap()).is_err());
                assert!(lane.recycle(completed).is_err());
                assert_eq!(lane.snapshot(), before);
                std::mem::forget(receipts);
            })
            .unwrap();
        assert!(fixture.is_terminal());
        assert!(fixture.ensure_clean().is_err());
        std::mem::forget(fixture);
        println!("CPU fixture terminal child assertions complete");
        return;
    }
    for auxiliary in ["0", "1"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "cpu_fixture_same_callback_terminal_reentry_cannot_mutate_owners",
                "--nocapture",
            ])
            .env(CHILD, auxiliary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains("CPU fixture terminal child assertions complete")
        );
    }
}
