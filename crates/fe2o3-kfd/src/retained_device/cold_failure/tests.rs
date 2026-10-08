#![cfg(test)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

struct Owner {
    original: Rc<()>,
    descriptors: [u64; 3],
    generation: u64,
    before_activation: bool,
    poisoned: bool,
    error: Option<DeviceBindingError>,
    closing_error: Option<DeviceBindingError>,
    panic_at: Option<&'static str>,
    panic_drops: Arc<AtomicUsize>,
    drops: Rc<Cell<usize>>,
    trace: Rc<RefCell<Vec<&'static str>>>,
    process_poison: Rc<Cell<bool>>,
}

impl Owner {
    fn new(error: Option<DeviceBindingError>) -> Self {
        Self {
            original: Rc::new(()),
            descriptors: [17, 23, 29],
            generation: 73,
            before_activation: true,
            poisoned: false,
            error,
            closing_error: None,
            panic_at: None,
            panic_drops: Arc::new(AtomicUsize::new(0)),
            drops: Rc::new(Cell::new(0)),
            trace: Rc::new(RefCell::new(Vec::new())),
            process_poison: Rc::new(Cell::new(false)),
        }
    }

    fn step(&self, name: &'static str) {
        self.trace.borrow_mut().push(name);
        if self.panic_at == Some(name) {
            // A real panic payload, not a manufactured error returned by check.
            std::panic::panic_any(CountedPanic(self.panic_drops.clone()));
        }
    }
}

// panic_any requires Send. Use a separate Send payload for actual panics;
// the owner itself deliberately stays !Send, as does the production token.
struct CountedPanic(Arc<AtomicUsize>);

impl Drop for CountedPanic {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.trace.borrow_mut().push("drop");
        self.drops.set(self.drops.get() + 1);
    }
}

impl ColdOwner for Owner {
    fn before_first_activation(&self) -> bool {
        self.before_activation
    }

    fn poisoned(&self) -> bool {
        self.poisoned
    }

    fn poison(&mut self) {
        self.poisoned = true;
        self.trace.borrow_mut().push("poison");
    }

    fn observe(&mut self) -> Result<(), DeviceBindingError> {
        self.step("observe");
        self.error.take().map_or(Ok(()), Err)
    }

    fn closing_process(&self) -> Result<(), DeviceBindingError> {
        self.step("closing");
        if self.closing_error.is_some() {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }
        Ok(())
    }
}

#[test]
fn clear_check_returns_the_one_original_without_disposal_or_generation_change() {
    let owner = Owner::new(None);
    let original = owner.original.clone();
    let drops = owner.drops.clone();
    let trace = owner.trace.clone();
    let Checked::Ready(owner) = check(owner) else {
        panic!("clear original must remain ready");
    };
    assert!(Rc::ptr_eq(&original, &owner.original));
    assert_eq!(owner.descriptors, [17, 23, 29]);
    assert_eq!(owner.generation, 73);
    assert!(!owner.poisoned);
    assert_eq!(drops.get(), 0);
    assert_eq!(*trace.borrow(), ["observe"]);
    drop(owner);
    assert_eq!(drops.get(), 1);
}

#[test]
fn new_reset_retains_exact_original_until_explicit_owner_drop() {
    let owner = Owner::new(Some(DeviceBindingError::WholeGpuResetObserved));
    let original = owner.original.clone();
    let drops = owner.drops.clone();
    let trace = owner.trace.clone();
    let Checked::Reset(owner) = check(owner) else {
        panic!("original newly observed reset required");
    };
    assert!(owner.poisoned);
    assert!(Rc::ptr_eq(&original, &owner.original));
    assert_eq!(owner.descriptors, [17, 23, 29]);
    assert_eq!(owner.generation, 73);
    assert_eq!(drops.get(), 0);
    assert_eq!(*trace.borrow(), ["observe", "closing", "poison"]);
    drop(owner);
    assert_eq!(drops.get(), 1);
}

#[test]
fn reset_followed_by_process_change_remains_unclassified_and_retained() {
    let mut owner = Owner::new(Some(DeviceBindingError::WholeGpuResetObserved));
    owner.closing_error = Some(DeviceBindingError::ProcessIncarnationChanged);
    let drops = owner.drops.clone();
    let trace = owner.trace.clone();
    let result = check(owner);
    assert!(matches!(
        &result,
        Checked::Refused {
            reason: Gfx942ColdDeviceRefusalReasonV1::Observation(
                DeviceBindingError::ProcessIncarnationChanged
            ),
            panic: None,
            ..
        }
    ));
    assert_eq!(drops.get(), 0);
    assert_eq!(*trace.borrow(), ["observe", "closing", "poison"]);
    drop(result);
    assert_eq!(drops.get(), 1);
}

#[test]
fn prior_activation_or_prior_poison_never_observes_or_classifies_a_reset() {
    for active in [true, false] {
        let mut owner = Owner::new(Some(DeviceBindingError::WholeGpuResetObserved));
        owner.before_activation = !active;
        owner.poisoned = !active;
        let trace = owner.trace.clone();
        let Checked::Refused { owner, reason, .. } = check(owner) else {
            panic!("precondition must refuse");
        };
        assert!(owner.poisoned);
        assert!(matches!(
            owner.error,
            Some(DeviceBindingError::WholeGpuResetObserved)
        ));
        assert_eq!(*trace.borrow(), ["poison"]);
        assert!(matches!(
            (active, reason),
            (
                true,
                Gfx942ColdDeviceRefusalReasonV1::NotBeforeFirstActivation
            ) | (false, Gfx942ColdDeviceRefusalReasonV1::AlreadyPoisoned)
        ));
    }
}

#[test]
fn syscall_protocol_topology_counter_and_prior_fence_errors_are_not_local_resets() {
    let errors = [
        DeviceBindingError::Syscall {
            operation: "scripted original subscription read",
            source: rustix::io::Errno::IO,
        },
        DeviceBindingError::ResetEventFenceProtocol,
        DeviceBindingError::CurrentnessFencePoisoned,
        DeviceBindingError::TopologySnapshotChanged,
        DeviceBindingError::ModelHistoryPoisoned,
        DeviceBindingError::ProcessIncarnationChanged,
        DeviceBindingError::ObservableCurrentnessChanged("DRM VRAM-loss counter"),
    ];
    for error in errors {
        let owner = Owner::new(Some(error));
        let trace = owner.trace.clone();
        let drops = owner.drops.clone();
        let result = check(owner);
        assert!(matches!(
            &result,
            Checked::Refused {
                reason: Gfx942ColdDeviceRefusalReasonV1::Observation(_),
                panic: None,
                ..
            }
        ));
        assert_eq!(*trace.borrow(), ["observe", "poison"]);
        assert_eq!(drops.get(), 0);
        drop(result);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn cold_observation_does_not_clear_an_existing_process_global_poison() {
    for already_poisoned in [false, true] {
        let failed = Owner::new(Some(DeviceBindingError::WholeGpuResetObserved));
        let gate = failed.process_poison.clone();
        gate.set(already_poisoned);
        let mut sibling = Owner::new(None);
        sibling.process_poison = gate.clone();
        let Checked::Reset(failed) = check(failed) else {
            panic!("new event required");
        };
        assert_eq!(gate.get(), already_poisoned);
        let Checked::Ready(sibling) = check(sibling) else {
            panic!("independent observation must remain unchanged");
        };
        // An observation success is not runtime admission. A previously
        // poisoned gate still refuses both old and newly checked devices.
        assert_eq!(!sibling.process_poison.get(), !already_poisoned);
        assert!(failed.poisoned);
        assert!(!sibling.poisoned);
    }
}

#[test]
fn observer_and_closing_panics_retain_original_owner_and_actual_panic_payload() {
    for phase in ["observe", "closing"] {
        let mut owner = Owner::new(Some(DeviceBindingError::WholeGpuResetObserved));
        owner.panic_at = Some(phase);
        let original = owner.original.clone();
        let drops = owner.drops.clone();
        let panic_drops = owner.panic_drops.clone();
        let trace = owner.trace.clone();
        let result = check(owner);
        let Checked::Refused {
            owner,
            reason,
            panic,
        } = &result
        else {
            panic!("panic is never a local reset classification");
        };
        assert!(matches!(
            reason,
            Gfx942ColdDeviceRefusalReasonV1::ObservationPanicked
        ));
        assert!(panic.as_ref().unwrap().is::<CountedPanic>());
        assert!(owner.poisoned);
        assert!(Rc::ptr_eq(&original, &owner.original));
        assert_eq!(drops.get(), 0);
        assert_eq!(panic_drops.load(Ordering::SeqCst), 0);
        let expected: &[&str] = if phase == "observe" {
            &["observe", "poison"]
        } else {
            &["observe", "closing", "poison"]
        };
        assert_eq!(&*trace.borrow(), expected);
        drop(result);
        assert_eq!(drops.get(), 1);
        assert_eq!(panic_drops.load(Ordering::SeqCst), 1);
    }
}
