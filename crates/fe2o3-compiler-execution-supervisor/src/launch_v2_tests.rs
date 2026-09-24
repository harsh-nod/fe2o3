use super::*;
use crate::AdmittedIssuerProgramV2 as Program;
use crate::authority_v2_test_process::{IO_TIMEOUT, frame, receive_packet, send_packet};
use fe2o3_broker_authority_service::{
    CURRENT_PROCESS_START_TIME_WORK_V2, LiveClientPidfdIdentityV2 as Client,
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionPolicyCapabilityV2 as PolicyCap,
    CompilerExecutionSigningKeyCapabilityV2 as Key,
};
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2 as MANIFEST_WORK;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOperationV2 as Operation, ProtectedStaticExecutableV2 as Image,
};
use fe2o3_static_preexec_manifest::{
    PREEXEC_SOURCE_FD_BASE, StaticPreexecManifestErrorV1 as StaticError,
    StaticPreexecObjectClassV1 as Class,
};
use rustix::{
    fs::{MemfdFlags, Mode, OFlags, SealFlags},
    io::FdFlags,
};
use std::{
    collections::BTreeMap,
    error::Error as StdError,
    os::{
        fd::{AsFd, AsRawFd},
        unix::fs::MetadataExt,
    },
    time::Instant,
};

const WORK_LIMIT: usize = 100_000_000_000;
const STORAGE_LIMIT: usize = 10_000_000;
const EXTRA: usize = 19;

#[path = "native_consuming_tests.rs"]
mod consuming;
pub(crate) use consuming::exercise as exercise_consuming;

#[derive(Debug, Eq, PartialEq)]
struct FdIdentity {
    object: (u64, u64, u32),
    flags: u32,
    pid: Option<i32>,
}

// Only the isolated single-test supervisor role may take a process-wide census.
fn fd_inventory() -> BTreeMap<i32, FdIdentity> {
    let descriptors: Vec<i32> = {
        let directory = std::fs::read_dir("/proc/self/fd").unwrap();
        directory
            .map(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_str()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
            .collect()
    };
    let mut inventory = BTreeMap::new();
    let mut disappeared = 0;
    // Finish all metadata queries before fdinfo reads can reuse the collector FD.
    for fd in descriptors {
        match std::fs::metadata(format!("/proc/self/fd/{fd}")) {
            Ok(metadata) => {
                assert!(
                    inventory
                        .insert(
                            fd,
                            FdIdentity {
                                object: (metadata.dev(), metadata.ino(), metadata.mode()),
                                flags: 0,
                                pid: None,
                            }
                        )
                        .is_none()
                );
            }
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => disappeared += 1,
            Err(error) => panic!("cannot inventory fd {fd}: {error}"),
        }
    }
    assert_eq!(
        disappeared, 1,
        "only the closed ReadDir descriptor may disappear"
    );
    for (fd, identity) in &mut inventory {
        let info = std::fs::read_to_string(format!("/proc/self/fdinfo/{fd}")).unwrap();
        let flags = info
            .lines()
            .find_map(|line| line.strip_prefix("flags:"))
            .unwrap();
        identity.flags = u32::from_str_radix(flags.trim(), 8).unwrap();
        identity.pid = info
            .lines()
            .find_map(|line| line.strip_prefix("Pid:"))
            .map(|pid| pid.trim().parse().unwrap());
    }
    inventory
}

fn references(object: Object) -> usize {
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (object.device(), object.inode()))
        .count()
}

// Keep the object alive while checking cleanup, excluding fd/inode reuse.
struct Witness {
    pin: OwnedFd,
    object: Object,
    remaining: usize,
}
impl Witness {
    fn new(file: &impl AsFd, owned_references: usize) -> Self {
        let pin = rustix::io::fcntl_dupfd_cloexec(file, 3).unwrap();
        let object = checks::object_identity(&pin, "cleanup witness").unwrap();
        Self {
            pin,
            object,
            remaining: references(object).checked_sub(owned_references).unwrap(),
        }
    }
    fn assert_released(&self) {
        assert_eq!(
            checks::object_identity(&self.pin, "cleanup witness").unwrap(),
            self.object
        );
        assert_eq!(
            references(self.object),
            self.remaining,
            "object {:?}",
            self.object
        );
    }
}

fn pidfd_references(pid: u32) -> usize {
    std::fs::read_dir("/proc/self/fdinfo")
        .unwrap()
        .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path()).ok())
        .filter(|record| {
            record.lines().any(|line| {
                line.strip_prefix("Pid:\t")
                    .and_then(|value| value.parse::<u32>().ok())
                    == Some(pid)
            })
        })
        .count()
}

fn resource(mut error: &(dyn StdError + 'static)) -> Resource {
    loop {
        if let Some(resource) = error.downcast_ref::<Resource>() {
            return *resource;
        }
        error = error.source().expect("expected a typed resource refusal");
    }
}

struct HandoffWitness {
    control: Witness,
    service: Witness,
}
impl HandoffWitness {
    fn expected_after_prepare(&self, client_pid: u32) -> BTreeMap<i32, FdIdentity> {
        let mut expected = fd_inventory();
        for witness in [&self.control, &self.service] {
            let pin = witness.pin.as_raw_fd();
            let object = (
                witness.object.device(),
                witness.object.inode(),
                witness.object.mode(),
            );
            assert_eq!(expected[&pin].object, object);
            let consumed: Vec<_> = expected
                .iter()
                .filter_map(|(&fd, identity)| {
                    (fd != pin && identity.object == object).then_some(fd)
                })
                .collect();
            assert_eq!(consumed.len(), 1, "exactly one consumed socket per witness");
            assert!(expected.remove(&consumed[0]).is_some());
        }
        // This target stays alive in the submitter fixture. Its sole local pidfd
        // belongs to Accepted; an anonymous-inode key cannot distinguish it.
        let client_pid = i32::try_from(client_pid).unwrap();
        let consumed: Vec<_> = expected
            .iter()
            .filter_map(|(&fd, identity)| (identity.pid == Some(client_pid)).then_some(fd))
            .collect();
        assert_eq!(consumed.len(), 1, "exactly one consumed client pidfd");
        assert!(expected.remove(&consumed[0]).is_some());
        expected
    }

    fn assert_released(&self) {
        self.control.assert_released();
        self.service.assert_released();
    }
}

fn accept(
    supervisor: &Supervisor,
    submitter: &OwnedFd,
    budget: &mut Budget<'_>,
) -> (Accepted, HandoffWitness, u32, usize) {
    send_packet(submitter, &frame(b"HOF2", 0), &[]).unwrap();
    let (payload, [control]) = receive_packet::<1>(submitter, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&payload[..4], b"HOF2");
    let pid = u32::from_le_bytes(payload[4..].try_into().unwrap());
    let pidfds = pidfd_references(pid);
    let witness = Witness::new(&control, 1);
    budget.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
    let (accepted, delta) = supervisor
        .accept_handoff(control, IO_TIMEOUT, budget)
        .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(accepted.manifest().client().pid(), pid);
    assert_ne!(
        accepted.submitter().uid(),
        rustix::process::geteuid().as_raw()
    );
    let (service, client, retained) = accepted.clone_launch_peers(budget).unwrap();
    budget.reserve_storage(retained).unwrap();
    let service_witness = Witness::new(&service, 2);
    drop((service, client));
    budget.release_storage(retained).unwrap();
    (
        accepted,
        HandoffWitness {
            control: witness,
            service: service_witness,
        },
        pid,
        pidfds,
    )
}

fn expected_work(fixture: &crate::tests::Fixture) -> (usize, usize) {
    let m = fixture.measurement();
    let measurement = Measurement::new(m.sha256(), m.byte_len(), 128 * 1024 * 1024).unwrap();
    let s = Supervisor::WORK + crate::authority_v2::tests::nested_work(fixture);
    let h = Accepted::WORK + s + MANIFEST_WORK + Client::REVALIDATION_WORK;
    let t = 3 * Program::WORK
        + 2 * Image::quota(measurement, Operation::Transfer)
            .unwrap()
            .work()
        + PolicyCap::IO_WORK
        + Key::IO_WORK;
    let c = s + t + Anchor::CLONE_TRANSFER_WORK;
    let r = s + t + Anchor::VALIDATE_TRANSFER_WORK;
    let pc = Accepted::WORK + Client::CLONE_TRANSFER_WORK;
    let pv = Accepted::WORK + Client::VALIDATE_TRANSFER_WORK;
    let k = Capability::IO_WORK;
    let p = CURRENT_PROCESS_START_TIME_WORK_V2;
    // Expand the production call graph, independently of observed work deltas.
    let check = s + h + 2 * k + p + r + pv;
    let revalidate = Prepared::WORK + check;
    let prepare = Prepared::WORK + s + h + c + pc + MANIFEST_WORK + 2 * k + p + check;
    (prepare, revalidate)
}

fn prepared_witnesses(owner: &Prepared) -> Vec<Witness> {
    prepared_witnesses_with_readiness(owner, true)
}

fn prepared_witnesses_with_readiness(owner: &Prepared, pin_readiness_writer: bool) -> Vec<Witness> {
    let mut witnesses = vec![
        Witness::new(&owner.launcher, 1),
        Witness::new(&owner.issuer, 1),
        Witness::new(&owner.static_manifest_file, 1),
    ];
    for (index, source) in owner.sources.iter().enumerate() {
        // Pidfds use target identities, not Linux's shared anonymous-inode key.
        if matches!(
            index,
            CLIENT_PIDFD_SOURCE_INDEX | EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX
        ) || (index == READINESS_SOURCE_INDEX && !pin_readiness_writer)
        {
            continue;
        }
        let owned = match index {
            STDOUT_SOURCE_INDEX | STDERR_SOURCE_INDEX | READINESS_SOURCE_INDEX => 2,
            // Accepted custody and the capability each retain their original FD.
            SERVICE_PEER_SOURCE_INDEX | LAUNCH_MANIFEST_SOURCE_INDEX => 2,
            _ => 1,
        };
        witnesses.push(Witness::new(source, owned));
    }
    witnesses
}

fn inspect_prepared(owner: &Prepared, client_pid: u32, anchor_pid: u32) {
    let manifest = owner.static_manifest();
    assert_eq!(manifest.parent_pid(), std::process::id() as i32);
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let start_time: u64 = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .nth(19)
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(start_time, 0);
    assert_eq!(manifest.parent_start_time(), start_time);
    assert_eq!(manifest.descriptors().len(), 12);
    assert_eq!(SOURCE_COUNT_V1, 12);
    assert_eq!(DESTINATION_FDS_V1, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    assert_eq!(
        manifest.executable(),
        &checks::object_identity(&owner.issuer, "issuer").unwrap()
    );
    let objects = checks::source_identities(&owner.sources).unwrap();
    for (index, entry) in manifest.descriptors().iter().enumerate() {
        assert_eq!(entry.source_fd(), PREEXEC_SOURCE_FD_BASE + index as i32);
        assert_eq!(entry.destination_fd(), index as i32);
        assert_eq!(entry.object(), &objects[index]);
        assert_eq!(
            entry.object().class(),
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
    assert_eq!(owner.service_manifest().client().pid(), client_pid);
    for (index, pid) in [
        (CLIENT_PIDFD_SOURCE_INDEX, client_pid),
        (EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX, anchor_pid),
    ] {
        let info = std::fs::read_to_string(format!(
            "/proc/self/fdinfo/{}",
            rustix::path::DecInt::from_fd(&owner.sources[index]).as_str()
        ))
        .unwrap();
        let expected = format!("Pid:\t{pid}");
        assert!(info.lines().any(|line| line == expected));
    }
    // Stdin's writer is deliberately not retained; it is already at EOF.
    assert_eq!(
        rustix::io::read(&owner.sources[STDIN_SOURCE_INDEX], &mut [0; 1]).unwrap(),
        0
    );
    for (writer, reader) in [
        (&owner.sources[STDOUT_SOURCE_INDEX], &owner.stdout_reader),
        (&owner.sources[STDERR_SOURCE_INDEX], &owner.stderr_reader),
        (
            &owner.sources[READINESS_SOURCE_INDEX],
            &owner.readiness_reader,
        ),
    ] {
        assert_eq!(rustix::io::write(writer, b"prepared").unwrap(), 8);
        let mut bytes = [0; 8];
        assert_eq!(rustix::io::read(reader, &mut bytes).unwrap(), 8);
        assert_eq!(&bytes, b"prepared");
    }
    image::validate(&owner.static_manifest_file, manifest, owner.manifest_object).unwrap();
    let debug = format!("{owner:?}");
    assert!(debug.contains("prepared-custody-only"));
    assert!(!debug.contains("OwnedFd"));
}

#[derive(Clone, Copy, Debug)]
enum Limit {
    Roomy,
    ExactWork,
    ShortWork,
    ExactPeak,
    ShortPeak,
    ExactFloor,
    ShortFloor,
    ShortEntry,
}
const LIMITS: [Limit; 8] = [
    Limit::Roomy,
    Limit::ExactWork,
    Limit::ShortWork,
    Limit::ExactPeak,
    Limit::ShortPeak,
    Limit::ExactFloor,
    Limit::ShortFloor,
    Limit::ShortEntry,
];
impl Limit {
    fn inputs(self, floor: usize, work: usize, peak: usize) -> (usize, usize, usize) {
        let work = match self {
            Self::ExactWork => work,
            Self::ShortWork => work - 1,
            Self::ShortEntry => ENTRY - 1,
            _ => WORK_LIMIT,
        };
        let storage = match self {
            Self::ExactPeak => peak,
            Self::ShortPeak => peak - 1,
            _ => STORAGE_LIMIT,
        };
        let prepaid = match self {
            Self::ExactFloor => floor,
            Self::ShortFloor => floor - 1,
            _ => floor + EXTRA,
        };
        (work, storage, prepaid)
    }
    fn succeeds(self) -> bool {
        matches!(
            self,
            Self::Roomy | Self::ExactWork | Self::ExactPeak | Self::ExactFloor
        )
    }
    fn check_error(self, error: &Error, budget: &Budget<'_>, peak: usize) {
        match self {
            Self::ShortWork | Self::ShortEntry => {
                assert!(matches!(resource(error), Resource::Work(_)))
            }
            Self::ShortPeak => {
                assert!(matches!(resource(error), Resource::Storage(_)));
                assert_eq!(budget.failed_storage(), Some(peak));
            }
            Self::ShortFloor => {
                assert!(matches!(error, Error::Resource(Resource::Accounting)));
                assert_eq!(budget.work(), ENTRY);
                assert_eq!(budget.failed_storage(), None);
            }
            _ => panic!("unexpected refusal for {self:?}: {error:?}"),
        }
    }
    fn failed_work(self, exact: usize) -> Option<usize> {
        match self {
            Self::ShortWork => Some(exact),
            Self::ShortEntry => Some(ENTRY),
            _ => None,
        }
    }
}

fn revalidation_boundaries(owner: &Prepared, supervisor: &Supervisor, exact_work: usize) {
    let floor = owner.retained_storage() + supervisor.retained_storage();
    let mut peak = 0;
    for case in LIMITS {
        let (work_limit, storage_limit, prepaid) = case.inputs(floor, exact_work, peak);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(prepaid).unwrap();
        let result = owner.revalidate(supervisor, &mut budget);
        assert_eq!(budget.storage(), prepaid, "{case:?}");
        if case.succeeds() {
            result.unwrap();
            assert_eq!(budget.work(), exact_work);
            if matches!(case, Limit::Roomy) {
                peak = budget.peak_storage();
            }
            assert_eq!(
                budget.peak_storage(),
                peak - if matches!(case, Limit::ExactFloor) {
                    EXTRA
                } else {
                    0
                }
            );
            assert_eq!(budget.failed_storage(), None);
        } else {
            case.check_error(&result.unwrap_err(), &budget, peak);
        }
        assert_eq!(work.failed_work(), case.failed_work(exact_work), "{case:?}");
    }
}

fn preparation_boundaries(
    supervisor: &Supervisor,
    submitter: &OwnedFd,
    outer: &mut Budget<'_>,
    exact_work: usize,
) {
    let mut peak = 0;
    for case in LIMITS {
        let (accepted, control, pid, pidfds) = accept(supervisor, submitter, outer);
        let consumed = accepted.retained_storage();
        let floor = supervisor.retained_storage() + consumed;
        let (work_limit, storage_limit, prepaid) = case.inputs(floor, exact_work, peak);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(prepaid).unwrap();
        assert_eq!(
            pidfds, 0,
            "the fixture client target has no other local pidfds"
        );
        let expected_descriptors = control.expected_after_prepare(pid);
        let result = supervisor.prepare_launch(accepted, &mut budget);
        assert_eq!(budget.storage(), prepaid, "{case:?}");
        if case.succeeds() {
            let (owner, delta) = result.unwrap();
            assert_eq!(budget.work(), exact_work);
            assert_eq!(
                owner.retained_storage(),
                consumed + delta.additional_storage()
            );
            if matches!(case, Limit::Roomy) {
                peak = budget.peak_storage();
            }
            assert_eq!(
                budget.peak_storage(),
                peak - if matches!(case, Limit::ExactFloor) {
                    EXTRA
                } else {
                    0
                }
            );
            budget.reserve_storage(delta.additional_storage()).unwrap();
            let retained = owner.retained_storage();
            let witnesses = prepared_witnesses(&owner);
            drop(owner);
            budget.release_storage(retained).unwrap();
            for witness in witnesses {
                witness.assert_released();
            }
        } else {
            case.check_error(&result.unwrap_err(), &budget, peak);
            budget.release_storage(consumed).unwrap();
        }
        assert_eq!(budget.storage(), prepaid - consumed);
        assert_eq!(work.failed_work(), case.failed_work(exact_work), "{case:?}");
        // Compare before opening another protocol session or dropping the pins.
        // A leaked staging FD still fails if it reused a consumed descriptor number.
        assert_eq!(
            fd_inventory(),
            expected_descriptors,
            "prepared cleanup {case:?}"
        );
        outer.release_storage(consumed).unwrap();
        control.assert_released();
        assert_eq!(pidfd_references(pid), pidfds, "consumed handoff {case:?}");
        let floor = outer.storage();
        supervisor.revalidate(outer).unwrap();
        assert_eq!(outer.storage(), floor);
    }
}

fn refused(owner: &Prepared, supervisor: &Supervisor, budget: &mut Budget<'_>) -> Error {
    let floor = budget.storage();
    let error = owner.revalidate(supervisor, budget).unwrap_err();
    assert_eq!(budget.storage(), floor);
    error
}

fn mutations(owner: &mut Prepared, supervisor: &Supervisor, budget: &mut Budget<'_>) {
    let pipes = [
        owner.sources[STDIN_SOURCE_INDEX].as_fd(),
        owner.sources[STDOUT_SOURCE_INDEX].as_fd(),
        owner.sources[STDERR_SOURCE_INDEX].as_fd(),
        owner.sources[READINESS_SOURCE_INDEX].as_fd(),
        owner.stdout_reader.as_fd(),
        owner.stderr_reader.as_fd(),
        owner.readiness_reader.as_fd(),
    ];
    for pipe in pipes {
        let flags = rustix::io::fcntl_getfd(pipe).unwrap();
        rustix::io::fcntl_setfd(pipe, FdFlags::empty()).unwrap();
        assert!(matches!(
            refused(owner, supervisor, budget),
            Error::InvalidDescriptor { .. }
        ));
        rustix::io::fcntl_setfd(pipe, flags).unwrap();
        let status = rustix::fs::fcntl_getfl(pipe).unwrap();
        rustix::fs::fcntl_setfl(pipe, status - OFlags::NONBLOCK).unwrap();
        assert!(matches!(
            refused(owner, supervisor, budget),
            Error::InvalidDescriptor { .. }
        ));
        rustix::fs::fcntl_setfl(pipe, status).unwrap();
    }
    for (left, right) in [
        (STDOUT_SOURCE_INDEX, STDERR_SOURCE_INDEX),
        (POLICY_SOURCE_INDEX, SIGNING_KEY_SOURCE_INDEX),
        (ROOT_SOURCE_INDEX, SERVICE_PEER_SOURCE_INDEX),
        (
            CLIENT_PIDFD_SOURCE_INDEX,
            EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX,
        ),
        (SERVICE_PEER_SOURCE_INDEX, EXTERNAL_ANCHOR_PEER_SOURCE_INDEX),
    ] {
        owner.sources.swap(left, right);
        assert!(matches!(
            refused(owner, supervisor, budget),
            Error::DescriptorChanged(_) | Error::Supervisor(_) | Error::Handoff(_)
        ));
        owner.sources.swap(left, right);
    }
    std::mem::swap(&mut owner.stdout_reader, &mut owner.stderr_reader);
    assert!(matches!(
        refused(owner, supervisor, budget),
        Error::DescriptorChanged(_)
    ));
    std::mem::swap(&mut owner.stdout_reader, &mut owner.stderr_reader);
    std::mem::swap(&mut owner.launcher, &mut owner.issuer);
    assert!(matches!(
        refused(owner, supervisor, budget),
        Error::Supervisor(_)
    ));
    std::mem::swap(&mut owner.launcher, &mut owner.issuer);

    let original = owner.static_manifest.clone();
    for index in 0..SOURCE_COUNT_V1 {
        let mut entries = original.descriptors().to_vec();
        let object = entries[index].object();
        let changed = match object.class() {
            Class::Fstat => Object::new(
                object.device(),
                object.inode(),
                object.size() + 1,
                object.mode(),
            ),
            Class::ProcessPidfd => Object::new_process_pidfd(
                object.device(),
                object.inode(),
                object.size() + 1,
                object.mode(),
            ),
        };
        entries[index] = Descriptor::for_index(index, index as i32, changed).unwrap();
        owner.static_manifest = StaticManifest::from_descriptors(
            original.parent_pid(),
            original.parent_start_time(),
            *original.executable(),
            &entries,
        )
        .unwrap();
        assert!(matches!(
            refused(owner, supervisor, budget),
            Error::DescriptorChanged("static pre-exec source table")
        ));
    }
    owner.static_manifest = StaticManifest::from_descriptors(
        original.parent_pid(),
        original.parent_start_time() + 1,
        *original.executable(),
        original.descriptors(),
    )
    .unwrap();
    assert!(matches!(
        refused(owner, supervisor, budget),
        Error::ParentChanged
    ));
    owner.static_manifest = original;

    let (substitute, object) = image::create(owner.static_manifest()).unwrap();
    assert!(!checks::same_object(&object, &owner.manifest_object));
    let original = std::mem::replace(&mut owner.static_manifest_file, substitute);
    assert!(matches!(
        refused(owner, supervisor, budget),
        Error::InvalidDescriptor { .. }
    ));
    owner.static_manifest_file = original;

    for index in [
        POLICY_SOURCE_INDEX,
        SIGNING_KEY_SOURCE_INDEX,
        LAUNCH_MANIFEST_SOURCE_INDEX,
    ] {
        let original = &owner.sources[index];
        let mut bytes = vec![0; original.metadata().unwrap().len() as usize];
        assert_eq!(
            rustix::io::pread(original, &mut bytes[..], 0).unwrap(),
            bytes.len()
        );
        let substitute = raw_image(
            &bytes,
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDONLY,
        );
        bytes.fill(0);
        assert!(!checks::same_object(
            &checks::object_identity(&substitute, "substitute").unwrap(),
            &checks::object_identity(original, "original").unwrap()
        ));
        let original = std::mem::replace(&mut owner.sources[index], substitute);
        assert!(matches!(
            refused(owner, supervisor, budget),
            Error::Supervisor(_) | Error::Capability(_)
        ));
        owner.sources[index] = original;
    }
    owner.revalidate(supervisor, budget).unwrap();
}

/// Only the existing real distinct-UID fixture calls this; no local admission shortcut.
pub(crate) fn exercise(peer: &OwnedFd, pidfd: &OwnedFd, submitter: &OwnedFd) {
    assert_eq!(rustix::process::geteuid().as_raw(), 65_533);
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget.reserve_storage(EXTRA).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (fixture, supervisor) = crate::authority_v2::tests::bound_fixture(peer, pidfd, &mut budget);
    let (expected_prepare_work, expected_revalidate_work) = expected_work(&fixture);
    let supervisor_storage = supervisor.retained_storage();
    assert_eq!(budget.storage(), EXTRA + supervisor_storage);
    let anchor_pid = supervisor.external_anchor_process().pid();
    send_packet(submitter, &frame(b"ANC2", anchor_pid), &[]).unwrap();
    let (accepted, control, pid, pidfds) = accept(&supervisor, submitter, &mut budget);
    let anchor_pidfds = pidfd_references(anchor_pid);
    let expected_manifest = *accepted.manifest().canonical_bytes();
    let consumed = accepted.retained_storage();
    let floor = budget.storage();
    let before = budget.work();
    let (mut owner, delta) = supervisor.prepare_launch(accepted, &mut budget).unwrap();
    let prepare_work = budget.work() - before;
    assert_eq!(prepare_work, expected_prepare_work);
    assert!(prepare_work > Prepared::WORK);
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        owner.retained_storage(),
        consumed + delta.additional_storage()
    );
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(
        budget.storage(),
        EXTRA + supervisor_storage + owner.retained_storage()
    );
    assert_eq!(
        owner.service_manifest().canonical_bytes(),
        &expected_manifest
    );
    assert_eq!(pidfd_references(pid), pidfds + 2);
    assert_eq!(pidfd_references(anchor_pid), anchor_pidfds + 1);
    inspect_prepared(&owner, pid, anchor_pid);
    let before = budget.work();
    owner.revalidate(&supervisor, &mut budget).unwrap();
    let revalidate_work = budget.work() - before;
    assert_eq!(revalidate_work, expected_revalidate_work);
    assert!(revalidate_work > Prepared::WORK);
    assert!(prepare_work > revalidate_work);
    revalidation_boundaries(&owner, &supervisor, expected_revalidate_work);
    mutations(&mut owner, &supervisor, &mut budget);
    let witnesses = prepared_witnesses(&owner);
    let retained = owner.retained_storage();
    drop(owner);
    budget.release_storage(retained).unwrap();
    for witness in witnesses {
        witness.assert_released();
    }
    control.assert_released();
    drop(control);
    assert_eq!(pidfd_references(pid), pidfds);
    assert_eq!(pidfd_references(anchor_pid), anchor_pidfds);
    assert_eq!(budget.storage(), EXTRA + supervisor_storage);
    assert!(budget.work_ledger_identity_v1() == ledger);

    preparation_boundaries(&supervisor, submitter, &mut budget, expected_prepare_work);
    let (accepted, control, pid, pidfds) = accept(&supervisor, submitter, &mut budget);
    let consumed = accepted.retained_storage();
    let mut denied_work = Work::new(WORK_LIMIT);
    let mut denied = Budget::new(&mut denied_work, STORAGE_LIMIT);
    assert!(denied.charge_work(WORK_LIMIT + 1).is_err());
    assert!(denied.reserve_storage(STORAGE_LIMIT + 1).is_err());
    denied
        .reserve_storage(EXTRA + supervisor_storage + consumed)
        .unwrap();
    let (owner, delta) = supervisor.prepare_launch(accepted, &mut denied).unwrap();
    denied.reserve_storage(delta.additional_storage()).unwrap();
    owner.revalidate(&supervisor, &mut denied).unwrap();
    assert_eq!(denied.work(), prepare_work + revalidate_work);
    let retained = owner.retained_storage();
    drop(owner);
    denied.release_storage(retained).unwrap();
    assert_eq!(denied.storage(), EXTRA + supervisor_storage);
    assert_eq!(denied.failed_storage(), Some(STORAGE_LIMIT + 1));
    assert_eq!(denied_work.failed_work(), Some(WORK_LIMIT + 1));
    budget.release_storage(consumed).unwrap();
    control.assert_released();
    drop(control);
    assert_eq!(pidfd_references(pid), pidfds);
    assert_eq!(pidfd_references(anchor_pid), anchor_pidfds);
    drop(supervisor);
    budget.release_storage(supervisor_storage).unwrap();
    assert_eq!(budget.storage(), EXTRA);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(work.failed_work(), None);
    drop(fixture);
}

fn static_fixture() -> StaticManifest {
    let entries: [_; 3] = std::array::from_fn(|index| {
        Descriptor::for_index(
            index,
            index as i32,
            Object::new(u64::MAX, index as u64 + 2, 1, 0o100600),
        )
        .unwrap()
    });
    StaticManifest::from_descriptors(42, 1, Object::new(u64::MAX, 1, 1, 0o100500), &entries)
        .unwrap()
}

fn raw_image(bytes: &[u8], mode: Mode, seals: SealFlags, access: OFlags) -> File {
    let writable = rustix::fs::memfd_create(
        c"fe2o3-native-prepared-test",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .map(File::from)
    .unwrap();
    assert_eq!(
        rustix::io::pwrite(&writable, bytes, 0).unwrap(),
        bytes.len()
    );
    rustix::fs::fchmod(&writable, mode).unwrap();
    rustix::fs::fcntl_add_seals(&writable, seals).unwrap();
    if access == OFlags::RDWR {
        return writable;
    }
    let directory = rustix::fs::open(
        c"/proc/self/fd",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    rustix::fs::openat(
        directory,
        rustix::path::DecInt::from_fd(&writable),
        access | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .unwrap()
}

#[test]
fn static_image_creation_binds_canonical_bytes_metadata_and_read_only_seals() {
    let manifest = static_fixture();
    let (file, object) = image::create(&manifest).unwrap();
    assert_eq!(
        references(object),
        1,
        "creation retains no writable or directory fd"
    );
    let stat = rustix::fs::fstat(&file).unwrap();
    assert_eq!(
        rustix::fs::FileType::from_raw_mode(stat.st_mode),
        rustix::fs::FileType::RegularFile
    );
    assert_eq!(stat.st_mode & 0o7777, MANIFEST_MODE_V1);
    assert_eq!(stat.st_uid, rustix::process::geteuid().as_raw());
    assert_eq!(stat.st_gid, rustix::process::getegid().as_raw());
    assert_eq!(stat.st_nlink, 0);
    assert_eq!(stat.st_size, BYTES as i64);
    assert_eq!(rustix::io::fcntl_getfd(&file).unwrap(), FdFlags::CLOEXEC);
    assert_eq!(
        rustix::fs::fcntl_getfl(&file).unwrap() & OFlags::ACCMODE,
        OFlags::RDONLY
    );
    assert_eq!(
        rustix::fs::fcntl_get_seals(&file).unwrap(),
        REQUIRED_MANIFEST_SEALS_V1
    );
    let mut bytes = [0; BYTES + 1];
    assert_eq!(rustix::io::pread(&file, &mut bytes[..], 0).unwrap(), BYTES);
    assert_eq!(&bytes[..BYTES], &manifest.encode());
    image::validate(&file, &manifest, object).unwrap();
    assert!(rustix::io::pwrite(&file, &[0], 0).is_err());
    assert!(rustix::fs::ftruncate(&file, 0).is_err());
    assert!(rustix::fs::fcntl_add_seals(&file, SealFlags::WRITE).is_err());
    image::validate(&file, &manifest, object).unwrap();
    let witness = Witness::new(&file, 1);
    drop(file);
    witness.assert_released();
}

#[test]
fn static_image_equal_bytes_do_not_substitute_for_the_original_object() {
    let manifest = static_fixture();
    let (original, expected) = image::create(&manifest).unwrap();
    let (substitute, other) = image::create(&manifest).unwrap();
    assert!(!checks::same_object(&expected, &other));
    image::validate(&original, &manifest, expected).unwrap();
    image::validate(&substitute, &manifest, other).unwrap();
    assert!(matches!(
        image::validate(&substitute, &manifest, expected),
        Err(Error::InvalidDescriptor { .. })
    ));
    for changed in [
        Object::new(
            expected.device() ^ 1,
            expected.inode(),
            expected.size(),
            expected.mode(),
        ),
        Object::new(
            expected.device(),
            expected.inode() ^ 1,
            expected.size(),
            expected.mode(),
        ),
        Object::new(
            expected.device(),
            expected.inode(),
            expected.size() + 1,
            expected.mode(),
        ),
        Object::new(
            expected.device(),
            expected.inode(),
            expected.size(),
            expected.mode() ^ 1,
        ),
        Object::new_process_pidfd(
            expected.device(),
            expected.inode(),
            expected.size(),
            expected.mode(),
        ),
    ] {
        assert!(matches!(
            image::validate(&original, &manifest, changed),
            Err(Error::InvalidDescriptor { .. })
        ));
    }
}

#[test]
fn static_image_revalidation_refuses_descriptor_mode_length_and_seal_drift() {
    let manifest = static_fixture();
    let (file, object) = image::create(&manifest).unwrap();
    rustix::io::fcntl_setfd(&file, FdFlags::empty()).unwrap();
    assert!(matches!(
        image::validate(&file, &manifest, object),
        Err(Error::InvalidDescriptor { .. })
    ));
    rustix::io::fcntl_setfd(&file, FdFlags::CLOEXEC).unwrap();
    rustix::fs::fchmod(&file, Mode::RUSR | Mode::WUSR).unwrap();
    assert!(matches!(
        image::validate(&file, &manifest, object),
        Err(Error::InvalidDescriptor { .. })
    ));
    rustix::fs::fchmod(&file, Mode::RUSR).unwrap();
    image::validate(&file, &manifest, object).unwrap();
    let bytes = manifest.encode();
    let extended = [0; BYTES + 1];
    for (bytes, mode, seals, access) in [
        (
            &bytes[..BYTES - 1],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDONLY,
        ),
        (
            &extended[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDONLY,
        ),
        (
            &bytes[..],
            Mode::RUSR | Mode::WUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDONLY,
        ),
        (
            &bytes[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDWR,
        ),
        (
            &bytes[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1 - SealFlags::WRITE,
            OFlags::RDONLY,
        ),
        (
            &bytes[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1 - SealFlags::GROW,
            OFlags::RDONLY,
        ),
        (
            &bytes[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1 - SealFlags::SHRINK,
            OFlags::RDONLY,
        ),
        (
            &bytes[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1 - SealFlags::SEAL,
            OFlags::RDONLY,
        ),
    ] {
        let file = raw_image(bytes, mode, seals, access);
        let actual = checks::object_identity(&file, "fixture image").unwrap();
        assert!(matches!(
            image::validate(&file, &manifest, actual),
            Err(Error::InvalidDescriptor { .. })
        ));
    }
}

#[test]
fn static_image_revalidation_checks_decoding_and_exact_expected_bytes() {
    let manifest = static_fixture();
    let canonical = manifest.encode();
    for (offset, expected) in [
        (0, Some(StaticError::InvalidMagic)),
        (
            64 + 3 * 40,
            Some(StaticError::NonzeroInactiveDescriptor { index: 3 }),
        ),
        (24, None),
    ] {
        let mut bytes = canonical;
        bytes[offset] ^= 2;
        let file = raw_image(
            &bytes,
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDONLY,
        );
        let actual = checks::object_identity(&file, "fixture image").unwrap();
        match (
            image::validate(&file, &manifest, actual).unwrap_err(),
            expected,
        ) {
            (Error::StaticManifest(actual), Some(expected)) => assert_eq!(actual, expected),
            (Error::DescriptorChanged("static pre-exec manifest bytes"), None) => {}
            (actual, expected) => {
                panic!("unexpected byte refusal: {actual:?}, expected {expected:?}")
            }
        }
    }
}
