use super::*;
use std::cell::RefCell;
use std::os::fd::{AsRawFd, BorrowedFd};

use fe2o3_static_preexec_manifest::StaticPreexecObjectClassV1 as Class;
use rustix::fs::{MemfdFlags, Mode};

struct Tracked<'a> {
    descriptor: BorrowedFd<'a>,
    slot: usize,
    trace: &'a RefCell<Vec<usize>>,
}

impl AsFd for Tracked<'_> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.trace.borrow_mut().push(self.slot);
        self.descriptor
    }
}

fn tracked<'a>(
    descriptor: &'a impl AsFd,
    slot: usize,
    trace: &'a RefCell<Vec<usize>>,
) -> Tracked<'a> {
    Tracked {
        descriptor: descriptor.as_fd(),
        slot,
        trace,
    }
}

fn anonymous_object() -> OwnedFd {
    rustix::fs::memfd_create(
        c"fe2o3-launch-checks-test",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .unwrap()
}

fn manifest_file(size: u64, mode: Mode, seals: SealFlags) -> OwnedFd {
    let writable = anonymous_object();
    rustix::fs::ftruncate(&writable, size).unwrap();
    rustix::fs::fchmod(&writable, mode).unwrap();
    rustix::fs::fcntl_add_seals(&writable, seals).unwrap();
    rustix::fs::open(
        format!("/proc/self/fd/{}", writable.as_raw_fd()),
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap()
}

fn invalid_pipe(role: &'static str) -> Failure {
    Failure::InvalidDescriptor {
        role,
        reason: "pipe type, access, status, or descriptor flags changed",
    }
}

fn invalid_manifest() -> Failure {
    Failure::InvalidDescriptor {
        role: "static pre-exec manifest",
        reason: "descriptor, object, access, mode, length, or seals changed",
    }
}

#[test]
fn fixed_source_roles_and_destination_abi_are_preserved() {
    assert_eq!(SOURCE_COUNT_V1, 12);
    assert_eq!(DESTINATION_FDS_V1, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    for (index, role) in [
        (STDIN_SOURCE_INDEX, "stdin"),
        (STDOUT_SOURCE_INDEX, "stdout"),
        (STDERR_SOURCE_INDEX, "stderr"),
        (ROOT_SOURCE_INDEX, "issuer root"),
        (SERVICE_PEER_SOURCE_INDEX, "rustc service peer"),
        (CLIENT_PIDFD_SOURCE_INDEX, "rustc pidfd"),
        (POLICY_SOURCE_INDEX, "issuer policy"),
        (SIGNING_KEY_SOURCE_INDEX, "issuer signing key"),
        (LAUNCH_MANIFEST_SOURCE_INDEX, "service launch manifest"),
        (READINESS_SOURCE_INDEX, "readiness writer"),
        (EXTERNAL_ANCHOR_PEER_SOURCE_INDEX, "external-anchor peer"),
        (EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX, "external-anchor pidfd"),
    ] {
        assert_eq!(source_role(index), role);
    }
}

#[test]
fn source_snapshots_are_fixed_order_with_inert_pidfd_class_tags() {
    let source = anonymous_object();
    let object = object_identity(&source, "fixture").unwrap();
    let trace = RefCell::new(Vec::new());
    let sources: [_; SOURCE_COUNT_V1] = std::array::from_fn(|i| tracked(&source, i, &trace));
    let identities = source_identities(&sources).unwrap();
    assert_eq!(*trace.borrow(), (0..SOURCE_COUNT_V1).collect::<Vec<_>>());
    for (index, identity) in identities.into_iter().enumerate() {
        assert!(same_object(&identity, &object));
        assert_eq!(identity.size(), object.size());
        assert_eq!(identity.mode(), object.mode());
        // Tagging the reserved slot is not admission of the underlying descriptor as a pidfd.
        assert_eq!(
            identity.class(),
            if matches!(
                index,
                CLIENT_PIDFD_SOURCE_INDEX | EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX
            ) {
                Class::ProcessPidfd
            } else {
                Class::Fstat
            }
        );
    }
}

#[test]
fn object_alias_key_ignores_size_mode_and_validation_class_only() {
    let object = Object::new(7, 11, 19, libc::S_IFREG | 0o400);
    assert!(same_object(
        &object,
        &Object::new_process_pidfd(7, 11, 0, 0)
    ));
    assert!(!same_object(
        &object,
        &Object::new(8, 11, 19, object.mode())
    ));
    assert!(!same_object(
        &object,
        &Object::new(7, 12, 19, object.mode())
    ));
}

#[test]
fn pipe_shape_rejects_each_forbidden_flag_access_and_object_type() {
    let valid = OFlags::RDONLY | OFlags::NONBLOCK;
    assert_eq!(
        pipe_shape(FdFlags::CLOEXEC, valid, libc::S_IFIFO, OFlags::RDONLY, "r"),
        Ok(())
    );
    for (flags, status, mode) in [
        (FdFlags::empty(), valid, libc::S_IFIFO),
        (FdFlags::CLOEXEC, OFlags::RDONLY, libc::S_IFIFO),
        (
            FdFlags::CLOEXEC,
            OFlags::WRONLY | OFlags::NONBLOCK,
            libc::S_IFIFO,
        ),
        (
            FdFlags::CLOEXEC,
            OFlags::RDWR | OFlags::NONBLOCK,
            libc::S_IFIFO,
        ),
        (FdFlags::CLOEXEC, valid | OFlags::APPEND, libc::S_IFIFO),
        (FdFlags::CLOEXEC, valid | OFlags::ASYNC, libc::S_IFIFO),
        (FdFlags::CLOEXEC, valid | OFlags::DIRECT, libc::S_IFIFO),
        (FdFlags::CLOEXEC, valid | OFlags::PATH, libc::S_IFIFO),
        (FdFlags::CLOEXEC, valid, libc::S_IFREG),
        (FdFlags::CLOEXEC, valid, libc::S_IFSOCK),
    ] {
        assert_eq!(
            pipe_shape(flags, status, mode, OFlags::RDONLY, "r"),
            Err(invalid_pipe("r"))
        );
    }
}

#[test]
fn protected_pipe_is_cloexec_nonblocking_and_directional() {
    let (reader, writer) = protected_pipe("fixture").unwrap();
    validate_pipe_end(&reader, OFlags::RDONLY, "reader").unwrap();
    validate_pipe_end(&writer, OFlags::WRONLY, "writer").unwrap();
    validate_pipe_pair(&writer, &reader, "fixture").unwrap();
    validate_readiness_capacity(&writer).unwrap();
    assert_eq!(
        validate_pipe_end(&writer, OFlags::RDONLY, "writer"),
        Err(invalid_pipe("writer"))
    );

    rustix::io::fcntl_setfd(&reader, FdFlags::empty()).unwrap();
    assert_eq!(
        validate_pipe_end(&reader, OFlags::RDONLY, "reader"),
        Err(invalid_pipe("reader"))
    );
    rustix::io::fcntl_setfd(&reader, FdFlags::CLOEXEC).unwrap();
    rustix::fs::fcntl_setfl(&reader, OFlags::empty()).unwrap();
    assert_eq!(
        validate_pipe_end(&reader, OFlags::RDONLY, "reader"),
        Err(invalid_pipe("reader"))
    );
}

#[test]
fn pair_checks_writer_then_reader_before_object_identity_and_alias() {
    let (reader, writer) = protected_pipe("fixture").unwrap();
    let (other_reader, _other_writer) = protected_pipe("other").unwrap();
    let trace = RefCell::new(Vec::new());
    let w = tracked(&writer, 0, &trace);
    let r = tracked(&reader, 1, &trace);
    validate_pipe_pair(&w, &r, "fixture").unwrap();
    assert_eq!(*trace.borrow(), [0, 0, 0, 1, 1, 1, 0, 1]);
    trace.borrow_mut().clear();

    rustix::io::fcntl_setfd(&writer, FdFlags::empty()).unwrap();
    rustix::io::fcntl_setfd(&reader, FdFlags::empty()).unwrap();
    assert_eq!(
        validate_pipe_pair(&w, &r, "fixture"),
        Err(invalid_pipe("fixture"))
    );
    assert_eq!(*trace.borrow(), [0, 0, 0]);
    trace.borrow_mut().clear();
    rustix::io::fcntl_setfd(&writer, FdFlags::CLOEXEC).unwrap();
    assert_eq!(
        validate_pipe_pair(&w, &r, "fixture"),
        Err(invalid_pipe("fixture"))
    );
    assert_eq!(*trace.borrow(), [0, 0, 0, 1, 1, 1]);
    rustix::io::fcntl_setfd(&reader, FdFlags::CLOEXEC).unwrap();
    assert_eq!(
        validate_pipe_pair(&writer, &other_reader, "fixture"),
        Err(Failure::DescriptorChanged("fixture"))
    );
    assert_eq!(
        validate_pipe_pair(&reader, &writer, "fixture"),
        Err(invalid_pipe("fixture"))
    );
}

#[test]
fn launcher_alias_checks_keep_legacy_short_circuit_and_all_source_scan() {
    let launcher = anonymous_object();
    let issuer = anonymous_object();
    let manifest = anonymous_object();
    let source = anonymous_object();
    let trace = RefCell::new(Vec::new());
    let alias = Err(Failure::DescriptorAlias(
        "static launcher aliases another launch role",
    ));
    let sources: [_; SOURCE_COUNT_V1] = std::array::from_fn(|i| tracked(&source, i + 3, &trace));
    let l = tracked(&launcher, 0, &trace);
    let same_issuer = tracked(&launcher, 1, &trace);
    let i = tracked(&issuer, 1, &trace);
    let m = tracked(&manifest, 2, &trace);
    assert_eq!(
        require_launcher_non_aliasing(&l, &same_issuer, &m, &sources),
        alias
    );
    assert_eq!(*trace.borrow(), [0, 1]);
    trace.borrow_mut().clear();
    let same_manifest = tracked(&launcher, 2, &trace);
    assert_eq!(
        require_launcher_non_aliasing(&l, &i, &same_manifest, &sources),
        alias
    );
    assert_eq!(*trace.borrow(), [0, 1, 2]);
    trace.borrow_mut().clear();

    require_launcher_non_aliasing(&l, &i, &m, &sources).unwrap();
    assert_eq!(
        *trace.borrow(),
        (0..SOURCE_COUNT_V1 + 3).collect::<Vec<_>>()
    );
    for alias_index in 0..SOURCE_COUNT_V1 {
        trace.borrow_mut().clear();
        let sources: [_; SOURCE_COUNT_V1] = std::array::from_fn(|index| {
            tracked(
                if index == alias_index {
                    &launcher
                } else {
                    &source
                },
                index + 3,
                &trace,
            )
        });
        assert_eq!(require_launcher_non_aliasing(&l, &i, &m, &sources), alias);
        assert_eq!(
            *trace.borrow(),
            (0..SOURCE_COUNT_V1 + 3).collect::<Vec<_>>()
        );
    }
}

#[test]
fn static_table_checks_cardinality_before_issuer_and_all_binding_fields() {
    let issuer = anonymous_object();
    let executable = object_identity(&issuer, "issuer").unwrap();
    let sources: [_; SOURCE_COUNT_V1] =
        std::array::from_fn(|i| Object::new(1, i as u64 + 1, 0, libc::S_IFREG));
    let entries: [_; SOURCE_COUNT_V1] = std::array::from_fn(|i| {
        Descriptor::for_index(i, DESTINATION_FDS_V1[i], sources[i]).unwrap()
    });
    let trace = RefCell::new(Vec::new());
    let i = tracked(&issuer, 0, &trace);
    let changed = Err(Failure::DescriptorChanged("static pre-exec source table"));
    assert_eq!(
        validate_static_manifest_sources(&executable, &entries[..11], &i, &sources),
        changed
    );
    assert!(trace.borrow().is_empty());
    validate_static_manifest_sources(&executable, &entries, &i, &sources).unwrap();
    assert_eq!(*trace.borrow(), [0]);
    assert_eq!(
        validate_static_manifest_sources(&Object::new(0, 0, 0, 0), &entries, &issuer, &sources),
        changed
    );
    for index in 0..SOURCE_COUNT_V1 {
        for replacement in [
            Descriptor::for_index(
                (index + 1) % SOURCE_COUNT_V1,
                DESTINATION_FDS_V1[index],
                sources[index],
            )
            .unwrap(),
            Descriptor::for_index(
                index,
                (DESTINATION_FDS_V1[index] + 1) % SOURCE_COUNT_V1 as i32,
                sources[index],
            )
            .unwrap(),
            Descriptor::for_index(
                index,
                DESTINATION_FDS_V1[index],
                Object::new(1, index as u64 + 1, 1, libc::S_IFREG),
            )
            .unwrap(),
            Descriptor::for_index(
                index,
                DESTINATION_FDS_V1[index],
                Object::new_process_pidfd(1, index as u64 + 1, 0, libc::S_IFREG),
            )
            .unwrap(),
        ] {
            let mut changed_entries = entries;
            changed_entries[index] = replacement;
            assert_eq!(
                validate_static_manifest_sources(&executable, &changed_entries, &issuer, &sources),
                changed
            );
        }
    }
}

#[test]
fn manifest_metadata_checks_exact_snapshot_only_after_other_predicates() {
    let file = manifest_file(
        PREEXEC_MANIFEST_BYTES_V1 as u64,
        Mode::RUSR,
        REQUIRED_MANIFEST_SEALS_V1,
    );
    let object = object_identity(&file, "manifest").unwrap();
    let trace = RefCell::new(Vec::new());
    let f = tracked(&file, 0, &trace);
    validate_manifest_metadata(&f, object).unwrap();
    assert_eq!(trace.borrow().len(), 5);
    trace.borrow_mut().clear();
    assert_eq!(
        validate_manifest_metadata(&f, Object::new(0, 0, 0, 0)),
        Err(invalid_manifest())
    );
    assert_eq!(trace.borrow().len(), 5);
    trace.borrow_mut().clear();
    rustix::io::fcntl_setfd(&file, FdFlags::empty()).unwrap();
    assert_eq!(
        validate_manifest_metadata(&f, object),
        Err(invalid_manifest())
    );
    assert_eq!(trace.borrow().len(), 4);

    for (size, mode, seals) in [
        (
            PREEXEC_MANIFEST_BYTES_V1 as u64 - 1,
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
        ),
        (
            PREEXEC_MANIFEST_BYTES_V1 as u64 + 1,
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
        ),
        (
            PREEXEC_MANIFEST_BYTES_V1 as u64,
            Mode::RUSR | Mode::WUSR,
            REQUIRED_MANIFEST_SEALS_V1,
        ),
        (
            PREEXEC_MANIFEST_BYTES_V1 as u64,
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1 - SealFlags::SEAL,
        ),
    ] {
        let file = manifest_file(size, mode, seals);
        let object = object_identity(&file, "manifest").unwrap();
        trace.borrow_mut().clear();
        assert_eq!(
            validate_manifest_metadata(&tracked(&file, 0, &trace), object),
            Err(invalid_manifest())
        );
        assert_eq!(trace.borrow().len(), 4);
    }
}

#[test]
fn manifest_metadata_query_error_precedes_invalid_descriptor_predicates() {
    let (reader, _writer) = pipe_with(PipeFlags::empty()).unwrap();
    let trace = RefCell::new(Vec::new());
    // The missing CLOEXEC flag must not hide the earlier seal-query failure.
    assert_eq!(
        validate_manifest_metadata(&tracked(&reader, 0, &trace), Object::new(0, 0, 0, 0)),
        Err(Failure::Io {
            operation: "inspect static manifest seals",
            source: Errno::INVAL,
        })
    );
    assert_eq!(*trace.borrow(), [0, 0, 0]);
}

#[test]
fn leaf_failures_preserve_legacy_categories_labels_and_errno() {
    use crate::launch::ProtectedIssuerLaunchPreparationErrorV1 as Legacy;

    for failure in [
        invalid_pipe("stdout"),
        invalid_manifest(),
        Failure::DescriptorChanged("static pre-exec source table"),
        Failure::DescriptorAlias("static launcher aliases another launch role"),
        Failure::Io {
            operation: "inspect readiness pipe capacity",
            source: Errno::INVAL,
        },
    ] {
        match (failure, Legacy::from(failure)) {
            (
                Failure::InvalidDescriptor {
                    role: expected_role,
                    reason: expected_reason,
                },
                Legacy::InvalidDescriptor { role, reason },
            ) => {
                assert_eq!((role, reason), (expected_role, expected_reason));
            }
            (Failure::DescriptorChanged(expected), Legacy::DescriptorChanged(actual))
            | (Failure::DescriptorAlias(expected), Legacy::DescriptorAlias(actual)) => {
                assert_eq!(actual, expected)
            }
            (
                Failure::Io {
                    operation: expected_operation,
                    source: expected_errno,
                },
                Legacy::Io { operation, source },
            ) => {
                assert_eq!(operation, expected_operation);
                assert_eq!(source.raw_os_error(), Some(expected_errno.raw_os_error()));
            }
            unexpected => panic!("changed legacy adapter: {unexpected:?}"),
        }
    }
}
