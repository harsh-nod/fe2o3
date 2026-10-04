//! Test-only wall-clock observations. No graph, budget or admission input.
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Route {
    Direct,
    SilentUnitErased,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Phase {
    TargetBindAndAdmit,
    Optimizer,
    RankedSourceReplay,
    FinalAdmission,
    ArtifactPreparation,
}

impl Route {
    pub(crate) const fn phases(self) -> [Phase; 5] {
        use Phase::*;
        match self {
            Self::Direct => [
                TargetBindAndAdmit,
                Optimizer,
                RankedSourceReplay,
                FinalAdmission,
                ArtifactPreparation,
            ],
            Self::SilentUnitErased => [
                RankedSourceReplay,
                TargetBindAndAdmit,
                Optimizer,
                FinalAdmission,
                ArtifactPreparation,
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PhaseOutcome {
    Complete,
    Refused,
    Panicked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Event {
    Started {
        route: Route,
        phase: Phase,
    },
    Finished {
        route: Route,
        phase: Phase,
        outcome: PhaseOutcome,
        elapsed_millis: u64,
    },
}

pub(crate) type Sink = Rc<dyn Fn(Event)>;

struct Observer {
    sink: Sink,
    failed: Cell<bool>,
    emitting: Cell<bool>,
}

thread_local! {
    static OBSERVER: RefCell<Option<Rc<Observer>>> = const { RefCell::new(None) };
}

// An observer's panic payload may itself panic on Drop. Disable that observer
// and leak only the exceptional payload instead of replacing a compiler panic.
fn isolate(action: impl FnOnce()) -> bool {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)) {
        Ok(()) => true,
        Err(payload) => {
            std::mem::forget(payload);
            false
        }
    }
}

impl Observer {
    fn emit(&self, event: Event) {
        if self.failed.get() {
            return;
        }
        if self.emitting.replace(true) {
            self.failed.set(true);
            return;
        }
        let succeeded = isolate(|| (self.sink)(event));
        self.emitting.set(false);
        if !succeeded {
            self.failed.set(true);
        }
    }
}

struct Restore(Option<Rc<Observer>>);

impl Drop for Restore {
    fn drop(&mut self) {
        let removed = OBSERVER.with(|slot| slot.replace(self.0.take()));
        // Restoring TLS precedes dropping the removed closure and its captures.
        isolate(|| drop(removed));
    }
}

/// Install on the actual callback thread. Nested scopes restore the prior sink.
pub(crate) fn with_sink<R>(sink: Sink, action: impl FnOnce() -> R) -> R {
    let next = Rc::new(Observer {
        sink,
        failed: Cell::new(false),
        emitting: Cell::new(false),
    });
    let _restore = Restore(OBSERVER.with(|slot| slot.replace(Some(next))));
    action()
}

struct Active {
    observer: Rc<Observer>,
    route: Route,
    phase: Phase,
    started: Instant,
}

pub(crate) struct PhaseGuard(Option<Active>);

/// No installed observer means no allocation, clock read or emitted event.
pub(crate) fn begin(route: Route, phase: Phase) -> PhaseGuard {
    let observer = OBSERVER.with(|slot| slot.borrow().clone());
    let Some(observer) = observer else {
        return PhaseGuard(None);
    };
    observer.emit(Event::Started { route, phase });
    if observer.failed.get() {
        return PhaseGuard(None);
    }
    PhaseGuard(Some(Active {
        observer,
        route,
        phase,
        // Exclude the start event's progress-file publication.
        started: Instant::now(),
    }))
}

impl PhaseGuard {
    pub(crate) fn complete(mut self) {
        self.finish(PhaseOutcome::Complete);
    }

    fn finish(&mut self, outcome: PhaseOutcome) {
        let Some(active) = self.0.take() else {
            return;
        };
        let elapsed_millis =
            u64::try_from(active.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        active.observer.emit(Event::Finished {
            route: active.route,
            phase: active.phase,
            outcome,
            elapsed_millis,
        });
        isolate(|| drop(active));
    }
}

impl Drop for PhaseGuard {
    fn drop(&mut self) {
        self.finish(if std::thread::panicking() {
            PhaseOutcome::Panicked
        } else {
            PhaseOutcome::Refused
        });
    }
}

#[path = "production_pipeline_checked_output_progress_lifecycle_v1_tests.rs"]
mod lifecycle;
