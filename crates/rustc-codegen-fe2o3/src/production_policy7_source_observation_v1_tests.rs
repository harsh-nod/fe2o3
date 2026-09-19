//! Borrow the actual Stage7 inside its sole live extraction phase. No owner escapes.
use super::*;
use std::cell::RefCell;

pub(crate) struct View<'a> {
    stage: &'a CheckedOutputTargetProductionCompilationPolicy7V1,
    pub(crate) direct: Option<&'a Direct>,
    pub(crate) artifacts: &'a PreparedPolicy7ArtifactsV1,
}
impl View<'_> {
    pub(crate) fn replay(&self, budget: &mut Budget<'_>) -> Result<(), String> {
        self.stage
            .verify_equivalence(budget)
            .map_err(|e| format!("{e:?}"))
    }
}
type Observer = Box<dyn for<'a, 'w> FnMut(View<'a>, &mut Budget<'w>) -> Result<(), String>>;
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
pub(super) fn observe_actual(
    stage: &CheckedOutputTargetProductionCompilationPolicy7V1,
    budget: &mut Budget<'_>,
) -> Result<(), String> {
    ACTIVE.with(|slot| {
        if let Some(observer) = slot.borrow_mut().as_mut() {
            observer(
                View {
                    stage,
                    direct: match &stage.artifacts.admitted {
                        Admitted7::Direct(owner) => Some(owner),
                        Admitted7::Erased(_) => None,
                    },
                    artifacts: &stage.artifacts,
                },
                budget,
            )?;
        }
        Ok(())
    })
}

#[test]
fn policy7_source_observer_nested_and_panic_scopes_restore_without_an_owner() {
    assert!(ACTIVE.with(|slot| slot.borrow().is_none()));
    with_observer(
        Box::new(|_, _| panic!("no actual stage in this test")),
        || {
            assert!(ACTIVE.with(|slot| slot.borrow().is_some()));
            with_observer(Box::new(|_, _| panic!("no nested stage")), || {});
            assert!(ACTIVE.with(|slot| slot.borrow().is_some()));
            assert!(
                std::panic::catch_unwind(|| {
                    with_observer(Box::new(|_, _| panic!("no stage")), || panic!("scope"));
                })
                .is_err()
            );
            assert!(ACTIVE.with(|slot| slot.borrow().is_some()));
        },
    );
    assert!(ACTIVE.with(|slot| slot.borrow().is_none()));
}
