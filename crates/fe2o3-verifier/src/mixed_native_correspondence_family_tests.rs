// Inert content tests, not execution authority.
use super::*;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::*;
use fe2o3_compiler_lineage::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcInvocationDescriptorV3, RustcUnitV2,
};
use fixture::{FLOOR, LIMIT};
use std::ffi::OsString;

#[test]
fn mixed_native_correspondence_headers_cover_independent_transcript_and_scope_shapes() {
    #[allow(dead_code)]
    struct Capture<'a> {
        owner: &'a Owner,
        profile: Profile,
        descriptor: &'a [u8],
        outer: &'a InertSemanticCompilerModuleHandoffV3,
    }
    let expected = size_of::<Capture<'_>>()
        + size_of::<AssertUnwindSafe<&mut Budget<'_>>>()
        + size_of::<Budget<'_>>()
        + size_of::<[usize; 12]>()
        + size_of::<DataLayoutTranscriptV3>()
        + size_of::<SemanticToLlvmAssociationTranscriptV3>()
        + size_of::<DataLayoutTranscriptInputsV3<'_>>()
        + size_of::<SemanticToLlvmAssociationInputsV3>()
        + size_of::<[(Coordinate, Coordinate); 13]>()
        + size_of::<sha2::Sha256>()
        + size_of::<[Result<Coordinate, ProductionTargetLineageErrorV3>; 2]>()
        + size_of::<Result<DataLayoutTranscriptV3, ProductionTargetLineageErrorV3>>()
        + size_of::<Result<SemanticToLlvmAssociationTranscriptV3, ProductionTargetLineageErrorV3>>(
        )
        + size_of::<[Result<(), Error>; 2]>()
        + size_of::<std::thread::Result<Result<(), Error>>>();
    assert_eq!(headers(), expected);
}

#[test]
fn mixed_native_correspondence_codec_scratch_covers_private_rows_and_layout_preimage() {
    #[allow(dead_code)]
    struct Range {
        start: u32,
        end: u32,
    }
    #[allow(dead_code)]
    struct Parsed<'a> {
        bytes: &'a [u8],
        ranges: Vec<Range>,
    }
    let codec_headers = size_of::<[Vec<u8>; 3]>()
        + size_of::<[Vec<Range>; 2]>()
        + size_of::<[Vec<&[u8]>; 2]>()
        + size_of::<[Parsed<'_>; 2]>()
        + size_of::<[&[u8]; 6]>()
        + size_of::<Box<[u8]>>()
        + size_of::<Result<Box<[u8]>, ProductionTargetLineageErrorV3>>();
    assert_eq!(transcript_headers(), codec_headers);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let preimage = canonical_semantic_target_layout_transcript_v1(
            profile.rustc_target(),
            PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
            64,
            profile.cpu(),
            profile.rustc_features(),
        )
        .unwrap();
        assert_eq!(semantic_layout_bytes(profile).unwrap(), preimage.len());
        let fixture = Fixture::new(profile, 2);
        let outer = fixture.outer(|_| {});
        let receipts = outer.capsule().receipts();
        let layout = receipts.data_layout().canonical_preimage();
        let association = receipts.semantic_to_llvm().canonical_preimage();
        let layout_fields = usize::from(u16::from_le_bytes(layout[14..16].try_into().unwrap()));
        let association_fields =
            usize::from(u16::from_le_bytes(association[14..16].try_into().unwrap()));
        assert_eq!((layout_fields, association_fields), (10, 15));
        let expected = codec_headers
            + 2 * (layout.len() + association.len() + preimage.len())
            + 2 * (layout_fields + association_fields) * size_of::<Range>()
            + (layout_fields + association_fields) * size_of::<&[u8]>();
        assert_eq!(
            transcript_scratch(profile, layout.len(), association.len()).unwrap(),
            expected
        );
        assert!(matches!(
            transcript_scratch(profile, usize::MAX, association.len()),
            Err(Resource::Arithmetic)
        ));
    }
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
            "--crate-name=content_fixture".into(),
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

macro_rules! content_id {
    ($ty:ty, $bytes:expr) => {{
        let bytes: &[u8] = $bytes;
        let receipt = <$ty>::from_canonical_preimage(bytes).unwrap();
        Coordinate::new(*receipt.identity().sha256(), receipt.identity().byte_len()).unwrap()
    }};
}

struct Fixture {
    owner: Owner,
    retained: usize,
    profile: Profile,
    descriptor: Vec<u8>,
    prefix: String,
    text: String,
}
impl Fixture {
    fn new(profile: Profile, count: usize) -> Self {
        let (owner, retained) = fixture::owner(count, "mixed-native-content");
        let descriptor = fixture::descriptor(&owner, profile);
        let prefix = match profile {
            Profile::Gfx942 => fe2o3_amdgcn_model::lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
            Profile::Gfx950 => fe2o3_amdgcn_model::lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
        }.unwrap();
        let text = fixture::append_descriptor(&prefix, &descriptor);
        Self {
            owner,
            retained,
            profile,
            descriptor,
            prefix,
            text,
        }
    }

    fn outer(
        &self,
        mutate: impl FnOnce(&mut [Vec<u8>; 15]),
    ) -> InertSemanticCompilerModuleHandoffV3 {
        let target = DeviceTargetV1::parse(self.profile.device_target()).unwrap();
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
            target,
            CodeObjectVersion::V6,
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
                .unwrap(),
            manifest,
            self.text.as_bytes(),
        )
        .unwrap();
        let semantic = content_id!(InertCanonicalSemanticMirReceiptV3, fixture::SEMANTIC);
        let target_bytes = b"inert target transcript component, independently checked by host";
        let layout = DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
            semantic_mir: semantic,
            target_binding: content_id!(InertTargetBindingReceiptV3, target_bytes),
            semantic_layout: derive_semantic_target_layout_identity_v1(
                self.profile.rustc_target(),
                fe2o3_amd_target::PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
                64,
                self.profile.cpu(),
                self.profile.rustc_features(),
            )
            .unwrap(),
            rustc_llvm_target: self.profile.rustc_target(),
            live_rustc_data_layout: fe2o3_amd_target::PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
            final_llvm_target: self.profile.rustc_target(),
            final_llvm_data_layout: PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
            default_pointer_width_bits: 64,
        })
        .unwrap();
        let commitment = InertFinalCompilerModuleCommitmentV3::from_handoff(&module).unwrap();
        let mut receipts = [
            b"inert inventory".to_vec(),
            b"inert preflight".to_vec(),
            fixture::SEMANTIC.to_vec(),
            b"inert middle".to_vec(),
            self.owner.canonical_bytes().to_vec(),
            b"inert correspondence".to_vec(),
            self.descriptor.clone(),
            b"inert proof association".to_vec(),
            target_bytes.to_vec(),
            layout.canonical_bytes().to_vec(),
            self.descriptor.clone(),
            module.symbol_manifest().canonical_bytes().to_vec(),
            self.prefix.as_bytes().to_vec(),
            vec![],
            commitment.canonical_bytes().to_vec(),
        ];
        receipts[13] =
            SemanticToLlvmAssociationTranscriptV3::new(SemanticToLlvmAssociationInputsV3 {
                semantic_mir: semantic,
                middle_end: content_id!(InertMiddleEndReceiptV3, &receipts[3]),
                kernel_ir: content_id!(InertKernelIrReceiptV3, &receipts[4]),
                mir_to_kir_correspondence: content_id!(
                    InertMirToKirCorrespondenceReceiptV3,
                    &receipts[5]
                ),
                formal_memory: content_id!(InertFormalMemoryReceiptV3, &receipts[6]),
                proof_binding: content_id!(InertProofBindingReceiptV3, &receipts[7]),
                target_binding: content_id!(InertTargetBindingReceiptV3, &receipts[8]),
                data_layout: content_id!(InertDataLayoutReceiptV3, &receipts[9]),
                abi: content_id!(InertAbiReceiptV3, &receipts[10]),
                export_manifest: content_id!(InertExportManifestReceiptV3, &receipts[11]),
                amdgpu_lowering: content_id!(InertAmdgpuLoweringReceiptV3, &receipts[12]),
                final_llvm: Coordinate::new(
                    *module.module_identity().sha256(),
                    module.module_identity().byte_len(),
                )
                .unwrap(),
                final_compiler_module_commitment: content_id!(
                    InertFinalCompilerModuleCommitmentReceiptV3,
                    &receipts[14]
                ),
            })
            .unwrap()
            .canonical_bytes()
            .to_vec();
        mutate(&mut receipts);
        macro_rules! receipt {
            ($ty:ty, $i:expr) => {
                <$ty>::from_canonical_preimage(receipts[$i].as_slice()).unwrap()
            };
        }
        let capsule = InertProductionSemanticCapsuleV3::new(
            invocation(self.profile),
            target,
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
        work: usize,
        storage: usize,
    ) -> (Result<(), Error>, usize, usize, usize) {
        let mut meter = Work::new(work);
        let mut budget = Budget::new(&mut meter, storage);
        let floor = FLOOR
            + self.retained
            + self.descriptor.capacity()
            + self.prefix.capacity()
            + self.text.capacity()
            + outer.canonical_bytes().len()
            + outer.capsule().canonical_bytes().len()
            + outer.module_handoff().canonical_bytes().len();
        budget.reserve_storage(floor).unwrap();
        let result = check_current(
            &self.owner,
            self.profile,
            &self.descriptor,
            outer,
            &mut budget,
        );
        assert_eq!(budget.storage(), floor);
        (
            result,
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
        )
    }
}

#[test]
fn mixed_native_correspondence_checks_actual_multi_root_text_without_granting_authority() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for count in [1, 2] {
            let fixture = Fixture::new(profile, count);
            let outer = fixture.outer(|_| {});
            fixture.run(&outer, LIMIT, LIMIT).0.unwrap();
            assert!(!outer.authenticates_producer());
            assert!(!outer.authenticates_compiler_origin());
            assert!(!outer.grants_compiler_authority());
            assert!(!outer.grants_artifact_authority());
            assert!(!outer.grants_worker_authority());
            assert!(!outer.grants_link_authority());
            assert!(!outer.grants_publication_authority());
            assert!(!outer.grants_load_authority());
            assert!(!outer.grants_launch_authority());
        }
    }
}

#[test]
fn mixed_native_correspondence_refuses_each_of_thirteen_well_formed_receipt_axes() {
    let fixture = Fixture::new(Profile::Gfx942, 2);
    for axis in 0..13 {
        let outer = fixture.outer(|receipts| {
            let original = SemanticToLlvmAssociationTranscriptV3::decode(&receipts[13]).unwrap();
            let mut input = original.inputs().unwrap();
            let fields = [
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
            *fields.into_iter().nth(axis).unwrap() = Coordinate::new([211; 32], 7).unwrap();
            receipts[13] = SemanticToLlvmAssociationTranscriptV3::new(input)
                .unwrap()
                .canonical_bytes()
                .to_vec();
        });
        assert!(
            matches!(
                fixture.run(&outer, LIMIT, LIMIT).0,
                Err(Error::Binding("exact semantic-to-LLVM receipt axis"))
            ),
            "axis {axis}"
        );
    }
}

#[test]
fn mixed_native_correspondence_refuses_layout_claims_and_lowering_or_graph_substitution() {
    let fixture = Fixture::new(Profile::Gfx942, 1);
    for field in 0..4 {
        let outer = fixture.outer(|receipts| {
            let original = DataLayoutTranscriptV3::decode(&receipts[9]).unwrap();
            let mut input = original.inputs().unwrap();
            match field {
                0 => input.semantic_mir = Coordinate::new([211; 32], 7).unwrap(),
                1 => input.target_binding = Coordinate::new([211; 32], 7).unwrap(),
                2 => input.semantic_layout = Coordinate::new([211; 32], 7).unwrap(),
                _ => {
                    input.final_llvm_data_layout =
                        fe2o3_amd_target::PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1
                }
            }
            receipts[9] = DataLayoutTranscriptV3::new(input)
                .unwrap()
                .canonical_bytes()
                .to_vec();
        });
        assert!(matches!(
            fixture.run(&outer, LIMIT, LIMIT).0,
            Err(Error::Binding("exact native layout transcript"))
        ));
    }
    for receipt in [4, 6, 10, 12] {
        let outer = fixture.outer(|rows| {
            rows[receipt][0] ^= 1;
        });
        assert!(
            fixture.run(&outer, LIMIT, LIMIT).0.is_err(),
            "receipt {receipt}"
        );
    }
    let mut altered = Fixture::new(Profile::Gfx942, 1);
    altered.text = altered.text.replacen("ret void", "unreachable", 1);
    assert!(matches!(
        altered.run(&altered.outer(|_| {}), LIMIT, LIMIT).0,
        Err(Error::Native(_))
    ));
}

#[test]
fn mixed_native_correspondence_preserves_exact_resource_errors_and_caller_floor() {
    let fixture = Fixture::new(Profile::Gfx942, 2);
    let outer = fixture.outer(|_| {});
    let measured = fixture.run(&outer, LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = fixture.run(&outer, measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for work_short in [false, true] {
        let result = fixture.run(
            &outer,
            measured.1 - usize::from(work_short),
            measured.3 - usize::from(!work_short),
        );
        let error = result.0.unwrap_err();
        match fixture::resource(&error).unwrap() {
            Resource::Work(limit) if work_short => {
                assert_eq!(limit.actual(), measured.1);
                assert_eq!(limit.limit(), measured.1 - 1);
            }
            Resource::Storage(limit) if !work_short => {
                assert_eq!(limit.actual(), measured.3);
                assert_eq!(limit.limit(), measured.3 - 1);
            }
            other => panic!("wrong correspondence resource: {other:?}"),
        }
    }
}
