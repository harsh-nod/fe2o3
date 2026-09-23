use super::*;
use crate::tests::Fixture;
use ed25519_dalek::SigningKey;
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2, CompilerExecutionPolicyCapabilityV2 as Cap,
};
use fe2o3_compiler_execution_protocol::sealed_static_issuer_runtime_measurement_v1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOperationV2 as Operation, ProtectedStaticExecutableV2 as Image,
};
use std::os::{fd::OwnedFd, unix::fs::MetadataExt};

type Supervisor = ProtectedIssuerSupervisorV2;
type Failure = ProtectedIssuerSupervisorErrorV2;
const EXTRA: usize = 19;
const WORK_LIMIT: usize = 10_000_000_000;
const STORAGE_LIMIT: usize = 10_000_000;
const SEED: [u8; 32] = [0x51; 32];

fn credentials() -> Credentials {
    Credentials::new(
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap()
}

fn policy(f: &Fixture, generation: u64, budget: &mut Budget<'_>) -> Policy {
    let (policy, delta) = Policy::new(
        generation,
        f.issuer_measurement(),
        sealed_static_issuer_runtime_measurement_v1(),
        SigningKey::from_bytes(&SEED).verifying_key().to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        budget,
    )
    .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    policy
}

fn measurement(f: &Fixture) -> Measurement {
    let m = f.measurement();
    Measurement::new(m.sha256(), m.byte_len(), 128 * 1024 * 1024).unwrap()
}

struct Inputs {
    program: Program,
    key: Key,
    anchor: Anchor,
    root: File,
}
impl Inputs {
    fn new(
        f: &Fixture,
        peer: &OwnedFd,
        pidfd: &OwnedFd,
        key_generation: u64,
        budget: &mut Budget<'_>,
    ) -> Self {
        let record = policy(f, 7, budget);
        let (cap, delta) = Cap::create(record, budget).unwrap();
        budget.reserve_storage(delta.additional_storage()).unwrap();
        budget
            .reserve_storage(2 * Image::file_storage(measurement(f)).unwrap())
            .unwrap();
        let (program, delta) =
            Program::provision(f.open(), f.measurement(), f.open(), cap, budget).unwrap();
        budget.reserve_storage(delta.additional_storage()).unwrap();
        let (key, temporary) = {
            let other = (key_generation != 7).then(|| policy(f, key_generation, budget));
            budget.reserve_storage(SEED.len()).unwrap();
            let mut seed = SEED;
            let (key, delta) = Key::create_and_zeroize(
                &mut seed,
                other.as_ref().unwrap_or_else(|| program.policy()),
                budget,
            )
            .unwrap();
            assert_eq!(seed, [0; 32]);
            budget.reserve_storage(delta.additional_storage()).unwrap();
            (
                key,
                SEED.len() + other.as_ref().map_or(0, Policy::retained_storage),
            )
        };
        budget.release_storage(temporary).unwrap();
        budget.reserve_storage(Anchor::PAIR_STORAGE).unwrap();
        let (anchor, delta) = Anchor::admit(
            rustix::io::fcntl_dupfd_cloexec(peer, 3).unwrap(),
            rustix::io::fcntl_dupfd_cloexec(pidfd, 3).unwrap(),
            AnchorIdentity::new(65_534, 65_534).unwrap(),
            budget,
        )
        .unwrap();
        budget.reserve_storage(delta.additional_storage()).unwrap();
        budget
            .reserve_storage(Supervisor::ROOT_FILE_STORAGE)
            .unwrap();
        Self {
            program,
            key,
            anchor,
            root: File::open(&f.root).unwrap(),
        }
    }
    fn floor(&self) -> usize {
        self.program.retained_storage()
            + self.key.retained_storage()
            + self.anchor.retained_storage()
            + Supervisor::ROOT_FILE_STORAGE
    }
    fn bind(
        self,
        credentials: Credentials,
        budget: &mut Budget<'_>,
    ) -> Result<(Supervisor, Storage)> {
        Supervisor::bind(
            self.program,
            credentials,
            self.root,
            self.key,
            self.anchor,
            budget,
        )
    }
}

fn prepared(f: &Fixture, peer: &OwnedFd, pidfd: &OwnedFd, generation: u64) -> Inputs {
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    Inputs::new(f, peer, pidfd, generation, &mut budget)
}

fn nested_work(f: &Fixture) -> usize {
    Program::WORK
        + Cap::IO_WORK
        + 2 * Image::quota(measurement(f), Operation::Revalidate)
            .unwrap()
            .work()
        + Key::IO_WORK
        + Anchor::REVALIDATION_WORK
}

fn root_references(root: &File) -> usize {
    let object = root.metadata().unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (object.dev(), object.ino()))
        .count()
}

fn pidfd_references(pid: u32) -> usize {
    // Count this uniquely held anchor process, not shared anon-inodes or other
    // tests' self pidfds. Only this fixture opens descriptors for that target.
    std::fs::read_dir("/proc/self/fdinfo")
        .unwrap()
        .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path()).ok())
        .filter(|record| {
            record.lines().any(|line| {
                line.strip_prefix("Pid:\t")
                    .and_then(|pid| pid.parse::<u32>().ok())
                    == Some(pid)
            })
        })
        .count()
}

fn resource(mut error: &(dyn Error + 'static)) -> Resource {
    loop {
        if let Some(resource) = error.downcast_ref::<Resource>() {
            return *resource;
        }
        error = error.source().expect("expected a typed resource refusal");
    }
}

/// Called only by the isolated, real distinct-UID process fixture.
pub(crate) fn exercise(peer: OwnedFd, pidfd: OwnedFd) {
    assert_eq!((credentials().uid(), credentials().gid()), (65_533, 65_533));
    let f = Fixture::new("native-supervisor");
    let root = File::open(&f.root).unwrap();
    let before = root_references(&root);
    let anchor_pid = u32::try_from(
        rustix::net::sockopt::socket_peercred(&peer)
            .unwrap()
            .pid
            .as_raw_nonzero()
            .get(),
    )
    .unwrap();
    let before_pidfds = pidfd_references(anchor_pid);
    assert!(before_pidfds >= 1);
    let nested = nested_work(&f);
    let bind_work = Supervisor::WORK + 2 * nested;
    let revalidation_work = Supervisor::WORK + nested;

    // One ledger from native policy construction through terminal owner release.
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget.reserve_storage(EXTRA).unwrap();
    let inputs = Inputs::new(&f, &peer, &pidfd, 7, &mut budget);
    let floor = inputs.floor();
    assert_eq!(budget.storage(), EXTRA + floor);
    let start_work = budget.work();
    let (supervisor, delta) = inputs.bind(credentials(), &mut budget).unwrap();
    assert_eq!(pidfd_references(anchor_pid), before_pidfds + 1);
    assert_eq!(budget.work() - start_work, bind_work);
    assert_eq!(budget.storage(), EXTRA + floor);
    assert_eq!(delta.additional_storage(), Supervisor::OWNER_GROWTH);
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(budget.storage(), EXTRA + supervisor.retained_storage());
    assert_eq!(supervisor.credentials(), credentials());
    assert_eq!(
        supervisor.external_anchor_service(),
        AnchorIdentity::new(65_534, 65_534).unwrap()
    );
    assert_eq!(supervisor.policy().generation(), 7);
    supervisor.revalidate(&mut budget).unwrap();
    assert_eq!(budget.work() - start_work, bind_work + revalidation_work);
    let debug = format!("{supervisor:?}");
    assert!(debug.contains("pre-session-custody-only"));
    for private in ["/proc/", "File {", "root_snapshot", "SigningKey"] {
        assert!(!debug.contains(private));
    }
    let retained = supervisor.retained_storage();
    drop(supervisor);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), EXTRA);
    assert_eq!(root_references(&root), before);
    assert_eq!(pidfd_references(anchor_pid), before_pidfds);

    let mut peak = 0;
    for case in 0..6 {
        let inputs = prepared(&f, &peer, &pidfd, 7);
        let floor = inputs.floor();
        let prepaid = if case == 3 { floor - 1 } else { floor + EXTRA };
        let work_limit = match case {
            1 => bind_work - 1,
            5 => ENTRY - 1,
            _ => bind_work,
        };
        let storage_limit = match case {
            2 => peak - 1,
            4 => peak,
            _ => STORAGE_LIMIT,
        };
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(prepaid).unwrap();
        let result = inputs.bind(credentials(), &mut budget);
        assert_eq!(budget.storage(), prepaid);
        match case {
            0 | 4 => {
                let (owner, delta) = result.unwrap();
                assert_eq!(owner.retained_storage(), floor + delta.additional_storage());
                assert_eq!(budget.work(), bind_work);
                if case == 0 {
                    peak = budget.peak_storage();
                } else {
                    assert_eq!(budget.peak_storage(), peak);
                }
                drop(owner);
            }
            1 | 5 => {
                let failure = result.unwrap_err();
                assert!(matches!(resource(&failure), Resource::Work(_)));
                if case == 1 {
                    assert!(matches!(failure, Failure::ExternalAnchor(_)));
                    assert_eq!(budget.work(), bind_work - Anchor::REVALIDATION_WORK + ENTRY);
                }
            }
            2 => {
                assert!(matches!(
                    resource(&result.unwrap_err()),
                    Resource::Storage(_)
                ));
                assert_eq!(budget.failed_storage(), Some(peak));
            }
            _ => {
                assert_eq!(resource(&result.unwrap_err()), Resource::Accounting);
                assert_eq!(budget.work(), ENTRY);
            }
        }
        if case == 5 {
            assert_eq!(budget.work(), 0);
        }
        if case == 1 {
            assert_eq!(work.failed_work(), Some(bind_work));
        }
        assert_eq!(root_references(&root), before);
        assert_eq!(pidfd_references(anchor_pid), before_pidfds);
    }

    let inputs = prepared(&f, &peer, &pidfd, 7);
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget.reserve_storage(inputs.floor()).unwrap();
    let (mut supervisor, delta) = inputs.bind(credentials(), &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let retained = supervisor.retained_storage();
    let mut peak = 0;
    for case in 0..6 {
        let prepaid = if case == 3 {
            retained - 1
        } else {
            retained + EXTRA
        };
        let limit = match case {
            1 => revalidation_work - 1,
            5 => ENTRY - 1,
            _ => revalidation_work,
        };
        let mut work = Work::new(limit);
        let mut budget = Budget::new(
            &mut work,
            match case {
                2 => peak - 1,
                4 => peak,
                _ => STORAGE_LIMIT,
            },
        );
        budget.reserve_storage(prepaid).unwrap();
        let result = supervisor.revalidate(&mut budget);
        assert_eq!(budget.storage(), prepaid);
        match case {
            0 | 4 => {
                result.unwrap();
                assert_eq!(budget.work(), revalidation_work);
                if case == 0 {
                    peak = budget.peak_storage();
                } else {
                    assert_eq!(budget.peak_storage(), peak);
                }
            }
            1 | 5 => {
                let failure = result.unwrap_err();
                assert!(matches!(resource(&failure), Resource::Work(_)));
                if case == 1 {
                    assert!(matches!(failure, Failure::ExternalAnchor(_)));
                    assert_eq!(
                        budget.work(),
                        revalidation_work - Anchor::REVALIDATION_WORK + ENTRY
                    );
                }
            }
            2 => {
                assert!(matches!(
                    resource(&result.unwrap_err()),
                    Resource::Storage(_)
                ));
                assert_eq!(budget.failed_storage(), Some(peak));
            }
            _ => assert_eq!(resource(&result.unwrap_err()), Resource::Accounting),
        }
        if case == 1 {
            assert_eq!(work.failed_work(), Some(revalidation_work));
        }
    }
    retained_drift(&mut supervisor, &peer, &f);
    drop(supervisor);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 0);
    assert_eq!(root_references(&root), before);
    assert_eq!(pidfd_references(anchor_pid), before_pidfds);

    for wrong_credentials in [false, true] {
        let inputs = prepared(&f, &peer, &pidfd, 8);
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.reserve_storage(inputs.floor() + EXTRA).unwrap();
        let expected_floor = budget.storage();
        let selected = if wrong_credentials {
            Credentials::new(credentials().uid(), 65_532).unwrap()
        } else {
            credentials()
        };
        let failure = inputs.bind(selected, &mut budget).unwrap_err();
        if wrong_credentials {
            assert!(matches!(failure, Failure::ServiceIdentityMismatch));
            assert_eq!(budget.work(), Supervisor::WORK);
        } else {
            assert!(matches!(
                failure,
                Failure::SigningKey(CompilerExecutionCapabilityErrorV2::Rejected(
                    "signing key is pinned to another native policy"
                ))
            ));
        }
        assert_eq!(budget.storage(), expected_floor);
        assert_eq!(root_references(&root), before);
        assert_eq!(pidfd_references(anchor_pid), before_pidfds);
    }

    let inputs = prepared(&f, &peer, &pidfd, 7);
    let prepaid = inputs.floor() + EXTRA;
    let total_work = bind_work + revalidation_work;
    let mut work = Work::new(total_work);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget.reserve_storage(prepaid).unwrap();
    assert!(budget.charge_work(total_work + 1).is_err());
    assert!(budget.reserve_storage(STORAGE_LIMIT).is_err());
    let (supervisor, delta) = inputs.bind(credentials(), &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    supervisor.revalidate(&mut budget).unwrap();
    assert_eq!(budget.failed_storage(), Some(prepaid + STORAGE_LIMIT));
    assert_eq!(budget.work(), total_work);
    let retained = supervisor.retained_storage();
    drop(supervisor);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), EXTRA);
    assert_eq!(work.failed_work(), Some(total_work + 1));
    assert_eq!(root_references(&root), before);
    assert_eq!(pidfd_references(anchor_pid), before_pidfds);
}

fn check(supervisor: &Supervisor) -> Result<()> {
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget
        .reserve_storage(supervisor.retained_storage() + EXTRA)
        .unwrap();
    let result = supervisor.revalidate(&mut budget);
    assert_eq!(budget.storage(), supervisor.retained_storage() + EXTRA);
    result
}

fn retained_drift(supervisor: &mut Supervisor, peer: &OwnedFd, fixture: &Fixture) {
    let original_credentials = supervisor.credentials;
    supervisor.credentials = Credentials::new(65_532, original_credentials.gid()).unwrap();
    assert!(matches!(
        check(supervisor),
        Err(Failure::ServiceIdentityMismatch)
    ));
    supervisor.credentials = original_credentials;
    rustix::fs::fchmod(&supervisor.root, rustix::fs::Mode::from_raw_mode(0o750)).unwrap();
    assert!(matches!(
        check(supervisor),
        Err(Failure::InvalidRoot("mode is not exactly 0700"))
    ));
    rustix::fs::fchmod(&supervisor.root, rustix::fs::Mode::from_raw_mode(0o700)).unwrap();
    rustix::io::fcntl_setfd(&supervisor.root, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(
        check(supervisor),
        Err(Failure::InvalidRoot("descriptor is inheritable"))
    ));
    rustix::io::fcntl_setfd(&supervisor.root, rustix::io::FdFlags::CLOEXEC).unwrap();
    let different = Fixture::new("native-supervisor-different-root");
    let original = std::mem::replace(&mut supervisor.root, File::open(&different.root).unwrap());
    assert!(matches!(check(supervisor), Err(Failure::RootChanged)));
    supervisor.root = original;
    let subdirectory = fixture.root.join("new-link");
    std::fs::create_dir(&subdirectory).unwrap();
    assert!(matches!(check(supervisor), Err(Failure::RootChanged)));
    std::fs::remove_dir(&subdirectory).unwrap();
    let flags = rustix::fs::fcntl_getfl(peer).unwrap();
    rustix::fs::fcntl_setfl(peer, flags - rustix::fs::OFlags::NONBLOCK).unwrap();
    assert!(matches!(check(supervisor), Err(Failure::ExternalAnchor(_))));
    rustix::fs::fcntl_setfl(peer, flags).unwrap();
    check(supervisor).unwrap();
}

#[test]
fn fixed_root_errors_preserve_native_kind_and_legacy_diagnostic_text() {
    let error = Failure::from(RootCheckError::Invalid("mode is not exactly 0700"));
    assert_eq!(
        error.to_string(),
        "invalid protected issuer root: mode is not exactly 0700"
    );
    let error = Failure::from(RootCheckError::Io {
        operation: "inspect protected issuer root",
        errno: rustix::io::Errno::BADF,
    });
    assert_eq!(
        error.source().unwrap().downcast_ref::<rustix::io::Errno>(),
        Some(&rustix::io::Errno::BADF)
    );
}

#[test]
fn credential_checks_reject_each_identity_axis_without_changing_process_state() {
    let uid = rustix::process::geteuid().as_raw();
    let gid = rustix::process::getegid().as_raw();
    let other = |id| if id == 1 { 2 } else { 1 };
    for (uid, gid) in [(other(uid), gid.max(1)), (uid.max(1), other(gid))] {
        assert!(matches!(
            require_credentials(Credentials::new(uid, gid).unwrap()),
            Err(Failure::ServiceIdentityMismatch)
        ));
    }
    if uid != 0 && gid != 0 {
        require_credentials(Credentials::new(uid, gid).unwrap()).unwrap();
    }
}
