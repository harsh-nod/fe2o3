//! Move one read-only test observer into the actual rustc callback thread.
use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1;
use rustc_middle::ty::TyCtxt;
use std::cell::RefCell;

pub(super) type Observer =
    Box<dyn for<'tcx> FnMut(TyCtxt<'tcx>, &AdmittedInertSemanticMirV1) + Send>;

thread_local! {
    static PENDING: RefCell<Option<Observer>> = const { RefCell::new(None) };
}

struct Restore(Option<Observer>);
impl Drop for Restore {
    fn drop(&mut self) {
        PENDING.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

pub(super) fn with_observer<R>(observer: Observer, action: impl FnOnce() -> R) -> R {
    let previous = PENDING.with(|slot| slot.replace(Some(observer)));
    let restore = Restore(previous);
    let result = action();
    drop(restore);
    result
}

pub(super) fn take_for_invocation() -> Option<Observer> {
    PENDING.with(|slot| slot.borrow_mut().take())
}

pub(super) fn in_callback<R>(observer: Option<Observer>, action: impl FnOnce() -> R) -> R {
    match observer {
        Some(observer) => {
            crate::collector::semantic_import_observation_v1_tests::with_observer(observer, action)
        }
        None => action(),
    }
}

#[test]
fn pending_observer_is_consumed_once_moved_across_threads_and_dropped_on_early_failure() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct DropCount(Arc<AtomicUsize>);
    impl Drop for DropCount {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicUsize::new(0));
    let observer = |dropped: Arc<AtomicUsize>| -> Observer {
        let marker = DropCount(dropped);
        Box::new(move |_, _| {
            let _ = &marker;
            panic!("no actual source is fabricated in this scope test")
        })
    };
    assert!(take_for_invocation().is_none());
    with_observer(observer(dropped.clone()), || {
        assert!(
            std::thread::spawn(|| take_for_invocation().is_none())
                .join()
                .unwrap()
        );
        let transfer = take_for_invocation();
        assert!(transfer.is_some() && take_for_invocation().is_none());
        let value = std::thread::spawn(move || in_callback(transfer, || 17))
            .join()
            .unwrap();
        assert_eq!(value, 17);
    });
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    with_observer(
        observer(dropped.clone()),
        || { /* argument refusal before transfer */ },
    );
    assert_eq!(dropped.load(Ordering::SeqCst), 2);
    with_observer(observer(dropped.clone()), || {
        with_observer(observer(dropped.clone()), || {
            drop(take_for_invocation());
        });
        let outer = take_for_invocation();
        assert!(outer.is_some());
        let unwind = std::thread::spawn(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                in_callback(outer, || std::panic::panic_any(33_u32));
            }))
        })
        .join()
        .unwrap();
        assert_eq!(*unwind.unwrap_err().downcast::<u32>().unwrap(), 33);
    });
    assert_eq!(dropped.load(Ordering::SeqCst), 4);
    assert!(take_for_invocation().is_none());
}
