//! Component oracles use genuine fixtures; actual native custody is tested by the managed source parents.
use super::*;
use fe2o3_kernel_opt::{
    InertExpandedHistoryBytesV1 as Bytes, materialize_expanded_history_v1, read_expanded_history_v1,
};
use fe2o3_lower_mir_kernel::with_checked_decoded_expanded_source_v1;
use std::mem::size_of;
use transport::serialized_history::{
    ExpandedHistoryTransportErrorV1 as HistoryError,
    ExpandedHistoryTransportStorageV1 as HistoryStorage,
    PreparedExpandedHistoryTransportV1 as Prepared, encode_live_history,
};

fn u64_at(bytes: &[u8], at: usize) -> usize {
    usize::try_from(u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())).unwrap()
}
fn directory_terminal(bytes: &[u8]) -> std::ops::Range<usize> {
    assert_eq!(&bytes[..8], b"F2EPH1\0\0");
    assert_eq!(&bytes[8..16], &[1, 0, 1, 0, 48, 0, 0, 0]);
    assert_eq!(u64_at(bytes, 16), bytes.len());
    assert_eq!(&bytes[24..32], &[1, 0, 0, 0, 0, 0, 0, 0]);
    let scalar = 48 + u64_at(bytes, 32);
    assert_eq!(scalar + u64_at(bytes, 40), bytes.len());
    assert_eq!(&bytes[scalar..scalar + 8], b"F2SPH1\0\0");
    let rounds = u16::from_le_bytes(bytes[scalar + 24..scalar + 26].try_into().unwrap()) as usize;
    assert!((1..=16).contains(&rounds));
    let mut at = scalar + 160 + rounds * 48;
    let mut result = None;
    for ordinal in 0..rounds {
        let directory = scalar + 160 + ordinal * 48;
        assert_eq!(
            u16::from_le_bytes(bytes[directory..directory + 2].try_into().unwrap()) as usize,
            ordinal
        );
        assert_eq!(&bytes[directory + 2..directory + 8], &[1, 0, 0, 0, 0, 0]);
        for field in 0..5 {
            let length = u64_at(bytes, directory + 8 + field * 8);
            assert!(length > 0);
            if ordinal + 1 == rounds && field == 1 {
                result = Some(at..at + length);
            }
            at = at.checked_add(length).unwrap();
        }
    }
    assert_eq!(at, bytes.len());
    result.unwrap()
}
#[test]
fn expanded_history_live_encoder_and_fully_decoded_nominal_subject_agree() {
    let mut changing = [0usize; 2];
    for (branch, erased) in [false, true].into_iter().enumerate() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [Some(3), None] {
                with_expanded(
                    erased,
                    profile,
                    bound,
                    |owner, execution, claims, caller| {
                        let caller_slot = std::ptr::from_ref(caller);
                        let caller_ledger = caller.work_ledger_identity_v1();
                        let caller_state = (
                            caller.work(),
                            caller.storage(),
                            caller.peak_storage(),
                            caller.failed_storage(),
                            caller.storage_limit(),
                        );
                        let mut replay_work = Work::new(
                            usize::try_from(
                                crate::production_canonical_phase_policy_v1::WORK_LIMIT,
                            )
                            .unwrap(),
                        );
                        let mut replay = Budget::new(
                            &mut replay_work,
                            fe2o3_kernel_opt::MAX_REFINED_FORWARDING_HISTORY_STORAGE_V1,
                        );
                        // Replay borrows genuine custody and pays its complete existing prefix.
                        replay.charge_work(caller_state.0).unwrap();
                        replay.reserve_storage(caller_state.1).unwrap();
                        let replay_slot = std::ptr::from_ref(&replay);
                        let replay_ledger = replay.work_ledger_identity_v1();
                        let budget = &mut replay;
                        let floor = budget.storage();
                        let bytes = encode_live_history(owner, execution, claims, budget).unwrap();
                        assert_eq!(budget.storage(), floor);
                        budget
                            .reserve_storage(bytes.storage().retained_storage())
                            .unwrap();
                        let range = directory_terminal(bytes.canonical_bytes());
                        assert_eq!(
                            &bytes.canonical_bytes()[range.clone()],
                            owner.output().canonical().canonical_bytes()
                        );
                        let frame =
                            read_expanded_history_v1(bytes.canonical_bytes(), budget).unwrap();
                        budget
                            .reserve_storage(frame.storage().retained_storage())
                            .unwrap();
                        assert_eq!(range, frame.final_graph_range());
                        let decoded = materialize_expanded_history_v1(&frame, budget).unwrap();
                        budget
                            .reserve_storage(decoded.storage().retained_storage())
                            .unwrap();
                        let historical = match owner.prefix() {
                            ProductionExpandedPrefixV1::Direct(v) => UnrolledOwnerV1::Direct(v),
                            ProductionExpandedPrefixV1::Erased(v) => UnrolledOwnerV1::Erased(v),
                        };
                        let roots = fixtures::typed_roots(historical.final_f());
                        let actual =
                            descriptor::decoded::live_fixture(owner, &roots, profile, budget)
                                .unwrap();
                        let actual_storage = size_of::<Vec<u8>>() + actual.capacity();
                        budget.reserve_storage(actual_storage).unwrap();
                        with_checked_decoded_expanded_source_v1(
                            owner.source_anchor(),
                            &decoded,
                            budget,
                            |view, budget| {
                                let wire =
                                    descriptor::decoded::fixture(&view, &roots, profile, budget)?;
                                let paid = size_of::<Vec<u8>>() + wire.capacity();
                                budget.reserve_storage(paid)?;
                                assert_eq!(wire, actual);
                                assert_eq!(view.origins(budget)?, owner.origins());
                                assert_eq!(view.kernels(budget)?, owner.kernels());
                                drop(wire);
                                budget.release_storage(paid)?;
                                for case in 0..8 {
                                    let mut hostile = fixtures::typed_roots(historical.final_f());
                                    let _ = fixtures::hostile(&mut hostile, case);
                                    assert!(matches!(
                                        descriptor::decoded::fixture(
                                            &view, &hostile, profile, budget
                                        ),
                                        Err(descriptor::E::Nominal(_))
                                    ));
                                }
                                let other = if profile == Profile::Gfx942 {
                                    Profile::Gfx950
                                } else {
                                    Profile::Gfx942
                                };
                                assert!(matches!(
                                    descriptor::decoded::fixture(&view, &roots, other, budget),
                                    Err(descriptor::E::Nominal(_))
                                ));
                                Ok::<_, descriptor::E>(())
                            },
                        )
                        .unwrap();
                        changing[branch] += usize::from(
                            unrolled(owner).canonical().canonical_bytes()
                                != owner.output().canonical().canonical_bytes(),
                        );
                        let second = encode_live_history(owner, execution, claims, budget).unwrap();
                        budget
                            .reserve_storage(second.storage().retained_storage())
                            .unwrap();
                        assert_eq!(second.canonical_bytes(), bytes.canonical_bytes());
                        drop(second);
                        drop(actual);
                        drop(decoded);
                        drop(frame);
                        drop(bytes);
                        budget.release_storage(budget.storage() - floor).unwrap();
                        assert_eq!(budget.storage(), caller_state.1);
                        assert!(budget.work() > caller_state.0);
                        assert_eq!(budget.failed_storage(), None);
                        assert!(budget.work_ledger_identity_v1() == replay_ledger);
                        assert!(std::ptr::eq(std::ptr::from_ref(budget), replay_slot));
                        drop(replay);
                        assert!(std::ptr::eq(std::ptr::from_ref(caller), caller_slot));
                        assert!(caller.work_ledger_identity_v1() == caller_ledger);
                        assert_eq!(
                            (
                                caller.work(),
                                caller.storage(),
                                caller.peak_storage(),
                                caller.failed_storage(),
                                caller.storage_limit(),
                            ),
                            caller_state,
                        );
                    },
                );
            }
        }
    }
    assert!(changing.into_iter().all(|n| n > 0));
}

#[test]
fn expanded_history_encoder_first_prefix_and_header_have_independent_resource_oracles() {
    with_expanded(
        false,
        Profile::Gfx942,
        None,
        |owner, execution, claims, caller| {
            let floor = caller.storage();
            let guard = 2 * size_of::<usize>()
                + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
                + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>()
                + size_of::<std::result::Result<Bytes, HistoryError>>();
            for cap in 0..5 {
                let mut work = Work::new(cap);
                let mut budget = Budget::new(&mut work, 256 * 1024 * 1024);
                budget.reserve_storage(floor).unwrap();
                let result = encode_live_history(owner, execution, claims, &mut budget);
                assert!(matches!(
                    result,
                    Err(HistoryError::Resource(Resource::Work(_)))
                ));
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (0, floor, floor + guard)
                );
                assert_eq!(budget.failed_storage(), None);
            }
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, floor + guard - 1);
            budget.reserve_storage(floor).unwrap();
            assert!(matches!(
                encode_live_history(owner, execution, claims, &mut budget),
                Err(HistoryError::Resource(Resource::Storage(_)))
            ));
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (0, floor, floor)
            );
            assert_eq!(budget.failed_storage(), Some(floor + guard));
        },
    );
}

// Called only with a genuine actual-source transport by the managed resource
// parent. No fake Input constructor or production visibility is introduced.
impl transport::ExpandedNativeTransportV3 {
    pub(crate) fn expanded_history_first_denial_for_test(
        self,
        floor: usize,
        storage_cut: bool,
    ) -> [usize; 7] {
        assert!(floor >= self.retained_storage_floor_v3());
        let guard = 2 * size_of::<usize>()
            + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
            + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>()
            + size_of::<std::result::Result<(Prepared, HistoryStorage), HistoryError>>();
        let storage = if storage_cut {
            floor + guard - 1
        } else {
            256 * 1024 * 1024
        };
        let work_limit = if storage_cut { 100 } else { 2 };
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage);
        budget.reserve_storage(floor).unwrap();
        let result = self.into_serialized_expanded_history_v1(&mut budget);
        if storage_cut {
            assert!(matches!(
                result,
                Err(HistoryError::Resource(Resource::Storage(_)))
            ));
            assert_eq!(budget.failed_storage(), Some(floor + guard));
            assert_eq!(budget.peak_storage(), floor);
        } else {
            assert!(matches!(
                result,
                Err(HistoryError::Resource(Resource::Work(_)))
            ));
            assert_eq!(budget.failed_storage(), None);
            assert_eq!(budget.peak_storage(), floor + guard);
        }
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), floor);
        let observation = [
            guard,
            floor,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage().unwrap_or(0),
            storage,
            work_limit,
        ];
        drop(result);
        budget.release_storage(floor).unwrap();
        assert_eq!(budget.storage(), 0);
        observation
    }
}

#[test]
fn expanded_history_wrapper_arithmetic_and_closed_directory_are_not_receipt_aliases() {
    let header = size_of::<Prepared>()
        .checked_sub(size_of::<transport::ExpandedNativeTransportV3>())
        .and_then(|n| n.checked_sub(size_of::<Bytes>()))
        .unwrap();
    assert_eq!(header, 2 * size_of::<usize>());
    let samples = [1usize, 48, 160, 1024, 4096];
    for length in samples {
        let mut backing = Vec::<u8>::new();
        backing.try_reserve_exact(length).unwrap();
        let independent = header + size_of::<Bytes>() + backing.capacity();
        assert_eq!(
            independent,
            size_of::<Prepared>() - size_of::<transport::ExpandedNativeTransportV3>()
                + backing.capacity()
        );
    }
}

#[test]
fn expanded_history_transport_preserves_typed_resource_and_nonresource_causes() {
    use std::error::Error as _;
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 0);
    let w = budget.charge_work(1).unwrap_err();
    let s = budget.reserve_storage(1).unwrap_err();
    for e in [
        w,
        s,
        Resource::Accounting,
        Resource::Allocation,
        Resource::Arithmetic,
    ] {
        let outer = HistoryError::Resource(e);
        assert!(outer.source().unwrap().downcast_ref::<Resource>().is_some());
        let nested = HistoryError::Source(
            fe2o3_lower_mir_kernel::DecodedExpandedSourceErrorV1::Callback(
                transport::ExpandedNativeTransportErrorV3::Resource(e),
            ),
        );
        assert!(
            nested
                .source()
                .unwrap()
                .source()
                .unwrap()
                .source()
                .unwrap()
                .downcast_ref::<Resource>()
                .is_some()
        );
    }
    assert!(HistoryError::Mismatch("test").source().is_none());
}

#[test]
fn expanded_history_serializer_uses_full_decoded_source_and_no_optimizer_constructor() {
    let source = include_str!("production_expanded_history_serializer_v1.rs");
    assert!(
        source.contains("materialize_expanded_history_v1")
            && source.contains("with_checked_decoded_expanded_source_v1")
    );
    assert!(
        source.contains("descriptor::decoded::produce")
            && source.contains("check_native_v12_text_descriptor_relation_v3")
    );
    assert!(!source.contains("prepare_checked_scalar_fixed_point_v1("));
    assert!(!source.contains("continue_expanded_production_policy_v1("));
    assert!(!source.contains("lower_native("));
}

// This is child-contract composition, not independent verification of each
// child implementation. No composite encoder or replay supplies expectations.
struct EncoderOracle {
    work: usize,
    live: usize,
    peak: usize,
    transfers: Vec<(usize, usize, usize)>,
}
impl EncoderOracle {
    fn reserve(&mut self, amount: usize) {
        let next = self.live.checked_add(amount).unwrap();
        self.transfers.push((self.work, self.peak, next));
        self.live = next;
        self.peak = self.peak.max(next);
    }
    fn child<T>(&mut self, run: impl FnOnce(&mut Budget<'_>) -> T) -> T {
        let mut work = Work::new(1usize << 40);
        let mut budget = Budget::new(&mut work, 256 * 1024 * 1024);
        budget.charge_work(self.work).unwrap();
        budget.reserve_storage(self.live).unwrap();
        let result = run(&mut budget);
        assert_eq!(budget.storage(), self.live, "child returns unreserved");
        self.work = budget.work();
        self.peak = self.peak.max(budget.peak_storage());
        result
    }
}

#[test]
fn expanded_history_encoder_composes_child_contracts_and_reachable_transfer_cuts() {
    use fe2o3_kernel_opt::{
        CanonicalRefinedForwardingHistoryInputsV1 as FInputs, LoopUnrollHistoryInputsV1 as UInputs,
        encode_expanded_history_v1, encode_loop_unroll_history_v1,
        encode_refined_forwarding_history_v1, encode_scalar_fixed_point_history_v1,
        read_loop_unroll_history_v1, read_refined_forwarding_history_v1,
        read_scalar_fixed_point_history_v1,
    };
    for erased in [false, true] {
        with_expanded(
            erased,
            Profile::Gfx942,
            Some(3),
            |owner, execution, claims, caller| {
                let floor = caller.storage();
                let guard = 2 * size_of::<usize>()
                    + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
                    + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>()
                    + size_of::<std::result::Result<Bytes, HistoryError>>();
                let mut expected = EncoderOracle {
                    work: 0,
                    live: floor,
                    peak: floor,
                    transfers: Vec::new(),
                };
                expected.reserve(guard);
                expected.work += 5;
                expected.child(|budget| owner.verify_equivalence(budget).unwrap());
                expected.reserve(
                    size_of::<FinalSource<'_>>()
                        + size_of::<ProductionExpandedPrefixV1<'_>>()
                        + size_of::<ProductionExpandedHistoryV1<'_>>(),
                );
                let source = final_f(owner);
                expected.child(|budget| claims.check_source_v1(source, execution, budget).unwrap());
                expected.reserve(size_of::<FInputs<'_>>() + size_of::<UInputs<'_, '_>>());
                let inputs = claims.source_inputs_v1(source, execution);
                let f = expected
                    .child(|budget| encode_refined_forwarding_history_v1(inputs, budget).unwrap());
                expected.reserve(f.storage().retained_storage());
                let ff = expected.child(|budget| {
                    read_refined_forwarding_history_v1(f.canonical_bytes(), budget).unwrap()
                });
                expected.reserve(ff.storage().retained_storage());
                let (output, origins, limits) = match owner.prefix() {
                    ProductionExpandedPrefixV1::Direct(v) => {
                        (v.output(), v.continuation().origins(), v.limits())
                    }
                    ProductionExpandedPrefixV1::Erased(v) => {
                        (v.output(), v.continuation().origins(), v.limits())
                    }
                };
                let u = expected.child(|budget| {
                    encode_loop_unroll_history_v1(
                        UInputs {
                            prefix: &ff,
                            output,
                            origins,
                            limits,
                        },
                        budget,
                    )
                    .unwrap()
                });
                expected.reserve(u.storage().retained_storage());
                let uf = expected.child(|budget| {
                    read_loop_unroll_history_v1(u.canonical_bytes(), budget).unwrap()
                });
                expected.reserve(uf.storage().retained_storage());
                let ProductionExpandedHistoryV1::ScalarCleanup(scalar) = owner.history();
                let s = expected.child(|budget| {
                    encode_scalar_fixed_point_history_v1(output, scalar, budget).unwrap()
                });
                expected.reserve(s.storage().retained_storage());
                let sf = expected.child(|budget| {
                    read_scalar_fixed_point_history_v1(s.canonical_bytes(), budget).unwrap()
                });
                expected.reserve(sf.storage().retained_storage());
                let wire =
                    expected.child(|budget| encode_expanded_history_v1(&uf, &sf, budget).unwrap());
                expected.reserve(wire.storage().retained_storage());

                let mut work = Work::new(expected.work);
                let mut budget = Budget::new(&mut work, expected.peak);
                budget.reserve_storage(floor).unwrap();
                let actual = encode_live_history(owner, execution, claims, &mut budget).unwrap();
                assert_eq!(actual.canonical_bytes(), wire.canonical_bytes());
                assert_eq!(
                    (budget.work(), budget.peak_storage(), budget.storage()),
                    (expected.work, expected.peak, floor)
                );
                assert_eq!(budget.failed_storage(), None);
                // Only cuts above every preceding child peak are reachable in the
                // complete encoder. Masked reservations are not claimed as covered.
                let mut reachable = 0;
                for &(accepted_work, prior_peak, request) in &expected.transfers {
                    if request <= prior_peak {
                        continue;
                    }
                    reachable += 1;
                    let mut work = Work::new(expected.work);
                    let mut budget = Budget::new(&mut work, request - 1);
                    budget.reserve_storage(floor).unwrap();
                    let failure = encode_live_history(owner, execution, claims, &mut budget);
                    assert!(matches!(
                        failure,
                        Err(HistoryError::Resource(Resource::Storage(_)))
                    ));
                    assert_eq!(
                        (budget.work(), budget.peak_storage(), budget.storage()),
                        (accepted_work, prior_peak, floor)
                    );
                    assert_eq!(budget.failed_storage(), Some(request));
                }
                assert!(reachable > 0);
                drop(actual);
                drop(wire);
                drop(sf);
                drop(s);
                drop(uf);
                drop(u);
                drop(ff);
                drop(f);
                assert_eq!(caller.storage(), floor);
            },
        );
    }
}
