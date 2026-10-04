//! Read-only test observation of the genuine admitted owner on its sole path.

use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1;
use rustc_middle::ty::TyCtxt;
use std::cell::RefCell;

type Observer = Box<dyn for<'tcx> FnMut(TyCtxt<'tcx>, &AdmittedInertSemanticMirV1)>;

thread_local! {
    static ACTIVE: RefCell<Option<Observer>> = const { RefCell::new(None) };
}

struct Restore(Option<Observer>);
impl Drop for Restore {
    fn drop(&mut self) {
        ACTIVE.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

pub(crate) fn with_observer<R>(observer: Observer, action: impl FnOnce() -> R) -> R {
    let previous = ACTIVE.with(|slot| slot.replace(Some(observer)));
    let restore = Restore(previous);
    let result = action();
    drop(restore);
    result
}

pub(super) fn observe_actual(tcx: TyCtxt<'_>, semantic: &AdmittedInertSemanticMirV1) {
    ACTIVE.with(|slot| {
        if let Some(observer) = slot.borrow_mut().as_mut() {
            observer(tcx, semantic);
        }
    });
}

#[test]
fn inactive_nested_and_unwinding_import_observers_restore_the_previous_scope() {
    assert!(ACTIVE.with(|slot| slot.borrow().is_none()));
    with_observer(
        Box::new(|_, _| panic!("no import in this scope test")),
        || {
            assert!(ACTIVE.with(|slot| slot.borrow().is_some()));
            with_observer(Box::new(|_, _| panic!("no nested import")), || {
                assert!(ACTIVE.with(|slot| slot.borrow().is_some()));
            });
            assert!(ACTIVE.with(|slot| slot.borrow().is_some()));
            let result = std::panic::catch_unwind(|| {
                with_observer(Box::new(|_, _| panic!("no unwinding import")), || {
                    std::panic::panic_any(0x414_u32)
                });
            });
            assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 0x414);
            assert!(ACTIVE.with(|slot| slot.borrow().is_some()));
        },
    );
    assert!(ACTIVE.with(|slot| slot.borrow().is_none()));
}
