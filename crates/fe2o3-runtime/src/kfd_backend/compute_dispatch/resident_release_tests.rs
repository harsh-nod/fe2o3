//! CPU ownership tests; native disposal and lane transport are qualified separately.

use super::*;
use std::cell::RefCell;
use std::rc::Rc;

struct Probe {
    index: usize,
    dropped: Rc<RefCell<Vec<usize>>>,
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.dropped.borrow_mut().push(self.index);
    }
}

fn probes(count: usize, dropped: &Rc<RefCell<Vec<usize>>>) -> Vec<Probe> {
    (0..count)
        .map(|index| Probe {
            index,
            dropped: Rc::clone(dropped),
        })
        .collect()
}

fn descriptors(count: usize) -> Vec<ResidentDataDescriptorV1> {
    (0..count)
        .map(|index| ResidentDataDescriptorV1 {
            allocation: index as u64 + 7,
            kind: if index % 2 == 0 {
                RuntimeMemoryKindV1::HostVisible
            } else {
                RuntimeMemoryKindV1::DeviceLocal
            },
            alignment: 4096,
            allocation_offset: index as u64 * 4096,
            byte_len: 4096,
            host_content_sha256: Some([index as u8; 32]),
            device_may_have_modified: index % 2 != 0,
        })
        .collect()
}

fn suffix(custody: &ResidentReleaseCustodyV1<Probe>) -> Vec<usize> {
    custody
        .remaining
        .as_slice()
        .iter()
        .map(|p| p.index)
        .collect()
}

#[test]
fn resident_release_success_is_forward_and_not_truncated_by_descriptors() {
    for count in [0, 1, 3] {
        for descriptor_count in [0, 1, 3, 5] {
            let dropped = Rc::new(RefCell::new(Vec::new()));
            let mut calls = Vec::new();
            let result = with_resident_release_custody_v1(
                descriptors(descriptor_count),
                probes(count, &dropped),
                |custody| {
                    release_resident_items_v1(custody, |probe| {
                        calls.push(probe.index);
                        drop(probe);
                        Ok(())
                    })?;
                    assert_eq!(custody.completed, count);
                    Ok(())
                },
                |_| panic!("successful cleanup must not retain the roster"),
            );
            assert_eq!(result, Ok(()));
            assert_eq!(calls, (0..count).collect::<Vec<_>>());
            assert_eq!(*dropped.borrow(), calls);
        }
    }
}

#[test]
fn resident_release_pre_handoff_errors_retain_original_storage_and_all_items() {
    for error in ["queue unavailable", "lane admission rejected"] {
        let dropped = Rc::new(RefCell::new(Vec::new()));
        let data = probes(3, &dropped);
        let data_pointer = data.as_ptr();
        let descriptors = descriptors(3);
        let descriptor_storage = (descriptors.as_ptr(), descriptors.capacity());
        let expected_descriptors = descriptors.clone();
        let mut retained = None;
        let result = with_resident_release_custody_v1(
            descriptors,
            data,
            |_| Err(error.to_owned()),
            |custody| retained = Some(custody),
        );
        assert_eq!(result, Err(error.to_owned()));
        let custody = retained.unwrap();
        assert_eq!(custody.completed, 0);
        assert_eq!(custody.active.as_ref().unwrap().index, 0);
        assert_eq!(suffix(&custody), [1, 2]);
        assert_eq!(
            custody.remaining.as_slice().as_ptr(),
            data_pointer.wrapping_add(1)
        );
        assert_eq!(custody._descriptors, expected_descriptors);
        assert_eq!(
            (
                custody._descriptors.as_ptr(),
                custody._descriptors.capacity()
            ),
            descriptor_storage
        );
        assert!(dropped.borrow().is_empty());
        drop(custody);
        assert_eq!(*dropped.borrow(), [0, 1, 2]);
    }
}

#[test]
fn resident_release_pre_handoff_panic_preserves_original_payload_and_full_roster() {
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let payload = Box::new(("before handoff", 17usize));
    let payload_pointer = &*payload as *const _;
    let mut retained = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_resident_release_custody_v1(
            descriptors(3),
            probes(3, &dropped),
            |_| std::panic::resume_unwind(payload),
            |custody| retained = Some(custody),
        )
    }));
    let payload = result.unwrap_err().downcast::<(&str, usize)>().unwrap();
    assert_eq!(&*payload as *const _, payload_pointer);
    let custody = retained.unwrap();
    assert_eq!(custody.completed, 0);
    assert_eq!(custody.active.as_ref().unwrap().index, 0);
    assert_eq!(suffix(&custody), [1, 2]);
    assert!(dropped.borrow().is_empty());
}

fn release_failure(fail_at: usize, panicked: bool) {
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let data = probes(3, &dropped);
    let data_pointer = data.as_ptr();
    let mut calls = Vec::new();
    let mut lower_retained = None;
    let mut outer_retained = None;
    let payload = Box::new(("native release", fail_at));
    let payload_pointer = &*payload as *const _;
    let mut payload = Some(payload);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_resident_release_custody_v1(
            descriptors(3),
            data,
            |custody| {
                release_resident_items_v1(custody, |probe| {
                    let index = probe.index;
                    calls.push(index);
                    if index == fail_at {
                        // The fake lower boundary honors the production consuming contract.
                        lower_retained = Some(probe);
                        if panicked {
                            std::panic::resume_unwind(payload.take().unwrap());
                        }
                        return Err(format!("release {index}"));
                    }
                    drop(probe);
                    Ok(())
                })
            },
            |custody| outer_retained = Some(custody),
        )
    }));
    if panicked {
        let payload = result.unwrap_err().downcast::<(&str, usize)>().unwrap();
        assert_eq!(&*payload as *const _, payload_pointer);
    } else {
        assert_eq!(result.unwrap(), Err(format!("release {fail_at}")));
    }
    let custody = outer_retained.unwrap();
    assert_eq!(custody.completed, fail_at);
    assert!(custody.active.is_none());
    assert_eq!(suffix(&custody), ((fail_at + 1)..3).collect::<Vec<_>>());
    assert_eq!(
        custody.remaining.as_slice().as_ptr(),
        data_pointer.wrapping_add(fail_at + 1)
    );
    assert_eq!(custody._descriptors, descriptors(3));
    assert_eq!(lower_retained.as_ref().unwrap().index, fail_at);
    assert_eq!(calls, (0..=fail_at).collect::<Vec<_>>());
    assert_eq!(*dropped.borrow(), (0..fail_at).collect::<Vec<_>>());
    drop(lower_retained);
    drop(custody);
    assert_eq!(*dropped.borrow(), [0, 1, 2]);
}

#[test]
fn resident_release_first_middle_last_error_retains_exact_partition() {
    for index in 0..3 {
        release_failure(index, false);
    }
}

#[test]
fn resident_release_first_middle_last_panic_retains_exact_partition() {
    for index in 0..3 {
        release_failure(index, true);
    }
}

#[test]
fn resident_release_error_formatting_panic_still_retains_suffix() {
    struct Unformattable;
    impl std::fmt::Display for Unformattable {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::panic::panic_any(("error formatting", 23usize));
        }
    }
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let mut lower_retained = None;
    let mut outer_retained = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_resident_release_custody_v1(
            descriptors(3),
            probes(3, &dropped),
            |custody| {
                release_resident_items_v1(custody, |probe| {
                    lower_retained = Some(probe);
                    Err(Unformattable).map_err(|error| format!("release: {error}"))
                })
            },
            |custody| outer_retained = Some(custody),
        )
    }));
    assert_eq!(
        *result.unwrap_err().downcast::<(&str, usize)>().unwrap(),
        ("error formatting", 23)
    );
    let custody = outer_retained.unwrap();
    assert_eq!(custody.completed, 0);
    assert!(custody.active.is_none());
    assert_eq!(suffix(&custody), [1, 2]);
    assert_eq!(lower_retained.as_ref().unwrap().index, 0);
    assert!(dropped.borrow().is_empty());
}

#[test]
fn resident_release_incomplete_success_retains_unhanded_items() {
    for (count, hand_off_first) in [(1, false), (3, false), (3, true)] {
        let dropped = Rc::new(RefCell::new(Vec::new()));
        let mut lower_retained = None;
        let mut outer_retained = None;
        let result = with_resident_release_custody_v1(
            descriptors(count),
            probes(count, &dropped),
            |custody| {
                if hand_off_first {
                    lower_retained = custody.active.take();
                }
                Ok(())
            },
            |custody| outer_retained = Some(custody),
        );
        assert_eq!(
            result,
            Err("KFD resident-data release callback left owned data".to_owned())
        );
        let custody = outer_retained.unwrap();
        assert_eq!(
            custody.active.as_ref().map(|p| p.index),
            (!hand_off_first).then_some(0)
        );
        assert_eq!(
            lower_retained.as_ref().map(|p| p.index),
            hand_off_first.then_some(0)
        );
        assert_eq!(suffix(&custody), (1..count).collect::<Vec<_>>());
        assert!(dropped.borrow().is_empty());
    }
}

#[test]
fn resident_release_real_lane_selection_rejection_keeps_installed_roster() {
    let mut backend = KfdRuntimeBackendV1::mock();
    assert!(backend.native_compute_lanes[backend.selected_compute_lane].is_none());
    let descriptors = descriptors(3);
    let expected_descriptors = descriptors.clone();
    let descriptor_storage = (descriptors.as_ptr(), descriptors.capacity());
    // Real typed storage, but no fabricated allocation authority or native device.
    let data = Vec::with_capacity(7);
    let data_storage = (data.as_ptr(), data.capacity());
    backend.resident_data = Some(ResidentDataRosterV1 { descriptors, data });
    for _ in 0..2 {
        let error = backend.release_resident_data().unwrap_err();
        let RuntimeBackendFailureV1::Rejected(error) = error else {
            panic!("lane selection should reject without terminalizing");
        };
        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Unsupported);
        assert_eq!(
            error.detail(),
            "selected KFD compute queue has not been materialized"
        );
        assert!(!backend.terminal);
        let roster = backend.resident_data.as_ref().unwrap();
        assert_eq!(roster.descriptors, expected_descriptors);
        assert_eq!(
            (roster.descriptors.as_ptr(), roster.descriptors.capacity()),
            descriptor_storage
        );
        assert!(roster.data.is_empty());
        assert_eq!((roster.data.as_ptr(), roster.data.capacity()), data_storage);
    }
}

#[test]
fn resident_release_real_empty_state_does_not_require_a_lane() {
    let mut backend = KfdRuntimeBackendV1::mock();
    assert!(backend.resident_data.is_none());
    assert!(backend.native_compute_lanes[backend.selected_compute_lane].is_none());
    assert!(backend.release_resident_data().is_ok());
    assert!(!backend.terminal);
    assert!(backend.resident_data.is_none());
}

#[test]
fn resident_release_custody_and_traversal_do_not_allocate_on_success() {
    for count in [0, 1, 3, 16] {
        let descriptors = descriptors(count);
        let data: Vec<usize> = (0..count).collect();
        let mut next = 0;
        let (result, allocations) = super::super::drain_capture::tests::counted(|| {
            with_resident_release_custody_v1(
                descriptors,
                data,
                |custody| {
                    release_resident_items_v1(custody, |index| {
                        assert_eq!(index, next);
                        next += 1;
                        Ok(())
                    })
                },
                |_| panic!("unexpected retention"),
            )
        });
        assert_eq!(result, Ok(()));
        assert_eq!(next, count);
        assert_eq!(allocations, 0);
    }
}
