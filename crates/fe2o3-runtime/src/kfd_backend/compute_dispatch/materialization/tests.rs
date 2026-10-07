//! Tests of the production custody drivers, not native allocation qualification.

use super::*;
use crate::kfd_backend::drain_capture::tests::counted;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Debug)]
struct Probe {
    index: usize,
    bytes: [u8; 4],
    dropped: Rc<RefCell<Vec<usize>>>,
}

impl Probe {
    fn new(index: usize, dropped: &Rc<RefCell<Vec<usize>>>) -> Self {
        Self {
            index,
            bytes: [0; 4],
            dropped: Rc::clone(dropped),
        }
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.dropped.borrow_mut().push(self.index);
    }
}

struct DropPanic {
    payload: Option<Box<(&'static str, usize)>>,
}

impl DropPanic {
    fn touch(&self) {}
}

impl Drop for DropPanic {
    fn drop(&mut self) {
        resume_unwind(self.payload.take().unwrap());
    }
}

struct Unformattable;

impl std::fmt::Display for Unformattable {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::panic::panic_any(("error formatting", 23usize));
    }
}

#[test]
fn materialization_success_transfers_each_owner_once_in_order() {
    for count in [0, 1, 3, 16] {
        let dropped = Rc::new(RefCell::new(Vec::new()));
        let mut calls = Vec::new();
        let result = materialize_with_custody_v1(
            (0..count).collect(),
            "capacity",
            |index, spec| {
                assert_eq!(index, *spec);
                calls.push(index);
                Ok(Probe::new(index, &dropped))
            },
            |_| panic!("successful materialization retained"),
        )
        .unwrap();
        assert_eq!(calls, (0..count).collect::<Vec<_>>());
        assert_eq!(result.iter().map(|p| p.index).collect::<Vec<_>>(), calls);
        assert!(dropped.borrow().is_empty());
        drop(result);
        assert_eq!(*dropped.borrow(), calls);
    }
}

#[test]
fn materialization_reserves_only_the_output_roster() {
    for count in [0, 1, 3, 16] {
        let specs = (0..count).collect::<Vec<usize>>();
        let (result, allocations) = counted(|| {
            materialize_with_custody_v1(
                specs,
                "capacity",
                |index, _| Ok(index),
                |_| panic!("success retained"),
            )
        });
        assert_eq!(allocations, usize::from(count != 0));
        assert_eq!(result.unwrap(), (0..count).collect::<Vec<_>>());
    }
}

fn materialization_failure(fail_at: usize, lower_owns_current: bool, panicked: bool) {
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let specs = vec![17usize, 29, 41];
    let spec_storage = (specs.as_ptr(), specs.capacity());
    let capture = Probe::new(99, &dropped);
    let native_drops = Rc::clone(&dropped);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let calls_in = Rc::clone(&calls);
    let lower = Rc::new(RefCell::new(None));
    let lower_in = Rc::clone(&lower);
    let payload = Box::new(("initializer", fail_at));
    let payload_pointer = &*payload as *const _;
    let mut payload = Some(payload);
    let mut retained = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        materialize_with_custody_v1(
            specs,
            "capacity",
            move |index, _| {
                let _ = &capture;
                calls_in.borrow_mut().push(index);
                if index == fail_at {
                    if lower_owns_current {
                        *lower_in.borrow_mut() = Some(Probe::new(index, &native_drops));
                    }
                    if panicked {
                        resume_unwind(payload.take().unwrap());
                    }
                    return Err(format!("initializer {index}"));
                }
                Ok(Probe::new(index, &native_drops))
            },
            |custody| retained = Some(custody),
        )
    }));
    if panicked {
        let payload = result.unwrap_err().downcast::<(&str, usize)>().unwrap();
        assert_eq!(&*payload as *const _, payload_pointer);
    } else {
        assert_eq!(
            result.unwrap().unwrap_err(),
            format!("initializer {fail_at}")
        );
    }
    let custody = retained.unwrap();
    assert_eq!(custody.specs, [17, 29, 41]);
    assert_eq!(
        (custody.specs.as_ptr(), custody.specs.capacity()),
        spec_storage
    );
    assert!(custody.initializer.is_some());
    assert_eq!(custody.data.len(), fail_at);
    assert!(custody.data.capacity() >= 3);
    assert_eq!(
        custody.data.iter().map(|p| p.index).collect::<Vec<_>>(),
        (0..fail_at).collect::<Vec<_>>()
    );
    assert_eq!(*calls.borrow(), (0..=fail_at).collect::<Vec<_>>());
    assert_eq!(
        lower.borrow().as_ref().map(|p| p.index),
        lower_owns_current.then_some(fail_at)
    );
    assert!(dropped.borrow().is_empty());
    drop(custody);
    drop(lower.borrow_mut().take());
    let mut actual = dropped.borrow().clone();
    actual.sort_unstable();
    let mut expected = (0..(fail_at + usize::from(lower_owns_current))).collect::<Vec<_>>();
    expected.push(99);
    assert_eq!(actual, expected);
}

#[test]
fn materialization_failure_retains_prefix_specs_and_callback_captures() {
    for index in 0..3 {
        for lower_owns_current in [false, true] {
            for panicked in [false, true] {
                materialization_failure(index, lower_owns_current, panicked);
            }
        }
    }
}

#[test]
fn materialization_capacity_failure_is_before_any_callback() {
    let called = Cell::new(false);
    let mut retained = None;
    // A ZST roster expresses deterministic capacity overflow without a huge allocation.
    let result = materialize_with_custody_v1(
        vec![(); usize::MAX],
        "native roster capacity",
        |_, _| {
            called.set(true);
            Ok(1u64)
        },
        |custody| retained = Some(custody),
    );
    assert_eq!(result, Err("native roster capacity".to_owned()));
    assert!(!called.get());
    let custody = retained.unwrap();
    assert_eq!(custody.specs.len(), usize::MAX);
    assert_eq!(custody.data.capacity(), 0);
    assert!(custody.initializer.is_some());
}

#[test]
fn materialization_formatting_panic_keeps_lower_and_returned_owners_separate() {
    for fail_at in 0..3 {
        let dropped = Rc::new(RefCell::new(Vec::new()));
        let lower = RefCell::new(None);
        let mut retained = None;
        let result = catch_unwind(AssertUnwindSafe(|| {
            materialize_with_custody_v1(
                vec![0usize, 1, 2],
                "capacity",
                |index, _| {
                    let item = Probe::new(index, &dropped);
                    if index == fail_at {
                        *lower.borrow_mut() = Some(item);
                        return Err(format!("native failure: {Unformattable}"));
                    }
                    Ok(item)
                },
                |custody| retained = Some(custody),
            )
        }));
        assert_eq!(
            *result.unwrap_err().downcast::<(&str, usize)>().unwrap(),
            ("error formatting", 23)
        );
        let custody = retained.unwrap();
        assert_eq!(custody.data.len(), fail_at);
        assert_eq!(lower.borrow().as_ref().unwrap().index, fail_at);
        assert!(dropped.borrow().is_empty());
    }
}

#[test]
fn materialization_capture_destructor_panic_retains_complete_output() {
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let output_drops = Rc::clone(&dropped);
    let payload = Box::new(("initializer destructor", 3usize));
    let pointer = &*payload as *const _;
    let bomb = DropPanic {
        payload: Some(payload),
    };
    let mut retained = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        materialize_with_custody_v1(
            vec![0usize, 1, 2],
            "capacity",
            move |index, _| {
                bomb.touch();
                Ok(Probe::new(index, &output_drops))
            },
            |custody| retained = Some(custody),
        )
    }));
    let payload = result.unwrap_err().downcast::<(&str, usize)>().unwrap();
    assert_eq!(&*payload as *const _, pointer);
    let custody = retained.unwrap();
    assert!(custody.initializer.is_none());
    assert_eq!(custody.specs, [0, 1, 2]);
    assert_eq!(custody.data.len(), 3);
    assert!(dropped.borrow().is_empty());
}

#[test]
fn materialization_metadata_destructor_panic_retains_complete_output() {
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let payload = Box::new(("metadata destructor", 1usize));
    let pointer = &*payload as *const _;
    let specs = vec![DropPanic {
        payload: Some(payload),
    }];
    let mut retained = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        materialize_with_custody_v1(
            specs,
            "capacity",
            |index, _| Ok(Probe::new(index, &dropped)),
            |custody| retained = Some(custody),
        )
    }));
    let payload = result.unwrap_err().downcast::<(&str, usize)>().unwrap();
    assert_eq!(&*payload as *const _, pointer);
    let custody = retained.unwrap();
    assert!(custody.specs.is_empty());
    assert!(custody.initializer.is_none());
    assert_eq!(custody.data.len(), 1);
    assert!(dropped.borrow().is_empty());
}

#[test]
fn session_success_transfers_once_and_missing_session_has_no_effect() {
    let called = Cell::new(false);
    let result = materialize_in_retained_session_v1(&mut None::<Probe>, |_| {
        called.set(true);
        Ok(())
    });
    assert_eq!(
        result.unwrap_err(),
        "KFD materialization session is missing"
    );
    assert!(!called.get());
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let mut slot = Some(Probe::new(17, &dropped));
    let (session, output) = materialize_in_retained_session_v1(&mut slot, |session| {
        session.bytes = [4; 4];
        Ok(Probe::new(29, &dropped))
    })
    .unwrap();
    assert!(slot.is_none());
    assert_eq!(session.index, 17);
    assert_eq!(session.bytes, [4; 4]);
    assert!(dropped.borrow().is_empty());
    drop((session, output));
    assert_eq!(*dropped.borrow(), [17, 29]);
}

#[test]
fn session_error_and_panic_leave_exact_mutated_owner_installed() {
    for panicked in [false, true] {
        let dropped = Rc::new(RefCell::new(Vec::new()));
        let mut slot = Some(Probe::new(17, &dropped));
        let pointer = slot.as_ref().unwrap() as *const _;
        let payload = Box::new(("session", 17usize));
        let payload_pointer = &*payload as *const _;
        let result = catch_unwind(AssertUnwindSafe(|| {
            materialize_in_retained_session_v1(&mut slot, |session| {
                session.bytes[0] = 7;
                if panicked {
                    resume_unwind(payload);
                }
                Err::<(), _>("session failure".to_owned())
            })
        }));
        if panicked {
            let payload = result.unwrap_err().downcast::<(&str, usize)>().unwrap();
            assert_eq!(&*payload as *const _, payload_pointer);
        } else {
            assert_eq!(result.unwrap().unwrap_err(), "session failure");
        }
        let session = slot.as_ref().unwrap();
        assert_eq!(session as *const _, pointer);
        assert_eq!(session.index, 17);
        assert_eq!(session.bytes, [7, 0, 0, 0]);
        assert!(dropped.borrow().is_empty());
    }
}

#[test]
fn overwrite_success_reuses_original_storage_without_allocation() {
    for count in [0, 1, 3, 16] {
        let descriptors = (0..count).collect::<Vec<usize>>();
        let data = vec![0usize; count];
        let storage = (data.as_ptr(), data.capacity());
        let (result, allocations) = counted(|| {
            overwrite_with_custody_v1(
                descriptors,
                data,
                |index, descriptor, item| {
                    assert_eq!(index, *descriptor);
                    *item = index + 7;
                    Ok(())
                },
                |_| panic!("success retained"),
            )
        });
        let data = result.unwrap();
        assert_eq!(allocations, 0);
        assert_eq!((data.as_ptr(), data.capacity()), storage);
        assert_eq!(data, (7..(count + 7)).collect::<Vec<_>>());
    }
}

fn overwrite_failure(fail_at: usize, panicked: bool, formatting: bool) {
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let data = (0..3).map(|i| Probe::new(i, &dropped)).collect::<Vec<_>>();
    let storage = (data.as_ptr(), data.capacity());
    let descriptors = vec![(0, [5u8; 32]), (1, [7; 32]), (2, [9; 32])];
    let metadata_storage = (descriptors.as_ptr(), descriptors.capacity());
    let expected = descriptors.clone();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let calls_in = Rc::clone(&calls);
    let capture = Probe::new(99, &dropped);
    let payload = Box::new(("overwrite", fail_at));
    let pointer = &*payload as *const _;
    let mut payload = Some(payload);
    let mut retained = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        overwrite_with_custody_v1(
            descriptors,
            data,
            move |index, descriptor, item| {
                let _ = &capture;
                assert_eq!(index, descriptor.0);
                assert_eq!(index, item.index);
                calls_in.borrow_mut().push(index);
                item.bytes[..2].fill(11);
                if index == fail_at {
                    if panicked {
                        resume_unwind(payload.take().unwrap());
                    }
                    if formatting {
                        return Err(format!("overwrite: {Unformattable}"));
                    }
                    return Err(format!("overwrite {index}"));
                }
                item.bytes[2..].fill(13);
                Ok(())
            },
            |custody| retained = Some(custody),
        )
    }));
    if panicked {
        let payload = result.unwrap_err().downcast::<(&str, usize)>().unwrap();
        assert_eq!(&*payload as *const _, pointer);
    } else if formatting {
        assert_eq!(
            *result.unwrap_err().downcast::<(&str, usize)>().unwrap(),
            ("error formatting", 23)
        );
    } else {
        assert_eq!(result.unwrap().unwrap_err(), format!("overwrite {fail_at}"));
    }
    let custody = retained.unwrap();
    assert_eq!((custody.data.as_ptr(), custody.data.capacity()), storage);
    assert_eq!(
        (custody.descriptors.as_ptr(), custody.descriptors.capacity()),
        metadata_storage
    );
    assert_eq!(custody.descriptors, expected);
    assert!(custody.overwrite.is_some());
    assert_eq!(*calls.borrow(), (0..=fail_at).collect::<Vec<_>>());
    for (index, item) in custody.data.iter().enumerate() {
        assert_eq!(item.index, index);
        assert_eq!(
            item.bytes,
            if index < fail_at {
                [11, 11, 13, 13]
            } else if index == fail_at {
                [11, 11, 0, 0]
            } else {
                [0; 4]
            }
        );
    }
    assert!(dropped.borrow().is_empty());
    drop(custody);
    assert_eq!(*dropped.borrow(), [0, 1, 2, 99]);
}

#[test]
fn overwrite_errors_and_panics_retain_full_roster_and_partial_writes() {
    for index in 0..3 {
        overwrite_failure(index, false, false);
        overwrite_failure(index, true, false);
        overwrite_failure(index, false, true);
    }
}

#[test]
fn overwrite_mismatched_rosters_reject_before_writing_and_retain_both() {
    for (metadata_count, data_count) in [(0, 3), (3, 0), (2, 3), (3, 2)] {
        let descriptors = (0..metadata_count).collect::<Vec<usize>>();
        let data = (0..data_count).collect::<Vec<usize>>();
        let metadata_storage = (descriptors.as_ptr(), descriptors.capacity());
        let storage = (data.as_ptr(), data.capacity());
        let mut retained = None;
        let result = overwrite_with_custody_v1(
            descriptors,
            data,
            |_, _, _| panic!("mismatch must reject before writes"),
            |custody| retained = Some(custody),
        );
        assert_eq!(
            result.unwrap_err(),
            "KFD resident-data overwrite roster mismatch"
        );
        let custody = retained.unwrap();
        assert_eq!(
            (custody.descriptors.as_ptr(), custody.descriptors.capacity()),
            metadata_storage
        );
        assert_eq!((custody.data.as_ptr(), custody.data.capacity()), storage);
        assert_eq!(custody.descriptors, (0..metadata_count).collect::<Vec<_>>());
        assert_eq!(custody.data, (0..data_count).collect::<Vec<_>>());
    }
}

#[test]
fn overwrite_capture_destructor_panic_retains_completed_writes() {
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let payload = Box::new(("overwrite destructor", 1usize));
    let pointer = &*payload as *const _;
    let bomb = DropPanic {
        payload: Some(payload),
    };
    let mut retained = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        overwrite_with_custody_v1(
            vec![17usize],
            vec![Probe::new(17, &dropped)],
            move |_, _, item| {
                bomb.touch();
                item.bytes = [7; 4];
                Ok(())
            },
            |custody| retained = Some(custody),
        )
    }));
    let payload = result.unwrap_err().downcast::<(&str, usize)>().unwrap();
    assert_eq!(&*payload as *const _, pointer);
    let custody = retained.unwrap();
    assert!(custody.overwrite.is_none());
    assert_eq!(custody.descriptors, [17]);
    assert_eq!(custody.data[0].bytes, [7; 4]);
    assert!(dropped.borrow().is_empty());
}

#[test]
fn overwrite_metadata_destructor_panic_retains_completed_writes() {
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let payload = Box::new(("overwrite metadata destructor", 1usize));
    let pointer = &*payload as *const _;
    let mut retained = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        overwrite_with_custody_v1(
            vec![DropPanic {
                payload: Some(payload),
            }],
            vec![Probe::new(17, &dropped)],
            |_, _, item| {
                item.bytes = [7; 4];
                Ok(())
            },
            |custody| retained = Some(custody),
        )
    }));
    let payload = result.unwrap_err().downcast::<(&str, usize)>().unwrap();
    assert_eq!(&*payload as *const _, pointer);
    let custody = retained.unwrap();
    assert!(custody.overwrite.is_none());
    assert!(custody.descriptors.is_empty());
    assert_eq!(custody.data[0].bytes, [7; 4]);
    assert!(dropped.borrow().is_empty());
}
