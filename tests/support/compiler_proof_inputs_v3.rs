use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV5;
use fe2o3_lower_mir_kernel::{
    InertCanonicalFormalMemoryAdmissionEvidenceV3, InertCanonicalMirToKirCorrespondenceEvidenceV3,
    ProductionFormalMemoryOwnerV1, ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5, PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5,
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
};
use sha2::{Digest, Sha256};

pub(crate) struct CanonicalCompilerProofInputsV3 {
    semantic_mir: Vec<u8>,
    middle_end: Vec<u8>,
    kernel_ir: Vec<u8>,
    correspondence: Vec<u8>,
    formal_memory: Vec<u8>,
}

impl CanonicalCompilerProofInputsV3 {
    pub(crate) fn semantic_mir(&self) -> &[u8] {
        &self.semantic_mir
    }

    pub(crate) fn middle_end(&self) -> &[u8] {
        &self.middle_end
    }

    pub(crate) fn kernel_ir(&self) -> &[u8] {
        &self.kernel_ir
    }

    pub(crate) fn correspondence(&self) -> &[u8] {
        &self.correspondence
    }

    pub(crate) fn formal_memory(&self) -> &[u8] {
        &self.formal_memory
    }
}

pub(crate) fn canonical_compiler_proof_inputs_v3(seed: u8) -> CanonicalCompilerProofInputsV3 {
    let semantic_kir = ProductionSemanticKirOwnerV1::try_lower(
        semantic_owner(seed),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let semantic = semantic_kir.semantic().semantic();
    let semantic_mir = semantic.canonical_encoding().to_vec();
    let semantic_identity = *semantic.semantic_sha256().as_bytes();
    let kernel_ir =
        VerifiedCanonicalKernelIrV5::from_module(semantic_kir.module().clone()).unwrap();
    let correspondence =
        InertCanonicalMirToKirCorrespondenceEvidenceV3::from_live_owner(&semantic_kir).unwrap();
    let formal_owner = ProductionFormalMemoryOwnerV1::try_admit(semantic_kir).unwrap();
    let formal_memory =
        InertCanonicalFormalMemoryAdmissionEvidenceV3::from_live_owner(&formal_owner).unwrap();

    CanonicalCompilerProofInputsV3 {
        semantic_mir,
        middle_end: middle_end_v5_bytes(semantic_identity, seed),
        kernel_ir: kernel_ir.into_canonical_bytes(),
        correspondence: correspondence.into_canonical_bytes(),
        formal_memory: formal_memory.into_canonical_bytes(),
    }
}

fn bytes(tag: u8, seed: u8) -> [u8; 32] {
    [tag.wrapping_add(seed); 32]
}

fn unit_type(seed: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(4, seed)),
        SemanticLayoutIdentityV1::from_sha256(bytes(4, seed)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    )
}

fn block(
    tag: u8,
    seed: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(tag, seed)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn semantic_owner(seed: u8) -> ProductionSemanticMirOwnerV1 {
    let type_id = SemanticTypeIdV1::from_index(0);
    let layout = SemanticLayoutIdentityV1::from_sha256(bytes(250, seed));
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2, seed)),
        layout,
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(type_id, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let statement = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Nop,
    );
    let block0 = block(10, seed, vec![statement], SemanticTerminatorKindV1::Return);
    let block1 = block(
        11,
        seed,
        vec![],
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(0),
        )),
    );
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(2, seed)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(2, seed)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(2, seed)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(2, seed)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(2, seed)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(3, seed)),
            type_id,
            SemanticLocalRoleV1::Return,
            SemanticSourceProvenanceV1::unavailable(),
        )],
        SemanticBlockIdV1::from_index(1),
        vec![block0, block1],
    )
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let launch =
        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
    let contract = SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap();
    let function = function.with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(format!("proof_input_test_{seed}").into_bytes()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(5, seed)),
        contract,
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout),
        vec![unit_type(seed)],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn middle_end_v5_bytes(source_semantic_identity: [u8; 32], seed: u8) -> Vec<u8> {
    const IDENTITY_DOMAIN: &[u8] = b"FE2O3/PRODUCTION-MIDDLE-END-EVIDENCE-IDENTITY/V5\0";
    const RANKED_IR: &[u8] = b"func @proof_input_test {\n  kernel.return\n}\n";
    const PASS_TAGS: [u8; 8] = [1, 2, 3, 4, 8, 5, 6, 7];
    const DECLARED_LENGTH_OFFSET: usize = 12;

    let mut encoded = Vec::new();
    encoded.extend_from_slice(b"F2MEV5\0\0");
    encoded.extend_from_slice(&5_u16.to_le_bytes());
    encoded.extend_from_slice(&0_u16.to_le_bytes());
    encoded.extend_from_slice(&0_u64.to_le_bytes());
    encoded.extend_from_slice(&0_u32.to_le_bytes());
    encoded
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5.len() as u16).to_le_bytes());
    encoded.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5);
    encoded
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5.len() as u16).to_le_bytes());
    encoded.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5);
    encoded.push(1);
    encoded.push(1);
    encoded.extend_from_slice(&0_u16.to_le_bytes());
    encoded.extend_from_slice(&source_semantic_identity);
    encoded.extend_from_slice(&bytes(90, seed));
    encoded.extend_from_slice(&(RANKED_IR.len() as u32).to_le_bytes());
    encoded.extend_from_slice(RANKED_IR);
    encoded.push(PASS_TAGS.len() as u8);
    for tag in PASS_TAGS {
        encoded.push(tag);
        encoded.push(1);
        encoded.extend_from_slice(&0_u32.to_le_bytes());
        encoded.push(0);
        encoded.push(0);
        encoded.extend_from_slice(&0_u16.to_le_bytes());
    }
    for _ in 0..4 + 6 + 10 + 2 {
        encoded.extend_from_slice(&0_u64.to_le_bytes());
    }
    encoded.extend_from_slice(&bytes(91, seed));

    let total_length = encoded.len() + 32;
    encoded[DECLARED_LENGTH_OFFSET..DECLARED_LENGTH_OFFSET + 8]
        .copy_from_slice(&(total_length as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN);
    digest.update((encoded.len() as u64).to_le_bytes());
    digest.update(&encoded);
    encoded.extend_from_slice(&<[u8; 32]>::from(digest.finalize()));
    assert_eq!(encoded.len(), total_length);
    encoded
}
