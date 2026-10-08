#![cfg(test)]

use super::*;
use crate::BackendPeerCopyPlacementV1;

#[test]
fn original_route_and_endpoint_eligibility_drive_readonly_native_or_staged_quotes() {
    let mut f = Fixture::new(None, false);
    let before = (
        f.backend.next_handle,
        f.backend.allocations.len(),
        f.backend.submissions.len(),
        f.backend.cooperative_staging_bytes,
    );
    let observe = |f: &Fixture| {
        f.backend
            .observe_peer_copy_placement_v1(f.stream, f.source, f.destination)
    };
    assert_eq!(
        observe(&f),
        Some(BackendPeerCopyPlacementV1::NativeXgmiCandidate)
    );
    f.record_mut(true).sdma_initialized = false;
    assert_eq!(
        observe(&f),
        Some(BackendPeerCopyPlacementV1::HostStaged { peak_bytes: 128 })
    );
    f.record_mut(true).sdma_initialized = true;
    let route = f.backend.compute_xgmi_routes.remove(&(0, 1)).unwrap();
    assert_eq!(
        observe(&f),
        Some(BackendPeerCopyPlacementV1::HostStaged { peak_bytes: 128 })
    );
    let original_limit = f.backend.cooperative_staging_limit_bytes;
    f.backend.cooperative_staging_limit_bytes = 127;
    assert_eq!(observe(&f), None);
    f.backend.cooperative_staging_limit_bytes = original_limit;
    f.backend.compute_xgmi_routes.insert((0, 1), route);
    assert_eq!(
        observe(&f),
        Some(BackendPeerCopyPlacementV1::NativeXgmiCandidate)
    );
    assert_eq!(
        before,
        (
            f.backend.next_handle,
            f.backend.allocations.len(),
            f.backend.submissions.len(),
            f.backend.cooperative_staging_bytes
        )
    );
    assert_eq!(
        f.backend
            .observe_peer_copy_placement_v1(u64::MAX, f.source, f.destination),
        None
    );
    let mut bad = f.source;
    bad.byte_offset = u64::MAX;
    assert_eq!(
        f.backend
            .observe_peer_copy_placement_v1(f.stream, bad, f.destination),
        None
    );
    assert_eq!(
        f.backend.children[0]
            .scripted_sdma
            .as_ref()
            .unwrap()
            .remaining_steps(),
        2
    );
    assert_eq!(
        f.backend.children[1]
            .scripted_sdma
            .as_ref()
            .unwrap()
            .remaining_steps(),
        2
    );
    f.clean();
}
