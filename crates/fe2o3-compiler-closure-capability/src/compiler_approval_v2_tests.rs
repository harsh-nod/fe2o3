//! Synthetic root-policy intake only: no production approval or runtime-guard evidence.
use super::*;
use ed25519_dalek::SigningKey;
use fe2o3_build_authority::COMPILER_APPROVAL_POLICY_WORK_V2 as POLICY_WORK;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3 as PROFILE_STORAGE,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3 as PROFILE_WORK,
    CompilerExecutionClientProfileErrorV3 as ProfileError,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV3 as IssuerPolicy,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, UNIX_EPOCH},
};

type Owner = ApprovedCompilerPolicyV2;
type Error = CompilerApprovalErrorV2;
const POLICY_REL: &str = "etc/fe2o3/build-authority/policy-v2";
const PROFILE_REL: &str = "etc/fe2o3/compiler-execution/client-profile-v3";
const FLOOR: usize = 19;
const LIMIT: usize = 8_000_000;
const LOAD_WORK: usize =
    IO_WORK + POLICY_WORK + PROFILE_WORK + ProfileCapability::IO_WORK + BIND_WORK;
const REVALIDATE_WORK: usize =
    8 + IO_WORK + ProfileCapability::IO_WORK + POLICY_BYTES + PROFILE_BYTES;

thread_local! {
    static PROBES: Cell<usize> = const { Cell::new(0) };
}

// This private probe models an intake prerequisite, never an actual immutable inode.
fn fixture_immutable(_: &File) -> Result<()> {
    PROBES.with(|calls| calls.set(calls.get() + 1));
    Ok(())
}
fn refuse_probe<const AT: usize>(file: &File) -> Result<()> {
    fixture_immutable(file)?;
    if PROBES.with(Cell::get) == AT {
        return Err(Error::Mismatch("fixture immutable probe refused"));
    }
    Ok(())
}
fn change_time<const AT: usize>(file: &File) -> Result<()> {
    fixture_immutable(file)?;
    if PROBES.with(Cell::get) == AT {
        file.set_times(fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1)))
            .unwrap();
    }
    Ok(())
}
fn owners() -> (u32, u32) {
    (
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    )
}
fn chmod(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}
fn fixture_profile(seed: u8) -> Profile {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (policy, charge) = IssuerPolicy::new(
        u64::from(seed),
        Measurement::new([seed; 32], 123).unwrap(),
        Measurement::new([seed + 1; 32], 456).unwrap(),
        SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[seed + 1; 32])
            .verifying_key()
            .to_bytes(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (profile, charge) = Profile::new(
        1000,
        1000,
        Service::new(1001, 1001).unwrap(),
        policy,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    profile
}

struct Tree {
    root: PathBuf,
    policy: CompilerApprovalPolicyV2,
    profile: Profile,
}
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "compiler-approval-intake-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        chmod(&root, 0o700);
        for relative in [
            "etc",
            "etc/fe2o3",
            "etc/fe2o3/build-authority",
            "etc/fe2o3/compiler-execution",
        ] {
            let path = root.join(relative);
            fs::create_dir(&path).unwrap();
            chmod(&path, 0o755);
        }
        let profile = fixture_profile(7);
        let closure =
            CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap();
        let policy = CompilerApprovalPolicyV2::new(
            closure,
            *profile.identity().as_bytes(),
            [8; 32],
            1,
            1002,
            1002,
            |_| Ok::<_, Resource>(()),
        )
        .unwrap();
        let tree = Self {
            root,
            policy,
            profile,
        };
        tree.write(POLICY_REL, tree.policy.canonical_bytes());
        tree.write(PROFILE_REL, tree.profile.canonical_bytes());
        tree
    }
    fn path(&self, relative: &str) -> PathBuf {
        if relative.is_empty() {
            self.root.clone()
        } else {
            self.root.join(relative)
        }
    }
    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.path(relative);
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        fs::write(&path, bytes).unwrap();
        chmod(&path, 0o444);
    }
    fn open(&self) -> Result<File> {
        rustix::fs::open(&self.root, tree::DIRECTORY_FLAGS, Mode::empty())
            .map(File::from)
            .map_err(|e| io("open synthetic approval root", e))
    }
    fn load(
        &self,
        probe: ImmutableCheck,
        budget: &mut Budget<'_>,
    ) -> Result<(Owner, CompilerApprovalStorageV2)> {
        let (uid, gid) = owners();
        Owner::load_using(|| self.open(), uid, gid, probe, budget)
    }
    fn revalidate(
        &self,
        owner: &Owner,
        probe: ImmutableCheck,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let (uid, gid) = owners();
        owner.revalidate_using(|| self.open(), uid, gid, probe, budget)
    }
    fn retained(&self, budget: &mut Budget<'_>) -> Owner {
        let floor = budget.storage();
        let (owner, charge) = self.load(fixture_immutable, budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(charge.retained_storage(), owner.required_retained_storage());
        budget.reserve_storage(charge.retained_storage()).unwrap();
        owner
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[derive(Debug)]
struct Usage {
    work: usize,
    live: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
struct DropNotice<'a>(&'a Cell<usize>);
impl Drop for DropNotice<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
fn observe<T>(
    work: usize,
    storage: usize,
    operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> (Result<T>, Usage) {
    let mut meter = Work::new(work);
    let mut budget = Budget::new(&mut meter, storage);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let address = &budget as *const Budget<'_> as usize;
    let result = operation(&mut budget);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(&budget as *const Budget<'_> as usize, address);
    let usage = Usage {
        work: budget.work(),
        live: budget.storage(),
        peak: budget.peak_storage(),
        failed_work: budget.failed_work(),
        failed_storage: budget.failed_storage(),
    };
    (result, usage)
}
fn failure<T>(result: Result<T>) -> Error {
    match result {
        Ok(_) => panic!("expected synthetic intake refusal"),
        Err(error) => error,
    }
}
fn resource(error: Error) -> Resource {
    match error {
        Error::Resource(error)
        | Error::Codec(CompilerApprovalPolicyErrorV2::Framing(
            fe2o3_build_authority::CompilerApprovalPolicyErrorV1::Charge(error),
        ))
        | Error::Capability(CapabilityError::Resource(error))
        | Error::Capability(CapabilityError::ProfileV3(ProfileError::Resource(error))) => error,
        error => panic!("expected resource refusal: {error:?}"),
    }
}

#[test]
fn synthetic_intake_retains_exact_policy_profile_sources_and_original_account() {
    let tree = Tree::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let address = &budget as *const Budget<'_> as usize;
    PROBES.with(|calls| calls.set(0));
    let owner = tree.retained(&mut budget);
    assert_eq!(PROBES.with(Cell::get), 3);
    assert_eq!(budget.work(), LOAD_WORK);
    assert_eq!(owner.address, address);
    assert!(owner.ledger == ledger);
    assert_eq!(owner.policy(), &tree.policy);
    assert!(!owner.policy().grants_authority());
    assert_eq!(
        owner.profile().profile().canonical_bytes(),
        tree.profile.canonical_bytes()
    );
    for (file, relative) in [
        (&owner.policy_file, POLICY_REL),
        (&owner.profile_file, PROFILE_REL),
    ] {
        let source = fs::metadata(tree.path(relative)).unwrap();
        let retained = file.metadata().unwrap();
        assert_eq!(
            (retained.dev(), retained.ino()),
            (source.dev(), source.ino())
        );
        rustix::fs::seek(file, rustix::fs::SeekFrom::Start(37)).unwrap();
    }
    let live = budget.storage();
    PROBES.with(|calls| calls.set(0));
    tree.revalidate(&owner, fixture_immutable, &mut budget)
        .unwrap();
    assert_eq!(PROBES.with(Cell::get), 3);
    assert_eq!(budget.work(), LOAD_WORK + REVALIDATE_WORK);
    assert_eq!(budget.storage(), live);
    assert!(budget.work_ledger_identity_v1() == ledger);
    for file in [&owner.policy_file, &owner.profile_file] {
        assert_eq!(
            rustix::fs::seek(file, rustix::fs::SeekFrom::Current(0)).unwrap(),
            37
        );
    }
    let retained = owner.required_retained_storage();
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(POLICY_BYTES, 352);
    assert_eq!(
        format!("/{}", POLICY_PATH.join("/")),
        format!("/{POLICY_REL}")
    );
    assert_eq!(
        format!("/{}", PROFILE_PATH.join("/")),
        format!("/{PROFILE_REL}")
    );
}

#[test]
fn outer_work_and_storage_are_prepaid_before_opening_any_source() {
    for (work, storage, used, denied_work, denied_storage) in [
        (7, LIMIT, 0, Some(8), None),
        (IO_WORK - 1, LIMIT, 8, Some(IO_WORK), None),
        (
            IO_WORK,
            FLOOR + FRAME - 1,
            IO_WORK,
            None,
            Some(FLOOR + FRAME),
        ),
        (IO_WORK, FLOOR + FRAME, IO_WORK, None, None),
    ] {
        let called = Cell::new(false);
        let dropped = Cell::new(0);
        let notice = DropNotice(&dropped);
        let (result, usage) = observe(work, storage, |budget| {
            Owner::load_using(
                || {
                    drop(notice);
                    called.set(true);
                    Err(Error::Mismatch("fixture root refused"))
                },
                0,
                0,
                fixture_immutable,
                budget,
            )
        });
        assert_eq!(usage.work, used);
        assert_eq!(usage.live, FLOOR);
        assert_eq!(usage.failed_work, denied_work);
        assert_eq!(usage.failed_storage, denied_storage);
        assert_eq!(dropped.get(), 1);
        assert_eq!(
            called.get(),
            denied_work.is_none() && denied_storage.is_none()
        );
        if called.get() {
            assert!(matches!(
                failure(result),
                Error::Mismatch("fixture root refused")
            ));
            assert_eq!(usage.peak, FLOOR + FRAME);
        } else {
            assert!(matches!(
                resource(failure(result)),
                Resource::Work(_) | Resource::Storage(_)
            ));
            assert_eq!(usage.peak, FLOOR);
        }
    }
}

#[test]
fn nested_decode_and_capability_work_use_the_original_ledger_without_refunds() {
    let tree = Tree::new();
    for (limit, accepted, failed) in [
        (IO_WORK + POLICY_WORK - 1, IO_WORK, IO_WORK + POLICY_WORK),
        (
            IO_WORK + POLICY_WORK + PROFILE_WORK - 1,
            IO_WORK + POLICY_WORK + 8,
            IO_WORK + POLICY_WORK + PROFILE_WORK,
        ),
        (
            LOAD_WORK - BIND_WORK - 1,
            IO_WORK + POLICY_WORK + PROFILE_WORK + 8,
            LOAD_WORK - BIND_WORK,
        ),
        (LOAD_WORK - 1, LOAD_WORK - BIND_WORK, LOAD_WORK),
    ] {
        let (result, usage) = observe(limit, LIMIT, |b| tree.load(fixture_immutable, b));
        assert!(matches!(resource(failure(result)), Resource::Work(_)));
        assert_eq!(usage.work, accepted);
        assert_eq!(usage.failed_work, Some(failed));
        assert_eq!(usage.live, FLOOR);
        assert!(usage.peak >= FLOOR + FRAME);
    }
}

#[test]
fn exact_and_one_short_storage_include_nested_profile_and_retained_owner() {
    let tree = Tree::new();
    let (result, usage) = observe(LOAD_WORK, LIMIT, |b| tree.load(fixture_immutable, b));
    let (owner, charge) = result.unwrap();
    let retained = owner.required_retained_storage();
    assert_eq!(charge.retained_storage(), retained);
    assert_eq!(
        retained,
        size_of::<Owner>()
            + size_of::<CompilerApprovalStorageV2>()
            + size_of::<(
                ProfileCapability,
                crate::CompilerExecutionCapabilityStorageV2,
            )>()
            - size_of::<ProfileCapability>()
            + PROFILE_BYTES // Sealed profile backing.
            + POLICY_BYTES // Original policy-file backing.
            + PROFILE_BYTES // Original profile-file backing.
    );
    assert_eq!(usage.work, LOAD_WORK);
    assert_eq!(usage.live, FLOOR);
    let peak = FLOOR
        + FRAME
        + PROFILE_STORAGE
            .max(tree.profile.retained_storage() + ProfileCapability::IO_STORAGE)
            .max(retained);
    assert_eq!(usage.peak, peak);
    drop(owner);
    for (storage, accepted) in [(peak - 1, false), (peak, true)] {
        let (result, usage) = observe(LOAD_WORK, storage, |b| tree.load(fixture_immutable, b));
        assert_eq!(usage.live, FLOOR);
        assert_eq!(usage.failed_storage, (!accepted).then_some(peak));
        if accepted {
            let (owner, charge) = result.unwrap();
            assert_eq!(charge.retained_storage(), owner.required_retained_storage());
            assert_eq!(usage.work, LOAD_WORK);
            assert_eq!(usage.peak, peak);
        } else {
            assert!(matches!(resource(failure(result)), Resource::Storage(_)));
        }
    }
}

#[test]
fn revalidation_exact_and_one_short_budgets_preserve_the_live_owner() {
    let tree = Tree::new();
    let retained = observe(LOAD_WORK, LIMIT, |b| tree.load(fixture_immutable, b))
        .0
        .unwrap()
        .0
        .required_retained_storage();
    // A separate caller reservation makes this peak reachable after constructor decoding.
    let caller_storage = PROFILE_STORAGE;
    let peak = FLOOR + retained + caller_storage + FRAME + ProfileCapability::IO_STORAGE;
    for (extra, storage, expected_work, success) in [
        (7, LIMIT, 0, false),
        (15, LIMIT, 8, false),
        (8 + IO_WORK - 1, LIMIT, 16, false),
        (
            8 + IO_WORK + ProfileCapability::IO_WORK - 1,
            LIMIT,
            8 + IO_WORK + 8,
            false,
        ),
        (
            REVALIDATE_WORK - 1,
            LIMIT,
            8 + IO_WORK + ProfileCapability::IO_WORK,
            false,
        ),
        (
            REVALIDATE_WORK,
            peak - 1,
            8 + IO_WORK + ProfileCapability::IO_WORK,
            false,
        ),
        (REVALIDATE_WORK, peak, REVALIDATE_WORK, true),
    ] {
        let mut work = Work::new(LOAD_WORK + extra);
        let mut budget = Budget::new(&mut work, storage);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = tree.retained(&mut budget);
        budget.reserve_storage(caller_storage).unwrap();
        let live = budget.storage();
        let result = tree.revalidate(&owner, fixture_immutable, &mut budget);
        assert_eq!(budget.work(), LOAD_WORK + expected_work);
        assert_eq!(budget.storage(), live);
        if success {
            result.unwrap();
            assert_eq!(budget.peak_storage(), peak);
        } else {
            assert!(matches!(
                resource(failure(result)),
                Resource::Work(_) | Resource::Storage(_)
            ));
            if storage == peak - 1 {
                assert_eq!(budget.failed_storage(), Some(peak));
            } else {
                assert!(budget.failed_work().is_some());
            }
        }
        drop(owner);
        budget.release_storage(retained).unwrap();
        budget.release_storage(caller_storage).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn owner_rejects_moved_budget_swapped_ledger_and_underreserved_storage() {
    let tree = Tree::new();
    let mut first_work = Work::new(LIMIT);
    let mut other_work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut first_work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = tree.retained(&mut budget);
    let mut original = Box::new(std::mem::replace(
        &mut budget,
        Budget::new(&mut other_work, LIMIT),
    ));
    budget.reserve_storage(original.storage()).unwrap();
    assert_eq!(&budget as *const Budget<'_> as usize, owner.address);
    assert!(budget.work_ledger_identity_v1() != owner.ledger);
    assert!(original.work_ledger_identity_v1() == owner.ledger);
    assert_ne!(&*original as *const Budget<'_> as usize, owner.address);
    for current in [&mut budget, &mut *original] {
        let before = current.work();
        assert!(matches!(
            resource(failure(tree.revalidate(&owner, fixture_immutable, current))),
            Resource::Accounting
        ));
        assert_eq!(current.work(), before + 8);
        assert!(matches!(
            resource(failure(
                owner.match_compiler(tree.policy.compiler_closure(), current)
            )),
            Resource::Accounting
        ));
        assert_eq!(current.work(), before + 16);
    }
    std::mem::swap(&mut budget, &mut *original);
    tree.revalidate(&owner, fixture_immutable, &mut budget)
        .unwrap();
    budget.release_storage(FLOOR + 1).unwrap();
    assert_eq!(budget.storage(), owner.required_retained_storage() - 1);
    let called = Cell::new(false);
    let (uid, gid) = owners();
    let error = failure(owner.revalidate_using(
        || {
            called.set(true);
            tree.open()
        },
        uid,
        gid,
        fixture_immutable,
        &mut budget,
    ));
    assert!(matches!(resource(error), Resource::Accounting));
    assert!(!called.get());
    assert!(matches!(
        resource(failure(
            owner.require_compiler(tree.policy.compiler_closure(), &mut budget)
        )),
        Resource::Accounting
    ));
}

#[test]
fn fixture_revalidation_then_compiler_match_checks_all_six_pins() {
    let tree = Tree::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = tree.retained(&mut budget);
    let live = budget.storage();
    for changed in 0..=6 {
        let mut pins = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]];
        if changed < 6 {
            pins[changed][0] ^= 128;
        }
        let closure =
            CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5]).unwrap();
        tree.revalidate(&owner, fixture_immutable, &mut budget)
            .unwrap();
        let before = budget.work();
        let result = owner.match_compiler(closure, &mut budget);
        assert_eq!(budget.work(), before + 520);
        assert_eq!(budget.storage(), live);
        if changed == 6 {
            result.unwrap();
        } else {
            assert!(matches!(
                failure(result),
                Error::Mismatch("compiler differs from root-approved closure")
            ));
        }
    }
    assert!(!owner.policy().grants_authority());
}

#[test]
fn compiler_comparison_work_is_prepaid_after_original_account_check() {
    let tree = Tree::new();
    for (extra, accepted, denied) in [(7, 0, Some(8)), (519, 8, Some(520)), (520, 520, None)] {
        let prefix = LOAD_WORK + REVALIDATE_WORK;
        let mut work = Work::new(prefix + extra);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = tree.retained(&mut budget);
        tree.revalidate(&owner, fixture_immutable, &mut budget)
            .unwrap();
        let live = budget.storage();
        let result = owner.match_compiler(tree.policy.compiler_closure(), &mut budget);
        assert_eq!(budget.work(), prefix + accepted);
        assert_eq!(budget.failed_work(), denied.map(|work| prefix + work));
        assert_eq!(budget.storage(), live);
        if denied.is_some() {
            assert!(matches!(resource(failure(result)), Resource::Work(_)));
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn opener_unwind_releases_scratch_but_keeps_accepted_work() {
    let mut work = Work::new(IO_WORK);
    let mut budget = Budget::new(&mut work, FLOOR + FRAME);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = catch_unwind(AssertUnwindSafe(|| {
        Owner::load_using(
            || panic!("fixture open panic"),
            0,
            0,
            fixture_immutable,
            &mut budget,
        )
    }));
    assert!(result.is_err());
    assert_eq!(budget.work(), IO_WORK);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR + FRAME);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn every_directory_requires_launcher_strict_modes_and_nofollow_traversal() {
    let tree = Tree::new();
    for relative in [
        "",
        "etc",
        "etc/fe2o3",
        "etc/fe2o3/build-authority",
        "etc/fe2o3/compiler-execution",
    ] {
        let path = tree.path(relative);
        for mode in [0o775, 0o757, 0o600, 0o300, 0o1700, 0o2700, 0o4700] {
            chmod(&path, mode);
            let (result, usage) = observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b));
            chmod(&path, 0o700);
            assert!(result.is_err(), "directory {relative:?}, mode {mode:o}");
            assert_eq!(usage.live, FLOOR);
        }
        let moved = path.with_extension("original");
        fs::rename(&path, &moved).unwrap();
        symlink(&moved, &path).unwrap();
        let refused = observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b))
            .0
            .is_err();
        fs::remove_file(&path).unwrap();
        fs::rename(&moved, &path).unwrap();
        assert!(refused, "symlink directory {relative:?}");
    }
    assert!(
        observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b))
            .0
            .is_ok()
    );
}

#[test]
fn both_fixed_files_reject_modes_hardlinks_symlinks_lengths_and_fifos() {
    let tree = Tree::new();
    for relative in [POLICY_REL, PROFILE_REL] {
        let path = tree.path(relative);
        let bytes = fs::read(&path).unwrap();
        for mode in [0o400, 0o644, 0o4444, 0o1444, 0o555] {
            chmod(&path, mode);
            let refused = observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b))
                .0
                .is_err();
            chmod(&path, 0o444);
            assert!(refused, "{relative}, mode {mode:o}");
        }
        let alias = path.with_extension("alias");
        fs::hard_link(&path, &alias).unwrap();
        assert!(
            observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b))
                .0
                .is_err()
        );
        fs::remove_file(&alias).unwrap();
        fs::rename(&path, &alias).unwrap();
        symlink(&alias, &path).unwrap();
        assert!(
            observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b))
                .0
                .is_err()
        );
        fs::remove_file(&path).unwrap();
        fs::rename(&alias, &path).unwrap();
        for length in [0, bytes.len() - 1, bytes.len() + 1] {
            tree.write(relative, &vec![0; length]);
            assert!(
                observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b))
                    .0
                    .is_err()
            );
        }
        fs::remove_file(&path).unwrap();
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &path,
            rustix::fs::FileType::Fifo,
            Mode::from_raw_mode(0o444),
            0,
        )
        .unwrap();
        assert!(
            observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b))
                .0
                .is_err()
        );
        fs::remove_file(&path).unwrap();
        tree.write(relative, &bytes);
    }
}

#[test]
fn wrong_owners_and_invalid_root_descriptors_are_refused() {
    let tree = Tree::new();
    let (uid, gid) = owners();
    for (u, g) in [(uid.wrapping_add(1), gid), (uid, gid.wrapping_add(1))] {
        assert!(
            observe(LIMIT, LIMIT, |b| Owner::load_using(
                || tree.open(),
                u,
                g,
                fixture_immutable,
                b
            ))
            .0
            .is_err()
        );
        for (relative, length) in [(POLICY_REL, POLICY_BYTES), (PROFILE_REL, PROFILE_BYTES)] {
            let file = File::open(tree.path(relative)).unwrap();
            assert!(tree::validate_file(&file, u, g, length).is_err());
        }
    }
    for flags in [
        rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY,
    ] {
        let result = observe(LIMIT, LIMIT, |b| {
            Owner::load_using(
                || {
                    rustix::fs::open(&tree.root, flags, Mode::empty())
                        .map(File::from)
                        .map_err(|e| io("fixture descriptor", e))
                },
                uid,
                gid,
                fixture_immutable,
                b,
            )
        })
        .0;
        assert!(result.is_err());
    }
}

#[test]
fn missing_or_legacy_profile_and_malformed_policy_never_create_an_owner() {
    let tree = Tree::new();
    let v2 = crate::native_capability::tests::profile(7);
    let v1 = crate::native_capability::tests::legacy_profile(7);
    tree.write(
        "etc/fe2o3/compiler-execution/client-profile-v2",
        v2.canonical_bytes(),
    );
    tree.write(
        "etc/fe2o3/compiler-execution/client-profile-v1",
        v1.canonical_bytes(),
    );
    fs::remove_file(tree.path(PROFILE_REL)).unwrap();
    assert!(matches!(
        failure(observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b)).0),
        Error::Capability(CapabilityError::Io {
            errno: libc::ENOENT,
            ..
        })
    ));
    for bytes in [
        v2.canonical_bytes().as_slice(),
        v1.canonical_bytes().as_slice(),
        &[0; PROFILE_BYTES][..],
    ] {
        tree.write(PROFILE_REL, bytes);
        assert!(matches!(
            failure(observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b)).0),
            Error::Capability(CapabilityError::ProfileV3(_))
        ));
    }
    tree.write(PROFILE_REL, tree.profile.canonical_bytes());
    tree.write(POLICY_REL, &[0; POLICY_BYTES]);
    assert!(matches!(
        failure(observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b)).0),
        Error::Codec(CompilerApprovalPolicyErrorV2::Framing(
            fe2o3_build_authority::CompilerApprovalPolicyErrorV1::Header,
        ))
    ));
    fs::remove_file(tree.path(POLICY_REL)).unwrap();
    assert!(matches!(
        failure(observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b)).0),
        Error::Capability(CapabilityError::Io {
            errno: libc::ENOENT,
            ..
        })
    ));
}

#[test]
fn correctly_encoded_but_unbound_v3_profile_is_a_typed_mismatch() {
    let tree = Tree::new();
    let other = fixture_profile(9);
    assert_ne!(other.identity(), tree.profile.identity());
    tree.write(PROFILE_REL, other.canonical_bytes());
    let (result, usage) = observe(LOAD_WORK, LIMIT, |b| tree.load(fixture_immutable, b));
    assert!(matches!(
        failure(result),
        Error::Mismatch("root policy names a different V3 client profile")
    ));
    assert_eq!(usage.work, LOAD_WORK);
    assert_eq!(usage.live, FLOOR);
}

#[test]
fn each_helper_credential_must_be_separate_from_both_bound_profile_services() {
    let tree = Tree::new();
    let anchor = tree.profile.external_anchor_service();
    for (uid, gid, message) in [
        (
            tree.profile.supervisor_uid(),
            1002,
            "proof helper UID aliases a V3 profile service",
        ),
        (
            anchor.uid(),
            1002,
            "proof helper UID aliases a V3 profile service",
        ),
        (
            1002,
            tree.profile.supervisor_gid(),
            "proof helper GID aliases a V3 profile service",
        ),
        (
            1002,
            anchor.gid(),
            "proof helper GID aliases a V3 profile service",
        ),
    ] {
        let policy = CompilerApprovalPolicyV2::new(
            tree.policy.compiler_closure(),
            *tree.profile.identity().as_bytes(),
            *tree.policy.runtime_manifest_identity(),
            1,
            uid,
            gid,
            |_| Ok::<_, Resource>(()),
        )
        .unwrap();
        assert!(!policy.grants_authority());
        tree.write(POLICY_REL, policy.canonical_bytes());
        let (result, usage) = observe(LOAD_WORK, LIMIT, |b| tree.load(fixture_immutable, b));
        assert!(matches!(failure(result), Error::Mismatch(actual) if actual == message));
        assert_eq!(usage.work, LOAD_WORK);
        assert_eq!(usage.live, FLOOR);
    }
    tree.write(POLICY_REL, tree.policy.canonical_bytes());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let owner = tree.retained(&mut budget);
    assert_eq!(owner.policy().proof_helper_uid(), 1002);
    assert_eq!(owner.policy().proof_helper_gid(), 1002);
    tree.revalidate(&owner, fixture_immutable, &mut budget)
        .unwrap();
}

#[test]
fn invalid_helper_credentials_in_root_policy_bytes_never_create_an_owner() {
    use fe2o3_build_authority::COMPILER_APPROVAL_POLICY_IDENTITY_DOMAIN_V2 as DOMAIN;
    use sha2::{Digest, Sha256};
    let tree = Tree::new();
    for (offset, expected) in [
        (20, CompilerApprovalPolicyErrorV2::InvalidProofHelperUid),
        (24, CompilerApprovalPolicyErrorV2::InvalidProofHelperGid),
    ] {
        for value in [0_u32, u32::MAX] {
            let mut bytes = *tree.policy.canonical_bytes();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            let mut hash = Sha256::new();
            hash.update(DOMAIN);
            hash.update(320_u64.to_le_bytes());
            hash.update(&bytes[..320]);
            bytes[320..].copy_from_slice(&hash.finalize());
            tree.write(POLICY_REL, &bytes);
            let (result, usage) = observe(LOAD_WORK, LIMIT, |b| tree.load(fixture_immutable, b));
            assert!(matches!(failure(result), Error::Codec(actual) if actual == expected));
            assert_eq!(usage.work, IO_WORK + POLICY_WORK);
            assert_eq!(usage.live, FLOOR);
        }
    }
}

#[test]
fn legacy_policy_path_and_v1_bytes_never_supply_production_approval() {
    use fe2o3_build_authority::{CompilerApprovalPolicyErrorV1, CompilerApprovalPolicyV1};
    let tree = Tree::new();
    let legacy = CompilerApprovalPolicyV1::new(
        tree.policy.compiler_closure(),
        *tree.profile.identity().as_bytes(),
        *tree.policy.runtime_manifest_identity(),
        1,
        |_| Ok::<_, Resource>(()),
    )
    .unwrap();
    let old_path = "etc/fe2o3/build-authority/policy-v1";
    for bytes in [legacy.canonical_bytes(), tree.policy.canonical_bytes()] {
        tree.write(old_path, bytes);
        if tree.path(POLICY_REL).exists() {
            fs::remove_file(tree.path(POLICY_REL)).unwrap();
        }
        assert!(matches!(
            failure(observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b)).0),
            Error::Capability(CapabilityError::Io {
                errno: libc::ENOENT,
                ..
            })
        ));
    }
    tree.write(POLICY_REL, legacy.canonical_bytes());
    assert!(matches!(
        failure(observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b)).0),
        Error::Codec(CompilerApprovalPolicyErrorV2::Framing(
            CompilerApprovalPolicyErrorV1::Header
        ))
    ));
}

#[test]
fn changing_only_helper_credentials_invalidates_the_retained_policy_origin() {
    for (uid, gid) in [(1003, 1002), (1002, 1003)] {
        let tree = Tree::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let owner = tree.retained(&mut budget);
        let changed = CompilerApprovalPolicyV2::new(
            tree.policy.compiler_closure(),
            *tree.profile.identity().as_bytes(),
            *tree.policy.runtime_manifest_identity(),
            1,
            uid,
            gid,
            |_| Ok::<_, Resource>(()),
        )
        .unwrap();
        assert_eq!(
            changed.client_profile_identity(),
            tree.policy.client_profile_identity()
        );
        assert_ne!(changed.identity(), tree.policy.identity());
        tree.write(POLICY_REL, changed.canonical_bytes());
        assert!(
            tree.revalidate(&owner, fixture_immutable, &mut budget)
                .is_err()
        );
    }
}

#[test]
fn each_immutable_probe_is_required_during_intake_and_revalidation() {
    let tree = Tree::new();
    for probe in [
        refuse_probe::<1> as ImmutableCheck,
        refuse_probe::<2>,
        refuse_probe::<3>,
    ] {
        PROBES.with(|calls| calls.set(0));
        let (result, usage) = observe(LIMIT, LIMIT, |b| tree.load(probe, b));
        assert!(matches!(
            failure(result),
            Error::Mismatch("fixture immutable probe refused")
        ));
        assert_eq!(usage.live, FLOOR);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = tree.retained(&mut budget);
        let live = budget.storage();
        PROBES.with(|calls| calls.set(0));
        assert!(matches!(
            failure(tree.revalidate(&owner, probe, &mut budget)),
            Error::Mismatch("fixture immutable probe refused")
        ));
        assert_eq!(budget.storage(), live);
    }
    let (result, usage) = observe(LIMIT, LIMIT, |b| tree.load(require_immutable, b));
    assert!(
        result.is_err(),
        "ordinary fixture must not satisfy the kernel immutable predicate"
    );
    assert_eq!(usage.live, FLOOR);
    fn failed_ioctl(_: &File) -> Result<()> {
        Err(io("fixture immutable ioctl", rustix::io::Errno::IO))
    }
    assert!(matches!(
        failure(observe(LIMIT, LIMIT, |b| tree.load(failed_ioctl, b)).0),
        Error::Capability(CapabilityError::Io {
            errno: libc::EIO,
            ..
        })
    ));
}

#[test]
fn metadata_changes_during_read_and_before_current_path_postcheck_are_refused() {
    for (probe, expected) in [
        (
            change_time::<1> as ImmutableCheck,
            "compiler approval file changed during read",
        ),
        (
            change_time::<2> as ImmutableCheck,
            "compiler approval path changed during admission",
        ),
    ] {
        let tree = Tree::new();
        PROBES.with(|calls| calls.set(0));
        let (result, usage) = observe(LIMIT, LIMIT, |b| tree.load(probe, b));
        assert!(matches!(failure(result), Error::Mismatch(reason) if reason == expected));
        assert_eq!(usage.live, FLOOR);
    }
}

#[test]
fn original_source_metadata_changes_and_identical_byte_inode_replacements_fail() {
    for relative in [POLICY_REL, PROFILE_REL] {
        for replace in [false, true] {
            let tree = Tree::new();
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let owner = tree.retained(&mut budget);
            let live = budget.storage();
            let path = tree.path(relative);
            if replace {
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, path.with_extension("retained")).unwrap();
                tree.write(relative, &bytes);
            } else {
                File::open(&path)
                    .unwrap()
                    .set_times(
                        fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1)),
                    )
                    .unwrap();
            }
            assert!(
                tree.revalidate(&owner, fixture_immutable, &mut budget)
                    .is_err()
            );
            assert_eq!(budget.storage(), live);
        }
    }
}

#[test]
fn preexisting_writer_cannot_change_source_bytes_behind_a_retained_owner() {
    for relative in [POLICY_REL, PROFILE_REL] {
        let tree = Tree::new();
        let path = tree.path(relative);
        let original = fs::read(&path).unwrap();
        chmod(&path, 0o644);
        let writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
        chmod(&path, 0o444);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = tree.retained(&mut budget);
        let live = budget.storage();
        let before = writer.metadata().unwrap();
        let offset = original.len() - 1;
        assert_eq!(
            rustix::io::pwrite(&writer, &[original[offset] ^ 1], offset as u64).unwrap(),
            1
        );
        let after = writer.metadata().unwrap();
        assert_eq!(
            (before.dev(), before.ino(), before.len(), before.mode()),
            (after.dev(), after.ino(), after.len(), after.mode())
        );
        assert!(
            tree.revalidate(&owner, fixture_immutable, &mut budget)
                .is_err()
        );
        assert_eq!(budget.storage(), live);
        assert_eq!(
            owner.policy().canonical_bytes(),
            tree.policy.canonical_bytes()
        );
        assert_eq!(
            owner.profile().profile().canonical_bytes(),
            tree.profile.canonical_bytes()
        );
    }
}

#[test]
fn immutable_probe_failure_precedes_decoding_malformed_policy_bytes() {
    let tree = Tree::new();
    tree.write(POLICY_REL, &[0; POLICY_BYTES]);
    PROBES.with(|calls| calls.set(0));
    let (result, usage) = observe(LIMIT, LIMIT, |b| tree.load(refuse_probe::<1>, b));
    assert!(matches!(
        failure(result),
        Error::Mismatch("fixture immutable probe refused")
    ));
    assert_eq!(usage.work, IO_WORK);
    assert_eq!(usage.live, FLOOR);
    assert_eq!(usage.peak, FLOOR + FRAME);
}

#[test]
fn current_path_rechecks_follow_retained_object_checks_and_reject_rotation() {
    for relative in [POLICY_REL, PROFILE_REL] {
        let tree = Tree::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = tree.retained(&mut budget);
        let (uid, gid) = owners();
        let live = budget.storage();
        let result = owner.revalidate_using(
            || {
                let bytes = fs::read(tree.path(relative)).unwrap();
                fs::rename(
                    tree.path(relative),
                    tree.path(relative).with_extension("retained"),
                )
                .unwrap();
                tree.write(relative, &bytes);
                tree.open()
            },
            uid,
            gid,
            fixture_immutable,
            &mut budget,
        );
        assert!(matches!(
            failure(result),
            Error::Mismatch("fixed policy or profile path changed")
        ));
        assert_eq!(budget.storage(), live);
    }
}

#[test]
fn current_directory_replacement_cannot_reuse_the_retained_child_inodes() {
    let tree = Tree::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = tree.retained(&mut budget);
    let directory = tree.path("etc/fe2o3/compiler-execution");
    fs::rename(&directory, directory.with_extension("retained")).unwrap();
    fs::create_dir(&directory).unwrap();
    chmod(&directory, 0o755);
    tree.write(PROFILE_REL, tree.profile.canonical_bytes());
    assert!(matches!(
        failure(tree.revalidate(&owner, fixture_immutable, &mut budget)),
        Error::Mismatch("fixed policy or profile path changed")
    ));
}

fn acl(directory: bool) -> Vec<u8> {
    let mode = if directory { 5_u16 } else { 4_u16 };
    let entries = [
        (1_u16, if directory { 7 } else { 4 }, u32::MAX),
        (2, 4, owners().0.wrapping_add(1)),
        (4, mode, u32::MAX),
        (16, mode, u32::MAX),
        (32, mode, u32::MAX),
    ];
    let mut bytes = 2_u32.to_le_bytes().to_vec();
    for (tag, permissions, id) in entries {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&permissions.to_le_bytes());
        bytes.extend_from_slice(&id.to_le_bytes());
    }
    bytes
}

#[test]
fn actual_acl_and_capability_attributes_are_rejected_when_supported() {
    let tree = Tree::new();
    for relative in [
        "",
        "etc",
        "etc/fe2o3",
        "etc/fe2o3/build-authority",
        "etc/fe2o3/compiler-execution",
        POLICY_REL,
        PROFILE_REL,
    ] {
        let path = tree.path(relative);
        let is_directory = path.is_dir();
        let attributes: &[&str] = if is_directory {
            &["system.posix_acl_access", "system.posix_acl_default"]
        } else {
            &["system.posix_acl_access", "security.capability"]
        };
        for attribute in attributes {
            let value = if *attribute == "security.capability" {
                let mut bytes = 0x0200_0001_u32.to_le_bytes().to_vec();
                for word in [1_u32, 0, 0, 0] {
                    bytes.extend_from_slice(&word.to_le_bytes());
                }
                bytes
            } else {
                acl(is_directory)
            };
            match rustix::fs::setxattr(&path, *attribute, &value, rustix::fs::XattrFlags::empty()) {
                Ok(()) => {}
                Err(rustix::io::Errno::OPNOTSUPP) => {
                    eprintln!(
                        "fixture filesystem does not support {attribute}; no attribute coverage claimed"
                    );
                    continue;
                }
                Err(rustix::io::Errno::PERM | rustix::io::Errno::ACCESS)
                    if *attribute == "security.capability" =>
                {
                    eprintln!(
                        "fixture user cannot set security.capability; no capability-xattr coverage claimed"
                    );
                    continue;
                }
                Err(error) => panic!("set fixture attribute {attribute}: {error}"),
            }
            let result = observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b)).0;
            rustix::fs::removexattr(&path, *attribute).unwrap();
            chmod(&path, if is_directory { 0o755 } else { 0o444 });
            assert!(matches!(
                failure(result),
                Error::Capability(CapabilityError::Rejected(
                    "trusted profile object has a forbidden attribute"
                ))
            ));
        }
    }
}
