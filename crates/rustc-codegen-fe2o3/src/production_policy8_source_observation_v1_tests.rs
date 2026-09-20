//! One invocation-owned observer, transferred onto the actual rustc thread.
use super::*;
use std::cell::RefCell;

#[derive(Clone, Copy)]
pub(crate) enum Owner<'a> {
    Direct(&'a Direct),
    Erased(&'a Erased),
}
impl Owner<'_> {
    pub(crate) fn original(&self) -> &Graph {
        match self {
            Self::Direct(v) => v
                .prefix()
                .prefix()
                .source_semantic_kir()
                .pre_ranked_executable()
                .unwrap(),
            Self::Erased(v) => v.prefix().prefix().original_source().executable(),
        }
    }
    pub(crate) fn output(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    pub(crate) fn historical_j(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().output(),
            Self::Erased(v) => v.prefix().output(),
        }
    }
    pub(crate) fn pairs(&self) -> usize {
        match self {
            Self::Direct(v) => v.continuation().proved_pairs(),
            Self::Erased(v) => v.continuation().proved_pairs(),
        }
    }
}
pub(crate) struct View<'a> {
    stage: &'a CheckedOutputTargetProductionCompilationPolicy8V1,
    pub(crate) owner: Owner<'a>,
    pub(crate) artifacts: &'a PreparedPolicy8ArtifactsV1,
    pub(crate) profile: Profile,
    pub(crate) ranked:
        &'a crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    pub(crate) entry_work: usize,
    pub(crate) prepared_work: usize,
    pub(crate) prepared_floor: usize,
}
impl View<'_> {
    pub(crate) fn history_round_trip(&self, budget: &mut Budget<'_>) -> Result<(), String> {
        super::history::tests::observe_stage(self.stage, budget).map_err(|e| format!("{e:?}"))
    }
    pub(crate) fn replay(&self, budget: &mut Budget<'_>) -> Result<(), String> {
        self.stage
            .verify_equivalence(budget)
            .map_err(|e| format!("{e:?}"))
    }
}
pub(crate) type Observer =
    Box<dyn for<'a, 'w> FnMut(View<'a>, &mut Budget<'w>) -> Result<(), String> + Send>;
thread_local! {
    static PENDING: RefCell<Option<Observer>> = const { RefCell::new(None) };
    static ACTIVE: RefCell<Option<Observer>> = const { RefCell::new(None) };
}
struct Restore {
    observer: Option<Observer>,
    active: bool,
}
impl Drop for Restore {
    fn drop(&mut self) {
        let slot = if self.active { &ACTIVE } else { &PENDING };
        slot.with(|slot| *slot.borrow_mut() = self.observer.take());
    }
}
pub(crate) fn with_observer<R>(observer: Observer, action: impl FnOnce() -> R) -> R {
    let previous = PENDING.with(|slot| slot.replace(Some(observer)));
    let guard = Restore {
        observer: previous,
        active: false,
    };
    let result = action();
    drop(guard);
    result
}
pub(crate) fn take_for_invocation() -> Option<Observer> {
    PENDING.with(|slot| slot.borrow_mut().take())
}
pub(crate) fn in_callback<R>(observer: Option<Observer>, action: impl FnOnce() -> R) -> R {
    let previous = ACTIVE.with(|slot| slot.replace(observer));
    let guard = Restore {
        observer: previous,
        active: true,
    };
    let result = action();
    drop(guard);
    result
}
pub(super) fn observe_actual(
    stage: &CheckedOutputTargetProductionCompilationPolicy8V1,
    entry_work: usize,
    budget: &mut Budget<'_>,
) -> Result<(), String> {
    ACTIVE.with(|slot| {
        if let Some(observer) = slot.borrow_mut().as_mut() {
            observer(
                View {
                    stage,
                    owner: match &stage.artifacts.admitted {
                        Admitted8::Direct(v) => Owner::Direct(v),
                        Admitted8::Erased(v) => Owner::Erased(v),
                    },
                    artifacts: &stage.artifacts,
                    profile: stage.bindings.rustc_target.profile(),
                    ranked: &stage.ranked_verification,
                    entry_work,
                    prepared_work: budget.work(),
                    prepared_floor: budget.storage(),
                },
                budget,
            )?;
        }
        Ok(())
    })
}

/// Consumes an actual unsigned source stage; never constructs a signed owner.
pub(crate) fn require_missing_signature(
    ranked: RankedVerifiedProductionCompilation,
    erased: bool,
    target: &str,
    budget: &mut Budget<'_>,
) -> Result<(), String> {
    let entry = budget.storage();
    scoped(entry, budget, move |budget| {
        let stage = ranked.lower_fixed_checked_output_policy8_with_budget_v1(budget)?;
        if matches!(&stage.artifacts.admitted, Admitted8::Erased(_)) != erased
            || stage.bindings.rustc_target.profile().device_target() != format!("{target}:xnack-")
            || stage.ranked_verification.roots().len() != 1 {
            return Err(execution_error("proof probe actual route/target/root"));
        }
        let expected = stage.ranked_verification.roots().first().ok_or_else(|| execution_error("proof probe actual source root"))?.semantic_root().index();
        let floor = budget.storage();
        let work = budget.work();
        let ledger = budget.work_ledger_identity_v1();
        let result = stage.prepare_native_worker_handoff_v1(budget);
        if !matches!(&result,
            Err(ProductionPipelineError::CheckedOutputPolicy8Stage(CheckedOutputPolicy8StageErrorV1::NativeSource(error)))
            if matches!(error.as_ref(), crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1::MissingSignedRankedReceipt { root } if *root == expected)) {
            return Err(execution_error("exact actual-source missing signed receipt"));
        }
        drop(result);
        if budget.storage() != floor || budget.work() <= work || budget.work_ledger_identity_v1() != ledger {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    }).map_err(|e| format!("{e:?}"))
}

#[test]
fn policy8_invocation_observer_moves_once_and_restores_on_panic_and_early_refusal() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Mark(Arc<AtomicUsize>);
    impl Drop for Mark {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let count = Arc::new(AtomicUsize::new(0));
    let make = || -> Observer {
        let mark = Mark(Arc::clone(&count));
        Box::new(move |_, _| {
            let _ = &mark;
            Err("no owner fabricated".into())
        })
    };
    with_observer(make(), || {});
    assert_eq!(count.load(Ordering::SeqCst), 1);
    with_observer(make(), || {
        with_observer(make(), || {
            drop(take_for_invocation());
        });
        let observer = take_for_invocation();
        assert!(observer.is_some() && take_for_invocation().is_none());
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                in_callback(observer, || {
                    assert!(ACTIVE.with(|slot| slot.borrow().is_some()));
                    std::panic::panic_any(73_u32);
                });
            }));
            assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 73);
            assert!(ACTIVE.with(|slot| slot.borrow().is_none()));
        })
        .join()
        .unwrap();
    });
    assert_eq!(count.load(Ordering::SeqCst), 3);
    assert!(take_for_invocation().is_none());
}
