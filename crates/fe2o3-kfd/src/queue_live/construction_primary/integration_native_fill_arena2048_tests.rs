//! Original-root capacity controls. Synthetic completions are not GPU evidence.

use super::*;

const LARGE: usize = GFX942_INDEPENDENT_FILL_ARENA2048_SLOTS_V1;
const ORDER: ArenaOrderV1 = ArenaOrderV1::IndependentDisjointWriteOnly;
const CAPACITY: ArenaCapacityV1 = ArenaCapacityV1::Independent2048;

fn setup_large(
    bytes: &[u8],
) -> (
    Box<ArenaRoot<'_, LARGE>>,
    Rc<RefCell<Trace>>,
    ResourceCreditAccountV1,
    usize,
) {
    setup_with_capacity::<LARGE>(bytes, ORDER, CAPACITY)
}

#[test]
fn independent2048_primary_retains_one_common_original_and_exact_prepaid_roster() {
    assert!(core::mem::size_of::<ArenaRoot<'static, LARGE>>() <= 65536);
    let captured = native_fill_cohort_cases::payload();
    let (root, trace, account, pointer) = setup_large(captured.exact_payload_bytes());
    assert!(
        root.preparation
            .1
            .require_native_fill_arena_capacity(ArenaCapacityV1::Original1024)
            .is_err()
    );
    let charged = account.usage();
    assert_eq!(charged.retained_records, LARGE + 4);
    let (root, result) = run_work(root, |root, entry| {
        root.construct_native_fill_arena(entry, 128 * 1024)
    });
    assert!(result.is_ok(), "{result:?}");
    originals(&root, &trace, pointer, captured.exact_payload_bytes());
    let owner = root.completed.as_ref().unwrap().dispatch.as_ref().unwrap();
    owner.primary_assert_independent2048_roster_v1();
    assert_eq!(account.usage(), charged);
    assert!(!trace.borrow().poison);
}

#[test]
fn independent2048_original_64k_ring_refuses_before_native_preparation() {
    let captured = native_fill_cohort_cases::payload();
    let (root, trace, account, pointer) = setup_large(captured.exact_payload_bytes());
    let charged = account.usage();
    let (root, result) = run_work(root, |root, entry| {
        root.construct_native_fill_arena(entry, 64 * 1024)
    });
    assert!(result.is_err());
    originals(&root, &trace, pointer, captured.exact_payload_bytes());
    assert!(!trace.borrow().calls.contains(&"allocate-ring"));
    assert_eq!(account.usage(), charged);
}

#[test]
fn independent2048_partial_error_and_unwind_keep_original_root_and_all_slot_charges() {
    for stage in [
        PreparationStageV1::Plan,
        PreparationStageV1::KernargRetain,
        PreparationStageV1::PacketResolve(1537),
        PreparationStageV1::Complete,
    ] {
        for panic in [false, true] {
            let captured = native_fill_cohort_cases::payload();
            let (mut root, trace, account, pointer) = setup_large(captured.exact_payload_bytes());
            let charged = account.usage();
            root.preparation.1.primary_inject_stage_v1(stage, panic);
            let (root, result) = run_work(root, |root, entry| {
                root.construct_native_fill_arena(entry, 128 * 1024)
            });
            assert!(result.is_err());
            root.preparation.1.primary_assert_failed_stage_v1(stage);
            originals(&root, &trace, pointer, captured.exact_payload_bytes());
            assert_eq!(account.usage(), charged);
            assert!(!trace.borrow().calls.contains(&"allocate-ring"));
        }
    }
}

#[test]
fn independent2048_shared_cpu_receipts_recycle_without_common_release_or_slot_replay() {
    shared_cpu_publication_and_recycle_with_capacity::<LARGE>(ORDER, CAPACITY);
}

#[test]
fn independent2048_currentness_and_unknown_creation_preserve_original_common_custody() {
    for panic in [false, true] {
        let captured = native_fill_cohort_cases::payload();
        let (root, trace, account, pointer) = setup_large(captured.exact_payload_bytes());
        let charged = account.usage();
        trace.borrow_mut().fault = Some(("currentness", 3, panic));
        let (root, result) = run_work(root, |root, entry| {
            root.construct_native_fill_arena(entry, 128 * 1024)
        });
        assert!(result.is_err());
        originals(&root, &trace, pointer, captured.exact_payload_bytes());
        assert_eq!(account.usage(), charged);
    }
    for mode in 1..=5 {
        let captured = native_fill_cohort_cases::payload();
        let (root, trace, account, pointer) = setup_large(captured.exact_payload_bytes());
        let charged = account.usage();
        trace.borrow_mut().create = mode;
        let (root, result) = run_work(root, |root, entry| {
            root.construct_native_fill_arena(entry, 128 * 1024)
        });
        assert!(result.is_err());
        originals(&root, &trace, pointer, captured.exact_payload_bytes());
        assert_eq!(account.usage(), charged);
        assert!(trace.borrow().poison);
        assert_eq!(trace.borrow().cleanup, 0);
    }
}

#[test]
fn independent2048_order_or_family_refusal_returns_every_original_before_preparation() {
    for wrong_order in [false, true] {
        let captured = native_fill_cohort_cases::payload();
        let (mut memory, trace) = setup_memory();
        let account = account(1);
        let mut total = 0;
        let packets = Gfx942IndependentFillArena2048PacketsV1::try_new(&account, |index| {
            total += (1 + index % 3) * 4;
            packet_with_order(
                index,
                0,
                wrong_order || index != 1537,
                if wrong_order {
                    ArenaOrderV1::Ordered
                } else {
                    ORDER
                },
            )
        })
        .unwrap();
        let original_packets = packets.packets().as_ptr();
        let token = memory.allocate::<HostVisibleCoherentGttV1>(total).unwrap();
        let output =
            Gfx942FixedDispatchDataV1::host_visible_uninitialized(memory.map(token).unwrap());
        let original_data = output.sdma_storage_identity();
        let charged = account.usage();
        let failure = Gfx942IndependentFillArena2048InputsV1::admit_local_outputs(
            native_fill_cohort_cases::program(captured.exact_payload_bytes()),
            packets,
            output,
        )
        .err()
        .unwrap();
        let (program, packets, output, _) = failure.into_parts();
        assert_eq!(
            program.envelope().bytes().as_ptr(),
            captured.exact_payload_bytes().as_ptr()
        );
        assert_eq!(packets.packets().as_ptr(), original_packets);
        assert_eq!(packets.packets().len(), LARGE);
        assert_eq!(output.sdma_storage_identity(), original_data);
        assert_eq!(account.usage(), charged);
        assert!(!trace.borrow().calls.contains(&"allocate-ring"));
    }
}
