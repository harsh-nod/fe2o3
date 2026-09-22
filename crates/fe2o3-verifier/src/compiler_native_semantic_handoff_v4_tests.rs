//! Real signed-source content fixtures; public test keys grant no runtime authority.
use super::*;
use crate::{
    CompilerRefinedForwardingOutputErrorV1 as Failure,
    recover_compiler_native_semantic_handoff_v4 as admit,
};
use fe2o3_amd_target::{
    PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1 as WORKER_LAYOUT,
    PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1 as RUSTC_LAYOUT,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::*;
use fe2o3_compiler_lineage::*;
use fe2o3_kernel_opt::{
    RefinedForwardingHistoryRoleV1 as Role, materialize_refined_forwarding_history_v1,
    read_refined_forwarding_history_v1,
};
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, InvocationDigestV3, RustcInvocationDescriptorV2,
    RustcInvocationDescriptorV3, RustcUnitV2, encode_descriptor_v3,
};
use sha2::{Digest, Sha256};
use std::{ffi::OsString, mem::size_of, sync::Arc};

type Handoff = InertSemanticCompilerModuleHandoffV4;
const METADATA: usize = INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4;

fn coordinate(digest: [u8; 32], length: u64) -> TargetLineageIdentityV3 {
    TargetLineageIdentityV3::new(digest, length).unwrap()
}
macro_rules! identity {
    ($ty:ty, $bytes:expr) => {{
        let bytes: &[u8] = $bytes;
        let receipt = <$ty>::from_canonical_preimage(bytes).unwrap();
        coordinate(*receipt.identity().sha256(), receipt.identity().byte_len())
    }};
}

fn invocation(profile: Profile) -> RustcInvocationDescriptorV3 {
    let environment = CompileEnvironmentV2::from_child_environment(
        [
            ("CARGO_CFG_TARGET_ARCH", "amdgcn"),
            ("FE2O3_HSACO_DIR", "/workspace/output"),
            ("FE2O3_TARGET", profile.device_target()),
            ("FE2O3_VERIFY_KERNEL_IR", "1"),
        ]
        .map(|(k, v)| (OsString::from(k), OsString::from(v))),
    )
    .unwrap();
    let unit = RustcUnitV2::new(
        "/workspace",
        vec![
            "/opt/rustc".into(),
            "--crate-name=capsule_fixture".into(),
            "input.rs".into(),
            "--crate-type=lib".into(),
            "-Zcodegen-backend=/opt/backend.so".into(),
        ],
    )
    .unwrap();
    RustcInvocationDescriptorV3::new(
        RustcInvocationDescriptorV2::new([4; 32], [6; 32], unit, environment).unwrap(),
        CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap(),
    )
    .unwrap()
}

struct CapsuleFixture {
    invocation: RustcInvocationDescriptorV3,
    module: Native,
    carrier: Vec<u8>,
    fields: [Vec<u8>; 14],
    receipts: [Vec<u8>; 15],
}
impl CapsuleFixture {
    fn new(unit: bool, profile: Profile) -> Self {
        Self::with_source_profile(unit, profile, profile)
    }
    fn with_source_profile(unit: bool, profile: Profile, source_profile: Profile) -> Self {
        let layout = |profile: Profile| {
            derive_semantic_target_layout_identity_v1(
                profile.rustc_target(),
                RUSTC_LAYOUT,
                64,
                profile.cpu(),
                profile.rustc_features(),
            )
            .unwrap()
        };
        let source = source_for_stores_with_layout(
            &STORES,
            SemanticLayoutIdentityV1::from_sha256(layout(source_profile).sha256()),
        );
        let (packet, fields) = fixture_from_source(unit, profile, &STORES, source);
        let RecoveryInput::Carrier(carrier) = RecoveryInput::new(
            frame(&fields, if unit { Route::Erased } else { Route::Direct }),
            packet,
            true,
        ) else {
            unreachable!()
        };
        let module = Native::decode(&fields[1]).unwrap();
        let descriptor = Descriptor::decode(&fields[2]).unwrap();
        let invocation = invocation(profile);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget
            .reserve_storage(fields.iter().map(Vec::capacity).sum())
            .unwrap();
        let history_frame = read_refined_forwarding_history_v1(&fields[0], &mut budget).unwrap();
        budget
            .reserve_storage(history_frame.storage().retained_storage())
            .unwrap();
        let history =
            materialize_refined_forwarding_history_v1(&history_frame, &mut budget).unwrap();
        budget
            .reserve_storage(history.storage().retained_storage())
            .unwrap();
        let final_envelope = NativeNeutralModuleRefV1::decode(&fields[8]).unwrap();
        let (catalog, storage) =
            Catalog::decode_with_budget(final_envelope.catalog_bytes(), &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let relation = fe2o3_amdgcn_model::check_native_v12_text_descriptor_relation_v1(
            history.graph(Role::F),
            &catalog,
            history.graph(Role::F).canonical().canonical_bytes(),
            profile,
            descriptor.table(),
            std::str::from_utf8(module.module_bytes()).unwrap(),
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(relation.storage().retained_storage())
            .unwrap();
        let paired =
            read_native_refined_forwarding_carrier_v1(&carrier, LIMIT, |_| Ok::<_, ()>(()))
                .unwrap();
        let lowering = InertNativeLoweringAssociationV1::new(NativeLoweringAssociationInputsV1 {
            final_native: *final_envelope.subject(),
            carrier: coordinate(*paired.identity().sha256(), paired.identity().byte_len()),
            descriptor: coordinate(
                *descriptor.identity().sha256(),
                descriptor.identity().byte_len(),
            ),
            pre_descriptor_llvm: coordinate(
                Sha256::digest(relation.pre_descriptor_llvm().as_bytes()).into(),
                relation.pre_descriptor_llvm().len() as u64,
            ),
            final_llvm: coordinate(
                *module.module_identity().sha256(),
                module.module_identity().byte_len(),
            ),
            module_handoff: coordinate(*module.identity().sha256(), module.identity().byte_len()),
            profile,
        });
        let semantic = identity!(InertCanonicalSemanticMirReceiptV3, &fields[3]);
        let roster = Roster::decode(&fields[10]).unwrap();
        assert_eq!(roster.canonical_kernel_order(), &[1, 0]);
        let workgroups: Vec<_> = (0..roster.root_count())
            .map(|i| {
                let r = roster.root(i).unwrap();
                MultiRootTargetWorkgroupInputV3 {
                    kernel: r.export_symbol(),
                    workgroup: r.workgroup(),
                }
            })
            .collect();
        let bound = history.graph(Role::B).canonical().identity();
        let target = MultiRootTargetBindingTranscriptV3::new(MultiRootTargetBindingInputsV3 {
            protected_rustc_invocation: coordinate(
                InvocationDigestV3::calculate(&invocation)
                    .unwrap()
                    .into_bytes(),
                encode_descriptor_v3(&invocation).unwrap().len() as u64,
            ),
            semantic_mir: semantic,
            native_neutral_subject: *NativeNeutralModuleRefV1::decode(&fields[4])
                .unwrap()
                .subject(),
            target_bound_kir: coordinate(*bound.digest(), bound.canonical_length()),
            configured_target: profile.device_target(),
            rustc_llvm_target: profile.rustc_target(),
            target_cpu: profile.cpu(),
            target_features: profile.rustc_features(),
            roster_identity: roster.roster_identity(),
            code_object_version: 6,
            wave_width_bits: 64,
            workgroups: &workgroups,
        })
        .unwrap();
        let data = DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
            semantic_mir: semantic,
            target_binding: identity!(InertTargetBindingReceiptV3, target.canonical_bytes()),
            semantic_layout: layout(profile),
            rustc_llvm_target: profile.rustc_target(),
            live_rustc_data_layout: RUSTC_LAYOUT,
            final_llvm_target: profile.rustc_target(),
            final_llvm_data_layout: WORKER_LAYOUT,
            default_pointer_width_bits: 64,
        })
        .unwrap();
        let commitment = InertFinalCompilerModuleCommitmentV3::from_handoff(&module).unwrap();
        let mut receipts = [
            b"inert test inventory".to_vec(),
            b"inert test preflight".to_vec(),
            fields[3].clone(),
            fields[10].clone(),
            fields[4].clone(),
            fields[11].clone(),
            fields[7].clone(),
            fields[6].clone(),
            target.canonical_bytes().to_vec(),
            data.canonical_bytes().to_vec(),
            fields[2].clone(),
            module.symbol_manifest().canonical_bytes().to_vec(),
            lowering.canonical_bytes().to_vec(),
            vec![],
            commitment.canonical_bytes().to_vec(),
        ];
        receipts[13] =
            SemanticToLlvmAssociationTranscriptV3::new(SemanticToLlvmAssociationInputsV3 {
                semantic_mir: semantic,
                middle_end: identity!(InertMiddleEndReceiptV3, &receipts[3]),
                kernel_ir: identity!(InertKernelIrReceiptV3, &receipts[4]),
                mir_to_kir_correspondence: identity!(
                    InertMirToKirCorrespondenceReceiptV3,
                    &receipts[5]
                ),
                formal_memory: identity!(InertFormalMemoryReceiptV3, &receipts[6]),
                proof_binding: identity!(InertProofBindingReceiptV3, &receipts[7]),
                target_binding: identity!(InertTargetBindingReceiptV3, &receipts[8]),
                data_layout: identity!(InertDataLayoutReceiptV3, &receipts[9]),
                abi: identity!(InertAbiReceiptV3, &receipts[10]),
                export_manifest: identity!(InertExportManifestReceiptV3, &receipts[11]),
                amdgpu_lowering: identity!(InertAmdgpuLoweringReceiptV3, &receipts[12]),
                final_llvm: coordinate(
                    *module.module_identity().sha256(),
                    module.module_identity().byte_len(),
                ),
                final_compiler_module_commitment: identity!(
                    InertFinalCompilerModuleCommitmentReceiptV3,
                    &receipts[14]
                ),
            })
            .unwrap()
            .canonical_bytes()
            .to_vec();
        drop(relation);
        drop(history);
        drop(history_frame);
        Self {
            invocation,
            module,
            carrier,
            fields,
            receipts,
        }
    }
    fn wire(&self) -> Vec<u8> {
        macro_rules! receipt {
            ($t:ty,$i:expr) => {
                <$t>::from_canonical_preimage(self.receipts[$i].as_slice()).unwrap()
            };
        }
        let base = InertProductionSemanticCapsuleV3::new(
            self.invocation.clone(),
            self.module.target(),
            OrderedInertSemanticLineageReceiptsV3::new(
                receipt!(InertRustcIdentityInventoryReceiptV3, 0),
                receipt!(InertRustcPreflightPlanReceiptV3, 1),
                receipt!(InertCanonicalSemanticMirReceiptV3, 2),
                receipt!(InertMiddleEndReceiptV3, 3),
                receipt!(InertKernelIrReceiptV3, 4),
                receipt!(InertMirToKirCorrespondenceReceiptV3, 5),
                receipt!(InertFormalMemoryReceiptV3, 6),
                receipt!(InertProofBindingReceiptV3, 7),
                receipt!(InertTargetBindingReceiptV3, 8),
                receipt!(InertDataLayoutReceiptV3, 9),
                receipt!(InertAbiReceiptV3, 10),
                receipt!(InertExportManifestReceiptV3, 11),
                receipt!(InertAmdgpuLoweringReceiptV3, 12),
                receipt!(InertSemanticToLlvmReceiptV3, 13),
                receipt!(InertFinalCompilerModuleCommitmentReceiptV3, 14),
            ),
        )
        .unwrap();
        let inner = InertProductionSemanticCapsuleLayoutV4::new::<()>(
            base.canonical_bytes().len(),
            self.carrier.len(),
        )
        .unwrap();
        let mut capsule = vec![0; inner.encoded_len()];
        capsule[inner.base_range()].copy_from_slice(base.canonical_bytes());
        capsule[inner.carrier_range()].copy_from_slice(&self.carrier);
        seal_inert_production_semantic_capsule_v4(inner, &mut capsule, LIMIT, |_| Ok::<_, ()>(()))
            .unwrap();
        let capsule = InertProductionSemanticCapsuleV4::decode_owned(capsule).unwrap();
        let outer =
            preflight_inert_semantic_compiler_module_handoff_v4(&capsule, &self.module).unwrap();
        let mut wire = vec![0; outer.encoded_len()];
        wire[outer.capsule_range()].copy_from_slice(capsule.canonical_bytes());
        wire[outer.module_handoff_range()].copy_from_slice(self.module.canonical_bytes());
        seal_inert_semantic_compiler_module_handoff_v4(
            outer,
            &mut wire,
            capsule.identity(),
            self.module.identity(),
            |_| Ok::<_, ()>(()),
        )
        .unwrap();
        wire
    }
}

fn decode(wire: Vec<u8>, budget: &mut Budget<'_>) -> Handoff {
    budget.reserve_storage(wire.capacity() + METADATA).unwrap();
    budget
        .charge_work(inert_semantic_compiler_module_handoff_decode_work_v4(wire.len()).unwrap())
        .unwrap();
    Handoff::decode_owned(wire).unwrap()
}

#[test]
fn native_capsule_admission_v4_retains_signed_pair_and_shared_backing_on_both_profiles() {
    for unit in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let fixture = CapsuleFixture::new(unit, profile);
            let wire = fixture.wire();
            let len = wire.len();
            let mut backing = Vec::with_capacity(len + 1024);
            backing.extend([0x37; 37]);
            backing.extend(wire);
            backing.extend([0x53; 53]);
            let backing = Arc::new(backing);
            let weak = Arc::downgrade(&backing);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, LIMIT);
            let floor = backing.capacity() + METADATA + 37;
            budget.reserve_storage(floor).unwrap();
            budget
                .charge_work(inert_semantic_compiler_module_handoff_decode_work_v4(len).unwrap())
                .unwrap();
            let handoff = Handoff::decode_shared_vec(backing.clone(), 37..37 + len).unwrap();
            assert_eq!(handoff.backing_capacity(), backing.capacity());
            let pointer = handoff.canonical_bytes().as_ptr();
            let module_pointer = handoff.module_handoff().canonical_bytes().as_ptr();
            let (owner, storage) = admit(handoff, &mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_eq!(owner.storage(), storage);
            assert_eq!(owner.handoff().canonical_bytes().as_ptr(), pointer);
            assert_eq!(
                owner.handoff().module_handoff().canonical_bytes().as_ptr(),
                module_pointer
            );
            assert_eq!(
                owner.recovered().output().canonical().canonical_bytes(),
                NativeNeutralModuleRefV1::decode(&fixture.fields[8])
                    .unwrap()
                    .graph_bytes()
            );
            drop(fixture);
            drop(backing);
            assert!(weak.upgrade().is_some());
            assert_eq!(owner.recovered().output().module().kernels.len(), 2);
            assert!(!owner.authenticates_execution());
            assert!(!owner.authenticates_rustc_abi());
            assert!(!owner.grants_artifact_or_launch_authority());
            match owner.recovered().source_proof() {
                Original::Direct(p) => assert_eq!(p.root_count(), 2),
                Original::Erased(p) => assert_eq!(p.root_count(), 2),
            }
            drop(owner);
            assert!(weak.upgrade().is_none());
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}

fn rejection(fixture: &CapsuleFixture, expected: &'static str) {
    let error = rejected(fixture);
    assert!(
        matches!(error, Failure::Mismatch(actual) if actual == expected),
        "{error:?}"
    );
}

fn rejected(fixture: &CapsuleFixture) -> Failure {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let handoff = decode(fixture.wire(), &mut budget);
    let floor = budget.storage();
    let before = budget.work();
    let ledger = budget.work_ledger_identity_v1();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    let error = admit(handoff, &mut budget)
        .err()
        .expect("semantic rejection after valid outer decoding");
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > before);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    assert!(budget.work_ledger_identity_v1() == ledger);
    error
}

#[test]
fn native_capsule_admission_v4_rejects_resealed_original_and_final_receipt_substitutions() {
    let mut fixture = CapsuleFixture::new(false, Profile::Gfx942);
    for (slot, expected) in [
        (2, "V4 semantic MIR"),
        (3, "V4 original middle end"),
        (4, "V4 original N/catalog"),
        (5, "V4 original correspondence"),
        (6, "V4 original formal memory"),
        (7, "V4 original proof binding"),
        (10, "V4 descriptor ABI"),
        (11, "V4 exact export manifest"),
    ] {
        let original = fixture.receipts[slot].clone();
        fixture.receipts[slot] = match slot {
            4 => fixture.fields[8].clone(),
            6 => fixture.fields[9].clone(),
            _ => b"other well-framed receipt preimage".to_vec(),
        };
        assert_ne!(fixture.receipts[slot], original);
        rejection(&fixture, expected);
        fixture.receipts[slot] = original;
    }
}

#[test]
fn native_capsule_admission_v4_rejects_all_resealed_semantic_association_axes() {
    let mut fixture = CapsuleFixture::new(true, Profile::Gfx950);
    let original = SemanticToLlvmAssociationTranscriptV3::decode(&fixture.receipts[13]).unwrap();
    for (index, length_only) in (0..13).flat_map(|i| [(i, false), (i, true)]) {
        let mut input = original.inputs().unwrap();
        let axes = [
            &mut input.semantic_mir,
            &mut input.middle_end,
            &mut input.kernel_ir,
            &mut input.mir_to_kir_correspondence,
            &mut input.formal_memory,
            &mut input.proof_binding,
            &mut input.target_binding,
            &mut input.data_layout,
            &mut input.abi,
            &mut input.export_manifest,
            &mut input.amdgpu_lowering,
            &mut input.final_llvm,
            &mut input.final_compiler_module_commitment,
        ];
        *axes[index] = if length_only {
            coordinate(axes[index].sha256(), axes[index].byte_len() + 1)
        } else {
            coordinate([0x71; 32], axes[index].byte_len())
        };
        fixture.receipts[13] = SemanticToLlvmAssociationTranscriptV3::new(input)
            .unwrap()
            .canonical_bytes()
            .to_vec();
        rejection(&fixture, "V4 exact semantic-to-LLVM axis");
    }
}

#[test]
fn native_capsule_admission_v4_rejects_all_resealed_native_lowering_axes() {
    let mut fixture = CapsuleFixture::new(false, Profile::Gfx942);
    let original = InertNativeLoweringAssociationV1::decode(&fixture.receipts[12]).unwrap();
    for index in 0..7 {
        let mut input = original.inputs();
        match index {
            0 => {
                input.final_native = *NativeNeutralModuleRefV1::decode(&fixture.fields[4])
                    .unwrap()
                    .subject()
            }
            1 => input.carrier = coordinate([0x72; 32], 79),
            2 => input.descriptor = coordinate([0x72; 32], 79),
            3 => input.pre_descriptor_llvm = coordinate([0x72; 32], 79),
            4 => input.final_llvm = coordinate([0x72; 32], 79),
            5 => input.module_handoff = coordinate([0x72; 32], 79),
            6 => input.profile = Profile::Gfx950,
            _ => unreachable!(),
        }
        fixture.receipts[12] = InertNativeLoweringAssociationV1::new(input)
            .canonical_bytes()
            .to_vec();
        rejection(&fixture, "V4 exact final-F native lowering");
    }
}

#[test]
fn native_capsule_admission_v4_rejects_genuinely_signed_wrong_profile_layout() {
    for unit in [false, true] {
        for (profile, source_profile) in [
            (Profile::Gfx950, Profile::Gfx942),
            (Profile::Gfx942, Profile::Gfx950),
        ] {
            let fixture = CapsuleFixture::with_source_profile(unit, profile, source_profile);
            rejection(&fixture, "V4 exact semantic/LLVM22 layout");
        }
    }
}

#[test]
fn native_capsule_admission_v4_exact_and_one_short_replay_budgets_preserve_ledger() {
    for unit in [false, true] {
        let wire = CapsuleFixture::new(unit, Profile::Gfx942).wire();
        let run = |work_limit, storage_limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.charge_work(11).unwrap();
            budget.reserve_storage(37).unwrap();
            let handoff = decode(wire.clone(), &mut budget);
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            assert!(budget.reserve_storage(usize::MAX).is_err());
            assert!(budget.charge_work(usize::MAX).is_err());
            let result = admit(handoff, &mut budget).map(|(owner, storage)| {
                assert_eq!(owner.storage(), storage);
                storage
            });
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
            assert!(budget.work_ledger_identity_v1() == ledger);
            let measured = (result, budget.work(), budget.peak_storage());
            drop(budget);
            assert_eq!(work.failed_work(), Some(usize::MAX));
            measured
        };
        let (result, used, peak) = run(usize::MAX, LIMIT);
        let baseline = result.unwrap();
        let (exact, exact_work, exact_peak) = run(used, peak);
        assert_eq!(exact.unwrap(), baseline);
        assert_eq!((exact_work, exact_peak), (used, peak));
        let (short, accepted, _) = run(used - 1, peak);
        assert!(
            matches!(short, Err(Failure::Resource(Resource::Work(error))) if error.actual() == used && error.limit() == used - 1)
        );
        assert!(accepted < used);
        let (short, _, _) = run(used, peak - 1);
        assert!(
            matches!(short, Err(Failure::Resource(Resource::Storage(error))) if error.actual() == peak && error.limit() == peak - 1),
            "{short:?}"
        );
    }
}

#[test]
fn native_capsule_admission_v4_requires_full_enclosing_capacity_before_replay() {
    let wire = CapsuleFixture::new(false, Profile::Gfx942).wire();
    for case in 0..5 {
        let mut backing = Vec::with_capacity(wire.len() + 1024);
        backing.extend([0x31; 37]);
        backing.extend(&wire);
        backing.extend([0x53; 53]);
        let capacity = backing.capacity();
        let backing = Arc::new(backing);
        let mut decode_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut decode_budget = Budget::new(&mut decode_work, LIMIT);
        decode_budget.reserve_storage(capacity + METADATA).unwrap();
        decode_budget
            .charge_work(inert_semantic_compiler_module_handoff_decode_work_v4(wire.len()).unwrap())
            .unwrap();
        let handoff = Handoff::decode_shared_vec(backing.clone(), 37..37 + wire.len()).unwrap();
        let required = capacity + METADATA;
        let header = size_of::<crate::RecoveredCompilerNativeSemanticHandoffV4>()
            + size_of::<(&Handoff, NativeRefinedForwardingCarrierIdentityV1)>();
        let (floor, limit) = match case {
            0 => (wire.len() + METADATA, LIMIT),
            1 => (required - 1, LIMIT),
            2 => (required, required + header - 1),
            3 => (required, LIMIT + 1),
            4 => (backing.len() + METADATA, LIMIT),
            _ => unreachable!(),
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        let error = admit(handoff, &mut budget).err().unwrap();
        match case {
            0 | 1 | 4 => assert!(matches!(error, Failure::Resource(Resource::Accounting))),
            2 => assert!(
                matches!(error, Failure::Resource(Resource::Storage(denied)) if denied.limit() == limit && denied.actual() == floor + header)
            ),
            3 => assert!(matches!(error, Failure::Mismatch("bounded storage cap"))),
            _ => unreachable!(),
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.work(), 8);
        assert_eq!(
            budget.failed_storage(),
            if case == 2 {
                Some(floor + header)
            } else {
                None
            }
        );
        assert_eq!(&backing[..37], &[0x31; 37]);
        assert_eq!(&backing[37 + wire.len()..], &[0x53; 53]);
    }
}

#[test]
fn native_capsule_admission_v4_rejects_resealed_target_axes_and_root_permutations() {
    let mut fixture = CapsuleFixture::new(true, Profile::Gfx950);
    let target = MultiRootTargetBindingTranscriptV3::decode(&fixture.receipts[8]).unwrap();
    let correct_rows: Vec<_> = (0..target.root_count())
        .map(|i| {
            let row = target.workgroup(i).unwrap();
            MultiRootTargetWorkgroupInputV3 {
                kernel: row.kernel(),
                workgroup: row.workgroup(),
            }
        })
        .collect();
    for mutation in 0..10 {
        let mut rows = correct_rows.clone();
        if mutation == 6 {
            rows.swap(0, 1);
        }
        if mutation == 7 {
            rows[0].workgroup = [2, 1, 1];
        }
        if mutation == 8 {
            rows.pop();
        }
        if mutation == 9 {
            rows[0].kernel = "foreign_root";
        }
        let mut inputs = MultiRootTargetBindingInputsV3 {
            protected_rustc_invocation: target.protected_rustc_invocation(),
            semantic_mir: target.semantic_mir(),
            native_neutral_subject: *target.native_neutral_subject(),
            target_bound_kir: target.target_bound_kir(),
            configured_target: target.configured_target(),
            rustc_llvm_target: target.rustc_llvm_target(),
            target_cpu: target.target_cpu(),
            target_features: target.target_features(),
            roster_identity: target.roster_identity(),
            code_object_version: target.code_object_version(),
            wave_width_bits: target.wave_width_bits(),
            workgroups: &rows,
        };
        match mutation {
            0 => inputs.protected_rustc_invocation = coordinate([0x73; 32], 81),
            1 => inputs.semantic_mir = coordinate([0x73; 32], 81),
            2 => {
                inputs.native_neutral_subject =
                    *NativeNeutralModuleRefV1::decode(&fixture.fields[8])
                        .unwrap()
                        .subject()
            }
            3 => inputs.target_bound_kir = coordinate([0x73; 32], 81),
            4 => inputs.roster_identity = [0x73; 32],
            5 => {
                inputs.configured_target = Profile::Gfx942.device_target();
                inputs.target_cpu = Profile::Gfx942.cpu();
                inputs.target_features = Profile::Gfx942.rustc_features();
            }
            6..=9 => (),
            _ => unreachable!(),
        }
        fixture.receipts[8] = MultiRootTargetBindingTranscriptV3::new(inputs)
            .unwrap()
            .canonical_bytes()
            .to_vec();
        rejection(
            &fixture,
            if matches!(mutation, 6 | 7 | 9) {
                "V4 semantic-order target workgroups"
            } else {
                "V4 exact N/B target coordinates"
            },
        );
    }
}

#[test]
fn native_capsule_admission_v4_rejects_resealed_layout_coordinates_and_legacy_worker_layout() {
    let mut fixture = CapsuleFixture::new(false, Profile::Gfx942);
    let layout = DataLayoutTranscriptV3::decode(&fixture.receipts[9]).unwrap();
    for mutation in 0..4 {
        let mut inputs = layout.inputs().unwrap();
        match mutation {
            0 => inputs.semantic_mir = coordinate([0x74; 32], 83),
            1 => inputs.target_binding = coordinate([0x74; 32], 83),
            2 => inputs.semantic_layout = coordinate([0x74; 32], 83),
            3 => inputs.final_llvm_data_layout = RUSTC_LAYOUT,
            _ => unreachable!(),
        }
        fixture.receipts[9] = DataLayoutTranscriptV3::new(inputs)
            .unwrap()
            .canonical_bytes()
            .to_vec();
        rejection(&fixture, "V4 exact semantic/LLVM22 layout");
    }
}

#[test]
fn native_capsule_admission_v4_rejects_outer_native_swap_despite_matching_commitment() {
    let mut fixture = CapsuleFixture::new(false, Profile::Gfx942);
    let mut llvm = fixture.module.module_bytes().to_vec();
    llvm.extend(b"; independently framed foreign module\n");
    fixture.module = Native::new(
        CompilerModuleKindV1::LlvmTextIr,
        fixture.module.target(),
        kd::CodeObjectVersion::V6,
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(
            fixture.module.target(),
            kd::CodeObjectVersion::V6,
        )
        .unwrap(),
        CompilerModuleSymbolManifestV1::decode(fixture.module.symbol_manifest().canonical_bytes())
            .unwrap(),
        &llvm,
    )
    .unwrap();
    fixture.receipts[14] = InertFinalCompilerModuleCommitmentV3::from_handoff(&fixture.module)
        .unwrap()
        .canonical_bytes()
        .to_vec();
    rejection(&fixture, "V4 exact outer/embedded native module");
}

#[test]
fn native_capsule_admission_v4_refuses_entry_work_before_any_storage_change() {
    let wire = CapsuleFixture::new(false, Profile::Gfx942).wire();
    let prepayment = inert_semantic_compiler_module_handoff_decode_work_v4(wire.len()).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(prepayment + 7);
    let mut budget = Budget::new(&mut work, LIMIT);
    let handoff = decode(wire, &mut budget);
    let floor = budget.storage();
    let error = admit(handoff, &mut budget).err().unwrap();
    assert!(
        matches!(error, Failure::Resource(Resource::Work(denied)) if denied.actual() == prepayment + 8 && denied.limit() == prepayment + 7)
    );
    assert_eq!(budget.work(), prepayment);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor);
    assert_eq!(budget.failed_storage(), None);
    drop(budget);
    assert_eq!(work.failed_work(), Some(prepayment + 8));
}

#[test]
fn native_capsule_admission_v4_does_not_retry_a_valid_legacy_target_record() {
    let mut fixture = CapsuleFixture::new(false, Profile::Gfx942);
    let target = MultiRootTargetBindingTranscriptV3::decode(&fixture.receipts[8]).unwrap();
    let rows: Vec<_> = (0..target.root_count())
        .map(|i| {
            let row = target.workgroup(i).unwrap();
            MultiRootTargetWorkgroupInputV2 {
                kernel: row.kernel(),
                workgroup: row.workgroup(),
            }
        })
        .collect();
    let legacy = MultiRootTargetBindingTranscriptV2::new(MultiRootTargetBindingInputsV2 {
        protected_rustc_invocation: target.protected_rustc_invocation(),
        semantic_mir: target.semantic_mir(),
        target_neutral_kir: coordinate(
            *target.native_neutral_subject().graph_digest(),
            target.native_neutral_subject().graph_length(),
        ),
        target_bound_kir: target.target_bound_kir(),
        configured_target: target.configured_target(),
        rustc_llvm_target: target.rustc_llvm_target(),
        target_cpu: target.target_cpu(),
        target_features: target.target_features(),
        roster_identity: target.roster_identity(),
        code_object_version: 6,
        wave_width_bits: 64,
        workgroups: &rows,
    })
    .unwrap();
    MultiRootTargetBindingTranscriptV2::decode(legacy.canonical_bytes()).unwrap();
    fixture.receipts[8] = legacy.canonical_bytes().to_vec();
    assert!(matches!(rejected(&fixture), Failure::TargetLineage(_)));
}
