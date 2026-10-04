use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fixture::{FLOOR, LIMIT};

#[test]
fn native_v18_v53_text_replay_headers_match_independent_scope_shapes() {
    #[allow(dead_code)]
    struct Capture<'a> {
        owner: &'a Owner,
        profile: Profile,
        lowering: &'a [u8],
        descriptor: &'a [u8],
        final_llvm: &'a str,
    }
    let expected = size_of::<Capture<'_>>()
        + size_of::<AssertUnwindSafe<&mut Budget<'_>>>()
        + size_of::<Budget<'_>>()
        + size_of::<[usize; 8]>()
        + size_of::<[Result<(), Error>; 2]>()
        + size_of::<std::thread::Result<Result<(), Error>>>()
        + size_of::<Result<(String, usize), TextError>>();
    assert_eq!(headers(), expected);
}

fn run(
    owner: &Owner,
    retained: usize,
    profile: Profile,
    lowering: &[u8],
    wire: &[u8],
    text: &str,
    work: usize,
    storage: usize,
) -> (Result<(), Error>, usize, usize, usize) {
    let backing = FLOOR + retained + lowering.len() + wire.len() + text.len();
    let mut meter = Work::new(work);
    let mut budget = Budget::new(&mut meter, storage);
    budget.reserve_storage(backing).unwrap();
    let result = check_current(owner, profile, lowering, wire, text, &mut budget);
    assert_eq!(budget.storage(), backing);
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

#[test]
fn native_v18_v53_text_replay_accepts_both_profiles_and_complete_multiple_roots() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for count in [1, 2] {
            let (owner, retained) = fixture::owner(count, "actual-native");
            let prefix = match profile {
                Profile::Gfx942 => lower_942(&owner),
                Profile::Gfx950 => lower_950(&owner),
            }
            .unwrap();
            let wire = fixture::descriptor(&owner, profile);
            let text = fixture::append_descriptor(&prefix, &wire);
            run(
                &owner,
                retained,
                profile,
                prefix.as_bytes(),
                &wire,
                &text,
                LIMIT,
                LIMIT,
            )
            .0
            .unwrap();
        }
    }
}

#[test]
fn native_v18_v53_text_replay_refuses_actual_prefix_suffix_graph_and_profile_changes() {
    let (owner, retained) = fixture::owner(2, "actual-native");
    let prefix = lower_942(&owner).unwrap();
    let wire = fixture::descriptor(&owner, Profile::Gfx942);
    let text = fixture::append_descriptor(&prefix, &wire);
    let altered_prefixes = [
        prefix.replacen("ret void", "unreachable", 1),
        prefix.replacen("target datalayout", "target badlayout", 1),
        prefix.replacen("kernel1", "foreign", 1),
        prefix.replacen("kir-version:18", "kir-version:12", 1),
    ];
    for changed in altered_prefixes {
        assert_ne!(changed, prefix);
        let changed_text = fixture::append_descriptor(&changed, &wire);
        assert!(
            run(
                &owner,
                retained,
                Profile::Gfx942,
                changed.as_bytes(),
                &wire,
                &changed_text,
                LIMIT,
                LIMIT
            )
            .0
            .is_err()
        );
        assert!(
            run(
                &owner,
                retained,
                Profile::Gfx942,
                prefix.as_bytes(),
                &wire,
                &changed_text,
                LIMIT,
                LIMIT
            )
            .0
            .is_err()
        );
    }
    let mut changed_wire = wire.clone();
    changed_wire[0] ^= 1;
    for changed in [
        prefix.clone(),
        format!("{text}\n"),
        format!("{text}{text}"),
        fixture::append_descriptor(&prefix, &changed_wire),
        text.replacen(SECTION_NAME, ".fe2o3.kd.v3", 1),
    ] {
        assert!(
            run(
                &owner,
                retained,
                Profile::Gfx942,
                prefix.as_bytes(),
                &wire,
                &changed,
                LIMIT,
                LIMIT
            )
            .0
            .is_err()
        );
    }
    let (foreign, foreign_retained) = fixture::owner(1, "foreign-native");
    assert!(
        run(
            &foreign,
            foreign_retained,
            Profile::Gfx942,
            prefix.as_bytes(),
            &wire,
            &text,
            LIMIT,
            LIMIT
        )
        .0
        .is_err()
    );
    assert!(
        run(
            &owner,
            retained,
            Profile::Gfx950,
            prefix.as_bytes(),
            &wire,
            &text,
            LIMIT,
            LIMIT
        )
        .0
        .is_err()
    );
}

#[test]
fn native_v18_v53_text_replay_has_exact_resource_limits_and_retires_output_on_refusal() {
    let (owner, retained) = fixture::owner(2, "resources");
    let prefix = lower_942(&owner).unwrap();
    let wire = fixture::descriptor(&owner, Profile::Gfx942);
    let text = fixture::append_descriptor(&prefix, &wire);
    let measured = run(
        &owner,
        retained,
        Profile::Gfx942,
        prefix.as_bytes(),
        &wire,
        &text,
        LIMIT,
        LIMIT,
    );
    measured.0.unwrap();
    let exact = run(
        &owner,
        retained,
        Profile::Gfx942,
        prefix.as_bytes(),
        &wire,
        &text,
        measured.1,
        measured.3,
    );
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for work_short in [true, false] {
        let denied = run(
            &owner,
            retained,
            Profile::Gfx942,
            prefix.as_bytes(),
            &wire,
            &text,
            measured.1 - usize::from(work_short),
            measured.3 - usize::from(!work_short),
        );
        let error = denied.0.unwrap_err();
        match fixture::resource(&error).unwrap() {
            Resource::Work(limit) if work_short => {
                assert_eq!(limit.actual(), measured.1);
                assert_eq!(limit.limit(), measured.1 - 1);
            }
            Resource::Storage(limit) if !work_short => {
                assert_eq!(limit.actual(), measured.3);
                assert_eq!(limit.limit(), measured.3 - 1);
            }
            other => panic!("wrong native resource: {other:?}"),
        }
    }
    let mut meter = Work::new(1);
    let mut budget = Budget::new(&mut meter, LIMIT);
    budget.charge_work(2).unwrap_err();
    let before = (budget.work(), budget.storage(), budget.peak_storage());
    let first = check_current(
        &owner,
        Profile::Gfx942,
        prefix.as_bytes(),
        &wire,
        &text,
        &mut budget,
    )
    .unwrap_err();
    let second = check_current(
        &owner,
        Profile::Gfx942,
        prefix.as_bytes(),
        &wire,
        &text,
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(fixture::resource(&first), fixture::resource(&second));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        before
    );
}
