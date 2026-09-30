use super::*;
use crate::kfd_backend::{settle_xgmi_queue_creation, settle_xgmi_queue_retirement};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

struct Probe {
    name: &'static str,
    drops: Rc<RefCell<Vec<&'static str>>>,
}

impl Probe {
    fn new(name: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Self {
        Self {
            name,
            drops: Rc::clone(drops),
        }
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.name);
    }
}

fn populated(drops: &Rc<RefCell<Vec<&'static str>>>) -> NativeCustody<Probe, Box<Probe>> {
    let mut storage = NativeCustody::new([
        Probe::new("session-0", drops),
        Probe::new("session-1", drops),
    ]);
    let (_, queues) = storage.parts_mut();
    *queues = [
        Some(Box::new(Probe::new("queue-0", drops))),
        Some(Box::new(Probe::new("queue-1", drops))),
    ];
    storage
}

#[test]
fn native_custody_preserves_argument_order_and_empty_queue_slots() {
    let mut storage = NativeCustody::<_, Box<i32>>::new([Box::new(11), Box::new(22)]);
    let addresses = storage
        .sessions()
        .each_ref()
        .map(|value| &**value as *const i32);
    assert_eq!(storage.sessions().each_ref().map(|value| **value), [11, 22]);
    assert!(storage.queues().iter().all(Option::is_none));
    *storage.sessions_mut()[1] = 33;
    let (sessions, queues) = storage.parts_mut();
    *sessions[0] = 44;
    queues[1] = Some(Box::new(55));
    assert_eq!(storage.sessions().each_ref().map(|value| **value), [44, 33]);
    assert_eq!(
        storage
            .sessions()
            .each_ref()
            .map(|value| &**value as *const i32),
        addresses
    );
    assert!(storage.queues()[0].is_none());
    assert_eq!(storage.queues()[1].as_deref(), Some(&55));
}

#[test]
fn native_custody_creation_installs_only_selected_queue_without_dropping_sessions() {
    for direction in 0..2 {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let mut storage = NativeCustody::new([
            Probe::new("session-0", &drops),
            Probe::new("session-1", &drops),
        ]);
        let mut roots = [0, 0];
        let mut terminal = false;
        let (sessions, queues) = storage.parts_mut();
        settle_xgmi_queue_creation(&mut roots, queues, &mut terminal, direction, |root| {
            assert_eq!(
                sessions[direction].name,
                if direction == 0 {
                    "session-0"
                } else {
                    "session-1"
                }
            );
            *root = 1;
            Ok::<_, ()>(Box::new(Probe::new("created", &drops)))
        })
        .unwrap();
        assert!(!terminal);
        assert_eq!(roots[direction], 1);
        assert_eq!(roots[1 - direction], 0);
        assert_eq!(
            storage.queues()[direction].as_ref().unwrap().name,
            "created"
        );
        assert!(storage.queues()[1 - direction].is_none());
        assert!(drops.borrow().is_empty());
        drop(storage);
        assert_eq!(*drops.borrow(), ["session-0", "session-1", "created"]);
    }
}

#[test]
fn native_custody_creation_error_and_unwind_leave_sessions_and_slots_rooted() {
    for direction in 0..2 {
        for panic in [false, true] {
            let drops = Rc::new(RefCell::new(Vec::new()));
            let mut storage = NativeCustody::<_, Box<Probe>>::new([
                Probe::new("session-0", &drops),
                Probe::new("session-1", &drops),
            ]);
            let mut roots = [0, 0];
            let mut terminal = false;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let (sessions, queues) = storage.parts_mut();
                settle_xgmi_queue_creation(&mut roots, queues, &mut terminal, direction, |root| {
                    assert_eq!(sessions.len(), 2);
                    *root = 7;
                    if panic {
                        std::panic::panic_any(71_u32);
                    }
                    Err::<Box<Probe>, _>(17_u32)
                })
            }));
            assert!(terminal);
            if panic {
                assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 71);
            } else {
                assert_eq!(result.unwrap(), Err(17));
            }
            assert_eq!(roots[direction], 7);
            assert!(storage.queues().iter().all(Option::is_none));
            assert!(drops.borrow().is_empty());
            drop(storage);
            assert_eq!(*drops.borrow(), ["session-0", "session-1"]);
        }
    }
}

#[test]
fn native_custody_retirement_error_and_unwind_retain_exact_queue_and_both_sessions() {
    for direction in 0..2 {
        for panic in [false, true] {
            let drops = Rc::new(RefCell::new(Vec::new()));
            let mut storage = populated(&drops);
            let addresses = storage
                .queues()
                .each_ref()
                .map(|queue| &**queue.as_ref().unwrap() as *const Probe);
            let mut terminal = false;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let (sessions, queues) = storage.parts_mut();
                settle_xgmi_queue_retirement(queues, &mut terminal, direction, |queue| {
                    assert_eq!(sessions.len(), 2);
                    assert_eq!(&**queue as *const Probe, addresses[direction]);
                    if panic {
                        std::panic::panic_any(81_u32);
                    }
                    Err::<(), _>(18_u32)
                })
            }));
            assert!(terminal);
            if panic {
                assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 81);
            } else {
                assert_eq!(result.unwrap(), Err(18));
            }
            assert_eq!(
                storage
                    .queues()
                    .each_ref()
                    .map(|queue| &**queue.as_ref().unwrap() as *const Probe),
                addresses
            );
            assert!(drops.borrow().is_empty());
            drop(storage);
            assert_eq!(
                *drops.borrow(),
                ["session-0", "session-1", "queue-0", "queue-1"]
            );
        }
    }
}

#[test]
fn native_custody_successful_reverse_retirement_precedes_session_drop() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut storage = populated(&drops);
    let mut terminal = false;
    for direction in (0..2).rev() {
        let (sessions, queues) = storage.parts_mut();
        settle_xgmi_queue_retirement(queues, &mut terminal, direction, |_| {
            assert_eq!(sessions[0].name, "session-0");
            assert_eq!(sessions[1].name, "session-1");
            Ok::<(), ()>(())
        })
        .unwrap();
        assert!(storage.queues()[direction].is_none());
    }
    assert!(!terminal);
    assert_eq!(*drops.borrow(), ["queue-1", "queue-0"]);
    drop(storage);
    assert_eq!(
        *drops.borrow(),
        ["queue-1", "queue-0", "session-0", "session-1"]
    );
}

#[test]
fn native_custody_ordinary_field_drop_order_matches_former_backend_fields() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    drop(populated(&drops));
    assert_eq!(
        *drops.borrow(),
        ["session-0", "session-1", "queue-0", "queue-1"]
    );
}
