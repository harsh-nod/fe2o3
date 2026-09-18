use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};

#[test]
fn native_xgmi_runtime_keeps_conservative_terminal_preflight_policy() {
    let mut roots: [Option<Box<i32>>; 2] = [None, None];
    let mut queues: [Option<()>; 2] = [None, None];
    let mut terminal = false;
    assert!(
        settle_xgmi_queue_creation(&mut roots, &mut queues, &mut terminal, 0, |_root| Err(()))
            .is_err()
    );
    assert!(terminal);
    assert!(roots.iter().all(Option::is_none));
    assert!(queues.iter().all(Option::is_none));
    assert!(std::mem::size_of::<[Gfx942NativeXgmiSdmaQueueCreationRootV1; 2]>() <= 4096);
}

#[test]
fn native_xgmi_success_installs_only_the_selected_direction() {
    for direction in 0..2 {
        let mut roots = [Some(Box::new(11)), Some(Box::new(22))];
        let pointers = roots
            .each_ref()
            .map(|root| &**root.as_ref().unwrap() as *const i32);
        let mut queues = [None, None];
        let mut terminal = false;
        settle_xgmi_queue_creation(&mut roots, &mut queues, &mut terminal, direction, |root| {
            Ok::<_, ()>(root.take().unwrap())
        })
        .unwrap();
        assert!(!terminal);
        assert!(roots[direction].is_none());
        assert_eq!(
            &**queues[direction].as_ref().unwrap() as *const _,
            pointers[direction]
        );
        assert_eq!(
            &**roots[1 - direction].as_ref().unwrap() as *const _,
            pointers[1 - direction]
        );
        assert!(queues[1 - direction].is_none());
    }
}

struct HostileDisplay;
impl fmt::Display for HostileDisplay {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic_any("diagnostic panic")
    }
}

#[test]
fn native_xgmi_error_latches_before_formatting_and_preserves_roots() {
    for direction in 0..2 {
        let mut roots = [Some(Box::new(11)), Some(Box::new(22))];
        let pointers = roots
            .each_ref()
            .map(|root| &**root.as_ref().unwrap() as *const i32);
        let mut queues: [Option<Box<i32>>; 2] = [None, None];
        queues[1 - direction] = Some(Box::new(33));
        let sibling = &**queues[1 - direction].as_ref().unwrap() as *const _;
        let mut terminal = false;
        let result = settle_xgmi_queue_creation(
            &mut roots,
            &mut queues,
            &mut terminal,
            direction,
            |_root| Err(HostileDisplay),
        );
        assert!(terminal);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            result.map_err(|error| format!("XGMI queue creation: {error}"))
        }))
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<&str>(), Some(&"diagnostic panic"));
        assert!(terminal);
        assert_eq!(
            roots
                .each_ref()
                .map(|root| &**root.as_ref().unwrap() as *const _),
            pointers
        );
        assert!(queues[direction].is_none());
        assert_eq!(
            &**queues[1 - direction].as_ref().unwrap() as *const _,
            sibling
        );
    }
}

#[test]
fn native_xgmi_panic_retains_selected_attempt_and_original_payload() {
    for direction in 0..2 {
        let mut roots = [None, None];
        let mut queues: [Option<()>; 2] = [None, None];
        let mut terminal = false;
        let owner = Box::new(7);
        let pointer = &*owner as *const _;
        let payload = Box::new(91);
        let payload_pointer = &*payload as *const _;
        let panic = catch_unwind(AssertUnwindSafe(|| {
            settle_xgmi_queue_creation(
                &mut roots,
                &mut queues,
                &mut terminal,
                direction,
                |root| -> Result<(), ()> {
                    *root = Some(owner);
                    panic_any(payload);
                },
            )
        }))
        .unwrap_err();
        assert!(terminal);
        assert_eq!(&**roots[direction].as_ref().unwrap() as *const _, pointer);
        assert!(roots[1 - direction].is_none());
        assert!(queues.iter().all(Option::is_none));
        assert_eq!(
            &**panic.downcast_ref::<Box<i32>>().unwrap() as *const _,
            payload_pointer
        );
    }
}

#[test]
fn native_xgmi_runtime_wires_persistent_roots_and_guards_teardown() {
    let source = include_str!("../../kfd_backend.rs");
    let backend = source
        .split("pub struct KfdNativeXgmiRuntimeBackendV1 {")
        .nth(1)
        .unwrap();
    assert!(
        backend
            .split("impl fmt::Debug")
            .next()
            .unwrap()
            .contains("queue_creation_roots: [Gfx942NativeXgmiSdmaQueueCreationRootV1; 2]")
    );
    let ensure = source
        .split("    fn ensure_queue(")
        .nth(1)
        .unwrap()
        .split("    fn restore_unmapped(")
        .next()
        .unwrap();
    assert!(
        ensure.find("self.require_live()?").unwrap()
            < ensure.find("self.queues[direction]").unwrap()
    );
    assert!(ensure.contains("&mut self.queue_creation_roots"));
    assert!(
        ensure.contains("Gfx942NativeXgmiSdmaQueueV1::create(source, destination, route, root)")
    );
    let drop = source
        .split("impl Drop for KfdNativeXgmiRuntimeBackendV1 {")
        .nth(1)
        .unwrap()
        .split("impl RuntimeBackendV1")
        .next()
        .unwrap();
    assert!(drop.find("!root.is_vacant()").unwrap() < drop.find("destroy_and_release").unwrap());
    let implementation = source
        .split("impl KfdNativeXgmiRuntimeBackendV1 {")
        .nth(1)
        .unwrap();
    let require = implementation
        .split("    fn require_live(")
        .nth(1)
        .unwrap()
        .split("    fn next_id(")
        .next()
        .unwrap();
    assert!(require.contains("!root.is_vacant()"));
    let shutdown = implementation
        .split("    pub fn shutdown_native_v1(")
        .nth(1)
        .unwrap()
        .split("impl RuntimeBackendV1")
        .next()
        .unwrap();
    assert!(
        shutdown.find("self.require_live()?").unwrap()
            < shutdown.find("destroy_and_release").unwrap()
    );
}
