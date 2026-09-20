//! Child of inert transport tests: genuine service histories stay unchanged.
use super::*;
use CanonicalPolicy8GraphPoolErrorV1 as PoolError;
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV12 as AdmissionError, KernelIrDecodeError,
    encode_module_v12,
};

#[derive(Debug)]
enum Rejection {
    Frame(TransportError),
    Admission(PoolError),
}
struct Observation {
    result: Result<(usize, usize), Rejection>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

fn execute(wire: &[u8], k: &Owner, budget: &mut Budget<'_>) -> Result<(usize, usize), Rejection> {
    let frame_storage;
    let result = {
        let frame = read_inert_policy8_history_v1(wire, k, budget).map_err(Rejection::Frame)?;
        frame_storage = frame.storage().retained_storage();
        budget.reserve_storage(frame_storage).unwrap();
        let floor = budget.storage();
        let result = admit_policy8_history_graph_pool_v1(&frame, budget);
        assert_eq!(budget.storage(), floor);
        result.map_err(Rejection::Admission).map(|pool| {
            let retained = pool.storage().retained_storage();
            budget.reserve_storage(retained).unwrap();
            assert!(std::ptr::eq(pool.frame(), &frame));
            assert!(std::ptr::eq(pool.external_output(), k));
            assert!(!pool.authenticates_execution());
            assert!(!pool.proves_semantic_preservation());
            assert!(!pool.grants_authority());
            for role in ROLES {
                let actual = pool.graph(role);
                assert_eq!(
                    actual.canonical().canonical_bytes(),
                    frame.graph_bytes(role)
                );
                assert_eq!(
                    *actual.canonical().identity().digest(),
                    frame.role(role).digest()
                );
                assert_eq!(
                    actual.canonical().identity().canonical_length(),
                    frame.role(role).canonical_length()
                );
                if let Some(index) = frame.role(role).pool_index() {
                    assert!(std::ptr::eq(
                        actual,
                        pool.stored_graph(index as usize).unwrap()
                    ));
                    assert!(!std::ptr::eq(actual, k));
                    assert_ne!(
                        actual.canonical().canonical_bytes().as_ptr(),
                        frame.graph_bytes(role).as_ptr()
                    );
                } else {
                    assert!(std::ptr::eq(actual, k));
                }
            }
            let count = pool.stored_graph_count();
            assert_eq!(count, frame.stored_graph_count());
            assert!(pool.stored_graph(count).is_none());
            assert!(pool.stored_graph(usize::MAX).is_none());
            drop(pool);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
            (count, retained)
        })
    };
    budget.release_storage(frame_storage).unwrap();
    result
}

fn run(
    wire: &[u8],
    k: &Owner,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
    prior_denials: bool,
) -> Observation {
    let mut work = Work::new(work_limit);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        if prior_denials {
            assert!(budget.charge_work(usize::MAX).is_err());
            assert!(budget.reserve_storage(usize::MAX).is_err());
        }
        let ledger = budget.work_ledger_identity_v1();
        let result = execute(wire, k, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    Observation {
        result,
        work: accepted,
        peak,
        failed_storage,
        failed_work: work.failed_work(),
    }
}

#[test]
fn genuine_mutation_and_noop_histories_admit_all_actual_roles_and_aliases() {
    for stores in [false, true] {
        for swaps in [false, true] {
            let p = prepared(stores, swaps);
            let wire = make(&p);
            let floor = p.floor + wire.storage().retained_storage();
            let result = run(
                wire.canonical_bytes(),
                p.tail.output(),
                floor,
                WORK,
                STORAGE,
                false,
            )
            .result
            .unwrap();
            assert!(result.0 <= 6);
            assert_eq!(p.tail.proved_pairs(), usize::from(swaps));
            assert_eq!(
                p.inputs().prefix.output.canonical().canonical_bytes()
                    != p.tail.output().canonical().canonical_bytes(),
                swaps
            );
            assert_eq!(
                p.prefix.continuation.rows().len(),
                if stores { 2 } else { 0 }
            );
        }
    }
}

#[test]
fn empty_complete_noop_borrows_k_without_any_graph_admission() {
    let p = prepared_module(&Module::new("empty-owned-pool"));
    let wire = make(&p);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = p.floor + wire.storage().retained_storage();
    budget.reserve_storage(floor).unwrap();
    let frame_storage;
    {
        let frame =
            read_inert_policy8_history_v1(wire.canonical_bytes(), p.tail.output(), &mut budget)
                .unwrap();
        frame_storage = frame.storage().retained_storage();
        budget.reserve_storage(frame_storage).unwrap();
        assert_eq!(frame.stored_graph_count(), 0);
        let start = budget.work();
        let pool = admit_policy8_history_graph_pool_v1(&frame, &mut budget).unwrap();
        assert_eq!(budget.work() - start, 1 + 7 * 82);
        assert_eq!(
            pool.storage().retained_storage(),
            size_of::<AdmittedPolicy8HistoryGraphPoolV1<'_, '_, '_>>()
        );
        for role in ROLES {
            assert!(std::ptr::eq(pool.graph(role), p.tail.output()));
        }
        assert_eq!(pool.stored_graph_count(), 0);
        drop(pool);
    }
    budget.release_storage(frame_storage).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn deterministic_cumulative_reader_and_admission_exact_and_one_short_limits() {
    let p = prepared(true, true);
    let wire = make(&p);
    let floor = p.floor + wire.storage().retained_storage();
    let baseline = run(
        wire.canonical_bytes(),
        p.tail.output(),
        floor,
        WORK,
        STORAGE,
        false,
    );
    let wanted = baseline.result.unwrap();
    assert!(wanted.0 >= 2);
    for _ in 0..2 {
        let exact = run(
            wire.canonical_bytes(),
            p.tail.output(),
            floor,
            baseline.work,
            baseline.peak,
            false,
        );
        assert_eq!(exact.result.unwrap(), wanted);
        assert_eq!(
            (
                exact.work,
                exact.peak,
                exact.failed_work,
                exact.failed_storage
            ),
            (baseline.work, baseline.peak, None, None)
        );
    }
    let short = run(
        wire.canonical_bytes(),
        p.tail.output(),
        floor,
        baseline.work - 1,
        baseline.peak,
        false,
    );
    assert!(matches!(
        short.result,
        Err(Rejection::Admission(PoolError::Resource(Resource::Work(_))))
    ));
    assert!(short.work < baseline.work);
    assert!(
        short
            .failed_work
            .is_some_and(|attempt| attempt > baseline.work - 1)
    );
    assert_eq!(short.failed_storage, None);
    let short = run(
        wire.canonical_bytes(),
        p.tail.output(),
        floor,
        baseline.work,
        baseline.peak - 1,
        false,
    );
    assert!(matches!(
        short.result,
        Err(Rejection::Admission(PoolError::Resource(
            Resource::Storage(_)
        ))) | Err(Rejection::Admission(PoolError::Admission {
            error: AdmissionError::Resource(Resource::Storage(_)),
            ..
        })) | Err(Rejection::Admission(PoolError::Admission {
            error: AdmissionError::Decode(KernelIrDecodeError::Resource(Resource::Storage(_))),
            ..
        }))
    ));
    assert!(short.peak < baseline.peak);
    assert!(
        short
            .failed_storage
            .is_some_and(|attempt| attempt > baseline.peak - 1)
    );
    assert_eq!(short.failed_work, None);
}

#[test]
fn actual_graph_admission_once_per_pool_matches_manual_cumulative_work() {
    let p = prepared(true, true);
    let wire = make(&p);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(p.floor + wire.storage().retained_storage())
        .unwrap();
    let frame_storage;
    {
        let frame =
            read_inert_policy8_history_v1(wire.canonical_bytes(), p.tail.output(), &mut budget)
                .unwrap();
        frame_storage = frame.storage().retained_storage();
        budget.reserve_storage(frame_storage).unwrap();
        let count = frame.stored_graph_count();
        assert!(count >= 2);
        let floor = budget.storage();
        let before = budget.work();
        let pool = admit_policy8_history_graph_pool_v1(&frame, &mut budget).unwrap();
        let actual_work = budget.work() - before;
        let retained = pool.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        let mut manual_work = 0;
        let mut manual_retained = 0;
        for index in 0..count {
            let before = budget.work();
            let (owner, storage) = Owner::from_canonical_bytes_with_verification_budget_v12(
                frame.unverified_pool_graph_bytes(index).unwrap(),
                &mut budget,
            )
            .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            manual_work += budget.work() - before;
            manual_retained += storage.retained_storage();
            assert_eq!(&owner, pool.stored_graph(index).unwrap());
            assert!(!std::ptr::eq(&owner, pool.stored_graph(index).unwrap()));
            drop(owner);
            budget.release_storage(storage.retained_storage()).unwrap();
        }
        assert_eq!(actual_work, 1 + 2 * count + 7 * 82 + manual_work);
        assert!(retained >= size_of_val(&pool) + count * size_of::<Owner>() + manual_retained);
        drop(pool);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), floor);
    }
    budget.release_storage(frame_storage).unwrap();
}

struct SixRoles {
    wire: Vec<u8>,
    owners: Vec<(Owner, usize)>,
    floor: usize,
}
fn six_roles(p: &Prepared8) -> SixRoles {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(p.floor).unwrap();
    let mut owners = Vec::new();
    for name in ["role-b", "role-c", "role-s", "role-o", "role-i", "role-j"] {
        let (owner, storage) =
            Owner::from_module_ref_with_verification_budget_v12(&Module::new(name), &mut budget)
                .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        owners.push((owner, storage.retained_storage()));
    }
    let (rows, row_storage) =
        encode_rows(p.inputs().continuation.occurrences, &mut budget).unwrap();
    budget
        .reserve_storage(row_storage.retained_storage())
        .unwrap();
    let mut claims = inputs(p, &rows);
    claims.roles = [
        &owners[0].0,
        &owners[1].0,
        &owners[2].0,
        &owners[3].0,
        &owners[4].0,
        &owners[5].0,
        p.tail.output(),
    ];
    // Deliberately not a semantic history: this graph-only layer must not claim it is.
    let wire = literal(claims);
    drop(rows);
    budget
        .release_storage(row_storage.retained_storage())
        .unwrap();
    let floor = p.floor
        + wire.capacity()
        + size_of::<SixRoles>()
        + owners.capacity() * size_of::<(Owner, usize)>()
        + owners.iter().map(|row| row.1).sum::<usize>();
    SixRoles {
        wire,
        owners,
        floor,
    }
}

fn pool_ranges(wire: &[u8]) -> Vec<std::ops::Range<usize>> {
    let mut cursor = section(wire, 0).start;
    let mut ranges = Vec::new();
    for _ in 0..word(wire, 20) {
        let length = word(wire, cursor);
        cursor += 4;
        ranges.push(cursor..cursor + length);
        cursor += length;
    }
    assert_eq!(cursor, section(wire, 0).end);
    ranges
}
fn replace_graph(wire: &[u8], index: usize, replacement: &[u8]) -> Vec<u8> {
    let mut pool = Vec::new();
    for (ordinal, range) in pool_ranges(wire).into_iter().enumerate() {
        let bytes = if ordinal == index {
            replacement
        } else {
            &wire[range]
        };
        pool.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_le_bytes());
        pool.extend_from_slice(bytes);
    }
    let mut changed = replace_section(wire, 0, &pool);
    for role in 0..7 {
        if word(wire, 56 + 44 * role + 40) == index {
            changed[56 + 44 * role + 32..56 + 44 * role + 40]
                .copy_from_slice(&(replacement.len() as u64).to_le_bytes());
        }
    }
    changed
}

#[test]
fn all_six_unique_graphs_are_retained_and_nonsemantic_prefix_stays_inert() {
    let p = prepared(true, true);
    let f = six_roles(&p);
    assert_eq!(f.owners.len(), 6);
    let observation = run(&f.wire, p.tail.output(), f.floor, WORK, STORAGE, false);
    assert_eq!(observation.result.unwrap().0, 6);
    let mut changed = f.wire.clone();
    // Same size, intentionally invalid nested records. Graph admission must not
    // silently pretend it replayed these opaque claims.
    for axis in [1, 2, 4, 5, 6] {
        let start = section(&changed, axis).start;
        changed[start] ^= 0x55;
    }
    assert_eq!(
        run(
            &changed,
            p.tail.output(),
            f.floor + changed.capacity(),
            WORK,
            STORAGE,
            false
        )
        .result
        .unwrap()
        .0,
        6
    );
}

#[test]
fn every_stored_role_digest_is_checked_against_its_actual_fresh_owner() {
    let p = prepared(true, true);
    let f = six_roles(&p);
    for (ordinal, role) in ROLES.into_iter().take(6).enumerate() {
        let mut changed = f.wire.clone();
        changed[56 + ordinal * 44] ^= 1;
        assert!(
            matches!(run(&changed, p.tail.output(), f.floor + changed.capacity(), WORK, STORAGE, false).result,
            Err(Rejection::Admission(PoolError::RoleIdentity(actual))) if actual == role)
        );
    }
    let mut changed = f.wire.clone();
    changed[56 + 6 * 44] ^= 1;
    assert!(matches!(
        run(
            &changed,
            p.tail.output(),
            f.floor + changed.capacity(),
            WORK,
            STORAGE,
            false
        )
        .result,
        Err(Rejection::Frame(TransportError::Role))
    ));
}

#[test]
fn aliased_roles_share_one_admission_and_consistent_forged_digest_is_refused() {
    let p = prepared(true, true);
    let f = six_roles(&p);
    let mut backing = Vec::new();
    for (index, range) in pool_ranges(&f.wire).into_iter().enumerate() {
        if index == 1 {
            continue;
        }
        let bytes = &f.wire[range];
        backing.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        backing.extend_from_slice(bytes);
    }
    let mut changed = replace_section(&f.wire, 0, &backing);
    set_word(&mut changed, 20, 5);
    changed[100..144].copy_from_slice(&f.wire[56..100]);
    for role in 2..6 {
        set_word(&mut changed, 56 + role * 44 + 40, role - 1);
    }
    let floor = f.floor + changed.capacity() + backing.capacity();
    assert_eq!(
        run(&changed, p.tail.output(), floor, WORK, STORAGE, false)
            .result
            .unwrap()
            .0,
        5
    );
    for role in [Role::B, Role::C] {
        changed[56 + role as usize * 44] ^= 1;
    }
    assert!(matches!(
        run(&changed, p.tail.output(), floor, WORK, STORAGE, false).result,
        Err(Rejection::Admission(PoolError::RoleIdentity(Role::B)))
    ));
}

#[test]
fn wrong_claimed_lengths_fail_framing_and_repaired_trailing_graph_fails_admission() {
    let p = prepared(true, true);
    let f = six_roles(&p);
    for ordinal in 0..7 {
        let mut changed = f.wire.clone();
        changed[56 + ordinal * 44 + 32] ^= 1;
        assert!(matches!(
            run(
                &changed,
                p.tail.output(),
                f.floor + changed.capacity(),
                WORK,
                STORAGE,
                false
            )
            .result,
            Err(Rejection::Frame(TransportError::Role))
        ));
    }
    let mut bad_graph = f.wire[pool_ranges(&f.wire)[0].clone()].to_vec();
    bad_graph.push(0);
    let changed = replace_graph(&f.wire, 0, &bad_graph);
    assert!(matches!(
        run(
            &changed,
            p.tail.output(),
            f.floor + changed.capacity(),
            WORK,
            STORAGE,
            false
        )
        .result,
        Err(Rejection::Admission(PoolError::Admission {
            index: 0,
            error: AdmissionError::Decode(_)
        }))
    ));
}

#[test]
fn invalid_canonical_schema_and_semantics_reject_actual_pool_bytes() {
    let p = prepared(true, true);
    let f = six_roles(&p);
    let original = &f.wire[pool_ranges(&f.wire)[0].clone()];
    let mut wrong_version = original.to_vec();
    wrong_version[8..10].copy_from_slice(&11u16.to_le_bytes());
    let invalid_semantics = encode_module_v12(&Module::new("")).unwrap();
    for (bytes, semantic) in [
        (wrong_version.as_slice(), false),
        (invalid_semantics.as_slice(), true),
    ] {
        let changed = replace_graph(&f.wire, 0, bytes);
        let error = run(
            &changed,
            p.tail.output(),
            f.floor + changed.capacity(),
            WORK,
            STORAGE,
            false,
        )
        .result
        .err()
        .unwrap();
        match (semantic, error) {
            (
                false,
                Rejection::Admission(PoolError::Admission {
                    index: 0,
                    error: AdmissionError::Decode(_),
                }),
            ) => {}
            (
                true,
                Rejection::Admission(PoolError::Admission {
                    index: 0,
                    error: AdmissionError::Verification(_),
                }),
            ) => {}
            other => panic!("exact actual graph refusal required: {other:?}"),
        }
    }
}

#[test]
fn every_partial_pool_failure_drops_prior_owners_and_restores_floor() {
    let p = prepared(true, true);
    let f = six_roles(&p);
    for index in 0..6 {
        let mut changed = f.wire.clone();
        let start = pool_ranges(&changed)[index].start;
        changed[start] ^= 0xff;
        assert!(
            matches!(run(&changed, p.tail.output(), f.floor + changed.capacity(), WORK, STORAGE, false).result,
            Err(Rejection::Admission(PoolError::Admission { index: actual, error: AdmissionError::Decode(_) })) if actual == index)
        );
    }
}

#[test]
fn wrong_external_k_fails_before_pool_and_equal_byte_k_preserves_actual_borrow() {
    let p = prepared(true, true);
    let wire = make(&p);
    let floor = p.floor + wire.storage().retained_storage();
    assert!(matches!(
        run(
            wire.canonical_bytes(),
            p.inputs().prefix.output,
            floor,
            WORK,
            STORAGE,
            false
        )
        .result,
        Err(Rejection::Frame(TransportError::Role))
    ));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let (equal, storage) = Owner::from_canonical_bytes_with_verification_budget_v12(
        p.tail.output().canonical().canonical_bytes(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert!(!std::ptr::eq(&equal, p.tail.output()));
    assert!(execute(wire.canonical_bytes(), &equal, &mut budget).is_ok());
    drop(equal);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn first_denials_survive_success_and_late_graph_failure() {
    let p = prepared(true, true);
    let f = six_roles(&p);
    let mut changed = f.wire.clone();
    let start = pool_ranges(&changed)[5].start;
    changed[start] ^= 0xff;
    for (wire, succeeds) in [(f.wire.as_slice(), true), (changed.as_slice(), false)] {
        let result = run(
            wire,
            p.tail.output(),
            f.floor + changed.capacity(),
            WORK,
            STORAGE,
            true,
        );
        assert_eq!(result.result.is_ok(), succeeds);
        assert_eq!(
            (result.failed_work, result.failed_storage),
            (Some(usize::MAX), Some(usize::MAX))
        );
        assert!(result.work > 17);
    }
}
