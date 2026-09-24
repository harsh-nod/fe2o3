use super::*;
use crate::native_tests::allocation;

const CHILD: i32 = 0x01020304;
const PARENT: i32 = 0x05060708;

fn fixture() -> NamespaceSet {
    let mut set = NamespaceSet {
        identities: [ZERO_IDENTITY; NAMESPACES.len()],
    };
    for (index, identity) in set.identities.iter_mut().enumerate() {
        *identity = NamespaceIdentity {
            device: 0x0102030405060708 + index as u64,
            inode: 0x1112131415161718 + index as u64,
        };
    }
    set.identities[3] = set.identities[2];
    set.identities[9] = set.identities[8];
    set
}

fn frame(set: &NamespaceSet) -> [u8; BYTES] {
    let mut bytes = [0xa5; BYTES];
    encode(CHILD as u32, PARENT as u32, set, &mut bytes);
    bytes
}

fn check(set: &NamespaceSet, bytes: &[u8]) -> Result<(), Error> {
    set.require_child_report(
        Pid::from_raw(CHILD).unwrap(),
        Pid::from_raw(PARENT).unwrap(),
        bytes,
    )
}

#[test]
fn golden_header_and_each_identity_have_fixed_little_endian_offsets() {
    let set = fixture();
    let bytes = frame(&set);
    assert_eq!(
        &bytes[..HEADER_BYTES],
        &[
            b'F', b'2', b'O', b'3', b'N', b'S', b'R', b'1', 1, 0, 0, 0, 192, 0, 0, 0, 4, 3, 2, 1,
            8, 7, 6, 5, 10, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
    for (index, bytes) in bytes[HEADER_BYTES..].chunks_exact(16).enumerate() {
        let identity_index = match index {
            3 => 2,
            9 => 8,
            other => other,
        };
        assert_eq!(
            &bytes[..8],
            &[8 + identity_index as u8, 7, 6, 5, 4, 3, 2, 1]
        );
        assert_eq!(
            &bytes[8..],
            &[24 + identity_index as u8, 23, 22, 21, 20, 19, 18, 17]
        );
    }
    let (result, allocations) = allocation::count(|| check(&set, &bytes));
    assert_eq!(result, Ok(()));
    assert_eq!(allocations, 0);
}

#[test]
fn every_single_byte_mutation_and_every_truncation_refuse_without_allocating() {
    let set = fixture();
    let original = frame(&set);
    for index in 0..BYTES {
        let mut bytes = original;
        bytes[index] ^= 1;
        let (result, allocations) = allocation::count(|| check(&set, &bytes));
        assert!(result.is_err(), "byte {index}");
        assert_eq!(allocations, 0);
    }
    for length in 0..BYTES {
        assert_eq!(
            check(&set, &original[..length]),
            Err(Error::InvalidState("namespace-report length differs"))
        );
    }
    for bytes in [&[0xa5][..], &[0; BYTES + 1][..], &[0; 2 * BYTES][..]] {
        assert_eq!(
            check(&set, bytes),
            Err(Error::InvalidState("namespace-report length differs"))
        );
    }
}

#[test]
fn invalid_pid_encodings_and_exact_expected_pid_bindings_refuse() {
    let set = fixture();
    for (offset, message) in [
        (16, "invalid namespace-report child PID"),
        (20, "invalid namespace-report parent PID"),
    ] {
        for value in [0_u32, i32::MAX as u32 + 1, u32::MAX] {
            let mut bytes = frame(&set);
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert_eq!(check(&set, &bytes), Err(Error::InvalidState(message)));
        }
    }
    for value in [1, i32::MAX] {
        let pid = Pid::from_raw(value).unwrap();
        let mut bytes = [0; BYTES];
        encode(value as u32, value as u32, &set, &mut bytes);
        assert_eq!(set.require_child_report(pid, pid, &bytes), Ok(()));
    }
    let child = Pid::from_raw(CHILD).unwrap();
    let parent = Pid::from_raw(PARENT).unwrap();
    assert_eq!(
        set.require_child_report(parent, parent, &frame(&set)),
        Err(Error::InvalidState("namespace-report child PID differs"))
    );
    assert_eq!(
        set.require_child_report(child, child, &frame(&set)),
        Err(Error::InvalidState("namespace-report parent PID differs"))
    );
}

#[test]
fn namespace_comparison_covers_both_coordinates_and_child_constraints_in_order() {
    let set = fixture();
    for (index, (name, _)) in NAMESPACES.iter().enumerate() {
        for inode in [false, true] {
            let mut changed = fixture();
            if inode {
                changed.identities[index].inode ^= 1;
            } else {
                changed.identities[index].device ^= 1;
            }
            let expected = match index {
                2 | 3 => "pid-for-children",
                8 | 9 => "time-for-children",
                _ => name,
            };
            assert_eq!(
                check(&set, &frame(&changed)),
                Err(Error::Namespace(expected))
            );
        }
    }
    for (active, pending, name) in [(2, 3, "pid"), (8, 9, "time")] {
        let mut changed = fixture();
        changed.identities[active].inode ^= 1;
        changed.identities[pending] = changed.identities[active];
        assert_eq!(check(&set, &frame(&changed)), Err(Error::Namespace(name)));
    }
    let mut changed = fixture();
    changed.identities[3].inode ^= 1;
    changed.identities[9].inode ^= 1;
    assert_eq!(
        check(&set, &frame(&changed)),
        Err(Error::Namespace("pid-for-children"))
    );
    changed.identities[3] = changed.identities[2];
    assert_eq!(
        check(&set, &frame(&changed)),
        Err(Error::Namespace("time-for-children"))
    );
    changed = fixture();
    changed.identities[0].inode ^= 1;
    changed.identities[1].inode ^= 1;
    assert_eq!(check(&set, &frame(&changed)), Err(Error::Namespace("user")));
}

#[test]
fn framing_validation_precedes_bindings_then_child_constraints_then_baseline() {
    let set = fixture();
    let mut bytes = frame(&set);
    bytes[32] ^= 1;
    bytes[80] ^= 1;
    bytes[16..20].fill(0);
    bytes[24] = 0;
    assert_eq!(
        check(&set, &bytes),
        Err(Error::InvalidState("namespace-report count differs"))
    );
    bytes[10] = 1;
    assert_eq!(
        check(&set, &bytes),
        Err(Error::InvalidState(
            "namespace-report reserved bytes differ"
        ))
    );
    bytes[12] = 0;
    assert_eq!(
        check(&set, &bytes),
        Err(Error::InvalidState(
            "namespace-report declared length differs"
        ))
    );
    bytes[8] = 0;
    assert_eq!(
        check(&set, &bytes),
        Err(Error::InvalidState("namespace-report version differs"))
    );
    bytes[0] = 0;
    assert_eq!(
        check(&set, &bytes),
        Err(Error::InvalidState("namespace-report magic differs"))
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Call {
    Unused,
    Pid,
    Parent,
    Open,
    Stat,
    Close,
}

struct Fake {
    calls: [Call; 32],
    used: usize,
    fail_at: Option<usize>,
    fail_close: bool,
    child: i32,
    parent: i32,
    namespace: usize,
    set: NamespaceSet,
}

impl Fake {
    fn new() -> Self {
        Self {
            calls: [Call::Unused; 32],
            used: 0,
            fail_at: None,
            fail_close: false,
            child: CHILD,
            parent: PARENT,
            namespace: 0,
            set: fixture(),
        }
    }
    fn call(&mut self, call: Call) -> Result<(), Errno> {
        self.calls[self.used] = call;
        self.used += 1;
        if self.fail_at == Some(self.used) {
            Err(Errno::INTR)
        } else if call == Call::Close && self.fail_close {
            Err(Errno::IO)
        } else {
            Ok(())
        }
    }
}

impl ChildSyscalls for Fake {
    fn getpid(&mut self) -> Result<i32, Errno> {
        self.call(Call::Pid)?;
        Ok(self.child)
    }
    fn getppid(&mut self) -> Result<i32, Errno> {
        self.call(Call::Parent)?;
        Ok(self.parent)
    }
    fn open(&mut self, path: &[u8; PROC_PATH_BYTES]) -> Result<i32, Errno> {
        assert_eq!(path, &CHILD_PATHS[self.namespace]);
        self.call(Call::Open)?;
        self.namespace += 1;
        Ok(self.namespace as i32 + 100)
    }
    fn stat(&mut self, fd: i32, out: &mut RawStat) -> Result<(), Errno> {
        assert_eq!(fd, self.namespace as i32 + 100);
        self.call(Call::Stat)?;
        let identity = self.set.identities[self.namespace - 1];
        out.device = identity.device;
        out.inode = identity.inode;
        Ok(())
    }
    fn close(&mut self, fd: i32) -> Result<(), Errno> {
        assert_eq!(fd, self.namespace as i32 + 100);
        self.call(Call::Close)
    }
}

#[test]
fn collector_has_exact_32_call_schedule_and_no_allocation_or_drop_staging() {
    let mut calls = Fake::new();
    let mut bytes = [0; BYTES];
    let (result, allocations) = allocation::count(|| capture_with(&mut calls, PARENT, &mut bytes));
    assert_eq!(result, Ok(()));
    assert_eq!(allocations, 0);
    assert_eq!(calls.used, 32);
    assert_eq!(&calls.calls[..2], &[Call::Pid, Call::Parent]);
    for triple in calls.calls[2..].chunks_exact(3) {
        assert_eq!(triple, &[Call::Open, Call::Stat, Call::Close]);
    }
    assert_eq!(bytes, frame(&fixture()));
    assert!(!needs_drop::<Fake>());
    assert!(!needs_drop::<RawSyscalls>());
    assert!(!needs_drop::<Result<(), Error>>());
}

#[test]
fn every_syscall_failure_refuses_without_retries_and_stat_failure_still_closes() {
    for fail_at in 1..=32 {
        let mut calls = Fake::new();
        calls.fail_at = Some(fail_at);
        let mut bytes = [0xa5; BYTES];
        let (result, allocations) =
            allocation::count(|| capture_with(&mut calls, PARENT, &mut bytes));
        assert!(matches!(
            result,
            Err(Error::Io {
                source: Errno::INTR,
                ..
            })
        ));
        assert_eq!(allocations, 0);
        let failed_stat = fail_at >= 4 && (fail_at - 4).is_multiple_of(3);
        assert_eq!(calls.used, fail_at + usize::from(failed_stat));
        if failed_stat {
            assert_eq!(calls.calls[calls.used - 1], Call::Close);
        }
        assert_eq!(bytes, [0xa5; BYTES]);
    }
    let mut calls = Fake::new();
    calls.fail_at = Some(4);
    calls.fail_close = true;
    assert_eq!(
        capture_with(&mut calls, PARENT, &mut [0; BYTES]),
        Err(io_error("inspect child proc namespace", Errno::INTR))
    );
    assert_eq!(calls.used, 5);
}

#[test]
fn collector_rejects_bad_pids_before_open_and_checks_child_namespace_constraints() {
    for expected in [0, -1, i32::MIN] {
        let mut calls = Fake::new();
        assert!(capture_with(&mut calls, expected, &mut [0; BYTES]).is_err());
        assert_eq!(calls.used, 0);
    }
    for child in [0, -1, i32::MIN] {
        let mut calls = Fake::new();
        calls.child = child;
        assert!(capture_with(&mut calls, PARENT, &mut [0; BYTES]).is_err());
        assert_eq!(calls.used, 1);
    }
    for parent in [0, -1, PARENT - 1] {
        let mut calls = Fake::new();
        calls.parent = parent;
        assert!(capture_with(&mut calls, PARENT, &mut [0; BYTES]).is_err());
        assert_eq!(calls.used, 2);
    }
    for (index, name) in [(3, "pid-for-children"), (9, "time-for-children")] {
        let mut calls = Fake::new();
        calls.set.identities[index].inode ^= 1;
        let mut bytes = [0xa5; BYTES];
        assert_eq!(
            capture_with(&mut calls, PARENT, &mut bytes),
            Err(Error::Namespace(name))
        );
        assert_eq!(calls.used, 32);
        assert_eq!(bytes, [0xa5; BYTES]);
    }
}

#[test]
fn raw_stat_layout_paths_and_full_cost_formulas_are_exact() {
    assert_eq!(size_of::<RawStat>(), 144);
    assert_eq!(align_of::<RawStat>(), 8);
    assert_eq!(size_of::<RawStat>(), size_of::<libc::stat>());
    assert_eq!(offset_of!(RawStat, device), offset_of!(libc::stat, st_dev));
    assert_eq!(offset_of!(RawStat, inode), offset_of!(libc::stat, st_ino));
    for ((_, suffix), raw) in NAMESPACES.iter().zip(CHILD_PATHS.iter()) {
        let end = raw.iter().position(|&byte| byte == 0).unwrap();
        let cstr = std::ffi::CStr::from_bytes_with_nul(&raw[..=end]).unwrap();
        let path = super::super::ProcPath::new(None, suffix).unwrap();
        assert_eq!(cstr, path.as_c_str().unwrap());
        assert!(raw[end..].iter().all(|&byte| byte == 0));
    }
    assert_eq!(CHILD_NAMESPACE_REPORT_CAPTURE_WORK, 41_216);
    assert_eq!(CHILD_NAMESPACE_REPORT_CHECK_WORK, 6_400);
    assert_eq!(
        CHILD_NAMESPACE_REPORT_CAPTURE_SCRATCH,
        4 * BYTES + PROC_PATH_BYTES + 2 * 144 + 8 * size_of::<Error>() + 1024
    );
    assert_eq!(
        CHILD_NAMESPACE_REPORT_CHECK_SCRATCH,
        4 * BYTES + 4 * size_of::<NamespaceSet>() + 8 * size_of::<Error>() + 1024
    );
}

#[test]
fn raw_namespace_stats_match_the_shared_current_thread_observer() {
    // Observation only: no fork, profile installation or namespace mutation.
    let set = NamespaceSet::capture_self().unwrap();
    for (identity, path) in set.identities.iter().zip(CHILD_PATHS.iter()) {
        let fd = RawSyscalls.open(path).unwrap();
        let mut stat = RawStat::ZERO;
        let observed = RawSyscalls.stat(fd, &mut stat);
        let closed = RawSyscalls.close(fd);
        observed.unwrap();
        closed.unwrap();
        assert_eq!(
            *identity,
            NamespaceIdentity {
                device: stat.device,
                inode: stat.inode
            }
        );
    }
    let Some(parent) = rustix::process::getppid() else {
        // A PID-1 runner has no visible parent; production direct children do.
        assert!(capture_child_namespace_report_pre_exec(0, &mut [0; BYTES]).is_err());
        return;
    };
    let mut bytes = [0; BYTES];
    let (result, allocations) = allocation::count(|| {
        capture_child_namespace_report_pre_exec(parent.as_raw_pid(), &mut bytes)
    });
    assert_eq!(result, Ok(()));
    assert_eq!(allocations, 0);
    assert_eq!(
        set.require_child_report(rustix::process::getpid(), parent, &bytes),
        Ok(())
    );
    let legacy = crate::ProtectedServiceNamespaceSetV1::capture_self().unwrap();
    legacy
        .require_child_report(rustix::process::getpid(), parent, &bytes)
        .unwrap();
    bytes[32] ^= 1;
    assert!(matches!(
        legacy.require_child_report(rustix::process::getpid(), parent, &bytes),
        Err(crate::ProtectedServiceProfileErrorV1::Namespace("user"))
    ));
}
