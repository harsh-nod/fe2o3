//! Fixed codec and content binding tests, not protected execution evidence.
use super::super::InertCompilerExecutionSubjectV1 as Legacy;
use super::*;
use crate::compiler_module_handoff::native_v4::tests::{outer, outer_with_carrier, wrap};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::sync::Arc;

type Subject = InertCompilerExecutionSubjectV2;
const LIMIT: usize = MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4;

fn fields() -> codec::Fields {
    super::super::tests::fields()
}
fn wire() -> [u8; 690] {
    *Subject::from_fields(fields()).unwrap().canonical_bytes()
}
fn reseal(bytes: &mut [u8; 690]) {
    let hash = SCHEMA.identity(&bytes[..658]);
    bytes[658..].copy_from_slice(&hash);
}
fn decode(bytes: &[u8]) -> Result<(Subject, InertCompilerExecutionSubjectStorageV2)> {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(bytes.len()).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = Subject::decode(bytes, &mut budget);
    assert_eq!(budget.storage(), bytes.len());
    assert_eq!(budget.work(), INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.failed_storage(), None);
    drop(budget);
    assert_eq!(work.failed_work(), None);
    result
}
fn replay(
    handoff: &Handoff,
    budget: &mut Budget<'_>,
) -> Result<(Subject, InertCompilerExecutionSubjectStorageV2)> {
    Subject::from_replay_evidence(
        fields().attempt,
        CompilerModuleHandoffSlotV4::Production,
        CompilerModuleHandoffTransactionIdentityV4::from_bytes([7; 32]),
        handoff,
        budget,
    )
}

#[test]
fn native_subject_v2_matches_independent_golden_hashes() {
    use sha2::{Digest, Sha256};
    let hex = |text: &str| -> [u8; 32] {
        std::array::from_fn(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap())
    };
    let subject = Subject::from_fields(fields()).unwrap();
    assert_eq!(&subject.canonical_bytes()[..8], b"F2O3CES2");
    assert_eq!(&subject.canonical_bytes()[8..10], &2_u16.to_le_bytes());
    assert_eq!(subject.identity().byte_len(), 690);
    // Independently calculated from the published field layout, not this codec.
    assert_eq!(
        *subject.identity().sha256(),
        hex("e06b5af4ce60eec43e5427cd9a3551b94a7fb2dc20dbf23705f9b1c6f51d7039")
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(subject.canonical_bytes())),
        hex("17f6e7f89357ebae6e652bd76327b159b9653327c4aa37134903075248e7d064")
    );
}

#[test]
fn native_subject_v2_roundtrip_rejects_every_changed_byte_and_legacy_version() {
    let original = Subject::from_fields(fields()).unwrap();
    assert_eq!(decode(original.canonical_bytes()).unwrap().0, original);
    assert!(Legacy::decode(original.canonical_bytes()).is_err());
    let legacy = Legacy::from_fields(fields()).unwrap();
    assert!(decode(legacy.canonical_bytes()).is_err());
    for offset in 0..690 {
        let mut changed = wire();
        changed[offset] ^= 1;
        assert!(decode(&changed).is_err(), "offset {offset}");
    }
    for (magic, version) in [(*b"F2O3CES1", 2_u16), (*b"F2O3CES2", 1)] {
        let mut changed = wire();
        changed[..8].copy_from_slice(&magic);
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        reseal(&mut changed);
        assert!(decode(&changed).is_err());
        assert!(Legacy::decode(&changed).is_err());
    }
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(690).unwrap();
    assert!(
        original
            .identity()
            .matches_canonical_bytes(original.canonical_bytes(), &mut budget)
            .unwrap()
    );
    assert!(
        !original
            .identity()
            .matches_canonical_bytes(legacy.canonical_bytes(), &mut budget)
            .unwrap()
    );
    assert!(!original.authenticates_compiler_execution());
    assert!(original.requires_protected_execution_attestation());
    assert!(!original.grants_compiler_authority());
    assert!(!original.grants_publication_authority());
    assert!(!original.grants_load_authority());
    assert!(!original.grants_launch_authority());
}

#[test]
fn native_subject_v2_resealed_noncanonical_and_untrusted_binding_axes() {
    for offset in [0, 8, 10, 12, 20, 24, 80, 81, 344, 346, 658] {
        let mut changed = wire();
        changed[offset] ^= 1;
        if offset == 24 {
            changed[24..32].fill(0);
        }
        if offset != 658 {
            reseal(&mut changed);
        }
        assert!(decode(&changed).is_err(), "offset {offset}");
    }
    for (offset, label) in [(88, "V4 handoff transaction"), (120, "rustc invocation")] {
        let mut changed = wire();
        changed[offset..offset + 32].fill(0);
        reseal(&mut changed);
        assert!(
            matches!(decode(&changed),Err(Failure::Framing(CompilerExecutionSubjectErrorV1::ZeroIdentity {field})) if field==label)
        );
    }
    for index in 0..7 {
        let start = 378 + 40 * index;
        for length in [false, true] {
            let mut changed = wire();
            let range = if length {
                start + 32..start + 40
            } else {
                start..start + 32
            };
            changed[range.clone()].fill(0);
            reseal(&mut changed);
            assert!(decode(&changed).is_err());
            // A different nonzero digest/length remains inert until exact typed
            // reconstruction compares it to the real publication subject.
            changed[range].fill(0x71);
            reseal(&mut changed);
            let decoded = decode(&changed).unwrap().0;
            assert_ne!(
                decoded.identity(),
                Subject::from_fields(fields()).unwrap().identity()
            );
            assert!(!decoded.authenticates_compiler_execution());
        }
    }
    for bytes in [&wire()[..689], &[0u8; 691][..]] {
        assert!(matches!(
            decode(bytes),
            Err(Failure::Framing(
                CompilerExecutionSubjectErrorV1::InvalidLength { .. }
            ))
        ));
    }
    let mut direct = wire();
    direct[32..80].fill(0);
    reseal(&mut direct);
    assert!(decode(&direct).is_ok());
    for range in [32..48, 48..80] {
        let mut mixed = wire();
        mixed[range].fill(0);
        reseal(&mut mixed);
        assert!(matches!(
            decode(&mixed),
            Err(Failure::Framing(CompilerExecutionSubjectErrorV1::Attempt(
                _
            )))
        ));
    }
    for pin in 0..6 {
        let mut changed = wire();
        changed[152 + 32 * pin..184 + 32 * pin].fill(0);
        reseal(&mut changed);
        assert!(matches!(
            decode(&changed),
            Err(Failure::Framing(
                CompilerExecutionSubjectErrorV1::CompilerClosure(_)
            ))
        ));
    }
}

#[test]
fn native_subject_v2_binds_complete_carrier_and_cached_invocation() {
    let a = outer_with_carrier(7, 19);
    let b = outer_with_carrier(7, 29);
    assert_eq!(
        a.capsule().base().canonical_bytes(),
        b.capsule().base().canonical_bytes()
    );
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(handoff_floor(&a).unwrap() + handoff_floor(&b).unwrap())
        .unwrap();
    let (first, storage) = replay(&a, &mut budget).unwrap();
    budget.reserve_storage(storage.0).unwrap();
    let (second, storage) = replay(&b, &mut budget).unwrap();
    budget.reserve_storage(storage.0).unwrap();
    assert_ne!(first.semantic_capsule(), second.semantic_capsule());
    assert_ne!(
        first.compiler_module_pair_binding(),
        second.compiler_module_pair_binding()
    );
    assert_ne!(first.outer_handoff(), second.outer_handoff());
    assert_ne!(first.identity(), second.identity());
    macro_rules! bound {
        ($field:ident, $identity:expr) => {
            let identity = $identity;
            assert_eq!(first.$field().sha256(), identity.sha256());
            assert_eq!(first.$field().byte_len(), identity.byte_len());
        };
    }
    let receipts = a.capsule().base().receipts();
    bound!(
        rustc_identity_inventory,
        receipts.rustc_identity_inventory().identity()
    );
    bound!(
        rustc_preflight_plan,
        receipts.rustc_preflight_plan().identity()
    );
    bound!(semantic_capsule, a.capsule().identity());
    bound!(
        final_compiler_module_commitment,
        receipts.final_compiler_module_commitment().identity()
    );
    bound!(compiler_module_handoff, a.module_handoff().identity());
    bound!(compiler_module_pair_binding, a.pair_binding_identity());
    bound!(outer_handoff, a.identity());
    assert_eq!(
        first.rustc_invocation_sha256(),
        &a.capsule().base().invocation_digest().into_bytes()
    );
    assert_ne!(
        first.rustc_invocation_sha256(),
        first.attempt().invocation().as_bytes()
    );
    assert_eq!(
        first.compiler_closure(),
        *a.capsule().base().compiler_closure()
    );
    drop((a, b));
    assert_eq!(decode(first.canonical_bytes()).unwrap().0, first);
}

#[test]
fn native_subject_v2_full_backing_floor_and_no_retention() {
    let source = outer(7);
    let n = source.canonical_bytes().len();
    let mut backing = Vec::with_capacity(n + 8192);
    backing.extend([0; 37]);
    backing.extend_from_slice(source.canonical_bytes());
    backing.extend([0; 53]);
    let backing = Arc::new(backing);
    let weak = Arc::downgrade(&backing);
    let handoff = Handoff::decode_shared_vec(backing, 37..37 + n).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = handoff_floor(&handoff).unwrap();
    budget.reserve_storage(floor - 1).unwrap();
    assert!(matches!(
        replay(&handoff, &mut budget),
        Err(Failure::Resource(Resource::Accounting))
    ));
    budget.reserve_storage(1).unwrap();
    let (subject, storage) = replay(&handoff, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(storage.0, RETAINED);
    budget.reserve_storage(storage.0).unwrap();
    drop(handoff);
    assert!(weak.upgrade().is_none());
    budget.release_storage(floor).unwrap();
    assert_eq!(decode(subject.canonical_bytes()).unwrap().0, subject);
}

#[test]
fn native_subject_v2_exact_and_one_short_resources_preserve_shared_history() {
    let h = outer(7);
    for decoding in [false, true] {
        let floor = 37
            + if decoding {
                690
            } else {
                handoff_floor(&h).unwrap()
            };
        let run = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(11).unwrap();
            assert!(budget.reserve_storage(usize::MAX).is_err());
            assert!(budget.charge_work(usize::MAX).is_err());
            let ledger = budget.work_ledger_identity_v1();
            let result = if decoding {
                Subject::decode(&wire(), &mut budget)
            } else {
                replay(&h, &mut budget)
            };
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
            assert!(budget.work_ledger_identity_v1() == ledger);
            let measured = (result, budget.work(), budget.peak_storage());
            drop(budget);
            assert_eq!(work.failed_work(), Some(usize::MAX));
            measured
        };
        let (result, work, storage) = run(usize::MAX, LIMIT);
        assert_eq!(result.unwrap().1.0, RETAINED);
        assert_eq!(work, 11 + INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2);
        assert_eq!(storage, floor + INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V2);
        run(work, storage).0.unwrap();
        assert!(matches!(
            run(work - 1, storage).0,
            Err(Failure::Resource(Resource::Work(_)))
        ));
        assert!(matches!(
            run(work, storage - 1).0,
            Err(Failure::Resource(Resource::Storage(_)))
        ));
    }
}

#[test]
fn native_subject_v2_entry_and_malformed_failures_preserve_resources() {
    let bytes = wire();
    for (work_limit, storage_limit, paid, denied) in [
        (7, LIMIT, 690, "work"),
        (usize::MAX, LIMIT + 1, 690, "accounting"),
        (usize::MAX, LIMIT, 689, "accounting"),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(paid).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = Subject::decode(&bytes, &mut budget);
        assert!(matches!(
            (denied, result),
            ("work", Err(Failure::Resource(Resource::Work(_))))
                | ("accounting", Err(Failure::Resource(Resource::Accounting)))
        ));
        assert_eq!(budget.storage(), paid);
        assert_eq!(budget.work(), if denied == "work" { 0 } else { 8 });
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    let mut malformed = bytes;
    malformed[0] ^= 1;
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(690 + 37).unwrap();
    budget.charge_work(11).unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    assert!(budget.charge_work(usize::MAX).is_err());
    assert!(matches!(
        Subject::decode(&malformed, &mut budget),
        Err(Failure::Framing(
            CompilerExecutionSubjectErrorV1::InvalidMagic
        ))
    ));
    assert_eq!(budget.storage(), 690 + 37);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    assert_eq!(budget.work(), 11 + INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2);
    drop(budget);
    assert_eq!(work.failed_work(), Some(usize::MAX));
}

#[test]
fn native_subject_v2_typed_replay_rejects_zero_transaction() {
    let h = outer(7);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = handoff_floor(&h).unwrap();
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        Subject::from_replay_evidence(
            fields().attempt,
            CompilerModuleHandoffSlotV4::Production,
            CompilerModuleHandoffTransactionIdentityV4::from_bytes([0; 32]),
            &h,
            &mut budget,
        ),
        Err(Failure::Framing(
            CompilerExecutionSubjectErrorV1::ZeroIdentity {
                field: "V4 handoff transaction"
            }
        ))
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2);
}

fn large_invocation_handoff() -> Handoff {
    use fe2o3_compiler_lineage::*;
    use fe2o3_rustc_invocation::{
        RustcInvocationDescriptorV2, RustcInvocationDescriptorV3, RustcUnitV2,
    };
    let source = outer(7);
    let base = source.capsule().base();
    let invocation = base.invocation();
    let mut argv = invocation
        .rustc()
        .argv()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let backend = argv.pop().unwrap();
    argv.extend((0..1024).map(|i| format!("--cfg=padding_{i}")));
    argv.push(backend);
    let unit = RustcUnitV2::new(invocation.rustc().working_directory(), argv).unwrap();
    let invocation = RustcInvocationDescriptorV3::new(
        RustcInvocationDescriptorV2::new(
            *invocation.rustc_executable_sha256(),
            *invocation.codegen_backend_sha256(),
            unit,
            invocation.compile_environment().clone(),
        )
        .unwrap(),
        *invocation.compiler_closure(),
    )
    .unwrap();
    let receipts = base.receipts();
    macro_rules! copy_receipt {
        ($name:ident, $type:ident) => {
            $type::from_canonical_preimage(receipts.$name().canonical_preimage()).unwrap()
        };
    }
    let receipts = OrderedInertSemanticLineageReceiptsV3::new(
        copy_receipt!(
            rustc_identity_inventory,
            InertRustcIdentityInventoryReceiptV3
        ),
        copy_receipt!(rustc_preflight_plan, InertRustcPreflightPlanReceiptV3),
        copy_receipt!(semantic_mir, InertCanonicalSemanticMirReceiptV3),
        copy_receipt!(middle_end, InertMiddleEndReceiptV3),
        copy_receipt!(kernel_ir, InertKernelIrReceiptV3),
        copy_receipt!(
            mir_to_kir_correspondence,
            InertMirToKirCorrespondenceReceiptV3
        ),
        copy_receipt!(formal_memory, InertFormalMemoryReceiptV3),
        copy_receipt!(proof_binding, InertProofBindingReceiptV3),
        copy_receipt!(target_binding, InertTargetBindingReceiptV3),
        copy_receipt!(data_layout, InertDataLayoutReceiptV3),
        copy_receipt!(abi, InertAbiReceiptV3),
        copy_receipt!(export_manifest, InertExportManifestReceiptV3),
        copy_receipt!(amdgpu_lowering, InertAmdgpuLoweringReceiptV3),
        copy_receipt!(semantic_to_llvm, InertSemanticToLlvmReceiptV3),
        copy_receipt!(
            final_compiler_module_commitment,
            InertFinalCompilerModuleCommitmentReceiptV3
        ),
    );
    let base = InertProductionSemanticCapsuleV3::new(invocation, base.target(), receipts).unwrap();
    let legacy = fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3::new(
        base,
        source.module_handoff().clone(),
    )
    .unwrap();
    wrap(&legacy, 7)
}

#[test]
fn native_subject_v2_cached_construction_and_decode_allocate_nothing() {
    for handoff in [outer(7), large_invocation_handoff()] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget
            .reserve_storage(handoff_floor(&handoff).unwrap())
            .unwrap();
        let (result, count) = allocation::count(|| replay(&handoff, &mut budget));
        assert_eq!(count, 0);
        let (subject, storage) = result.unwrap();
        budget.reserve_storage(storage.0).unwrap();
        let (result, count) =
            allocation::count(|| Subject::decode(subject.canonical_bytes(), &mut budget));
        assert_eq!(count, 0);
        assert_eq!(result.unwrap().0, subject);
    }
}

mod allocation {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };
    struct Counter;
    #[global_allocator]
    static ALLOCATOR: Counter = Counter;
    thread_local! { static COUNT: Cell<Option<usize>>=const { Cell::new(None) }; }
    fn record() {
        let _ = COUNT.try_with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1));
            }
        });
    }
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            record();
            // SAFETY: the unchanged allocator layout is forwarded to System.
            unsafe { System.alloc(l) }
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            record();
            // SAFETY: the unchanged allocator layout is forwarded to System.
            unsafe { System.alloc_zeroed(l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            record();
            // SAFETY: this System allocation's pointer/layout and new size are unchanged.
            unsafe { System.realloc(p, l, n) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            // SAFETY: pointer/layout originated from the forwarding methods above.
            unsafe { System.dealloc(p, l) }
        }
    }
    pub(super) fn count<T>(f: impl FnOnce() -> T) -> (T, usize) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                COUNT.with(|c| c.set(None));
            }
        }
        COUNT.with(|c| assert!(c.replace(Some(0)).is_none()));
        let _reset = Reset;
        let result = f();
        (result, COUNT.with(|c| c.get().unwrap()))
    }
}
