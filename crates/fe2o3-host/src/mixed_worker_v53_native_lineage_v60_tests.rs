//! Composed inert content checks, not protected execution or publication evidence.
use super::*;
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_amd_target::{
    PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1, PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
    ProductionAmdTargetProfileV1 as Profile,
};
use fe2o3_amdgcn_model::{
    NativeV12TextDescriptorReplayErrorV3 as TextError,
    NativeV18TextDescriptorReplayErrorV60 as NativeError,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::CodeObjectVersion;
use fe2o3_compiler_ffi::*;
use fe2o3_compiler_lineage::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18 as Owner;
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, InvocationDigestV3, RustcInvocationDescriptorV2,
    RustcInvocationDescriptorV3, RustcUnitV2, encode_descriptor_v3,
};
use fe2o3_verifier::{
    MixedNativeCorrespondenceErrorV60, MixedTargetSelectionSubjectV53,
    with_mixed_target_selection_v53,
};
use sha2::{Digest, Sha256};
use std::ffi::OsString;

#[path = "../../fe2o3-amdgcn-model/src/native_v18_replay_fixture_v60.rs"]
mod native_fixture;
use native_fixture::{FLOOR, LIMIT, SEMANTIC};

const SSA: [u8; 32] = [7; 32];
const WITNESS: &[u8] = b"inert prefix witness; no executed request is constructed";
const GENERATED: &[u8] = b"verus! { proof fn inert_transport_shape() {} }\n";

fn invocation(profile: Profile) -> RustcInvocationDescriptorV3 {
    let environment = CompileEnvironmentV2::from_child_environment(
        [
            ("CARGO_CFG_TARGET_ARCH", "amdgcn"),
            ("FE2O3_HSACO_DIR", "/workspace/output"),
            ("FE2O3_TARGET", profile.device_target()),
            ("FE2O3_VERIFY_KERNEL_IR", "1"),
        ]
        .map(|(key, value)| (OsString::from(key), OsString::from(value))),
    )
    .unwrap();
    let unit = RustcUnitV2::new(
        "/workspace",
        vec![
            "/opt/rustc".into(),
            "--crate-name=inert_host_content".into(),
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

fn part(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value);
}

// Public test key and openly claimed runtime identities. A strict signature
// binds this inert wire only; it neither runs Verus nor authenticates a runtime.
fn self_signed_receipt(owner: &Owner) -> Vec<u8> {
    let key = SigningKey::from_bytes(&[21; 32]);
    let source: [u8; 32] = Sha256::digest(SEMANTIC).into();
    let witness: [u8; 32] = Sha256::digest(WITNESS).into();
    let context = [17; 32];
    let mut generated = Sha256::new();
    generated.update(b"fe2o3-generated-verus-proof-input-v3\0");
    generated.update((GENERATED.len() as u64).to_le_bytes());
    generated.update(GENERATED);
    let generated: [u8; 32] = generated.finalize().into();
    let graph = (
        *owner.identity().digest(),
        owner.canonical_bytes().len() as u64,
    );
    let roots = owner.module().kernels.len() as u64;
    let census = [roots, roots, 0, 0, 0, 0];
    let mut statement = Sha256::new();
    for value in [
        b"FE2O3/ORIGINAL-MIR/POLICY11/LICM/STORE-CONSENSUS/TYPED/V50\0".as_slice(),
        &source,
        &SSA,
        &witness,
        &context,
    ] {
        part(&mut statement, value);
    }
    for _ in 0..4 {
        part(&mut statement, &graph.0);
        part(&mut statement, &graph.1.to_le_bytes());
    }
    for count in census {
        part(&mut statement, &count.to_le_bytes());
    }
    part(&mut statement, &generated);
    let statement: [u8; 32] = statement.finalize().into();
    let mut wire = b"FE2O3/MIXED/TYPED-SOURCE-TAIL/V50\0".to_vec();
    wire.extend_from_slice(&50u16.to_le_bytes());
    wire.extend_from_slice(&[4, 1]);
    wire.extend_from_slice(&11u16.to_le_bytes());
    wire.extend_from_slice(&source);
    wire.extend_from_slice(&SSA);
    for _ in 0..4 {
        wire.extend_from_slice(&graph.0);
        wire.extend_from_slice(&graph.1.to_le_bytes());
    }
    for digest in [statement, generated, witness, context] {
        wire.extend_from_slice(&digest);
    }
    wire.extend_from_slice(&(WITNESS.len() as u64).to_le_bytes());
    wire.extend_from_slice(&1u64.to_le_bytes());
    for count in census {
        wire.extend_from_slice(&count.to_le_bytes());
    }
    wire.extend_from_slice(&[18; 32]);
    for _ in 0..5 {
        wire.extend_from_slice(&[19; 32]);
    }
    wire.extend_from_slice(&[20; 32]);
    wire.extend_from_slice(&key.verifying_key().to_bytes());
    let signature = key.sign(&wire);
    wire.extend_from_slice(&signature.to_bytes());
    wire
}

macro_rules! content_identity {
    ($ty:ty, $bytes:expr) => {{
        let bytes: &[u8] = $bytes;
        let receipt = <$ty>::from_canonical_preimage(bytes).unwrap();
        TargetLineageIdentityV3::new(*receipt.identity().sha256(), receipt.identity().byte_len())
            .unwrap()
    }};
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    Signature,
    ProofInput(usize),
    ForwardedReceipt,
    NativeOpcode,
    DescriptorSection,
    Layout,
    Association(usize),
}

struct Fixture {
    owner: Owner,
    retained: usize,
    profile: Profile,
    descriptor: Vec<u8>,
    lowering: String,
}
impl Fixture {
    fn new(profile: Profile, roots: usize) -> Self {
        let (owner, retained) = native_fixture::owner(roots, "inert-host-native-lineage");
        let descriptor = native_fixture::descriptor(&owner, profile);
        let lowering = match profile {
            Profile::Gfx942 => fe2o3_amdgcn_model::lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
            Profile::Gfx950 => fe2o3_amdgcn_model::lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
        }.unwrap();
        Self {
            owner,
            retained,
            profile,
            descriptor,
            lowering,
        }
    }

    fn middle<'a>(&'a self, receipt: &'a [u8]) -> MixedMiddleEndInputV50<'a> {
        MixedMiddleEndInputV50 {
            semantic_mir: SEMANTIC,
            source_ssa_identity: &SSA,
            original: self.owner.canonical_bytes(),
            prefix: self.owner.canonical_bytes(),
            licm: self.owner.canonical_bytes(),
            forwarded: self.owner.canonical_bytes(),
            prefix_witness: WITNESS,
            generated_source: GENERATED,
            execution_receipt: receipt,
        }
    }

    fn outer(&self, mutation: Mutation) -> InertSemanticCompilerModuleHandoffV3 {
        let invocation = invocation(self.profile);
        let semantic =
            InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(SEMANTIC).unwrap();
        let subject = MixedTargetSelectionSubjectV53 {
            owner: &self.owner,
            invocation: TargetLineageIdentityV3::new(
                *InvocationDigestV3::calculate(&invocation)
                    .unwrap()
                    .as_bytes(),
                encode_descriptor_v3(&invocation).unwrap().len() as u64,
            )
            .unwrap(),
            semantic_mir: &semantic,
            descriptor: &self.descriptor,
            profile: self.profile,
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget
            .reserve_storage(self.retained + self.descriptor.capacity())
            .unwrap();
        let target =
            with_mixed_target_selection_v53(&subject, &mut budget, |bytes, _| Ok(bytes.to_vec()))
                .unwrap();
        let mut receipt = self_signed_receipt(&self.owner);
        if matches!(mutation, Mutation::Signature) {
            *receipt.last_mut().unwrap() ^= 1;
        }
        let input = self.middle(&receipt);
        let mut middle = vec![
            0;
            MixedMiddleEndLayoutV50::new::<()>(input)
                .unwrap()
                .encoded_len()
        ];
        encode_mixed_middle_end_v50(
            input,
            &mut middle,
            MIXED_MIDDLE_END_WORKING_STORAGE_V50,
            |_| Ok::<_, ()>(()),
        )
        .unwrap();
        let lowering = if matches!(mutation, Mutation::NativeOpcode) {
            let changed = self.lowering.replacen("ret void", "unreachable", 1);
            assert_ne!(changed, self.lowering);
            changed
        } else {
            self.lowering.clone()
        };
        let mut text = native_fixture::append_descriptor(&lowering, &self.descriptor);
        if matches!(mutation, Mutation::DescriptorSection) {
            text = text.replacen(".fe2o3.kd.v53", ".fe2o3.kd.v3", 1);
        }
        let target_id = DeviceTargetV1::parse(self.profile.device_target()).unwrap();
        let mut names = (0..self.owner.module().kernels.len())
            .map(|i| format!("kernel{i}"))
            .collect::<Vec<_>>();
        names.sort();
        let symbols = names
            .iter()
            .map(|name| format!("{name}.kd"))
            .collect::<Vec<_>>();
        let manifest = CompilerModuleSymbolManifestV1::new(
            names
                .iter()
                .map(|name| (CompilerModuleSymbolRoleV1::KernelEntry, name.as_str()))
                .chain(symbols.iter().map(|symbol| {
                    (
                        CompilerModuleSymbolRoleV1::KernelDescriptor,
                        symbol.as_str(),
                    )
                })),
        )
        .unwrap();
        let module = CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            target_id,
            CodeObjectVersion::V6,
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target_id, CodeObjectVersion::V6)
                .unwrap(),
            manifest,
            text.as_bytes(),
        )
        .unwrap();
        let layout = DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
            semantic_mir: content_identity!(InertCanonicalSemanticMirReceiptV3, SEMANTIC),
            target_binding: content_identity!(InertTargetBindingReceiptV3, &target),
            semantic_layout: derive_semantic_target_layout_identity_v1(
                self.profile.rustc_target(),
                PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
                64,
                self.profile.cpu(),
                self.profile.rustc_features(),
            )
            .unwrap(),
            rustc_llvm_target: self.profile.rustc_target(),
            live_rustc_data_layout: PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
            final_llvm_target: self.profile.rustc_target(),
            final_llvm_data_layout: if matches!(mutation, Mutation::Layout) {
                PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1
            } else {
                PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1
            },
            default_pointer_width_bits: 64,
        })
        .unwrap();
        let commitment = InertFinalCompilerModuleCommitmentV3::from_handoff(&module).unwrap();
        let mut rows = [
            b"inert inventory".to_vec(),
            b"inert preflight".to_vec(),
            SEMANTIC.to_vec(),
            middle,
            self.owner.canonical_bytes().to_vec(),
            b"inert original correspondence".to_vec(),
            self.descriptor.clone(),
            vec![],
            target,
            layout.canonical_bytes().to_vec(),
            self.descriptor.clone(),
            module.symbol_manifest().canonical_bytes().to_vec(),
            lowering.as_bytes().to_vec(),
            vec![],
            commitment.canonical_bytes().to_vec(),
        ];
        if matches!(mutation, Mutation::ForwardedReceipt) {
            rows[4][0] ^= 1;
        }
        let lineage = |identity: TargetLineageIdentityV3| {
            InertLineageContentIdentityV3::new(identity.sha256(), identity.byte_len()).unwrap()
        };
        let mut proof_inputs = [
            lineage(content_identity!(
                InertCanonicalSemanticMirReceiptV3,
                &rows[2]
            )),
            lineage(content_identity!(InertMiddleEndReceiptV3, &rows[3])),
            lineage(content_identity!(InertKernelIrReceiptV3, &rows[4])),
            lineage(content_identity!(
                InertMirToKirCorrespondenceReceiptV3,
                &rows[5]
            )),
            lineage(content_identity!(InertFormalMemoryReceiptV3, &rows[6])),
        ];
        if let Mutation::ProofInput(axis) = mutation {
            proof_inputs[axis] = InertLineageContentIdentityV3::new([99; 32], 1).unwrap();
        }
        let proof = InertProofBindingAssociationV4::new(
            InertProofBindingAssociationInputsV4::new(
                proof_inputs[0],
                proof_inputs[1],
                proof_inputs[2],
                proof_inputs[3],
                proof_inputs[4],
            ),
            &receipt,
        )
        .unwrap();
        rows[7] = proof.canonical_bytes().to_vec();
        let mut association = SemanticToLlvmAssociationInputsV3 {
            semantic_mir: content_identity!(InertCanonicalSemanticMirReceiptV3, &rows[2]),
            middle_end: content_identity!(InertMiddleEndReceiptV3, &rows[3]),
            kernel_ir: content_identity!(InertKernelIrReceiptV3, &rows[4]),
            mir_to_kir_correspondence: content_identity!(
                InertMirToKirCorrespondenceReceiptV3,
                &rows[5]
            ),
            formal_memory: content_identity!(InertFormalMemoryReceiptV3, &rows[6]),
            proof_binding: content_identity!(InertProofBindingReceiptV3, &rows[7]),
            target_binding: content_identity!(InertTargetBindingReceiptV3, &rows[8]),
            data_layout: content_identity!(InertDataLayoutReceiptV3, &rows[9]),
            abi: content_identity!(InertAbiReceiptV3, &rows[10]),
            export_manifest: content_identity!(InertExportManifestReceiptV3, &rows[11]),
            amdgpu_lowering: content_identity!(InertAmdgpuLoweringReceiptV3, &rows[12]),
            final_llvm: TargetLineageIdentityV3::new(
                *module.module_identity().sha256(),
                module.module_identity().byte_len(),
            )
            .unwrap(),
            final_compiler_module_commitment: content_identity!(
                InertFinalCompilerModuleCommitmentReceiptV3,
                &rows[14]
            ),
        };
        if let Mutation::Association(axis) = mutation {
            let axes = [
                &mut association.semantic_mir,
                &mut association.middle_end,
                &mut association.kernel_ir,
                &mut association.mir_to_kir_correspondence,
                &mut association.formal_memory,
                &mut association.proof_binding,
                &mut association.target_binding,
                &mut association.data_layout,
                &mut association.abi,
                &mut association.export_manifest,
                &mut association.amdgpu_lowering,
                &mut association.final_llvm,
                &mut association.final_compiler_module_commitment,
            ];
            *axes.into_iter().nth(axis).unwrap() =
                TargetLineageIdentityV3::new([211; 32], 7).unwrap();
        }
        rows[13] = SemanticToLlvmAssociationTranscriptV3::new(association)
            .unwrap()
            .canonical_bytes()
            .to_vec();
        macro_rules! receipt {
            ($ty:ty, $i:expr) => {
                <$ty>::from_canonical_preimage(rows[$i].as_slice()).unwrap()
            };
        }
        let capsule = InertProductionSemanticCapsuleV3::new(
            invocation,
            target_id,
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
        InertSemanticCompilerModuleHandoffV3::new(capsule, module).unwrap()
    }

    fn run(
        &self,
        outer: &InertSemanticCompilerModuleHandoffV3,
        profile: Profile,
        work: usize,
        storage: usize,
    ) -> (Result<()>, usize, usize, usize) {
        let table =
            decode_mixed_descriptor_v53(&self.descriptor, &mut |_| Ok::<_, ()>(())).unwrap();
        let floor = FLOOR
            + self.retained
            + self.descriptor.capacity()
            + self.lowering.capacity()
            + outer.canonical_bytes().len()
            + outer.capsule().canonical_bytes().len()
            + outer.module_handoff().canonical_bytes().len()
            + MIXED_DESCRIPTOR_READER_STORAGE_V53;
        let mut meter = CanonicalKernelIrWorkBudgetV1::new(work);
        let mut budget = Budget::new(&mut meter, storage);
        budget.reserve_storage(floor).unwrap();
        let result = validate_lineage(outer, &table, profile, &mut budget);
        assert_eq!(budget.storage(), floor);
        if let Err(error) = &result {
            if let Some(resource) = native_fixture::resource(error) {
                let before = (budget.work(), budget.storage(), budget.peak_storage());
                let repeated = validate_lineage(outer, &table, profile, &mut budget).unwrap_err();
                assert_eq!(native_fixture::resource(&repeated), Some(resource));
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    before
                );
            }
        }
        (
            result,
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
        )
    }
}

fn native_error(error: &AdmissionError) -> Option<&MixedNativeCorrespondenceErrorV60> {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(error) = current {
        if let Some(error) = error.downcast_ref::<MixedNativeCorrespondenceErrorV60>() {
            return Some(error);
        }
        current = error.source();
    }
    None
}

#[test]
fn mixed_v53_composed_lineage_accepts_exact_inert_content_on_both_profiles_and_rosters() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for roots in [1, 2] {
            let fixture = Fixture::new(profile, roots);
            let outer = fixture.outer(Mutation::None);
            fixture.run(&outer, profile, LIMIT, LIMIT).0.unwrap();
            assert!(!outer.authenticates_producer());
            assert!(!outer.authenticates_compiler_origin());
            assert!(!outer.grants_compiler_authority());
            assert!(!outer.grants_artifact_authority());
            assert!(!outer.grants_worker_authority());
            assert!(!outer.grants_link_authority());
            assert!(!outer.grants_publication_authority());
            assert!(!outer.grants_load_authority());
            assert!(!outer.grants_launch_authority());
            let receipt = self_signed_receipt(&fixture.owner);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let view = check_inert_typed_source_receipt_v53(fixture.middle(&receipt), &mut budget)
                .unwrap();
            assert_eq!(
                view.claimed_signer(),
                SigningKey::from_bytes(&[21; 32]).verifying_key().to_bytes()
            );
            assert!(!view.authenticates_execution());
            assert!(!view.grants_load_or_launch_authority());
        }
    }
}

#[test]
fn mixed_v53_composed_lineage_rejects_rebound_native_and_layout_content() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let fixture = Fixture::new(profile, 2);
        for mutation in [
            Mutation::NativeOpcode,
            Mutation::DescriptorSection,
            Mutation::Layout,
        ] {
            let outer = fixture.outer(mutation);
            let error = fixture.run(&outer, profile, LIMIT, LIMIT).0.unwrap_err();
            let exact_refusal = match mutation {
                Mutation::NativeOpcode => matches!(
                    native_error(&error),
                    Some(MixedNativeCorrespondenceErrorV60::Native(
                        NativeError::Binding("exact actual-owner lowering receipt")
                    ))
                ),
                Mutation::DescriptorSection => matches!(
                    native_error(&error),
                    Some(MixedNativeCorrespondenceErrorV60::Native(
                        NativeError::Text(TextError::Invalid("complete native text length"))
                    ))
                ),
                Mutation::Layout => matches!(
                    native_error(&error),
                    Some(MixedNativeCorrespondenceErrorV60::Binding(
                        "exact native layout transcript"
                    ))
                ),
                _ => unreachable!(),
            };
            assert!(
                exact_refusal,
                "{mutation:?}: must reach the actual production native check: {error:?}"
            );
        }
    }
}

#[test]
fn mixed_v53_composed_lineage_checks_every_native_association_after_prior_receipt_gates() {
    let fixture = Fixture::new(Profile::Gfx942, 2);
    for axis in 0..13 {
        let outer = fixture.outer(Mutation::Association(axis));
        let error = fixture
            .run(&outer, fixture.profile, LIMIT, LIMIT)
            .0
            .unwrap_err();
        assert!(
            matches!(
                native_error(&error),
                Some(MixedNativeCorrespondenceErrorV60::Binding(
                    "exact semantic-to-LLVM receipt axis"
                ))
            ),
            "axis {axis}: {error:?}"
        );
    }
}

#[test]
fn mixed_v53_composed_lineage_preserves_signature_proof_graph_and_profile_refusals() {
    let fixture = Fixture::new(Profile::Gfx942, 2);
    for mutation in [Mutation::Signature, Mutation::ForwardedReceipt]
        .into_iter()
        .chain((0..5).map(Mutation::ProofInput))
    {
        let outer = fixture.outer(mutation);
        let error = fixture
            .run(&outer, fixture.profile, LIMIT, LIMIT)
            .0
            .unwrap_err();
        assert!(
            native_error(&error).is_none(),
            "earlier gate must reject {mutation:?}"
        );
        if matches!(mutation, Mutation::Signature) {
            assert!(matches!(error, AdmissionError::MixedReceiptV53(_)));
        }
        if matches!(mutation, Mutation::ProofInput(_)) {
            assert!(matches!(
                error,
                AdmissionError::MixedV53("proof association differs from exact typed execution")
            ));
        }
    }
    let outer = fixture.outer(Mutation::None);
    let error = fixture
        .run(&outer, Profile::Gfx950, LIMIT, LIMIT)
        .0
        .unwrap_err();
    assert!(
        native_error(&error).is_none(),
        "selection must reject a foreign profile first"
    );
}

#[test]
fn mixed_v53_composed_lineage_keeps_exact_one_short_sticky_resources_and_caller_floor() {
    let fixture = Fixture::new(Profile::Gfx942, 2);
    let outer = fixture.outer(Mutation::None);
    let measured = fixture.run(&outer, fixture.profile, LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = fixture.run(&outer, fixture.profile, measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for work_short in [false, true] {
        let denied = fixture.run(
            &outer,
            fixture.profile,
            measured.1 - usize::from(work_short),
            measured.3 - usize::from(!work_short),
        );
        let error = denied.0.unwrap_err();
        match native_fixture::resource(&error).unwrap() {
            Resource::Work(limit) if work_short => {
                assert_eq!(limit.actual(), measured.1);
                assert_eq!(limit.limit(), measured.1 - 1);
            }
            Resource::Storage(limit) if !work_short => {
                assert_eq!(limit.actual(), measured.3);
                assert_eq!(limit.limit(), measured.3 - 1);
            }
            other => panic!("wrong composed lineage resource: {other:?}"),
        }
    }
}
