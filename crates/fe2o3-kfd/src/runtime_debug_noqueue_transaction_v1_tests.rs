//! CPU-only transaction injection. No device, Kernel, queue or syscall is fabricated.
use super::*;
use std::{cell::Cell, rc::Rc};
use crate::engineering_gfx950::debug_cold::retention::RetainNativeOnDropV1;

#[derive(Clone, Copy, Debug)]
enum Failure {
    None,
    Register,
    Enable,
    Output,
    Check(usize),
    PanicRegister,
    PanicEnable,
    PanicCheck(usize),
}
struct FakeTransport {
    failure: Failure,
    calls: Vec<&'static str>,
    checks: usize,
    root: u64,
    observed: Vec<(i32, bool)>,
}
impl FakeTransport {
    fn new(failure: Failure) -> Self {
        Self {
            failure,
            calls: Vec::new(),
            checks: 0,
            root: 0,
            observed: Vec::new(),
        }
    }
}
impl DebugActivationTransportV1 for FakeTransport {
    fn check_currentness(&mut self) -> Result<(), MetadataErrorV1> {
        self.calls.push("check");
        self.checks += 1;
        if self.root != 0 {
            // SAFETY: supplied only by the live actual MetadataStorage in this
            // synchronous transaction. No fabricated address is dereferenced.
            let root = unsafe { &*(self.root as usize as *const abi::RDebugAbiV11) };
            assert_eq!(root.version, 11);
            assert_ne!(root.breakpoint, 0);
            self.observed.push((root.state, root.map != 0));
        }
        match self.failure {
            Failure::Check(n) if n == self.checks => Err(MetadataErrorV1::Currentness),
            Failure::PanicCheck(n) if n == self.checks => panic!("injected currentness unwind"),
            _ => Ok(()),
        }
    }
    fn register_trap(&mut self, trap_base: u64, gpu_id: u32) -> Result<(), MetadataErrorV1> {
        self.calls.push("register");
        // Synthetic inert transport values only, never passed to a kernel.
        assert_eq!(trap_base, 0x4000);
        assert_eq!(gpu_id, 7);
        match self.failure {
            Failure::Register => Err(MetadataErrorV1::NativeTrap),
            Failure::PanicRegister => panic!("injected registration unwind"),
            _ => Ok(()),
        }
    }
    fn enable_runtime(&mut self, root_address: u64) -> Result<(), MetadataErrorV1> {
        self.calls.push("enable");
        self.root = root_address;
        assert_ne!(root_address, 0);
        assert_eq!(root_address % 8, 0);
        // SAFETY: the private engine derives this from its retained live Record.
        let root = unsafe { &*(root_address as usize as *const abi::RDebugAbiV11) };
        assert_eq!(root.version, 11);
        assert_eq!(root.map, 0);
        assert_eq!(root.state, abi::RT_CONSISTENT_V1);
        assert_eq!(root.reserved0, 0);
        assert_eq!(root.reserved1, 0);
        assert_eq!(root.loader_base, 0);
        match self.failure {
            Failure::Enable => Err(MetadataErrorV1::NativeRuntime),
            Failure::Output => Err(MetadataErrorV1::RuntimeOutput),
            Failure::PanicEnable => panic!("injected runtime unwind"),
            _ => Ok(()),
        }
    }
}
fn storage() -> MetadataStorageV1 {
    MetadataStorageV1::prepare(b"retained-original-elf-test-bytes", -4096).unwrap()
}
struct Marker(Rc<Cell<usize>>);
impl Drop for Marker {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
struct SyntheticCustody {
    storage: MetadataStorageV1,
    _kernel_mapping: Marker,
    _trap_mapping: Marker,
    _device_and_fd: Marker,
}
fn custody() -> (RetainNativeOnDropV1<SyntheticCustody>, Rc<Cell<usize>>) {
    let dropped = Rc::new(Cell::new(0));
    let mut held = RetainNativeOnDropV1::new(SyntheticCustody {
        storage: storage(),
        _kernel_mapping: Marker(dropped.clone()),
        _trap_mapping: Marker(dropped.clone()),
        _device_and_fd: Marker(dropped.clone()),
    });
    // Mirrors the actual cold owner: armed before its first VM effect, not
    // retrospectively after successful metadata registration.
    held.retain_before_native_effect();
    (held, dropped)
}
#[test]
fn successful_transaction_uses_exact_owned_root_and_real_list_protocol() {
    let _notification_lock = super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    let mut stored = storage();
    let root_before = &stored.record[0].root as *const _ as usize as u64;
    let elf_before = stored.original_elf.as_ptr();
    let uri_before = stored.record[0].link.name;
    let mut transport = FakeTransport::new(Failure::None);
    stored.activate_no_queue(0x4000, 7, &mut transport).unwrap();
    assert_eq!(
        transport.calls,
        [
            "check", "register", "check", "enable", "check", "check", "check", "check", "check"
        ]
    );
    assert_eq!(transport.root, root_before);
    assert_eq!(
        transport.observed,
        [(0, false), (1, false), (1, false), (0, true), (0, true)]
    );
    assert_eq!(stored.phase, Phase::ActivePresent);
    assert_eq!(
        stored.record[0].root.map,
        &stored.record[0].link as *const _ as usize as u64
    );
    let moved = Box::new(stored);
    assert_eq!(
        transport.root,
        &moved.record[0].root as *const _ as usize as u64
    );
    assert_eq!(moved.original_elf.as_ptr(), elf_before);
    assert_eq!(moved.record[0].link.name, uri_before);
    assert_eq!(moved.record[0].link.load_bias, (-4096_i64) as u64);
    assert_eq!(moved.record[0].link.next, 0);
    assert_eq!(moved.record[0].link.previous, 0);
    assert_eq!(moved.record[0].link.dynamic, 0);
}
#[test]
fn repeated_activation_and_bad_addresses_never_call_transport() {
    let _notification_lock = super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    for (address, gpu) in [(0, 7), (1, 7), (0x4001, 7), (0x4000, 0)] {
        let mut stored = storage();
        let mut transport = FakeTransport::new(Failure::None);
        assert_eq!(
            stored.activate_no_queue(address, gpu, &mut transport),
            Err(MetadataErrorV1::Mapping)
        );
        assert!(transport.calls.is_empty());
        assert!(stored.is_prepared());
    }
    let mut stored = storage();
    let mut transport = FakeTransport::new(Failure::None);
    stored.activate_no_queue(0x4000, 7, &mut transport).unwrap();
    transport.calls.clear();
    assert_eq!(
        stored.activate_no_queue(0x4000, 7, &mut transport),
        Err(MetadataErrorV1::Transition)
    );
    assert!(transport.calls.is_empty());
}
#[test]
fn inherited_metadata_refuses_before_any_native_attempt() {
    let _notification_lock = super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    let mut stored = storage();
    stored.opener_pid = stored.opener_pid.wrapping_add(1);
    let mut transport = FakeTransport::new(Failure::None);
    assert_eq!(
        stored.activate_no_queue(0x4000, 7, &mut transport),
        Err(MetadataErrorV1::Transition)
    );
    assert!(transport.calls.is_empty());
}
#[test]
fn syscall_refusals_keep_all_custody_and_forbid_retry() {
    let _notification_lock = super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    for failure in [Failure::Register, Failure::Enable, Failure::Output] {
        let (mut held, dropped) = custody();
        let mut transport = FakeTransport::new(failure);
        assert!(
            held.get_mut()
                .storage
                .activate_no_queue(0x4000, 7, &mut transport)
                .is_err()
        );
        assert_eq!(held.get().storage.phase, Phase::Poisoned);
        let calls = transport.calls.len();
        assert_eq!(
            held.get_mut()
                .storage
                .activate_no_queue(0x4000, 7, &mut transport),
            Err(MetadataErrorV1::Transition)
        );
        assert_eq!(transport.calls.len(), calls);
        drop(held);
        assert_eq!(dropped.get(), 0);
    }
}
#[test]
fn every_currentness_boundary_refuses_without_freeing_native_custody() {
    let _notification_lock = super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    for check in 1..=7 {
        let (mut held, dropped) = custody();
        let mut transport = FakeTransport::new(Failure::Check(check));
        assert_eq!(
            held.get_mut()
                .storage
                .activate_no_queue(0x4000, 7, &mut transport),
            Err(MetadataErrorV1::Currentness)
        );
        assert_eq!(transport.checks, check);
        assert_eq!(
            held.get().storage.phase,
            if check == 1 {
                Phase::Prepared
            } else {
                Phase::Poisoned
            }
        );
        drop(held);
        assert_eq!(dropped.get(), 0);
    }
}
#[test]
fn every_injected_unwind_keeps_all_native_custody() {
    let _notification_lock = super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    let failures = [
        Failure::PanicRegister,
        Failure::PanicEnable,
        Failure::PanicCheck(1),
        Failure::PanicCheck(2),
        Failure::PanicCheck(3),
        Failure::PanicCheck(4),
        Failure::PanicCheck(5),
        Failure::PanicCheck(6),
        Failure::PanicCheck(7),
    ];
    for failure in failures {
        let (mut held, dropped) = custody();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            held.get_mut()
                .storage
                .activate_no_queue(0x4000, 7, &mut FakeTransport::new(failure))
        }));
        assert!(result.is_err());
        assert_eq!(dropped.get(), 0);
    }
}
#[test]
fn ordinary_success_drop_retains_the_same_entire_custody() {
    let _notification_lock = super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    let (mut held, dropped) = custody();
    held.get_mut()
        .storage
        .activate_no_queue(0x4000, 7, &mut FakeTransport::new(Failure::None))
        .unwrap();
    drop(held);
    assert_eq!(dropped.get(), 0);
}
#[test]
fn failed_registration_does_not_advertise_version_eleven() {
    let _notification_lock = super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    let mut stored = storage();
    assert_eq!(
        stored.activate_no_queue(0x4000, 7, &mut FakeTransport::new(Failure::Register)),
        Err(MetadataErrorV1::NativeTrap)
    );
    assert_eq!(stored.record[0].root.version, 0);
    assert_eq!(stored.record[0].root.map, 0);
    assert!(stored.take_storage_to_retain().is_some());
}
#[test]
fn failed_runtime_keeps_absent_list_and_original_elf_together() {
    let _notification_lock = super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    let mut stored = storage();
    assert_eq!(
        stored.activate_no_queue(0x4000, 7, &mut FakeTransport::new(Failure::Enable)),
        Err(MetadataErrorV1::NativeRuntime)
    );
    assert_eq!(stored.record[0].root.version, 11);
    assert_eq!(stored.record[0].root.map, 0);
    let (record, elf) = stored.take_storage_to_retain().unwrap();
    let uri = std::str::from_utf8(&record[0].uri)
        .unwrap()
        .trim_end_matches('\0');
    assert_eq!(
        uri,
        format!(
            "memory://{}#offset=0x{:x}&size={}",
            std::process::id(),
            elf.as_ptr() as usize,
            elf.len()
        )
    );
    assert_eq!(record[0].link.name, record[0].uri.as_ptr() as usize as u64);
    assert_eq!(elf, b"retained-original-elf-test-bytes");
    // Fake transport alone exposed no actual native pointers; this extracted
    // synthetic pair may be reclaimed explicitly after checking its relationship.
}
