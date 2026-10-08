//! Native-free controls of the conservative custody census, not reset evidence.
#![cfg(test)]
use super::*;

#[test]
fn cold_native_census_refuses_each_nonempty_scalar_owner_axis() {
    let mut backend = KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1();
    assert!(backend.cold_native_empty_v1());
    for axis in 0..12 {
        match axis {
            0 => backend.compute_completion_reservations = 1,
            1 => backend.sdma_completion_reservations = 1,
            2 => backend.has_admitted_peer_gate = true,
            3 => backend.native_dirty_extents = 1,
            4 => backend.staged_context_bytes = 1,
            5 => backend.sdma_enabled = true,
            6 => backend.selected_compute_lane = 1,
            7 => {
                backend.pending_compute_streams.insert(1, VecDeque::new());
            }
            8 => {
                backend.compute_module_retain_counts.insert(1, 1);
            }
            9 => {
                backend.stream_submission_tails.insert(1, 1);
            }
            10 => {
                backend.published_sdma_submissions.push(1);
            }
            _ => {
                backend.quiescent_sdma_submissions.insert(1);
            }
        }
        assert!(!backend.cold_native_empty_v1(), "axis {axis}");
        backend.compute_completion_reservations = 0;
        backend.sdma_completion_reservations = 0;
        backend.has_admitted_peer_gate = false;
        backend.native_dirty_extents = 0;
        backend.staged_context_bytes = 0;
        backend.sdma_enabled = false;
        backend.selected_compute_lane = 0;
        backend.pending_compute_streams.clear();
        backend.compute_module_retain_counts.clear();
        backend.stream_submission_tails.clear();
        backend.published_sdma_submissions.clear();
        backend.quiescent_sdma_submissions.clear();
        assert!(backend.cold_native_empty_v1());
    }
}

#[test]
fn cold_route_census_uses_exact_child_for_allocation_module_and_kernel_aliases() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    for axis in 0..4 {
        let route = RoutedHandleV1 { child: 1, local: 7 };
        match axis {
            0 => {
                backend.allocations.insert(9, route);
            }
            1 => {
                backend.generated_allocations.insert(9, route);
            }
            2 => {
                backend.modules.insert(9, route);
            }
            _ => {
                backend.kernels.insert(9, route);
            }
        }
        assert!(backend.cold_route_empty_v1(0));
        assert!(!backend.cold_route_empty_v1(1));
        backend.allocations.clear();
        backend.generated_allocations.clear();
        backend.modules.clear();
        backend.kernels.clear();
        assert!(backend.cold_route_empty_v1(1));
    }
    backend.compute_xgmi_children[1] = Some(9);
    assert!(backend.cold_route_empty_v1(0));
    assert!(!backend.cold_route_empty_v1(1));
    backend.compute_xgmi_children[1] = None;
}

#[test]
fn cold_route_scope_uses_original_stream_and_never_an_unrelated_child() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let first = backend.create_stream_v1(7).unwrap();
    let second = backend.create_stream_v1(8).unwrap();
    let a = backend.generated_cold_scope_v1(7, first).unwrap();
    let b = backend.generated_cold_scope_v1(8, second).unwrap();
    assert_eq!((a.child, b.child), (Some(0), Some(1)));
    assert_eq!((a.stream, b.stream), (first, second));
    assert_eq!(a.local_stream, b.local_stream);
    assert!(
        backend
            .children
            .iter()
            .all(|child| child.admitted_device.cold().is_none())
    );
    backend.destroy_stream_v1(first).unwrap();
    backend.destroy_stream_v1(second).unwrap();
}

#[test]
fn cold_route_census_rejects_selected_or_orphan_stream_and_dependency_metadata() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let first = backend.create_stream_v1(7).unwrap();
    let second = backend.create_stream_v1(8).unwrap();
    for stream in [first, second, u64::MAX] {
        for axis in 0..3 {
            match axis {
                0 => {
                    backend.cooperative_stream_pending_counts.insert(stream, 1);
                }
                1 => {
                    backend.native_stream_submission_counts.insert(stream, 1);
                }
                _ => {
                    backend.cooperative_stream_tails.insert(stream, 1);
                }
            }
            assert_eq!(backend.cold_route_empty_v1(0), stream == second);
            backend.cooperative_stream_pending_counts.clear();
            backend.native_stream_submission_counts.clear();
            backend.cooperative_stream_tails.clear();
        }
    }
    backend.cooperative_dependency_retain_counts.insert(1, 1);
    assert!(!backend.cold_route_empty_v1(0));
    backend.cooperative_dependency_retain_counts.clear();
    backend.event_submission_retain_counts.insert(1, 1);
    assert!(!backend.cold_route_empty_v1(0));
    backend.event_submission_retain_counts.clear();
    backend.kernel_modules.insert(1, 1);
    assert!(!backend.cold_route_empty_v1(0));
    backend.kernel_modules.clear();
    backend.destroy_stream_v1(first).unwrap();
    backend.destroy_stream_v1(second).unwrap();
}
