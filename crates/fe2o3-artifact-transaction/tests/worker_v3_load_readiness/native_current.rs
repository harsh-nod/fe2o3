//! Filesystem-custody tests reuse opaque transaction fixtures, not compiler authority.
use super::*;
use fe2o3_artifact_transaction::{
    NativeCurrentPublicationErrorV1 as Error, NativeCurrentPublicationLimitsV1 as Limits,
    NativeCurrentPublicationV1 as Native, RetainedDurableDirectoryV1 as Directory,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

fn directory(state: &PublishedV3) -> Directory {
    fs::set_permissions(state.output(), fs::Permissions::from_mode(0o700)).unwrap();
    Directory::admit_service_owned(
        rustix::fs::open(
            state.output(),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn limits(state: &PublishedV3) -> Limits {
    Limits::new(
        state.envelope.len(),
        fs::metadata(publication_artifact(&state.output()))
            .unwrap()
            .len() as usize,
    )
    .unwrap()
}

#[test]
fn native_current_retains_real_lock_and_original_account() {
    let state = setup(101);
    state.publish_readiness();
    let directory = directory(&state);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 2_000_000_000);
    budget.reserve_storage(Native::DIRECTORY_STORAGE).unwrap();
    let floor = budget.storage();
    let (owner, storage) =
        Native::recover(&directory, state.attempt, limits(&state), &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    assert_eq!(owner.readiness().exact_envelope_bytes(), state.envelope);
    assert_eq!(
        owner.exact_artifact_bytes(),
        fs::read(publication_artifact(&state.output())).unwrap()
    );
    assert!(!owner.grants_load_authority() && !owner.grants_launch_authority());
    owner.revalidate(&mut budget).unwrap();
    let retained_floor = budget.storage();
    assert!(matches!(
        Native::recover(&directory, state.attempt, limits(&state), &mut budget),
        Err(Error::Busy)
    ));
    assert!(budget.storage() > retained_floor);
    let mut foreign_work = Work::new(usize::MAX);
    let mut foreign = Budget::new(&mut foreign_work, 2_000_000_000);
    foreign.reserve_storage(owner.retained_storage()).unwrap();
    let before = foreign.storage();
    assert!(owner.revalidate(&mut foreign).is_err());
    assert_eq!(foreign.storage(), before);
    assert!(
        owner.revalidate(&mut budget).is_err(),
        "failure is terminal, not rebound to original account"
    );
    drop(owner);
    let mut fresh_work = Work::new(usize::MAX);
    let mut fresh = Budget::new(&mut fresh_work, 2_000_000_000);
    fresh.reserve_storage(Native::DIRECTORY_STORAGE).unwrap();
    Native::recover(&directory, state.attempt, limits(&state), &mut fresh).unwrap();
}

#[test]
fn native_current_refuses_bounds_and_budget_before_admission() {
    let state = setup(102);
    state.publish_readiness();
    let directory = directory(&state);
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 2_000_000_000);
    budget.reserve_storage(Native::DIRECTORY_STORAGE).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        Native::recover(&directory, state.attempt, limits(&state), &mut budget),
        Err(Error::Resource(_))
    ));
    assert_eq!(budget.storage(), floor);
    for limit in [
        Limits::new(state.envelope.len() - 1, 1024).unwrap(),
        Limits::new(state.envelope.len(), 1).unwrap(),
    ] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 2_000_000_000);
        budget.reserve_storage(Native::DIRECTORY_STORAGE).unwrap();
        assert!(Native::recover(&directory, state.attempt, limit, &mut budget).is_err());
        assert!(budget.storage() > Native::DIRECTORY_STORAGE);
    }
    assert!(
        recover_worker_v3_load_readiness_for_attempt_v1(&state.output(), state.attempt).is_ok()
    );
}

#[test]
fn native_current_detects_retained_file_mutation_without_repair() {
    let state = setup(103);
    state.publish_readiness();
    let directory = directory(&state);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 2_000_000_000);
    budget.reserve_storage(Native::DIRECTORY_STORAGE).unwrap();
    let (owner, storage) =
        Native::recover(&directory, state.attempt, limits(&state), &mut budget).unwrap();
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    let artifact = publication_artifact(&state.output());
    let original = fs::read(&artifact).unwrap();
    let mut changed = original.clone();
    changed[0] ^= 1;
    fs::write(&artifact, &changed).unwrap();
    let floor = budget.storage();
    assert!(owner.revalidate(&mut budget).is_err());
    assert!(budget.storage() > floor);
    assert_eq!(fs::read(&artifact).unwrap(), changed);
    fs::write(&artifact, original).unwrap();
    assert!(owner.revalidate(&mut budget).is_err());
}

#[test]
fn native_current_refuses_recovery_residue_without_mutation() {
    for journal in [false, true] {
        let state = setup(if journal { 104 } else { 105 });
        state.publish_readiness();
        let directory = directory(&state);
        let path = if journal {
            let record = fs::read_dir(state.output())
                .unwrap()
                .map(|e| e.unwrap().path())
                .find(|p| {
                    p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(".fe2o3-link-publication-v1-")
                        && !p.file_name().unwrap().to_string_lossy().ends_with(".redo")
                })
                .unwrap();
            PathBuf::from(format!("{}.redo", record.to_str().unwrap()))
        } else {
            state.output().join(".fe2o3-attempts-v1.recovery")
        };
        let residue = b"not admitted for recovery";
        fs::write(&path, residue).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 2_000_000_000);
        budget.reserve_storage(Native::DIRECTORY_STORAGE).unwrap();
        assert!(Native::recover(&directory, state.attempt, limits(&state), &mut budget).is_err());
        assert_eq!(fs::read(&path).unwrap(), residue);
        fs::remove_file(path).unwrap();
        assert!(
            recover_worker_v3_load_readiness_for_attempt_v1(&state.output(), state.attempt).is_ok()
        );
    }
}

#[test]
fn native_current_revalidates_readiness_and_ignores_unrelated_temporaries() {
    let state = setup(106);
    state.publish_readiness();
    let directory = directory(&state);
    let envelope = readiness_entry(&state.output(), ".envelope");
    let temp = PathBuf::from(format!(
        "{}.tmp-inert",
        envelope.to_str().unwrap().trim_end_matches(".envelope")
    ));
    fs::write(&temp, b"unrelated temporary content").unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 2_000_000_000);
    budget.reserve_storage(Native::DIRECTORY_STORAGE).unwrap();
    let (owner, storage) =
        Native::recover(&directory, state.attempt, limits(&state), &mut budget).unwrap();
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    assert_eq!(fs::read(temp).unwrap(), b"unrelated temporary content");
    let mut mutated = state.envelope.clone();
    mutated[0] ^= 1;
    fs::write(envelope, mutated).unwrap();
    assert!(owner.revalidate(&mut budget).is_err());
}
