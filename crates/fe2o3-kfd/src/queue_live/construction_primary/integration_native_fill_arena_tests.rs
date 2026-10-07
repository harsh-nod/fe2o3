//! CPU-only use of the actual original-root arena construction and slot kernels.

use super::*;
use crate::queue::dispatch_binding::{
    GFX942_NATIVE_FILL_ARENA_SLOTS_V1 as SLOTS, Gfx942NativeFillArenaInputsV1,
    Gfx942NativeFillArenaPacketsV1, Gfx942NativeFillArenaStorageV1,
};
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};

type ArenaRoot<'a> = Root<(
    Vec<ValidatedKernelEnvelope<'a>>,
    FixedDispatchPreparationCustodyV1<SLOTS, Gfx942NativeFillArenaPacketsV1>,
    Gfx942NativeFillArenaStorageV1,
)>;

fn packet(index: usize, offset: u64, conditional: bool) -> Gfx942FixedDispatchPacketV1 {
    let count = 1 + (index % 3) as u64;
    let mut bytes = vec![0; 272];
    bytes[8..16].copy_from_slice(&count.to_le_bytes());
    let packet = Gfx942FixedDispatchPacketV1::new(
        0,
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        bytes.into_boxed_slice(),
        Box::new([Gfx942DispatchBufferBindingV1::new(0, 0, offset, count * 4)]),
    );
    if conditional {
        packet.require_conditional_fill_v1()
    } else {
        packet
    }
}

fn packets(
    account: &ResourceCreditAccountV1,
    change: Option<(u64, bool)>,
) -> (Gfx942NativeFillArenaPacketsV1, usize) {
    let mut end = 0;
    let packets = Gfx942NativeFillArenaPacketsV1::try_new(account, |index| {
        let (offset, conditional) = if index == 1 {
            change.unwrap_or((end, true))
        } else {
            (end, true)
        };
        let packet = packet(index, offset, conditional);
        end += (1 + (index % 3) as u64) * 4;
        packet
    })
    .unwrap();
    (packets, end as usize)
}

fn account(records: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 64 << 20),
        records,
    )
    .unwrap()
}

fn setup(
    bytes: &[u8],
) -> (
    Box<ArenaRoot<'_>>,
    Rc<RefCell<Trace>>,
    ResourceCreditAccountV1,
    usize,
) {
    let (mut memory, trace) = setup_memory();
    let account = account(SLOTS + 4);
    let (packets, total) = packets(&account, None);
    let token = memory.allocate::<HostVisibleCoherentGttV1>(total).unwrap();
    let output = Gfx942FixedDispatchDataV1::host_visible_uninitialized(memory.map(token).unwrap());
    let inputs = Gfx942NativeFillArenaInputsV1::admit(
        native_fill_cohort_cases::program(bytes),
        packets,
        output,
    )
    .unwrap();
    let Gfx942NativeFillArenaInputsV1 {
        programs,
        packets,
        data,
    } = inputs;
    let mut storage = Gfx942NativeFillArenaStorageV1::preallocate(account.clone()).unwrap();
    let custody = FixedDispatchPreparationCustodyV1::new_native_fill_arena(
        packets,
        data,
        storage.premises.take().unwrap(),
    );
    trace.borrow_mut().initial_data = Some(memory.observation());
    let root = Root::new_with(memory, (programs, custody, storage));
    let pointer = &*root as *const ArenaRoot<'_> as usize;
    (root, trace, account, pointer)
}

fn originals(root: &ArenaRoot<'_>, trace: &Rc<RefCell<Trace>>, pointer: usize, bytes: &[u8]) {
    assert_common(root, pointer, trace);
    assert_platform(root, None);
    assert_eq!(root.preparation.0.len(), 1);
    assert_eq!(
        root.preparation.0[0].envelope().bytes().as_ptr(),
        bytes.as_ptr()
    );
    assert_eq!(root.preparation.0[0].envelope().bytes().len(), bytes.len());
    if let Some(owner) = root
        .dispatch
        .as_ref()
        .or_else(|| root.completed.as_ref().and_then(|c| c.dispatch.as_ref()))
    {
        owner.require_arena_backing_v1().unwrap();
        let identities = owner.primary_fixture_identities_v1();
        assert_eq!(identities.len(), 3);
        assert_partition(root, identities);
    }
}

#[test]
fn native_fill_arena_primary_keeps_one_original_data_and_1024_charged_slot_owners() {
    assert!(core::mem::size_of::<ArenaRoot<'static>>() <= 65536);
    let captured = native_fill_cohort_cases::payload();
    let (root, trace, account, pointer) = setup(captured.exact_payload_bytes());
    let charged = account.usage();
    assert_eq!(charged.retained_records, SLOTS + 4);
    let (root, result) = run_work(root, |root, entry| {
        root.construct_native_fill_arena(entry, 65536)
    });
    assert!(result.is_ok(), "{result:?}");
    originals(&root, &trace, pointer, captured.exact_payload_bytes());
    assert_eq!(account.usage(), charged);
    assert!(root.preparation.2.settled());
    assert!(!trace.borrow().poison);
}

#[test]
fn native_fill_arena_record_refusal_refunds_only_effect_free_metadata() {
    let account = account(SLOTS + 2);
    assert!(Gfx942NativeFillArenaStorageV1::preallocate(account.clone()).is_err());
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(account.usage().retained_records, 0);
}

#[test]
fn native_fill_arena_partition_alias_gap_and_wrong_family_return_original_data() {
    let captured = native_fill_cohort_cases::payload();
    for (offset, conditional) in [(0, true), (12, true), (4, false)] {
        let (mut memory, _trace) = setup_memory();
        let account = account(1);
        let (packets, total) = packets(&account, Some((offset, conditional)));
        let original_packets = packets.packets().as_ptr();
        let charged = account.usage();
        let token = memory.allocate::<HostVisibleCoherentGttV1>(total).unwrap();
        let output =
            Gfx942FixedDispatchDataV1::host_visible_uninitialized(memory.map(token).unwrap());
        let original = output.sdma_storage_identity();
        let failure = match Gfx942NativeFillArenaInputsV1::admit(
            native_fill_cohort_cases::program(captured.exact_payload_bytes()),
            packets,
            output,
        ) {
            Err(failure) => failure,
            Ok(_) => panic!("invalid arena partition admitted"),
        };
        let (program, packets, output, _) = failure.into_parts();
        assert_eq!(
            program.envelope().bytes().as_ptr(),
            captured.exact_payload_bytes().as_ptr()
        );
        assert_eq!(packets.packets().len(), SLOTS);
        assert_eq!(packets.packets().as_ptr(), original_packets);
        assert_eq!(account.usage(), charged);
        assert_eq!(output.sdma_storage_identity(), original);
    }
}

#[test]
fn native_fill_arena_small_ring_precedes_native_preparation() {
    let captured = native_fill_cohort_cases::payload();
    let (root, trace, account, pointer) = setup(captured.exact_payload_bytes());
    let charged = account.usage();
    let (root, result) = run_work(root, |root, entry| {
        root.construct_native_fill_arena(entry, 4096)
    });
    assert!(result.is_err());
    originals(&root, &trace, pointer, captured.exact_payload_bytes());
    assert!(!trace.borrow().calls.contains(&"allocate-ring"));
    assert_eq!(account.usage(), charged);
}

#[test]
fn native_fill_arena_partial_error_and_panic_retain_original_root_and_all_debits() {
    for stage in [
        PreparationStageV1::Plan,
        PreparationStageV1::KernargRetain,
        PreparationStageV1::PacketResolve(517),
        PreparationStageV1::Complete,
    ] {
        for panic in [false, true] {
            let captured = native_fill_cohort_cases::payload();
            let (mut root, trace, account, pointer) = setup(captured.exact_payload_bytes());
            let charged = account.usage();
            root.preparation.1.primary_inject_stage_v1(stage, panic);
            let (root, result) = run_work(root, |root, entry| {
                root.construct_native_fill_arena(entry, 65536)
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
fn native_fill_arena_1024_original_receipts_use_shared_cpu_publication_and_recycle() {
    let captured = native_fill_cohort_cases::payload();
    let (root, trace, account, pointer) = setup(captured.exact_payload_bytes());
    let (mut root, result) = run_work(root, |root, entry| {
        root.construct_native_fill_arena(entry, 65536)
    });
    assert!(result.is_ok(), "{result:?}");
    let charged = account.usage();
    let foreign_account = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 64 << 20),
        SLOTS + 3,
    )
    .unwrap();
    let mut foreign = Gfx942NativeFillArenaStorageV1::preallocate(foreign_account).unwrap();
    let completed = root.completed.as_mut().unwrap();
    let queue = completed.engine.resources[0].key;
    let common = completed.dispatch.take().unwrap();
    let ((), common, poisoned, _) =
        ComputeAqlQueueSessionV1::with_ordinary_binding_session_v1(queue, common, |session| {
            let registry = &mut root.preparation.2;
            let mut receipts = Vec::with_capacity(SLOTS);
            for index in 0..SLOTS {
                receipts.push(Some(
                    session
                        .submit_arena_binding_for_test(registry.recipe(index).unwrap(), |_, _| {
                            Ok(index as u64)
                        })
                        .unwrap(),
                ));
            }
            assert!(!registry.settled());
            let first = receipts[0].as_ref().unwrap();
            assert!(
                registry
                    .recipe(0)
                    .unwrap()
                    .validate_original_batch(first)
                    .is_ok()
            );
            assert!(
                registry
                    .recipe(1)
                    .unwrap()
                    .validate_original_batch(first)
                    .is_err()
            );
            assert!(
                foreign
                    .recipe(0)
                    .unwrap()
                    .validate_original_batch(first)
                    .is_err()
            );
            assert!(
                registry
                    .recipe(SLOTS - 1)
                    .unwrap()
                    .completed_range_for_test(session.dispatch.as_ref().unwrap(), 4)
                    .is_err()
            );
            // Synthetic CPU completion order is deliberately not a hardware claim.
            for index in (0..SLOTS).rev() {
                let completed = session
                    .complete_arena_binding_for_test(
                        registry.recipe(index).unwrap(),
                        receipts[index].take().unwrap(),
                    )
                    .unwrap();
                session
                    .recycle_arena_binding_for_test(registry.recipe(index).unwrap(), completed)
                    .unwrap();
                if index == SLOTS - 1 {
                    assert!(
                        !registry.settled(),
                        "other actual original CPU receipts remain published"
                    );
                    assert_eq!(
                        registry
                            .recipe(index)
                            .unwrap()
                            .completed_range_for_test(session.dispatch.as_ref().unwrap(), 4)
                            .unwrap(),
                        (8184, 4, 1)
                    );
                    assert!(
                        registry
                            .recipe(index)
                            .unwrap()
                            .completed_range_for_test(session.dispatch.as_ref().unwrap(), 8)
                            .is_err()
                    );
                    session
                        .dispatch
                        .as_ref()
                        .unwrap()
                        .require_arena_backing_v1()
                        .unwrap();
                }
                assert!(
                    session
                        .submit_arena_binding_for_test(
                            registry.recipe(index).unwrap(),
                            |_, _| panic!("single-use arena slot replay")
                        )
                        .is_err()
                );
            }
            assert!(registry.settled());
        });
    root.completed.as_mut().unwrap().dispatch = Some(common);
    assert!(!poisoned);
    originals(&root, &trace, pointer, captured.exact_payload_bytes());
    assert_eq!(account.usage(), charged);
}

#[test]
fn native_fill_arena_currentness_and_uncertain_create_keep_common_root() {
    for panic in [false, true] {
        let captured = native_fill_cohort_cases::payload();
        let (root, trace, account, pointer) = setup(captured.exact_payload_bytes());
        let charged = account.usage();
        trace.borrow_mut().fault = Some(("currentness", 3, panic));
        let (root, result) = run_work(root, |root, entry| {
            root.construct_native_fill_arena(entry, 65536)
        });
        assert!(result.is_err());
        originals(&root, &trace, pointer, captured.exact_payload_bytes());
        assert_eq!(account.usage(), charged);
    }
    for mode in 1..=5 {
        let captured = native_fill_cohort_cases::payload();
        let (root, trace, account, pointer) = setup(captured.exact_payload_bytes());
        let charged = account.usage();
        trace.borrow_mut().create = mode;
        let (root, result) = run_work(root, |root, entry| {
            root.construct_native_fill_arena(entry, 65536)
        });
        assert!(result.is_err());
        originals(&root, &trace, pointer, captured.exact_payload_bytes());
        assert_eq!(account.usage(), charged);
        assert!(trace.borrow().poison);
        assert_eq!(trace.borrow().cleanup, 0);
    }
}
