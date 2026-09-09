use std::ffi::OsString;

use ed25519_dalek::{Signer as _, SigningKey};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_lineage::{
    InertAbiReceiptV3, InertAmdgpuLoweringReceiptV3, InertCanonicalKernelIrV13ReceiptV5,
    InertCanonicalSemanticMirReceiptV3, InertCapabilityRefinementReceiptKindV1,
    InertCapabilityRefinementReceiptV1, InertCompilerProofOwnerInputsV5, InertCompilerProofOwnerV5,
    InertDataLayoutReceiptV3, InertExportManifestReceiptV3,
    InertFinalCompilerModuleCommitmentReceiptV3, InertFormalMemoryReceiptV3,
    InertKernelIrReceiptV3, InertLineageContentIdentityV3, InertMiddleEndReceiptV3,
    InertMirToKirCorrespondenceReceiptV3, InertMultiRootProofLineageV3,
    InertMultiRootStaticCapabilityEvidenceAssociationV1, InertProductionSemanticCapsuleV3,
    InertProofBindingAssociationInputsV4, InertProofBindingAssociationV4,
    InertProofBindingReceiptV3, InertRustcIdentityInventoryReceiptV3,
    InertRustcPreflightPlanReceiptV3, InertSemanticToLlvmReceiptV3,
    InertStaticCapabilityEvidenceAssociationErrorV1,
    InertStaticCapabilityEvidenceAssociationInputsV1, InertStaticCapabilityEvidenceAssociationV1,
    InertTargetBindingReceiptV3, MachineRefinementContentIdentityV1,
    MultiRootCanonicalKirVersionV3, MultiRootNeutralKirIdentityV3, MultiRootProofRosterInputsV3,
    MultiRootProofRosterKindV3, MultiRootProofRosterRootInputV3, MultiRootProofRosterTranscriptV3,
    OrderedInertSemanticLineageReceiptsV3, TargetMachineRefinementReceiptPartsV1,
    TargetMachineRefinementReceiptV1, TargetMachineRefinementTargetV1,
    decode_compiler_instruction_selection_correspondence_identity_v1,
    decode_post_llvm_stage_custody_identity_v1,
};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2, FunctionalRefinementResultV2, SafeReferenceKindV2,
    UnsignedFunctionalRefinementReceiptV2, VerusToolchainIdentityV2,
};
use fe2o3_kernel_descriptor::DeviceTargetV1;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, DebugSourceMapDocumentV2, DebugSourceMapFileV1, DebugSourceMapSpanV1,
    Function, Kernel, KernelContextSourceIdentityV1, KernelContextTypeV1, LaunchDomain,
    LaunchExtent, Module, Operation, PreparedSimulationBundleV8, SemanticAggregateStorageMapV8,
    SemanticKernelStorageV1, SemanticKernelStorageV2, SemanticStorageMapV8, Signature,
    SimulationBundleIdentityV8, SimulationProductionKirIdentityV8, SimulationSourceLineageV1,
    Terminator, ValueId, VerifiedCanonicalKernelIrV13, VerifiedSimulationBundleV8,
};
use fe2o3_lower_mir_kernel::{
    InertCanonicalMirToKirCorrespondenceEvidenceV4, ProductionSemanticKirLimitsV1,
    ProductionSemanticKirOwnerV1, ProductionSourceRefinementEvidenceV1,
};
use fe2o3_mir_model::semantic_mir_v1 as sm;
use fe2o3_pliron::InertProductionMiddleEndEvidenceV5;
use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1};
use fe2o3_proof_contracts::{
    ArtifactIdentityV1, CapabilityDiagnosticIdV1, CapabilityObligationSpecV1, CapabilityOutcomeV1,
    CapabilityPropertyIdV1, CapabilityResultSpecV1, CapabilitySubjectFieldV1, CapabilitySubjectV1,
    DigestV1, EvidenceIdentityV1, ExactToolIdentityV1, ExecutableKirIdentityV1,
    InertCapabilityObligationSetIdentityV1, InertCapabilityObligationSetV1,
    InertCapabilityResultSetV1, KernelIdentityV1, KernelRootIdentityV1, LaunchContractIdentityV1,
    StatementIdentityV1, TargetModelIdentityV1,
};
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcInvocationDescriptorV3, RustcUnitV2,
};
use fe2o3_verifier::{
    CanonicalProductionMirPlironVerusExecutionEvidenceV1,
    CompilerCapabilityEvidenceValidationErrorV1, CompilerProofInputValidationErrorV3,
    CompilerProofInputValidationErrorV4, CompilerProofInputValidationErrorV5,
    ProductionMirPlironVerusExecutionClaimsV1, ValidatedCompilerCapabilityEvidenceV1,
    ValidatedCompilerProofInputsV4, validate_compiler_capability_evidence_v1,
    validate_compiler_multi_root_proof_inputs_v5, validate_compiler_proof_inputs_v4,
    validate_compiler_proof_inputs_v5,
};
use sha2::{Digest as _, Sha256};

#[path = "../../../tests/support/compiler_proof_inputs_v3.rs"]
mod compiler_proof_inputs_v3;
use compiler_proof_inputs_v3::{
    CanonicalCompilerProofInputsV3, current_compiler_proof_inputs_v4_from_frozen_source,
    frozen_compiler_proof_inputs_v4,
};

const TARGET: &str = "gfx942:sramecc+:xnack-";
const EPOCH: u64 = 7;
const KERNEL_IDENTITY: [u8; 32] = [21; 32];
const ROOT_IDENTITY: [u8; 32] = [3; 32];
const TARGET_MODEL_IDENTITY: [u8; 32] = [24; 32];
const LAUNCH_CONTRACT_IDENTITY: [u8; 32] = [25; 32];

struct Fixture {
    capsule: InertProductionSemanticCapsuleV3,
    executable_kir_receipt: InertCanonicalKernelIrV13ReceiptV5,
    association: InertStaticCapabilityEvidenceAssociationV1,
    subject: CapabilitySubjectV1,
    obligation_set: InertCapabilityObligationSetIdentityV1,
    source: Option<InertCapabilityRefinementReceiptV1>,
    machine: Option<InertCapabilityRefinementReceiptV1>,
    bundle_identity: SimulationBundleIdentityV8,
    bundle_epoch: u64,
}

fn digest(seed: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([seed; 32])
}

fn lineage(sha256: &[u8; 32], byte_len: u64) -> InertLineageContentIdentityV3 {
    InertLineageContentIdentityV3::new(*sha256, byte_len).unwrap()
}

fn signed_v4_source_evidence(
    pliron_identity: DigestV1,
) -> CanonicalProductionMirPlironVerusExecutionEvidenceV1 {
    let binding = FunctionalRefinementBindingV2::new(
        SafeReferenceKindV2::SourceAndMir,
        digest(101),
        digest(102),
        digest(103),
        digest(104),
        digest(105),
        digest(106),
    )
    .unwrap();
    let toolchain = VerusToolchainIdentityV2::new(
        digest(107),
        digest(108),
        digest(109),
        digest(110),
        digest(111),
    )
    .unwrap();
    let signing = SigningKey::from_bytes(&[42; 32]);
    let verifying_key = signing.verifying_key().to_bytes();
    let policy = FunctionalRefinementImportPolicyV2::new(
        verifying_key,
        toolchain,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
    )
    .unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        digest(112),
        FunctionalRefinementResultV2::Proved,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
    )
    .unwrap();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
    let imported = importer
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    let claims = ProductionMirPlironVerusExecutionClaimsV1::new(
        digest(113),
        digest(114),
        pliron_identity,
        digest(115),
        digest(116),
        binding,
        imported.signer_identity(),
        toolchain,
        imported.execution_identity(),
        imported.receipt_identity().digest(),
        3,
    )
    .unwrap();
    CanonicalProductionMirPlironVerusExecutionEvidenceV1::new(claims, verifying_key, wire).unwrap()
}

fn validated_v4_source_owner() -> ValidatedCompilerProofInputsV4 {
    validate_v4_source_owner(&current_compiler_proof_inputs_v4_from_frozen_source()).unwrap()
}

fn validate_v4_source_owner(
    canonical: &CanonicalCompilerProofInputsV3,
) -> Result<ValidatedCompilerProofInputsV4, CompilerProofInputValidationErrorV4> {
    let semantic_mir =
        InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(canonical.semantic_mir())
            .unwrap();
    let middle_end =
        InertMiddleEndReceiptV3::from_canonical_preimage(canonical.middle_end()).unwrap();
    let kernel_ir = InertKernelIrReceiptV3::from_canonical_preimage(canonical.kernel_ir()).unwrap();
    let correspondence =
        InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(canonical.correspondence())
            .unwrap();
    let formal_memory =
        InertFormalMemoryReceiptV3::from_canonical_preimage(canonical.formal_memory()).unwrap();
    let middle =
        InertProductionMiddleEndEvidenceV5::decode(middle_end.canonical_preimage()).unwrap();
    let evidence =
        signed_v4_source_evidence(DigestV1::from_untrusted_bytes(*middle.identity().sha256()));
    let association = InertProofBindingAssociationV4::new(
        InertProofBindingAssociationInputsV4::new(
            lineage(
                semantic_mir.identity().sha256(),
                semantic_mir.identity().byte_len(),
            ),
            lineage(
                middle_end.identity().sha256(),
                middle_end.identity().byte_len(),
            ),
            lineage(
                kernel_ir.identity().sha256(),
                kernel_ir.identity().byte_len(),
            ),
            lineage(
                correspondence.identity().sha256(),
                correspondence.identity().byte_len(),
            ),
            lineage(
                formal_memory.identity().sha256(),
                formal_memory.identity().byte_len(),
            ),
        ),
        evidence.canonical_bytes(),
    )
    .unwrap();
    let proof =
        InertProofBindingReceiptV3::from_canonical_preimage(association.canonical_bytes()).unwrap();
    validate_compiler_proof_inputs_v4(
        &proof,
        &semantic_mir,
        &middle_end,
        &kernel_ir,
        &correspondence,
        &formal_memory,
    )
}

fn os_entries(entries: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    entries
        .iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        .collect()
}

fn invocation(seed: u8) -> RustcInvocationDescriptorV3 {
    let closure = CompilerClosureV2::new(
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
        [seed.wrapping_add(3); 32],
        [seed.wrapping_add(4); 32],
        [seed.wrapping_add(5); 32],
        [seed.wrapping_add(6); 32],
    )
    .unwrap();
    let rustc = RustcUnitV2::new(
        "/workspace/fe2o3",
        vec![
            "/opt/fe2o3/rustc".into(),
            "--crate-name".into(),
            "capability_fixture".into(),
            "src/lib.rs".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
        ],
    )
    .unwrap();
    let environment = CompileEnvironmentV2::from_child_environment(os_entries(&[
        ("CARGO_CFG_TARGET_ARCH", "amdgcn"),
        ("FE2O3_HSACO_DIR", "/workspace/fe2o3/target/fe2o3"),
        ("FE2O3_TARGET", TARGET),
        ("FE2O3_VERIFY_KERNEL_IR", "1"),
    ]))
    .unwrap();
    let v2 = RustcInvocationDescriptorV2::new(
        [seed.wrapping_add(4); 32],
        [seed.wrapping_add(6); 32],
        rustc,
        environment,
    )
    .unwrap();
    RustcInvocationDescriptorV3::new(v2, closure).unwrap()
}

fn executable_kir(
    kernel: [u8; 32],
    target: [u8; 32],
    launch: [u8; 32],
) -> VerifiedCanonicalKernelIrV13 {
    let context = KernelContextTypeV1::new("entry", kernel, target, launch);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::kernel_context_issue(
        ValueId(0),
        context,
        KernelContextSourceIdentityV1::new([1; 32], [2; 32], [3; 32], [4; 32]),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry =
        Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
    let mut module = Module::new("capability-verifier-fixture");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    VerifiedCanonicalKernelIrV13::from_module(module).unwrap()
}

type MultiRootSubject = (u8, [u8; 32], [u8; 32], [u8; 32]);

fn multi_root_executable_kir(subjects: &[MultiRootSubject]) -> VerifiedCanonicalKernelIrV13 {
    let mut module = Module::new("multi-root-capability-verifier-fixture");
    for (ordinal, kernel, root, launch) in subjects {
        let name = format!("kernel_{ordinal}");
        let context =
            KernelContextTypeV1::new(name.as_str(), *kernel, TARGET_MODEL_IDENTITY, *launch);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::kernel_context_issue(
            ValueId(0),
            context,
            KernelContextSourceIdentityV1::new(
                [100 + ordinal; 32],
                [110 + ordinal; 32],
                *root,
                [120 + ordinal; 32],
            ),
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            name.as_str(),
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            name.as_str(),
            name.as_str(),
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
    }
    VerifiedCanonicalKernelIrV13::from_module(module).unwrap()
}

struct ExactV13BundleCustody {
    kernel_ir: VerifiedCanonicalKernelIrV13,
    bundle_identity: SimulationBundleIdentityV8,
    graph_epoch: u64,
}

fn exact_v13_bundle_custody(kernel_ir: VerifiedCanonicalKernelIrV13) -> ExactV13BundleCustody {
    let kir_digest = *kernel_ir.identity().digest();
    let kir_length = kernel_ir.identity().canonical_length();
    let (_, module) = VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(
        kernel_ir.canonical_bytes().to_vec(),
    )
    .unwrap();
    let kernel_count = u32::try_from(module.kernels.len()).unwrap();
    let prepared = PreparedSimulationBundleV8::new(
        SimulationSourceLineageV1::new([0x31; 32], 401, [0x32; 32], 402).unwrap(),
        SimulationProductionKirIdentityV8::new(13, kir_digest, kir_length).unwrap(),
        EPOCH,
        TARGET,
        kernel_ir,
    )
    .unwrap();
    let source_map = DebugSourceMapDocumentV2::new(
        prepared.debug_source_map_binding(),
        vec![DebugSourceMapFileV1::new([0x33; 32], 16, "capability-fixture.rs".into()).unwrap()],
        vec![],
        vec![DebugSourceMapSpanV1::new([0x33; 32], 1, 2, 1, 2).unwrap()],
        vec![],
        vec![],
    )
    .unwrap();
    let semantic_mir = b"fe2o3-verifier/exact-v13-bundle-v8".to_vec();
    let semantic_mir_sha256 = <[u8; 32]>::from(Sha256::digest(&semantic_mir));
    let storage = SemanticStorageMapV8::new(
        *prepared.subject_identity(),
        1,
        semantic_mir_sha256,
        semantic_mir.len() as u64,
        [0x34; 32],
        *prepared.canonical_kir_v13_digest(),
        prepared.canonical_kir_v13_length(),
        (0..kernel_count)
            .map(|ordinal| SemanticKernelStorageV1::new(ordinal, ordinal, ordinal, vec![]))
            .collect(),
        vec![],
    )
    .unwrap();
    let aggregate = SemanticAggregateStorageMapV8::new(
        *prepared.subject_identity(),
        *prepared.canonical_kir_v13_digest(),
        prepared.canonical_kir_v13_length(),
        (0..kernel_count)
            .map(|ordinal| SemanticKernelStorageV2::new(ordinal, ordinal, ordinal, 0, 1, vec![]))
            .collect(),
    )
    .unwrap();
    let bundle = prepared
        .finalize(source_map, semantic_mir, storage, aggregate)
        .unwrap();
    let bundle_identity = bundle.identity();
    let decoded =
        VerifiedSimulationBundleV8::from_canonical_bytes(bundle.into_canonical_bytes()).unwrap();
    assert_eq!(decoded.identity(), bundle_identity);
    assert_eq!(decoded.production_kir_identity().version(), 13);
    assert_eq!(decoded.production_kir_identity().digest(), kir_digest);
    assert_eq!(
        decoded.production_kir_identity().canonical_length(),
        kir_length
    );
    assert_eq!(decoded.final_graph_epoch(), EPOCH);
    let kernel_ir =
        VerifiedCanonicalKernelIrV13::from_canonical_bytes(decoded.canonical_kir_v13().to_vec())
            .unwrap();
    assert_eq!(kernel_ir.identity().digest(), &kir_digest);
    assert_eq!(kernel_ir.identity().canonical_length(), kir_length);
    ExactV13BundleCustody {
        kernel_ir,
        bundle_identity,
        graph_epoch: decoded.final_graph_epoch(),
    }
}

fn opaque(label: &str) -> Vec<u8> {
    format!("fe2o3-capability/{label}").into_bytes()
}

fn machine_refinement_receipt(seed: u8) -> InertCapabilityRefinementReceiptV1 {
    let receipt =
        TargetMachineRefinementReceiptV1::from_parts(TargetMachineRefinementReceiptPartsV1 {
            target: TargetMachineRefinementTargetV1::Gfx942,
            post_llvm_custody: decode_post_llvm_stage_custody_identity_v1(
                [seed; 32],
                u64::from(seed) + 1,
            )
            .unwrap(),
            instruction_selection:
                decode_compiler_instruction_selection_correspondence_identity_v1(
                    [seed.wrapping_add(1); 32],
                    u64::from(seed) + 2,
                )
                .unwrap(),
            decoded_isa: MachineRefinementContentIdentityV1::new(
                [seed.wrapping_add(2); 32],
                u64::from(seed) + 3,
            )
            .unwrap(),
            final_code_object:
                fe2o3_compiler_lineage::ExactCompilerStageContentIdentityV1::calculate(&[seed; 8])
                    .unwrap(),
            machine_refinement_sha256: [seed.wrapping_add(3); 32],
            required_families: 1,
            established_families: 1,
        })
        .unwrap();
    InertCapabilityRefinementReceiptV1::from_canonical_preimage(
        InertCapabilityRefinementReceiptKindV1::Machine,
        receipt.canonical_bytes().to_vec(),
    )
    .unwrap()
}

fn subject(kir: [u8; 32], epoch: u64) -> CapabilitySubjectV1 {
    subject_with_context(
        kir,
        epoch,
        KERNEL_IDENTITY,
        ROOT_IDENTITY,
        TARGET_MODEL_IDENTITY,
        LAUNCH_CONTRACT_IDENTITY,
    )
}

fn subject_with_context(
    kir: [u8; 32],
    epoch: u64,
    kernel: [u8; 32],
    root: [u8; 32],
    target: [u8; 32],
    launch: [u8; 32],
) -> CapabilitySubjectV1 {
    CapabilitySubjectV1::new(
        KernelIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(kernel)),
        KernelRootIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(root)),
        ExecutableKirIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(kir)),
        epoch,
        TargetModelIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(target)),
        LaunchContractIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(launch)),
    )
    .unwrap()
}

fn capability_sets(
    subject: CapabilitySubjectV1,
    properties: &[CapabilityPropertyIdV1],
    non_proven: Option<usize>,
    source_evidence: Option<[u8; 32]>,
    machine_evidence: Option<[u8; 32]>,
) -> (InertCapabilityObligationSetV1, InertCapabilityResultSetV1) {
    let obligations = InertCapabilityObligationSetV1::from_specs(
        subject,
        properties
            .iter()
            .enumerate()
            .map(|(index, property)| {
                CapabilityObligationSpecV1::new(
                    *property,
                    StatementIdentityV1::from_untrusted_digest(digest(30 + index as u8)),
                )
            })
            .collect(),
    )
    .unwrap();
    let results = InertCapabilityResultSetV1::from_specs(
        subject,
        obligations.identity(),
        obligations
            .obligations()
            .iter()
            .enumerate()
            .map(|(index, obligation)| {
                let outcome = if non_proven == Some(index) {
                    CapabilityOutcomeV1::Incomplete {
                        diagnostic: CapabilityDiagnosticIdV1::INCOMPLETE_ANALYSIS,
                        detail: ArtifactIdentityV1::new(digest(80), digest(81)),
                    }
                } else {
                    let evidence = if obligation.property()
                        == CapabilityPropertyIdV1::SOURCE_MIR_TO_KIR_REFINEMENT
                    {
                        source_evidence.expect("source property has exact evidence")
                    } else if obligation.property() == CapabilityPropertyIdV1::MACHINE_REFINEMENT {
                        machine_evidence.expect("machine property has exact evidence")
                    } else {
                        [40 + index as u8; 32]
                    };
                    CapabilityOutcomeV1::Proven {
                        evidence: EvidenceIdentityV1::from_untrusted_digest(
                            DigestV1::from_untrusted_bytes(evidence),
                        ),
                        tool: ExactToolIdentityV1::new(
                            digest(50 + index as u8),
                            digest(60 + index as u8),
                        ),
                        proof_artifact: ArtifactIdentityV1::new(
                            digest(70 + index as u8),
                            digest(71 + index as u8),
                        ),
                    }
                };
                CapabilityResultSpecV1::new(obligation.identity(), outcome)
            })
            .collect(),
    )
    .unwrap();
    (obligations, results)
}

fn fixture(
    properties: &[CapabilityPropertyIdV1],
    non_proven: Option<usize>,
    proof_kir_splice: bool,
    executable_identity_splice: bool,
) -> Fixture {
    fixture_with_context_subject(
        properties,
        non_proven,
        proof_kir_splice,
        executable_identity_splice,
        KERNEL_IDENTITY,
        ROOT_IDENTITY,
        TARGET_MODEL_IDENTITY,
        LAUNCH_CONTRACT_IDENTITY,
    )
}

fn genuine_source_refinement_receipt(
    final_kir_sha256: [u8; 32],
    final_kir_bytes: u64,
) -> InertCapabilityRefinementReceiptV1 {
    let unit = sm::SemanticTypeIdV1::from_index(0);
    let unit_type = sm::SemanticTypeDeclV1::new(
        sm::SemanticTypeIdentityV1::from_sha256([4; 32]),
        sm::SemanticLayoutIdentityV1::from_sha256([4; 32]),
        sm::SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            sm::SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            sm::SemanticRustcVariantsV1::Single { index: 0 },
            sm::SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            sm::SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        sm::SemanticTypeShapeV1::Unit,
    );
    let source = sm::SemanticSourceProvenanceV1::unavailable();
    let abi = sm::SemanticFunctionAbiV1::from_rustc(
        sm::SemanticAbiIdentityV1::from_sha256([5; 32]),
        sm::SemanticLayoutIdentityV1::from_sha256([250; 32]),
        sm::SemanticCanonAbiV1::GpuKernel,
        sm::SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        sm::SemanticAbiValueV1::new(unit, sm::SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let block = sm::SemanticBasicBlockV1::new(
        sm::SemanticBlockIdentityV1::from_sha256([6; 32]),
        source,
        vec![],
        sm::SemanticTerminatorV1::new(source, sm::SemanticTerminatorKindV1::Return),
    )
    .unwrap();
    let dimensions = sm::SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let contract = sm::SemanticKernelSourceContractV1::new(
        Some(
            sm::SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                .unwrap(),
        ),
        None,
        None,
    )
    .unwrap();
    let function = sm::SemanticFunctionDeclV1::new(
        sm::SemanticFunctionIdentityV1::from_sha256([7; 32]),
        sm::SemanticFunctionRoleV1::KernelRoot,
        sm::SemanticItemDefinitionIdentityV1::from_sha256([8; 32]),
        sm::SemanticMonomorphizationIdentityV1::from_sha256([9; 32]),
        sm::SemanticGenericTypeArgumentsIdentityV1::from_sha256([10; 32]),
        sm::SemanticConstGenericArgumentsIdentityV1::from_sha256([11; 32]),
        source,
        abi,
        vec![sm::SemanticLocalDeclV1::new(
            sm::SemanticLocalIdentityV1::from_sha256([12; 32]),
            unit,
            sm::SemanticLocalRoleV1::Return,
            source,
        )],
        sm::SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap()
    .with_kernel_entry(sm::SemanticKernelEntryV1::new(
        sm::SemanticLinkSymbolV1::new(b"source_refinement_fixture".to_vec()).unwrap(),
        sm::SemanticKernelBindingIdentityV1::from_sha256([13; 32]),
        contract,
    ));
    let admitted = sm::InertSemanticMirRequestV1::new(
        sm::SemanticTargetDataLayoutV1::gfx942(sm::SemanticLayoutIdentityV1::from_sha256(
            [250; 32],
        )),
        vec![unit_type],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![sm::SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(sm::SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let lowered =
        ProductionSemanticKirOwnerV1::try_lower(semantic, ProductionSemanticKirLimitsV1::default())
            .unwrap();
    let induction = fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(
        lowered.semantic().semantic(),
        sm::SemanticFunctionIdV1::from_index(0),
    )
    .unwrap();
    let evidence =
        ProductionSourceRefinementEvidenceV1::from_live_owner(&lowered, &induction).unwrap();
    let root = &evidence.roots()[0];
    let neutral = MultiRootNeutralKirIdentityV3::new(
        MultiRootCanonicalKirVersionV3::V13,
        final_kir_bytes,
        final_kir_sha256,
        EPOCH,
    )
    .unwrap();
    let roster = |kind, payload: &'static [u8]| {
        let roots = [MultiRootProofRosterRootInputV3 {
            semantic_root: root.semantic_root(),
            semantic_root_identity: root.semantic_root_identity(),
            kernel_binding: root.kernel_binding(),
            source_rank: 1,
            workgroup: [64, 1, 1],
            logical_name: root.export_symbol(),
            export_symbol: root.export_symbol(),
            kernel_id: root.kernel_id(),
            payload,
        }];
        MultiRootProofRosterTranscriptV3::new(MultiRootProofRosterInputsV3 {
            kind,
            semantic_mir_sha256: evidence.semantic_mir_sha256(),
            neutral_kir: neutral,
            roster_identity: [14; 32],
            canonical_kernel_order: &[0],
            roots: &roots,
        })
        .unwrap()
    };
    let lineage = InertMultiRootProofLineageV3::new(
        roster(MultiRootProofRosterKindV3::MiddleEnd, b"middle-end"),
        roster(
            MultiRootProofRosterKindV3::Correspondence,
            b"correspondence",
        ),
        roster(MultiRootProofRosterKindV3::FormalMemory, b"formal-memory"),
        roster(MultiRootProofRosterKindV3::VerusExecution, b"verus"),
    )
    .unwrap();
    InertCapabilityRefinementReceiptV1::from_checked_source_evidence_v1(
        evidence.canonical_bytes(),
        &lineage,
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn fixture_with_context_subject(
    properties: &[CapabilityPropertyIdV1],
    non_proven: Option<usize>,
    proof_kir_splice: bool,
    executable_identity_splice: bool,
    subject_kernel: [u8; 32],
    subject_root: [u8; 32],
    subject_target: [u8; 32],
    subject_launch: [u8; 32],
) -> Fixture {
    let ExactV13BundleCustody {
        kernel_ir: kir_owner,
        bundle_identity,
        graph_epoch: bundle_epoch,
    } = exact_v13_bundle_custody(executable_kir(
        KERNEL_IDENTITY,
        TARGET_MODEL_IDENTITY,
        LAUNCH_CONTRACT_IDENTITY,
    ));
    let mut kir_digest = *kir_owner.identity().digest();
    if executable_identity_splice {
        kir_digest = [99; 32];
    }
    let subject = subject_with_context(
        kir_digest,
        EPOCH,
        subject_kernel,
        subject_root,
        subject_target,
        subject_launch,
    );
    let source = properties
        .contains(&CapabilityPropertyIdV1::SOURCE_MIR_TO_KIR_REFINEMENT)
        .then(|| {
            genuine_source_refinement_receipt(kir_digest, kir_owner.canonical_bytes().len() as u64)
        });
    let machine = properties
        .contains(&CapabilityPropertyIdV1::MACHINE_REFINEMENT)
        .then(|| machine_refinement_receipt(31));
    let (obligations, results) = capability_sets(
        subject,
        properties,
        non_proven,
        source.as_ref().map(|receipt| receipt.identity().sha256()),
        machine.as_ref().map(|receipt| receipt.identity().sha256()),
    );
    let executable_kir_receipt =
        InertCanonicalKernelIrV13ReceiptV5::from_canonical_preimage(kir_owner.canonical_bytes())
            .unwrap();

    let semantic_mir =
        InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(opaque("semantic-mir"))
            .unwrap();
    let middle_end =
        InertMiddleEndReceiptV3::from_canonical_preimage(opaque("middle-end")).unwrap();
    let kernel_ir =
        InertKernelIrReceiptV3::from_canonical_preimage(kir_owner.canonical_bytes()).unwrap();
    let correspondence =
        InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(opaque("correspondence"))
            .unwrap();
    let formal_memory =
        InertFormalMemoryReceiptV3::from_canonical_preimage(opaque("formal-memory")).unwrap();
    let proof_kir = if proof_kir_splice {
        InertLineageContentIdentityV3::new([98; 32], 98).unwrap()
    } else {
        lineage(
            kernel_ir.identity().sha256(),
            kernel_ir.identity().byte_len(),
        )
    };
    let proof = InertProofBindingAssociationV4::new(
        InertProofBindingAssociationInputsV4::new(
            lineage(
                semantic_mir.identity().sha256(),
                semantic_mir.identity().byte_len(),
            ),
            lineage(
                middle_end.identity().sha256(),
                middle_end.identity().byte_len(),
            ),
            proof_kir,
            lineage(
                correspondence.identity().sha256(),
                correspondence.identity().byte_len(),
            ),
            lineage(
                formal_memory.identity().sha256(),
                formal_memory.identity().byte_len(),
            ),
        ),
        b"opaque-signed-verus-evidence",
    )
    .unwrap();
    let proof_binding =
        InertProofBindingReceiptV3::from_canonical_preimage(proof.canonical_bytes()).unwrap();

    let receipts = OrderedInertSemanticLineageReceiptsV3::new(
        InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(opaque("inventory")).unwrap(),
        InertRustcPreflightPlanReceiptV3::from_canonical_preimage(opaque("preflight")).unwrap(),
        semantic_mir,
        middle_end,
        kernel_ir,
        correspondence,
        formal_memory,
        proof_binding,
        InertTargetBindingReceiptV3::from_canonical_preimage(opaque("target-binding")).unwrap(),
        InertDataLayoutReceiptV3::from_canonical_preimage(opaque("data-layout")).unwrap(),
        InertAbiReceiptV3::from_canonical_preimage(opaque("abi")).unwrap(),
        InertExportManifestReceiptV3::from_canonical_preimage(opaque("exports")).unwrap(),
        InertAmdgpuLoweringReceiptV3::from_canonical_preimage(opaque("lowering")).unwrap(),
        InertSemanticToLlvmReceiptV3::from_canonical_preimage(opaque("semantic-to-llvm")).unwrap(),
        InertFinalCompilerModuleCommitmentReceiptV3::from_canonical_preimage(opaque(
            "final-module",
        ))
        .unwrap(),
    );
    let capsule = InertProductionSemanticCapsuleV3::new(
        invocation(1),
        DeviceTargetV1::parse(TARGET).unwrap(),
        receipts,
    )
    .unwrap();
    let receipts = capsule.receipts();
    let inputs = InertStaticCapabilityEvidenceAssociationInputsV1::new(
        lineage(capsule.identity().sha256(), capsule.identity().byte_len()),
        lineage(
            &executable_kir_receipt.identity().sha256(),
            executable_kir_receipt.identity().byte_len(),
        ),
        lineage(
            receipts.proof_binding().identity().sha256(),
            receipts.proof_binding().identity().byte_len(),
        ),
        lineage(
            receipts.target_binding().identity().sha256(),
            receipts.target_binding().identity().byte_len(),
        ),
        lineage(
            receipts.amdgpu_lowering().identity().sha256(),
            receipts.amdgpu_lowering().identity().byte_len(),
        ),
        lineage(
            receipts.semantic_to_llvm().identity().sha256(),
            receipts.semantic_to_llvm().identity().byte_len(),
        ),
        lineage(
            receipts
                .final_compiler_module_commitment()
                .identity()
                .sha256(),
            receipts
                .final_compiler_module_commitment()
                .identity()
                .byte_len(),
        ),
        source
            .as_ref()
            .map(InertCapabilityRefinementReceiptV1::identity),
        machine
            .as_ref()
            .map(InertCapabilityRefinementReceiptV1::identity),
    );
    let obligation_set = obligations.identity();
    let association =
        InertStaticCapabilityEvidenceAssociationV1::new(inputs, &obligations, &results).unwrap();
    Fixture {
        capsule,
        executable_kir_receipt,
        association,
        subject,
        obligation_set,
        source,
        machine,
        bundle_identity,
        bundle_epoch,
    }
}

fn base_properties() -> [CapabilityPropertyIdV1; 15] {
    [
        CapabilityPropertyIdV1::TYPING,
        CapabilityPropertyIdV1::BOUNDS,
        CapabilityPropertyIdV1::INITIALIZATION,
        CapabilityPropertyIdV1::HIERARCHICAL_OWNERSHIP,
        CapabilityPropertyIdV1::DATA_RACE_FREEDOM,
        CapabilityPropertyIdV1::ATOMIC_LEGALITY,
        CapabilityPropertyIdV1::UNIFORMITY,
        CapabilityPropertyIdV1::BARRIER_CONVERGENCE,
        CapabilityPropertyIdV1::WORKGROUP_MEMORY_EPOCHS,
        CapabilityPropertyIdV1::TENSOR_LAYOUT,
        CapabilityPropertyIdV1::EFFECTS,
        CapabilityPropertyIdV1::RESOURCE_LEGALITY,
        CapabilityPropertyIdV1::TARGET_CAPABILITY_CLOSURE,
        CapabilityPropertyIdV1::SOURCE_MIR_TO_KIR_REFINEMENT,
        CapabilityPropertyIdV1::MACHINE_REFINEMENT,
    ]
}

fn validate(
    fixture: Fixture,
) -> Result<ValidatedCompilerCapabilityEvidenceV1, CompilerCapabilityEvidenceValidationErrorV1> {
    assert_ne!(fixture.bundle_identity.as_bytes(), &[0; 32]);
    assert_eq!(fixture.bundle_epoch, EPOCH);
    validate_compiler_capability_evidence_v1(
        fixture.association.canonical_bytes(),
        &fixture.capsule,
        &fixture.executable_kir_receipt,
        fixture.subject,
        fixture.obligation_set,
        fixture.source,
        fixture.machine,
    )
}

fn replace_association_inputs(
    fixture: &mut Fixture,
    inputs: InertStaticCapabilityEvidenceAssociationInputsV1,
) {
    let obligations = InertCapabilityObligationSetV1::decode_canonical(
        fixture.association.obligation_set_bytes(),
    )
    .unwrap();
    let results =
        InertCapabilityResultSetV1::decode_canonical(fixture.association.result_set_bytes())
            .unwrap();
    fixture.association =
        InertStaticCapabilityEvidenceAssociationV1::new(inputs, &obligations, &results).unwrap();
}

struct NativeV5Fixture {
    source_owner: ValidatedCompilerProofInputsV4,
    capability: ValidatedCompilerCapabilityEvidenceV1,
    kir_receipt: InertCanonicalKernelIrV13ReceiptV5,
    association: InertCompilerProofOwnerV5,
    policy: [u8; 32],
}

struct MultiRootNativeV5Fixture {
    source_owner: ValidatedCompilerProofInputsV4,
    capabilities: Vec<ValidatedCompilerCapabilityEvidenceV1>,
    kir_receipt: InertCanonicalKernelIrV13ReceiptV5,
    subjects: [CapabilitySubjectV1; 2],
    proof_lineage: InertMultiRootProofLineageV3,
    capability_associations: InertMultiRootStaticCapabilityEvidenceAssociationV1,
    policy: [u8; 32],
    bundle_identity: SimulationBundleIdentityV8,
    bundle_epoch: u64,
}

fn native_v5_fixture() -> NativeV5Fixture {
    let capability = validate(fixture(&base_properties(), None, false, false)).unwrap();
    let source_owner = validated_v4_source_owner();
    let kir_receipt = InertCanonicalKernelIrV13ReceiptV5::from_canonical_preimage(
        capability.kernel_ir().canonical_bytes(),
    )
    .unwrap();
    let policy = [77; 32];
    let source_refinement = capability.source_refinement().unwrap().identity();
    let machine_refinement = capability.machine_refinement().unwrap().identity();
    let capability_association = capability.association().identity();
    let proof = source_owner.receipt_identity();
    let association = InertCompilerProofOwnerV5::new(
        InertCompilerProofOwnerInputsV5::new(
            lineage(proof.sha256(), proof.byte_len()),
            source_owner.association().inputs().semantic_mir(),
            *source_owner.semantic_mir().semantic_sha256().as_bytes(),
            kir_receipt.identity(),
            capability.kernel_ir().identity().canonical_length(),
            capability.association().subject(),
            policy,
            source_refinement,
            machine_refinement,
            capability_association,
        )
        .unwrap(),
    )
    .unwrap();
    NativeV5Fixture {
        source_owner,
        capability,
        kir_receipt,
        association,
        policy,
    }
}

fn proof_lineage_for_subjects(
    subjects: [CapabilitySubjectV1; 2],
    semantic_mir_sha256: [u8; 32],
    canonical_kir_bytes: u64,
    payload_seed: u8,
) -> InertMultiRootProofLineageV3 {
    let neutral = MultiRootNeutralKirIdentityV3::new(
        MultiRootCanonicalKirVersionV3::V13,
        canonical_kir_bytes,
        *subjects[0].executable_kir().digest().as_bytes(),
        subjects[0].executable_kir_epoch(),
    )
    .unwrap();
    let roster = |kind, label: &'static [u8]| {
        let first_payload = [label, &[payload_seed]].concat();
        let second_payload = [label, &[payload_seed.wrapping_add(1)]].concat();
        let roots = [
            MultiRootProofRosterRootInputV3 {
                semantic_root: 0,
                semantic_root_identity: *subjects[0].root().digest().as_bytes(),
                kernel_binding: *subjects[0].kernel().digest().as_bytes(),
                source_rank: 1,
                workgroup: [64, 1, 1],
                logical_name: "kernel_0",
                export_symbol: "kernel_0",
                kernel_id: "kernel_0",
                payload: &first_payload,
            },
            MultiRootProofRosterRootInputV3 {
                semantic_root: 1,
                semantic_root_identity: *subjects[1].root().digest().as_bytes(),
                kernel_binding: *subjects[1].kernel().digest().as_bytes(),
                source_rank: 1,
                workgroup: [64, 1, 1],
                logical_name: "kernel_1",
                export_symbol: "kernel_1",
                kernel_id: "kernel_1",
                payload: &second_payload,
            },
        ];
        MultiRootProofRosterTranscriptV3::new(MultiRootProofRosterInputsV3 {
            kind,
            semantic_mir_sha256,
            neutral_kir: neutral,
            roster_identity: [payload_seed; 32],
            canonical_kernel_order: &[0, 1],
            roots: &roots,
        })
        .unwrap()
    };
    InertMultiRootProofLineageV3::new(
        roster(MultiRootProofRosterKindV3::MiddleEnd, b"middle"),
        roster(
            MultiRootProofRosterKindV3::Correspondence,
            b"correspondence",
        ),
        roster(MultiRootProofRosterKindV3::FormalMemory, b"memory"),
        roster(MultiRootProofRosterKindV3::VerusExecution, b"verus"),
    )
    .unwrap()
}

fn copy_refinement_receipt(
    receipt: &InertCapabilityRefinementReceiptV1,
) -> InertCapabilityRefinementReceiptV1 {
    InertCapabilityRefinementReceiptV1::from_canonical_preimage(
        receipt.kind(),
        receipt.canonical_preimage().to_vec(),
    )
    .unwrap()
}

fn multi_root_native_v5_fixture() -> MultiRootNativeV5Fixture {
    let base = fixture(&base_properties(), None, false, false);
    let coordinates = [
        (0, KERNEL_IDENTITY, ROOT_IDENTITY, LAUNCH_CONTRACT_IDENTITY),
        (1, [22; 32], [4; 32], [26; 32]),
    ];
    let ExactV13BundleCustody {
        kernel_ir,
        bundle_identity,
        graph_epoch: bundle_epoch,
    } = exact_v13_bundle_custody(multi_root_executable_kir(&coordinates));
    let kir_receipt =
        InertCanonicalKernelIrV13ReceiptV5::from_canonical_preimage(kernel_ir.canonical_bytes())
            .unwrap();
    let kir_digest = *kernel_ir.identity().digest();
    let subjects = [
        subject_with_context(
            kir_digest,
            EPOCH,
            coordinates[0].1,
            coordinates[0].2,
            TARGET_MODEL_IDENTITY,
            coordinates[0].3,
        ),
        subject_with_context(
            kir_digest,
            EPOCH,
            coordinates[1].1,
            coordinates[1].2,
            TARGET_MODEL_IDENTITY,
            coordinates[1].3,
        ),
    ];
    let source = base.source.as_ref().unwrap();
    let machine = base.machine.as_ref().unwrap();
    let common = base.association.inputs();
    let association_inputs = InertStaticCapabilityEvidenceAssociationInputsV1::new(
        common.capsule(),
        lineage(
            &kir_receipt.identity().sha256(),
            kir_receipt.identity().byte_len(),
        ),
        common.proof_binding(),
        common.target_binding(),
        common.target_lowering(),
        common.semantic_to_llvm(),
        common.final_compiler_module(),
        Some(source.identity()),
        Some(machine.identity()),
    );
    let mut capabilities = Vec::new();
    let mut association_bytes = Vec::new();
    for selected_subject in subjects {
        let (obligations, results) = capability_sets(
            selected_subject,
            &base_properties(),
            None,
            Some(source.identity().sha256()),
            Some(machine.identity().sha256()),
        );
        let expected_obligations = obligations.identity();
        let association = InertStaticCapabilityEvidenceAssociationV1::new(
            association_inputs,
            &obligations,
            &results,
        )
        .unwrap();
        association_bytes.push(association.canonical_bytes().to_vec());
        capabilities.push(
            validate_compiler_capability_evidence_v1(
                association.canonical_bytes(),
                &base.capsule,
                &kir_receipt,
                selected_subject,
                expected_obligations,
                Some(copy_refinement_receipt(source)),
                Some(copy_refinement_receipt(machine)),
            )
            .unwrap(),
        );
    }
    let capability_associations = InertMultiRootStaticCapabilityEvidenceAssociationV1::new(
        association_bytes
            .iter()
            .map(|bytes| InertStaticCapabilityEvidenceAssociationV1::decode(bytes).unwrap())
            .collect(),
    )
    .unwrap();
    let source_owner = validated_v4_source_owner();
    let proof_lineage = proof_lineage_for_subjects(
        subjects,
        *source_owner.semantic_mir().semantic_sha256().as_bytes(),
        kernel_ir.identity().canonical_length(),
        121,
    );
    MultiRootNativeV5Fixture {
        source_owner,
        capabilities,
        kir_receipt,
        subjects,
        proof_lineage,
        capability_associations,
        policy: [77; 32],
        bundle_identity,
        bundle_epoch,
    }
}

fn multi_root_owner(
    fixture: &MultiRootNativeV5Fixture,
    selected_subject_ordinal: u32,
    capability_ordinal: usize,
) -> InertCompilerProofOwnerV5 {
    let capability = &fixture.capabilities[capability_ordinal];
    let proof = fixture.source_owner.receipt_identity();
    InertCompilerProofOwnerV5::new_multi_root_for_association_at_ordinal(
        InertCompilerProofOwnerInputsV5::new(
            lineage(proof.sha256(), proof.byte_len()),
            fixture.source_owner.association().inputs().semantic_mir(),
            *fixture
                .source_owner
                .semantic_mir()
                .semantic_sha256()
                .as_bytes(),
            fixture.kir_receipt.identity(),
            fixture.kir_receipt.identity().byte_len(),
            capability.association().subject(),
            fixture.policy,
            capability.source_refinement().unwrap().identity(),
            capability.machine_refinement().unwrap().identity(),
            fixture.capability_associations.identity(),
        )
        .unwrap(),
        &fixture.proof_lineage,
        fixture.subjects.to_vec(),
        selected_subject_ordinal,
        capability.association().identity(),
    )
    .unwrap()
}

fn replace_v5_inputs(fixture: &mut NativeV5Fixture, inputs: InertCompilerProofOwnerInputsV5) {
    fixture.association = InertCompilerProofOwnerV5::new(inputs).unwrap();
}

#[test]
fn current_v4_source_fixture_preserves_frozen_payloads_and_replays() {
    let frozen = frozen_compiler_proof_inputs_v4();
    let current = current_compiler_proof_inputs_v4_from_frozen_source();
    assert_eq!(current.semantic_mir(), frozen.semantic_mir());
    assert_eq!(current.middle_end(), frozen.middle_end());
    assert_eq!(current.kernel_ir(), frozen.kernel_ir());
    assert_eq!(current.formal_memory(), frozen.formal_memory());
    let old =
        InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(frozen.correspondence()).unwrap();
    let new =
        InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(current.correspondence()).unwrap();
    let start =
        frozen.correspondence().len() - old.semantic_u32_induction().canonical_bytes().len();
    assert_eq!(
        &current.correspondence()[..start],
        &frozen.correspondence()[..start]
    );
    assert_eq!(
        current.correspondence().len(),
        frozen.correspondence().len()
    );
    assert_ne!(
        new.semantic_u32_induction().work_units(),
        old.semantic_u32_induction().work_units()
    );
    assert_ne!(new.identity(), old.identity());
    drop(validate_v4_source_owner(&current).unwrap());
}

#[test]
fn historical_v4_source_fixture_rejects_stale_induction_work() {
    let frozen = frozen_compiler_proof_inputs_v4();
    let correspondence =
        InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(frozen.correspondence()).unwrap();
    assert_eq!(correspondence.semantic_u32_induction().work_units(), 12);
    assert!(
        correspondence
            .semantic_u32_induction()
            .certificates()
            .is_empty()
    );
    assert!(matches!(
        validate_v4_source_owner(&frozen),
        Err(CompilerProofInputValidationErrorV4::Stage(
            CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                detail: "retained semantic induction report differs from deterministic replay"
            }
        ))
    ));
}

#[test]
fn complete_proven_static_evidence_is_retained_without_authority() {
    let validated = validate(fixture(&base_properties(), None, false, false)).unwrap();
    assert_eq!(validated.results().results().len(), 15);
    assert!(validated.source_refinement().is_some());
    assert!(validated.machine_refinement().is_some());
    assert_eq!(
        validated.kernel_ir().identity().digest(),
        validated
            .association()
            .subject()
            .executable_kir()
            .digest()
            .as_bytes()
    );
}

#[test]
fn native_v5_owner_accepts_exact_v13_graph_and_epoch() {
    let fixture = native_v5_fixture();
    let owner = validate_compiler_proof_inputs_v5(
        fixture.association.canonical_bytes(),
        &fixture.source_owner,
        &fixture.kir_receipt,
        fixture.capability,
        fixture.policy,
    )
    .unwrap();
    assert_eq!(
        owner
            .association()
            .inputs()
            .subject()
            .executable_kir_epoch(),
        EPOCH
    );
    assert_eq!(
        owner.capability().kernel_ir().identity().digest(),
        owner
            .association()
            .inputs()
            .subject()
            .executable_kir()
            .digest()
            .as_bytes()
    );
}

#[test]
fn multi_root_v5_owner_selects_and_retains_the_second_subject() {
    let mut fixture = multi_root_native_v5_fixture();
    assert_ne!(fixture.bundle_identity.as_bytes(), &[0; 32]);
    assert_eq!(fixture.bundle_epoch, EPOCH);
    let association = multi_root_owner(&fixture, 1, 1);
    let capability = fixture.capabilities.remove(1);
    let owner = validate_compiler_multi_root_proof_inputs_v5(
        association.canonical_bytes(),
        &fixture.source_owner,
        &fixture.kir_receipt,
        capability,
        fixture.policy,
        fixture.proof_lineage,
        fixture.capability_associations,
        1,
    )
    .unwrap();
    assert_eq!(owner.selected_subject_ordinal(), 1);
    assert_eq!(
        owner.selected().association().selected_subject(),
        fixture.subjects[1]
    );
    for kind in [
        MultiRootProofRosterKindV3::MiddleEnd,
        MultiRootProofRosterKindV3::Correspondence,
        MultiRootProofRosterKindV3::FormalMemory,
        MultiRootProofRosterKindV3::VerusExecution,
    ] {
        assert_eq!(owner.proof_lineage().roster(kind).root_count(), 2);
    }
    assert_eq!(owner.capability_associations().entries().len(), 2);
}

#[test]
fn multi_root_v5_owner_rejects_wrong_ordinal_cross_root_association_and_stale_policy() {
    let mut wrong_ordinal = multi_root_native_v5_fixture();
    let association = multi_root_owner(&wrong_ordinal, 1, 1);
    let capability = wrong_ordinal.capabilities.remove(1);
    assert!(matches!(
        validate_compiler_multi_root_proof_inputs_v5(
            association.canonical_bytes(),
            &wrong_ordinal.source_owner,
            &wrong_ordinal.kir_receipt,
            capability,
            wrong_ordinal.policy,
            wrong_ordinal.proof_lineage,
            wrong_ordinal.capability_associations,
            0,
        ),
        Err(
            CompilerProofInputValidationErrorV5::SelectedSubjectOrdinalMismatch {
                expected: 0,
                actual: 1,
            }
        )
    ));

    let mut cross_root = multi_root_native_v5_fixture();
    let association = multi_root_owner(&cross_root, 1, 1);
    let first_capability = cross_root.capabilities.remove(0);
    assert!(matches!(
        validate_compiler_multi_root_proof_inputs_v5(
            association.canonical_bytes(),
            &cross_root.source_owner,
            &cross_root.kir_receipt,
            first_capability,
            cross_root.policy,
            cross_root.proof_lineage,
            cross_root.capability_associations,
            1,
        ),
        Err(CompilerProofInputValidationErrorV5::IdentityMismatch(
            "V13 graph subject"
        ))
    ));

    let mut stale_policy = multi_root_native_v5_fixture();
    let association = multi_root_owner(&stale_policy, 1, 1);
    let capability = stale_policy.capabilities.remove(1);
    assert!(matches!(
        validate_compiler_multi_root_proof_inputs_v5(
            association.canonical_bytes(),
            &stale_policy.source_owner,
            &stale_policy.kir_receipt,
            capability,
            [78; 32],
            stale_policy.proof_lineage,
            stale_policy.capability_associations,
            1,
        ),
        Err(CompilerProofInputValidationErrorV5::IdentityMismatch(
            "compiler policy"
        ))
    ));
}

#[test]
fn native_v5_owner_rejects_omission_downgrade_and_v8_projection() {
    let omitted = native_v5_fixture();
    assert!(matches!(
        validate_compiler_proof_inputs_v5(
            &[],
            &omitted.source_owner,
            &omitted.kir_receipt,
            omitted.capability,
            omitted.policy,
        ),
        Err(CompilerProofInputValidationErrorV5::Association(_))
    ));

    let downgraded = native_v5_fixture();
    let mut bytes = downgraded.association.canonical_bytes().to_vec();
    bytes[8..10].copy_from_slice(&4_u16.to_le_bytes());
    assert!(matches!(
        validate_compiler_proof_inputs_v5(
            &bytes,
            &downgraded.source_owner,
            &downgraded.kir_receipt,
            downgraded.capability,
            downgraded.policy,
        ),
        Err(CompilerProofInputValidationErrorV5::Association(
            fe2o3_compiler_lineage::InertCompilerProofOwnerErrorV5::UnsupportedVersion
        ))
    ));

    let mut projected = native_v5_fixture();
    let v8_receipt = InertCanonicalKernelIrV13ReceiptV5::from_canonical_preimage(
        projected.source_owner.kernel_ir().canonical_bytes(),
    )
    .unwrap();
    let inputs = projected.association.inputs();
    replace_v5_inputs(
        &mut projected,
        InertCompilerProofOwnerInputsV5::new(
            inputs.legacy_proof_binding(),
            inputs.semantic_mir_receipt(),
            inputs.semantic_mir_identity(),
            v8_receipt.identity(),
            v8_receipt.identity().byte_len(),
            inputs.subject(),
            inputs.compiler_policy(),
            inputs.source_refinement(),
            inputs.machine_refinement(),
            inputs.capability_association(),
        )
        .unwrap(),
    );
    assert!(matches!(
        validate_compiler_proof_inputs_v5(
            projected.association.canonical_bytes(),
            &projected.source_owner,
            &v8_receipt,
            projected.capability,
            projected.policy,
        ),
        Err(CompilerProofInputValidationErrorV5::IdentityMismatch(
            "canonical KIR V13 receipt preimage"
        ))
    ));
}

#[test]
fn native_v5_owner_rejects_stale_epoch_and_cross_subject_substitution() {
    for (kernel, root, target, launch, epoch) in [
        (
            [91; 32],
            ROOT_IDENTITY,
            TARGET_MODEL_IDENTITY,
            LAUNCH_CONTRACT_IDENTITY,
            EPOCH,
        ),
        (
            KERNEL_IDENTITY,
            [92; 32],
            TARGET_MODEL_IDENTITY,
            LAUNCH_CONTRACT_IDENTITY,
            EPOCH,
        ),
        (
            KERNEL_IDENTITY,
            ROOT_IDENTITY,
            [93; 32],
            LAUNCH_CONTRACT_IDENTITY,
            EPOCH,
        ),
        (
            KERNEL_IDENTITY,
            ROOT_IDENTITY,
            TARGET_MODEL_IDENTITY,
            [94; 32],
            EPOCH,
        ),
        (
            KERNEL_IDENTITY,
            ROOT_IDENTITY,
            TARGET_MODEL_IDENTITY,
            LAUNCH_CONTRACT_IDENTITY,
            EPOCH + 1,
        ),
    ] {
        let mut fixture = native_v5_fixture();
        let inputs = fixture.association.inputs();
        let substituted = subject_with_context(
            *inputs.subject().executable_kir().digest().as_bytes(),
            epoch,
            kernel,
            root,
            target,
            launch,
        );
        replace_v5_inputs(
            &mut fixture,
            InertCompilerProofOwnerInputsV5::new(
                inputs.legacy_proof_binding(),
                inputs.semantic_mir_receipt(),
                inputs.semantic_mir_identity(),
                inputs.executable_kir_receipt(),
                inputs.executable_kir_bytes(),
                substituted,
                inputs.compiler_policy(),
                inputs.source_refinement(),
                inputs.machine_refinement(),
                inputs.capability_association(),
            )
            .unwrap(),
        );
        assert!(matches!(
            validate_compiler_proof_inputs_v5(
                fixture.association.canonical_bytes(),
                &fixture.source_owner,
                &fixture.kir_receipt,
                fixture.capability,
                fixture.policy,
            ),
            Err(CompilerProofInputValidationErrorV5::IdentityMismatch(
                "V13 graph subject"
            ))
        ));
    }
}

#[test]
fn native_v5_owner_rejects_policy_and_refinement_receipt_substitution() {
    let mut wrong_policy = native_v5_fixture();
    let inputs = wrong_policy.association.inputs();
    replace_v5_inputs(
        &mut wrong_policy,
        InertCompilerProofOwnerInputsV5::new(
            inputs.legacy_proof_binding(),
            inputs.semantic_mir_receipt(),
            inputs.semantic_mir_identity(),
            inputs.executable_kir_receipt(),
            inputs.executable_kir_bytes(),
            inputs.subject(),
            [78; 32],
            inputs.source_refinement(),
            inputs.machine_refinement(),
            inputs.capability_association(),
        )
        .unwrap(),
    );
    assert!(matches!(
        validate_compiler_proof_inputs_v5(
            wrong_policy.association.canonical_bytes(),
            &wrong_policy.source_owner,
            &wrong_policy.kir_receipt,
            wrong_policy.capability,
            wrong_policy.policy,
        ),
        Err(CompilerProofInputValidationErrorV5::IdentityMismatch(
            "compiler policy"
        ))
    ));

    for substitute_source in [true, false] {
        let mut fixture = native_v5_fixture();
        let inputs = fixture.association.inputs();
        let replacement = if substitute_source {
            genuine_source_refinement_receipt([97; 32], 97).identity()
        } else {
            machine_refinement_receipt(32).identity()
        };
        replace_v5_inputs(
            &mut fixture,
            InertCompilerProofOwnerInputsV5::new(
                inputs.legacy_proof_binding(),
                inputs.semantic_mir_receipt(),
                inputs.semantic_mir_identity(),
                inputs.executable_kir_receipt(),
                inputs.executable_kir_bytes(),
                inputs.subject(),
                inputs.compiler_policy(),
                if substitute_source {
                    replacement
                } else {
                    inputs.source_refinement()
                },
                if substitute_source {
                    inputs.machine_refinement()
                } else {
                    replacement
                },
                inputs.capability_association(),
            )
            .unwrap(),
        );
        let expected = if substitute_source {
            "source refinement receipt"
        } else {
            "machine refinement receipt"
        };
        assert!(matches!(
            validate_compiler_proof_inputs_v5(
                fixture.association.canonical_bytes(),
                &fixture.source_owner,
                &fixture.kir_receipt,
                fixture.capability,
                fixture.policy,
            ),
            Err(CompilerProofInputValidationErrorV5::IdentityMismatch(field)) if field == expected
        ));
    }
}

#[test]
fn non_proven_omitted_and_dynamic_results_fail_closed() {
    assert!(matches!(
        validate(fixture(&base_properties(), Some(0), false, false)),
        Err(CompilerCapabilityEvidenceValidationErrorV1::OutcomeNotProven { .. })
    ));

    let omitted = fixture(&[CapabilityPropertyIdV1::BOUNDS], None, false, false);
    let complete = fixture(&base_properties(), None, false, false);
    assert!(matches!(
        validate_compiler_capability_evidence_v1(
            omitted.association.canonical_bytes(),
            &omitted.capsule,
            &omitted.executable_kir_receipt,
            omitted.subject,
            complete.obligation_set,
            omitted.source,
            omitted.machine,
        ),
        Err(CompilerCapabilityEvidenceValidationErrorV1::ObligationSetMismatch)
    ));

    assert!(matches!(
        validate(fixture(
            &[CapabilityPropertyIdV1::DYNAMIC_LAUNCH_PRECONDITIONS],
            None,
            false,
            false,
        )),
        Err(CompilerCapabilityEvidenceValidationErrorV1::DynamicLaunchClaimInStaticEvidence)
    ));
}

#[test]
fn downgraded_association_and_omitted_refinement_receipts_fail_closed() {
    let downgraded = fixture(&base_properties(), None, false, false);
    let mut bytes = downgraded.association.canonical_bytes().to_vec();
    bytes[8..10].copy_from_slice(&0_u16.to_le_bytes());
    assert!(matches!(
        validate_compiler_capability_evidence_v1(
            &bytes,
            &downgraded.capsule,
            &downgraded.executable_kir_receipt,
            downgraded.subject,
            downgraded.obligation_set,
            downgraded.source,
            downgraded.machine,
        ),
        Err(CompilerCapabilityEvidenceValidationErrorV1::Association(
            InertStaticCapabilityEvidenceAssociationErrorV1::UnsupportedVersion
        ))
    ));

    let mut missing_source = fixture(&base_properties(), None, false, false);
    missing_source.source = None;
    assert!(matches!(
        validate(missing_source),
        Err(
            CompilerCapabilityEvidenceValidationErrorV1::MissingRefinementReceipt {
                kind: InertCapabilityRefinementReceiptKindV1::SourceMirToKir,
            }
        )
    ));

    let mut missing_machine = fixture(&base_properties(), None, false, false);
    missing_machine.machine = None;
    assert!(matches!(
        validate(missing_machine),
        Err(
            CompilerCapabilityEvidenceValidationErrorV1::MissingRefinementReceipt {
                kind: InertCapabilityRefinementReceiptKindV1::Machine,
            }
        )
    ));
}

#[test]
fn stale_epoch_and_executable_kir_substitution_fail_closed() {
    let stale = fixture(&base_properties(), None, false, false);
    let stale_subject = subject(
        *stale.subject.executable_kir().digest().as_bytes(),
        EPOCH + 1,
    );
    assert!(matches!(
        validate_compiler_capability_evidence_v1(
            stale.association.canonical_bytes(),
            &stale.capsule,
            &stale.executable_kir_receipt,
            stale_subject,
            stale.obligation_set,
            stale.source,
            stale.machine,
        ),
        Err(
            CompilerCapabilityEvidenceValidationErrorV1::ExpectedSubjectMismatch(
                CapabilitySubjectFieldV1::ExecutableKirEpoch
            )
        )
    ));

    assert!(matches!(
        validate(fixture(&base_properties(), None, false, true)),
        Err(CompilerCapabilityEvidenceValidationErrorV1::ExecutableKirIdentityMismatch)
    ));
}

#[test]
fn pre_optimization_identity_cannot_authorize_the_post_optimization_graph() {
    assert!(matches!(
        validate(fixture(&base_properties(), None, false, true)),
        Err(CompilerCapabilityEvidenceValidationErrorV1::ExecutableKirIdentityMismatch)
    ));

    let pre_optimization = fixture(&base_properties(), None, false, false);
    let post_optimization_subject = subject(
        *pre_optimization
            .subject
            .executable_kir()
            .digest()
            .as_bytes(),
        EPOCH + 1,
    );
    assert!(matches!(
        validate_compiler_capability_evidence_v1(
            pre_optimization.association.canonical_bytes(),
            &pre_optimization.capsule,
            &pre_optimization.executable_kir_receipt,
            post_optimization_subject,
            pre_optimization.obligation_set,
            pre_optimization.source,
            pre_optimization.machine,
        ),
        Err(
            CompilerCapabilityEvidenceValidationErrorV1::ExpectedSubjectMismatch(
                CapabilitySubjectFieldV1::ExecutableKirEpoch
            )
        )
    ));
}

#[test]
fn capsule_proof_and_refinement_receipt_splicing_fail_closed() {
    let first = fixture(&base_properties(), None, false, false);
    let second = fixture(&base_properties(), None, true, false);
    assert!(
        validate_compiler_capability_evidence_v1(
            first.association.canonical_bytes(),
            &second.capsule,
            &first.executable_kir_receipt,
            first.subject,
            first.obligation_set,
            first.source,
            first.machine,
        )
        .is_err()
    );

    assert!(matches!(
        validate(fixture(&base_properties(), None, true, false)),
        Err(CompilerCapabilityEvidenceValidationErrorV1::ProofBindingKirMismatch)
    ));

    let mut receipt_splice = fixture(&base_properties(), None, false, false);
    receipt_splice.machine = Some(machine_refinement_receipt(33));
    assert!(matches!(
        validate(receipt_splice),
        Err(CompilerCapabilityEvidenceValidationErrorV1::RefinementReceiptMismatch { .. })
    ));
}

#[test]
fn reordered_refinement_kinds_and_duplicate_lineage_coordinates_fail_closed() {
    let mut reordered = fixture(&base_properties(), None, false, false);
    std::mem::swap(&mut reordered.source, &mut reordered.machine);
    assert!(matches!(
        validate(reordered),
        Err(
            CompilerCapabilityEvidenceValidationErrorV1::WrongRefinementReceiptKind {
                expected: InertCapabilityRefinementReceiptKindV1::SourceMirToKir,
                actual: InertCapabilityRefinementReceiptKindV1::Machine,
            }
        )
    ));

    let mut duplicated = fixture(&base_properties(), None, false, false);
    let original = duplicated.association.inputs();
    replace_association_inputs(
        &mut duplicated,
        InertStaticCapabilityEvidenceAssociationInputsV1::new(
            original.capsule(),
            original.kernel_ir(),
            original.kernel_ir(),
            original.target_binding(),
            original.target_lowering(),
            original.semantic_to_llvm(),
            original.final_compiler_module(),
            original.source_refinement(),
            original.machine_refinement(),
        ),
    );
    assert!(matches!(
        validate(duplicated),
        Err(
            CompilerCapabilityEvidenceValidationErrorV1::LineageReceiptMismatch {
                field: "V4 proof binding",
            }
        )
    ));
}

#[test]
fn kir_context_rejects_cross_kernel_root_target_and_launch_subjects() {
    for (kernel, root, target, launch, expected) in [
        (
            [91; 32],
            ROOT_IDENTITY,
            TARGET_MODEL_IDENTITY,
            LAUNCH_CONTRACT_IDENTITY,
            CapabilitySubjectFieldV1::Kernel,
        ),
        (
            KERNEL_IDENTITY,
            [92; 32],
            TARGET_MODEL_IDENTITY,
            LAUNCH_CONTRACT_IDENTITY,
            CapabilitySubjectFieldV1::Root,
        ),
        (
            KERNEL_IDENTITY,
            ROOT_IDENTITY,
            [93; 32],
            LAUNCH_CONTRACT_IDENTITY,
            CapabilitySubjectFieldV1::TargetModel,
        ),
        (
            KERNEL_IDENTITY,
            ROOT_IDENTITY,
            TARGET_MODEL_IDENTITY,
            [94; 32],
            CapabilitySubjectFieldV1::LaunchContract,
        ),
    ] {
        let hostile = fixture_with_context_subject(
            &base_properties(),
            None,
            false,
            false,
            kernel,
            root,
            target,
            launch,
        );
        assert!(matches!(
            validate(hostile),
            Err(CompilerCapabilityEvidenceValidationErrorV1::ContextSubjectMismatch(field))
                if field == expected
        ));
    }
}

#[test]
fn every_subject_axis_is_bound_to_protected_expectations() {
    for (axis, expected_field) in [
        CapabilitySubjectFieldV1::Kernel,
        CapabilitySubjectFieldV1::Root,
        CapabilitySubjectFieldV1::ExecutableKir,
        CapabilitySubjectFieldV1::ExecutableKirEpoch,
        CapabilitySubjectFieldV1::TargetModel,
        CapabilitySubjectFieldV1::LaunchContract,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = fixture(&base_properties(), None, false, false);
        let original = fixture.subject;
        let expected = CapabilitySubjectV1::new(
            KernelIdentityV1::from_untrusted_digest(if axis == 0 {
                digest(90)
            } else {
                original.kernel().digest()
            }),
            KernelRootIdentityV1::from_untrusted_digest(if axis == 1 {
                digest(91)
            } else {
                original.root().digest()
            }),
            ExecutableKirIdentityV1::from_untrusted_digest(if axis == 2 {
                digest(92)
            } else {
                original.executable_kir().digest()
            }),
            if axis == 3 { EPOCH + 1 } else { EPOCH },
            TargetModelIdentityV1::from_untrusted_digest(if axis == 4 {
                digest(94)
            } else {
                original.target_model().digest()
            }),
            LaunchContractIdentityV1::from_untrusted_digest(if axis == 5 {
                digest(95)
            } else {
                original.launch_contract().digest()
            }),
        )
        .unwrap();
        assert!(matches!(
            validate_compiler_capability_evidence_v1(
                fixture.association.canonical_bytes(),
                &fixture.capsule,
                &fixture.executable_kir_receipt,
                expected,
                fixture.obligation_set,
                fixture.source,
                fixture.machine,
            ),
            Err(CompilerCapabilityEvidenceValidationErrorV1::ExpectedSubjectMismatch(field))
                if field == expected_field
        ));
    }
}
