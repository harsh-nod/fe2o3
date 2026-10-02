//! Real router ownership and range sequencing with scripted transport bytes.
//! These tests do not execute native DMA or establish hardware concurrency.

use super::*;
use crate::{
    BackendCancellationV1, BackendDirectedPeerDependencyV1, BackendDirectedPeerRouteV1,
    BackendDirectedScalarPeerCopyV1, RuntimeDirectedScalarPeerCopyBackendV1,
};

fn fixture(bytes: usize, failure: Option<Stage>, unwind: bool) -> Fixture {
    let mut f = Fixture::with_child_sizes(
        failure,
        unwind,
        2,
        &[bytes + 17, bytes + 43],
        vec![vec![], vec![]],
    );
    f.source.byte_offset = 5;
    f.destination.byte_offset = 19;
    f.source.byte_len = bytes as u64;
    f.destination.byte_len = bytes as u64;
    let KfdRuntimeSdmaStorageV1::Device(owner) = &mut f.record_mut(true).sdma_storage else {
        unreachable!()
    };
    for (i, byte) in owner.scripted_bytes_mut().unwrap().iter_mut().enumerate() {
        *byte = (i.wrapping_mul(37).wrapping_add(11) % 251) as u8;
    }
    f
}

fn drive(f: &mut Fixture, id: u64) -> BackendPollV1 {
    for _ in 0..64 {
        let status = f.backend.progress_cooperative_copy(id).unwrap();
        f.backend.assert_cooperative_indexes_consistent();
        if status != BackendPollV1::Pending {
            return status;
        }
    }
    panic!("bounded range copy did not settle")
}

#[test]
fn native_subranges_copy_independent_offsets_and_preserve_complete_original_owners() {
    for bytes in [1, 47, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize + 37] {
        let mut f = fixture(bytes, None, false);
        let source = f.owner(true).scripted_bytes().unwrap().to_vec();
        let mut expected = f.owner(false).scripted_bytes().unwrap().to_vec();
        expected[19..19 + bytes].copy_from_slice(&source[5..5 + bytes]);
        let identities = [
            f.owner(true).scripted_owner_id(),
            f.owner(false).scripted_owner_id(),
        ];
        assert!(!full_extent(f.record(true), f.source));
        assert!(!full_extent(f.record(false), f.destination));
        let id = f.submit(&[]);
        let window = f.root(id).window;
        assert_eq!(window.source_logical_bytes(), source.len() as u64);
        assert_eq!(window.destination_logical_bytes(), expected.len() as u64);
        assert_eq!(
            (
                window.source_offset(),
                window.destination_offset(),
                window.bytes()
            ),
            (5, 19, bytes as u64)
        );
        assert!(f.copy(id).staging.is_empty());
        assert_eq!(f.copy(id).scratch_byte_len, 0);
        for _ in 0..2 {
            assert_eq!(
                f.backend.progress_cooperative_copy(id).unwrap(),
                BackendPollV1::Pending
            );
        }
        assert_eq!(f.root(id).phase, Phase::Published);
        let trace = f.root(id).trace.clone();
        for _ in 0..3 {
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert_eq!(
                f.backend.wait_v1(id, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
            assert_eq!(
                f.backend.drain_v1(id, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
        }
        assert_eq!(f.root(id).trace, trace);
        assert_eq!(
            f.backend.cancel_v1(id).unwrap(),
            BackendCancellationV1::TooLate
        );
        assert_eq!(f.backend.compute_xgmi_children, [Some(id), Some(id)]);
        assert_eq!(drive(&mut f, id), BackendPollV1::Succeeded);
        assert_eq!(f.owner(true).scripted_bytes().unwrap(), source);
        assert_eq!(f.owner(false).scripted_bytes().unwrap(), expected);
        assert_eq!(
            [
                f.owner(true).scripted_owner_id(),
                f.owner(false).scripted_owner_id()
            ],
            identities
        );
        assert_eq!(f.root(id).window, window);
        for stage in [Stage::Finish, Stage::Retire, Stage::Restore] {
            assert_eq!(
                f.root(id)
                    .trace
                    .iter()
                    .filter(|entry| **entry == stage)
                    .count(),
                1
            );
        }
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        f.clean();
    }
}

#[test]
fn native_subranges_invalid_windows_are_rejected_before_handles_or_custody() {
    for invalid in 0..6 {
        let mut f = fixture(47, None, false);
        let identities = [
            f.owner(true).scripted_owner_id(),
            f.owner(false).scripted_owner_id(),
        ];
        match invalid {
            0 => {
                f.source.byte_len = 0;
                f.destination.byte_len = 0;
            }
            1 => f.source.byte_offset = u64::MAX,
            2 => f.destination.byte_offset = u64::MAX,
            3 => f.source.byte_offset = 18,
            4 => f.destination.byte_offset = 44,
            5 => f.destination.byte_len -= 1,
            _ => unreachable!(),
        }
        let next = f.backend.next_handle;
        assert!(matches!(
            f.backend
                .peer_copy_v1(f.stream, f.source, f.destination, &[]),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(f.backend.next_handle, next);
        assert!(f.backend.submissions.is_empty());
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        assert_eq!(
            [
                f.owner(true).scripted_owner_id(),
                f.owner(false).scripted_owner_id()
            ],
            identities
        );
        f.backend.assert_cooperative_indexes_consistent();
        f.clean();
    }
}

#[test]
fn native_subranges_retained_window_drift_poisons_both_before_further_effects() {
    for published in [false, true] {
        for corruption in 0..3 {
            let mut f = fixture(47, None, false);
            let id = f.submit(&[]);
            assert_eq!(
                f.backend.progress_cooperative_copy(id).unwrap(),
                BackendPollV1::Pending
            );
            if published {
                assert_eq!(
                    f.backend.progress_cooperative_copy(id).unwrap(),
                    BackendPollV1::Pending
                );
            }
            let trace = f.root(id).trace.clone();
            let window = f.root(id).window;
            if corruption == 2 {
                let bytes = vec![0; f.record(false).bytes.len() + 1];
                f.record_mut(false).bytes = bytes.into();
            } else {
                let RoutedSubmissionV1::CooperativeCopy(copy) =
                    f.backend.submissions.get_mut(&id).unwrap()
                else {
                    unreachable!()
                };
                if corruption == 0 {
                    copy.source_region.byte_offset += 1;
                } else {
                    copy.destination_region.byte_offset += 1;
                }
            }
            assert!(matches!(
                f.backend.progress_cooperative_copy(id),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert!(f.backend.terminal && f.backend.children.iter().all(|child| child.terminal));
            assert_eq!(f.root(id).window, window);
            assert_eq!(f.root(id).trace, trace);
            assert_eq!(
                f.root(id)
                    .scripted_owners
                    .iter()
                    .filter(|owner| owner.is_some())
                    .count(),
                if published { 2 } else { 0 }
            );
            if published {
                assert_eq!(f.backend.compute_xgmi_children, [Some(id), Some(id)]);
            }
            // Corrupted/uncertain roots intentionally remain retained until process exit.
        }
    }
}

#[test]
fn native_subranges_fault_prefixes_retain_window_original_owners_and_pair_quarantine() {
    for stage in STAGES.into_iter().chain([Stage::NextPacket]) {
        for unwind in [false, true] {
            let bytes = if stage == Stage::NextPacket {
                GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize + 1
            } else {
                47
            };
            let mut f = fixture(bytes, Some(stage), unwind);
            let identities = [
                f.owner(true).scripted_owner_id(),
                f.owner(false).scripted_owner_id(),
            ];
            let id = f.submit(&[]);
            let window = f.root(id).window;
            let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), Failure> {
                for _ in 0..64 {
                    f.backend.flush_stream_v1(f.stream)?;
                }
                Ok(())
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(RuntimeBackendFailureV1::Terminal(_))
                ));
            }
            assert!(f.backend.terminal && f.backend.children.iter().all(|child| child.terminal));
            assert_eq!(f.root(id).window, window);
            assert_eq!(f.root(id).trace.last(), Some(&stage));
            assert!(!f.root(id).is_quiescent());
            assert_eq!(
                f.root(id)
                    .scripted_owners
                    .each_ref()
                    .map(|owner| owner.as_ref().unwrap().scripted_owner_id()),
                identities
            );
            assert_eq!(f.backend.compute_xgmi_children, [Some(id), Some(id)]);
            assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        }
    }
}

#[test]
fn native_subranges_directed_chain_and_fanout_keep_guards_through_late_admission() {
    for fanout in [false, true] {
        for late in [None, Some(Phase::Published), Some(Phase::Ready)] {
            let mut f = Fixture::with_child_sizes(
                None,
                false,
                2,
                &[91, 117, 143],
                vec![vec![], vec![], vec![]],
            );
            f.backend.compute_xgmi_routes.remove(&(2, 3));
            for pair in [(0, 2), (1, 2)] {
                f.backend.compute_xgmi_routes.insert(
                    pair,
                    Route::Scripted {
                        failure: None,
                        unwind: false,
                        pending_samples: 2,
                    },
                );
            }
            let allocations: [u64; 3] = std::array::from_fn(|child| {
                *f.backend
                    .allocations
                    .iter()
                    .find(|(_, route)| route.child == child)
                    .unwrap()
                    .0
            });
            let streams = [
                f.backend.create_stream_v1(7).unwrap(),
                f.stream,
                f.backend.create_stream_v1(9).unwrap(),
            ];
            let offsets = [5, 19, 41];
            let identities = allocations.map(|allocation| {
                let route = f.backend.allocations[&allocation];
                let KfdRuntimeSdmaStorageV1::Device(owner) = &mut f.backend.children[route.child]
                    .allocations
                    .get_mut(&route.local)
                    .unwrap()
                    .sdma_storage
                else {
                    unreachable!()
                };
                for (i, byte) in owner.scripted_bytes_mut().unwrap().iter_mut().enumerate() {
                    *byte = ((i * 37 + route.child * 59) % 251) as u8;
                }
                owner.scripted_owner_id()
            });
            let original: [Vec<u8>; 3] = allocations.map(|allocation| {
                let route = f.backend.allocations[&allocation];
                let KfdRuntimeSdmaStorageV1::Device(owner) =
                    &f.backend.children[route.child].allocations[&route.local].sdma_storage
                else {
                    unreachable!()
                };
                owner.scripted_bytes().unwrap().to_vec()
            });
            let route = |source: usize, destination: usize| BackendDirectedPeerRouteV1 {
                stream: streams[destination],
                source_device: 7 + source as u64,
                destination_device: 7 + destination as u64,
                source: BackendMemoryRegionV1 {
                    allocation: allocations[source],
                    access: RuntimeAccessV1::Read,
                    byte_offset: offsets[source],
                    byte_len: 47,
                },
                destination: BackendMemoryRegionV1 {
                    allocation: allocations[destination],
                    access: RuntimeAccessV1::Write,
                    byte_offset: offsets[destination],
                    byte_len: 47,
                },
            };
            let first_route = route(0, 1);
            let first = f
                .backend
                .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                    route: first_route,
                    dependencies: &[],
                })
                .unwrap();
            if let Some(phase) = late {
                for _ in 0..16 {
                    assert_eq!(
                        f.backend.progress_retained_directed_peer_v1(first).unwrap(),
                        BackendPollV1::Pending
                    );
                    if f.root(first).phase == phase {
                        break;
                    }
                }
                assert_eq!(f.root(first).phase, phase);
            }
            let event = f
                .backend
                .record_event_v1(first_route.stream, first)
                .unwrap();
            let dependency = BackendDirectedPeerDependencyV1 {
                event,
                producer_submission: first,
            };
            let last = f
                .backend
                .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                    route: route(if fanout { 0 } else { 1 }, 2),
                    dependencies: if fanout {
                        &[]
                    } else {
                        std::slice::from_ref(&dependency)
                    },
                })
                .unwrap();
            f.backend.release_event_v1(event).unwrap();
            if fanout {
                assert!(f.copy(last).dependencies.is_empty());
                if late.is_none() {
                    // Independent siblings share resource custody, not success edges.
                    for _ in 0..2 {
                        assert_eq!(
                            f.backend.progress_retained_directed_peer_v1(first).unwrap(),
                            BackendPollV1::Pending
                        );
                    }
                    assert_eq!(f.root(first).phase, Phase::Published);
                }
            }
            for id in [first, last] {
                assert!(f.copy(id).staging.is_empty());
                assert!(f.copy(id).compute_xgmi.is_some());
                assert!(f.backend.directed_identity_is_intact_v1(id));
            }
            assert_eq!(
                f.backend
                    .drain_v1(last, Instant::now() + Duration::from_secs(2))
                    .unwrap(),
                BackendPollV1::Succeeded
            );
            assert_eq!(f.copy(first).status(), BackendPollV1::Succeeded);
            for (index, allocation) in allocations.into_iter().enumerate() {
                let endpoint = f.backend.allocations[&allocation];
                let KfdRuntimeSdmaStorageV1::Device(owner) =
                    &f.backend.children[index].allocations[&endpoint.local].sdma_storage
                else {
                    unreachable!()
                };
                let mut expected = original[index].clone();
                if index != 0 {
                    let offset = offsets[index] as usize;
                    expected[offset..offset + 47].copy_from_slice(&original[0][5..52]);
                }
                assert_eq!(owner.scripted_owner_id(), identities[index]);
                assert_eq!(owner.scripted_bytes().unwrap(), expected);
            }
            f.clean();
        }
    }
}
