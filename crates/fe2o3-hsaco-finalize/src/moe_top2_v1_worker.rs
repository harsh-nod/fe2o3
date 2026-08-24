//! Exact T8/E4/K2/C4 MoE admission for the direct upstream LLVM/LLD worker.

use std::{error::Error, fmt};

use fe2o3_compiler_ffi::{
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolRoleV1,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, ExplicitValueKind, ExplicitValueType, HiddenValueKind,
    KernelKind,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, CanonicalCodeObjectDigest, CodeObjectVersion,
    DeviceTargetV1, OwnershipSemantics, decode_device_descriptor_table_v1,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, InertDecodedWorkerExchangeV2, InertFirstBuildWorkerV2EvidenceV1,
    InspectedRawWorkerV2HsacoV1, WorkerInputKindV1, WorkerOptimizationLevelV1, WorkerOptionsV1,
    WorkerProtocolError, WorkerStageV1, WorkerV2RawHsacoInspectionError,
    worker_v2_hsaco_admission::{
        WorkerV2RawLaunchContractV1, WorkerV2RawLaunchDiagnosticProfileV1,
        inspect_worker_v2_raw_hsaco_with_launch_v1,
    },
};

const TARGET: &str = "gfx942:xnack-";
const KERNEL: &str = "moe_top2_route_f32_t8_e4_k2_c4_v1";
const DESCRIPTOR: &str = "moe_top2_route_f32_t8_e4_k2_c4_v1.kd";
const BODY_SHA256: &str = "b703e4b9bf89f77887b6c1578475b0a556851e7235342efd5247acf999ca3b39";
const EXACT_DATA_LAYOUT: &str = concat!(
    "e-m:e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-",
    "p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-",
    "v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-",
    "n32:64-S32-A5-G1-ni:7:8:9",
);
const EXCHANGE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/MOE-TOP2-T8-E4-K2-C4/DIRECT-WORKER-EXCHANGE/V1\0";

const SECTION_NAMES: [&str; 17] = [
    ".fe2o3.kd.v1",
    ".fe2o3.moe.source.v1",
    ".fe2o3.moe.namespace.v1",
    ".fe2o3.moe.crate.v1",
    ".fe2o3.moe.authority.v1",
    ".fe2o3.moe.mir.v1",
    ".fe2o3.moe.fnabi.v1",
    ".fe2o3.moe.compiler.v1",
    ".fe2o3.moe.terminals.v3",
    ".fe2o3.moe.abi.v1",
    ".fe2o3.moe.effects.v1",
    ".fe2o3.moe.profile.v1",
    ".fe2o3.moe.routing.v1",
    ".fe2o3.moe.kir.v1",
    ".fe2o3.moe.descriptor.v1",
    ".fe2o3.moe.provider.v1",
    ".fe2o3.moe.layout.v1",
];

const SECTION_IDENTITIES: [&str; 15] = [
    "0e4570bd52866dd23b8b00d83983aadc818c77580de8f7f5e2982e12a57e20e2",
    "4180ef61545684e646bd5227333e7514d22a2d379d7d657397df4d41f7a192d1",
    "fce826d20b8f2e4eca29180a2d9fc34949b51a07841dd7f79258625fc6a9f296",
    "0ecec41db62eae781429526170aa60a73437f4cd8261b7e4d34ffe62309ad6e9",
    "934c2205973e24216d537c5f89bc65d8e15dd68376dce477d1768e2936b4fc13",
    "f796180c590cd84125921f2aaeb85ab13ef1b5c0502c1b1316bf9a2114fd30f6",
    "4950c225e0cdbdce4e1230166984949970290dedc19e8dc4cd31f865f1625a4a",
    "3dbbe3ec9d58a7c285a14159294051498378f291525d8445113b17aab9b0e08b",
    "4c225cf47613b98e7baca366167bfa4c27ae43ec47433b49d1df5a1d960fb4aa",
    "496368f70c211b001417fb904622971d008ca24442beaef3e4c6c175b4f5f6ba",
    "100bc49f34627485a959b7201a238bbf8421df800d7f1028bbfff6bd8c51edd1",
    "a94a13c1ad0ac1498e1c6cc63416dc1cda2f7c14c5e4c1c422e354820fc09315",
    "3dfa5db91762403106e7d3a1581700b1d03282f5dd15727761e5cc42c63731b2",
    "7852334c9d38cd4544c535377650554344e8e59de2dc822f4f2492dfea998743",
    "9a0e923eef32bce3ef2de4663fc4d395cfd2179c55dd586180d9c25faa377536",
];

const SUCCESS_DIAGNOSTICS: [&str; 6] = [
    "post_link.check=exports status=ok symbols=[moe_top2_route_f32_t8_e4_k2_c4_v1,moe_top2_route_f32_t8_e4_k2_c4_v1.kd]",
    "post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-",
    "post_link.check=moe_top2_t8_e4_k2_c4_v1_profile status=ok tokens=8 experts=4 top_k=2 capacity=4 workgroup=[64,1,1] retained_grid=[1,1,1] explicit_kernarg_size=128 kernarg_size=384 kernarg_align=8 group_size=0 private_size=0 wavefront_size=64 calls=0 atomics=0 lds_bytes=0 spills=0 dynamic_stack=false provider_closure=none descriptor_binding=byte_exact rust_descriptor_admission=required",
    "post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c",
    "post_link.check=unresolved status=ok symbols=[]",
    "post_link.kernel name=moe_top2_route_f32_t8_e4_k2_c4_v1 symbol=moe_top2_route_f32_t8_e4_k2_c4_v1.kd kernarg_size=384 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=[64,1,1]",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeTop2V1CompilerPinsV1;

impl MoeTop2V1CompilerPinsV1 {
    pub const fn exact_t8_e4_k2_c4() -> Self {
        Self
    }

    pub fn source_identity(self) -> [u8; 32] {
        decode_hex_32(SECTION_IDENTITIES[0]).expect("fixed source identity")
    }

    pub fn source_authority_identity(self) -> [u8; 32] {
        decode_hex_32(SECTION_IDENTITIES[3]).expect("fixed authority identity")
    }

    pub fn kernel_ir_identity(self) -> [u8; 32] {
        decode_hex_32(SECTION_IDENTITIES[12]).expect("fixed KIR identity")
    }

    pub fn descriptor_profile_identity(self) -> [u8; 32] {
        decode_hex_32(SECTION_IDENTITIES[13]).expect("fixed descriptor identity")
    }

    pub const fn authenticates_compiler_origin(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeTop2V1DirectWorkerPinsV1 {
    executable: ContentIdentityV1,
    worker_build_identity_sha256: [u8; 32],
    llvm_build_identity_sha256: [u8; 32],
}

impl MoeTop2V1DirectWorkerPinsV1 {
    pub fn new(
        executable: ContentIdentityV1,
        worker_build_identity: &str,
        llvm_build_identity: &str,
    ) -> Result<Self, MoeTop2V1WorkerErrorV1> {
        if executable.byte_len() == 0 || executable.sha256() == &[0; 32] {
            return Err(profile_mismatch("worker executable identity"));
        }
        validate_identity_text(worker_build_identity, "worker build identity")?;
        validate_identity_text(llvm_build_identity, "LLVM build identity")?;
        Ok(Self {
            executable,
            worker_build_identity_sha256: sha256(worker_build_identity.as_bytes()),
            llvm_build_identity_sha256: sha256(llvm_build_identity.as_bytes()),
        })
    }

    pub const fn executable(self) -> ContentIdentityV1 {
        self.executable
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeTop2V1DirectWorkerExpectationV1 {
    handoff_sha256: [u8; 32],
    module: ContentIdentityV1,
    envelope_sha256: [u8; 32],
    manifest_sha256: [u8; 32],
    compiler: MoeTop2V1CompilerPinsV1,
    worker: MoeTop2V1DirectWorkerPinsV1,
}

impl MoeTop2V1DirectWorkerExpectationV1 {
    pub fn from_exact_compiler_handoff(
        handoff: &CompilerModuleHandoffV2,
        expected_handoff_sha256: [u8; 32],
        worker: MoeTop2V1DirectWorkerPinsV1,
    ) -> Result<Self, MoeTop2V1WorkerErrorV1> {
        validate_exact_handoff(handoff)?;
        if expected_handoff_sha256 == [0; 32]
            || handoff.identity().sha256() != &expected_handoff_sha256
        {
            return Err(profile_mismatch("pinned compiler handoff identity"));
        }
        Ok(Self {
            handoff_sha256: expected_handoff_sha256,
            module: ContentIdentityV1::calculate(handoff.module_bytes()),
            envelope_sha256: handoff.envelope().identity().as_bytes(),
            manifest_sha256: *handoff.symbol_manifest().identity().sha256(),
            compiler: MoeTop2V1CompilerPinsV1,
            worker,
        })
    }

    pub const fn compiler_pins(self) -> MoeTop2V1CompilerPinsV1 {
        self.compiler
    }

    pub const fn authenticates_pin_origin(self) -> bool {
        false
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MoeTop2V1WorkerExchangeIdentityV1([u8; 32]);

impl MoeTop2V1WorkerExchangeIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedMoeTop2V1WorkerExchangeV1 {
    identity: MoeTop2V1WorkerExchangeIdentityV1,
    compiler_module: ContentIdentityV1,
    linked_output: ContentIdentityV1,
}

impl ValidatedMoeTop2V1WorkerExchangeV1 {
    pub const fn identity(self) -> MoeTop2V1WorkerExchangeIdentityV1 {
        self.identity
    }

    pub const fn compiler_module_identity(self) -> ContentIdentityV1 {
        self.compiler_module
    }

    pub const fn linked_output_identity(self) -> ContentIdentityV1 {
        self.linked_output
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct InspectedMoeTop2V1WorkerV2HsacoV1 {
    exchange: ValidatedMoeTop2V1WorkerExchangeV1,
    raw: InspectedRawWorkerV2HsacoV1,
}

impl InspectedMoeTop2V1WorkerV2HsacoV1 {
    pub const fn exchange(&self) -> ValidatedMoeTop2V1WorkerExchangeV1 {
        self.exchange
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.raw.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.raw.code_object_version()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedMoeTop2V1WorkerExchangeV1,
        InspectedRawWorkerV2HsacoV1,
    ) {
        (self.exchange, self.raw)
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum MoeTop2V1WorkerErrorV1 {
    WorkerProtocol(WorkerProtocolError),
    RawHsaco(WorkerV2RawHsacoInspectionError),
    Descriptor(fe2o3_kernel_descriptor::DecodeError),
    ProfileMismatch(&'static str),
}

impl fmt::Display for MoeTop2V1WorkerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkerProtocol(error) => write!(formatter, "MoE Worker V2 failed: {error}"),
            Self::RawHsaco(error) => write!(formatter, "MoE raw HSACO failed: {error}"),
            Self::Descriptor(error) => write!(formatter, "MoE descriptor failed: {error}"),
            Self::ProfileMismatch(field) => {
                write!(formatter, "exact T8/E4/K2/C4 MoE profile mismatch: {field}")
            }
        }
    }
}

impl Error for MoeTop2V1WorkerErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::WorkerProtocol(error) => Some(error),
            Self::RawHsaco(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            Self::ProfileMismatch(_) => None,
        }
    }
}

pub fn validate_moe_top2_v1_worker_exchange_v1(
    source: &InertFirstBuildWorkerV2EvidenceV1,
    expected: MoeTop2V1DirectWorkerExpectationV1,
) -> Result<ValidatedMoeTop2V1WorkerExchangeV1, MoeTop2V1WorkerErrorV1> {
    let bootstrap = InertDecodedWorkerExchangeV2::decode(
        source.bootstrap_request_bytes(),
        source.bootstrap().response().canonical_bytes(),
    )
    .map_err(MoeTop2V1WorkerErrorV1::WorkerProtocol)?;
    let replay = InertDecodedWorkerExchangeV2::decode(
        source.authorized_request_bytes(),
        source.authorized().response().canonical_bytes(),
    )
    .map_err(MoeTop2V1WorkerErrorV1::WorkerProtocol)?;
    validate_exchange(&bootstrap, expected)?;
    validate_exchange(&replay, expected)?;
    if bootstrap.response().output().map(|value| value.bytes())
        != replay.response().output().map(|value| value.bytes())
    {
        return Err(profile_mismatch("bootstrap/replay output reproducibility"));
    }
    if source.compiler_envelope_identity().as_bytes() != expected.envelope_sha256
        || source.symbol_manifest().identity().sha256() != &expected.manifest_sha256
        || source.worker_measurement().executable() != expected.worker.executable
        || sha256(
            source
                .worker_measurement()
                .worker_build_identity()
                .as_bytes(),
        ) != expected.worker.worker_build_identity_sha256
        || sha256(source.worker_measurement().llvm_build_identity().as_bytes())
            != expected.worker.llvm_build_identity_sha256
    {
        return Err(profile_mismatch("retained compiler/worker lineage"));
    }
    let output = replay
        .response()
        .output()
        .ok_or_else(|| profile_mismatch("completed replay output"))?;
    if source.output_identity() != output.identity()
        || !source.output_identity().matches(source.output_bytes())
    {
        return Err(profile_mismatch("retained output identity"));
    }
    Ok(ValidatedMoeTop2V1WorkerExchangeV1 {
        identity: calculate_exchange_identity(expected, &bootstrap, &replay),
        compiler_module: expected.module,
        linked_output: output.identity(),
    })
}

pub fn inspect_moe_top2_v1_worker_v2_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
    expected: MoeTop2V1DirectWorkerExpectationV1,
) -> Result<InspectedMoeTop2V1WorkerV2HsacoV1, MoeTop2V1WorkerErrorV1> {
    let exchange = validate_moe_top2_v1_worker_exchange_v1(&source, expected)?;
    let raw = inspect_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::MOE_TOP2_V1,
        WorkerV2RawLaunchDiagnosticProfileV1::MoeTop2V1,
    )
    .map_err(MoeTop2V1WorkerErrorV1::RawHsaco)?;
    if raw.target().to_string() != TARGET || raw.code_object_version() != CodeObjectVersion::V6 {
        return Err(profile_mismatch("inspected target/code-object version"));
    }
    if raw.policy().observed_kernels().len() != 1
        || raw.policy().observed_kernels()[0].entry() != KERNEL
        || raw.policy().observed_kernels()[0].descriptor() != DESCRIPTOR
    {
        return Err(profile_mismatch("inspected kernel/descriptor closure"));
    }
    validate_hsaco_metadata(raw.exact_bytes())?;
    Ok(InspectedMoeTop2V1WorkerV2HsacoV1 { exchange, raw })
}

fn validate_exact_handoff(handoff: &CompilerModuleHandoffV2) -> Result<(), MoeTop2V1WorkerErrorV1> {
    let expected_manifest = [
        (CompilerModuleSymbolRoleV1::KernelEntry, KERNEL),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, DESCRIPTOR),
    ];
    if handoff.kind() != CompilerModuleKindV1::LlvmTextIr
        || handoff.target().to_string() != TARGET
        || handoff.code_object_version() != CodeObjectVersion::V6
        || handoff.envelope().inspection().import_count() != 0
        || handoff.envelope().inspection().export_count() != 0
        || handoff
            .envelope()
            .inspection()
            .requires_compiler_module_definition_count()
            != 0
        || handoff.symbol_manifest().entries().collect::<Vec<_>>() != expected_manifest
    {
        return Err(profile_mismatch("compiler handoff envelope/manifest"));
    }
    let (body, sections) = decode_exact_sections(handoff.module_bytes())?;
    if !matches_hex(&sha256(body), BODY_SHA256) {
        return Err(profile_mismatch("compiler LLVM body identity"));
    }
    for (bytes, expected) in sections.iter().skip(1).take(15).zip(SECTION_IDENTITIES) {
        if !matches_hex(bytes, expected) {
            return Err(profile_mismatch("source/KIR/compiler/profile identity"));
        }
    }
    if sections[16] != sha256(EXACT_DATA_LAYOUT.as_bytes()) {
        return Err(profile_mismatch("target-machine data-layout identity"));
    }
    validate_descriptor(&sections[0])
}

fn validate_descriptor(bytes: &[u8]) -> Result<(), MoeTop2V1WorkerErrorV1> {
    let table =
        decode_device_descriptor_table_v1(bytes).map_err(MoeTop2V1WorkerErrorV1::Descriptor)?;
    let [kernel] = table.kernels() else {
        return Err(profile_mismatch("descriptor kernel closure"));
    };
    let launch = kernel.launch();
    let exact_block = match launch.block_size() {
        BlockSizeV1::Exact(value) => value,
        _ => return Err(profile_mismatch("descriptor block size")),
    };
    if table.canonical_code_object_digest() != CanonicalCodeObjectDigest::from_bytes([0; 32])
        || table.code_object_version() != CodeObjectVersion::V6
        || table.device_target().to_string() != TARGET
        || kernel.logical_name().as_str() != KERNEL
        || kernel.entry_name().as_str() != KERNEL
        || kernel.descriptor_symbol().as_str() != DESCRIPTOR
        || kernel.abi_layout().explicit_argument_size() != 128
        || kernel.abi_layout().kernarg_segment_size() != 384
        || kernel.abi_layout().kernarg_segment_alignment() != 8
        || [exact_block.x(), exact_block.y(), exact_block.z()] != [64, 1, 1]
        || [
            launch.max_grid().x(),
            launch.max_grid().y(),
            launch.max_grid().z(),
        ] != [1, 1, 1]
        || launch.max_flat_workgroup_size() != 64
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
        || kernel.arguments().len() != 8
    {
        return Err(profile_mismatch("descriptor ABI/resource profile"));
    }
    let names = [
        "logits",
        "top2_experts",
        "requested_counts",
        "admitted_counts",
        "expert_offsets",
        "route_slots",
        "permutation",
        "inverse",
    ];
    for (index, (argument, name)) in kernel.arguments().iter().zip(names).enumerate() {
        let input = index == 0;
        if argument.source_index() != index as u16
            || argument.name().as_str() != name
            || argument.access()
                != if input {
                    AccessMode::ReadOnly
                } else {
                    AccessMode::ReadWrite
                }
            || argument.alias()
                != if input {
                    AliasSemantics::SharedReadOnly
                } else {
                    AliasSemantics::Exclusive
                }
            || argument.ownership()
                != if input {
                    OwnershipSemantics::SharedBorrow
                } else {
                    OwnershipSemantics::UniqueBorrow
                }
        {
            return Err(profile_mismatch("descriptor argument roles/aliasing"));
        }
        let components: Vec<_> = argument.physical_components().collect();
        if components.len() != 2
            || components[0].1 != (index * 16) as u32
            || components[1].1 != (index * 16 + 8) as u32
        {
            return Err(profile_mismatch("descriptor physical ABI"));
        }
    }
    Ok(())
}

fn validate_exchange(
    exchange: &InertDecodedWorkerExchangeV2,
    expected: MoeTop2V1DirectWorkerExpectationV1,
) -> Result<(), MoeTop2V1WorkerErrorV1> {
    let request = exchange.request();
    let response = exchange.response();
    if request.target().to_string() != TARGET
        || request.code_object_version() != CodeObjectVersion::V6
        || request.options() != WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true)
        || request.compiler_envelope_identity().as_bytes() != expected.envelope_sha256
        || request.compiler_module().kind() != WorkerInputKindV1::LlvmTextIr
        || request.compiler_module().identity() != expected.module
        || !request.external_providers().is_empty()
        || !request.import_symbols().is_empty()
        || !request.export_symbols().is_empty()
        || request.final_symbols() != [KERNEL, DESCRIPTOR]
        || request.worker_executable() != expected.worker.executable
        || sha256(request.worker_build_identity().as_bytes())
            != expected.worker.worker_build_identity_sha256
        || sha256(request.llvm_build_identity().as_bytes())
            != expected.worker.llvm_build_identity_sha256
    {
        return Err(profile_mismatch("exact Worker V2 request"));
    }
    if !response.binds_request(request)
        || response.worker_build_identity() != request.worker_build_identity()
        || response.stage() != WorkerStageV1::Complete
        || response.device_library_provider().is_some()
        || response.diagnostics() != SUCCESS_DIAGNOSTICS
    {
        return Err(profile_mismatch("exact Worker V2 response"));
    }
    let output = response
        .output()
        .ok_or_else(|| profile_mismatch("completed Worker V2 output"))?;
    if output.request_identity() != request.identity()
        || output.compiler_envelope_identity() != request.compiler_envelope_identity()
        || !output.identity().matches(output.bytes())
        || output.identity().byte_len() > request.output_constraints().max_bytes()
    {
        return Err(profile_mismatch("Worker V2 output binding"));
    }
    Ok(())
}

fn validate_hsaco_metadata(bytes: &[u8]) -> Result<(), MoeTop2V1WorkerErrorV1> {
    let inspected =
        fe2o3_hsaco::inspect(bytes).map_err(|_| profile_mismatch("independent COV6 metadata"))?;
    let [kernel] = inspected.kernels() else {
        return Err(profile_mismatch("metadata kernel closure"));
    };
    if kernel.name() != KERNEL
        || kernel.symbol() != DESCRIPTOR
        || kernel.kernarg_segment_size() != 384
        || kernel.kernarg_segment_alignment() != 8
        || kernel.implicit_argument_offset() != Some(128)
        || kernel.implicit_argument_size() != 256
        || kernel.group_segment_fixed_size() != 0
        || kernel.private_segment_fixed_size() != 0
        || kernel.wavefront_size() != 64
        || kernel.max_flat_workgroup_size() != 64
        || kernel.required_workgroup_size() != Some([64, 1, 1])
        || kernel.cluster_dims().is_some()
        || kernel.kind() != KernelKind::Normal
        || kernel.uniform_work_group_size()
        || kernel.uses_dynamic_stack()
        || kernel.device_enqueue_symbol().is_some()
        || kernel.sgpr_count() == 0
        || kernel.vgpr_count() == 0
        || kernel.sgpr_spill_count().unwrap_or(0) != 0
        || kernel.vgpr_spill_count().unwrap_or(0) != 0
        || kernel.explicit_arguments().len() != 16
        || kernel
            .hidden_arguments()
            .iter()
            .any(|argument| argument.value_kind() == HiddenValueKind::DynamicLdsSize)
    {
        return Err(profile_mismatch("metadata ABI/resource closure"));
    }
    let pointer_names = [
        "logits.data",
        "top2.data",
        "requested.data",
        "admitted.data",
        "offsets.data",
        "slots.data",
        "permutation.data",
        "inverse.data",
    ];
    let length_names = [
        "logits.len",
        "top2.len",
        "requested.len",
        "admitted.len",
        "offsets.len",
        "slots.len",
        "permutation.len",
        "inverse.len",
    ];
    for role in 0..8 {
        let pointer = &kernel.explicit_arguments()[role * 2];
        let length = &kernel.explicit_arguments()[role * 2 + 1];
        let input = role == 0;
        if pointer.name() != Some(pointer_names[role])
            || pointer.offset() != (role * 16) as u64
            || pointer.size() != 8
            || pointer.value_kind() != ExplicitValueKind::GlobalBuffer
            || pointer.value_type().is_some()
                && pointer.value_type()
                    != Some(if input {
                        ExplicitValueType::F32
                    } else {
                        ExplicitValueType::U32
                    })
            || pointer.address_space() != Some(ArgumentAddressSpace::Global)
            || pointer.access()
                != Some(if input {
                    ArgumentAccess::ReadOnly
                } else {
                    ArgumentAccess::ReadWrite
                })
            || length.name() != Some(length_names[role])
            || length.offset() != (role * 16 + 8) as u64
            || length.size() != 8
            || length.value_kind() != ExplicitValueKind::ByValue
            || length.value_type().is_some() && length.value_type() != Some(ExplicitValueType::U64)
        {
            return Err(profile_mismatch("metadata explicit argument ABI"));
        }
    }
    Ok(())
}

fn decode_exact_sections(module: &[u8]) -> Result<(&[u8], Vec<Vec<u8>>), MoeTop2V1WorkerErrorV1> {
    let text = std::str::from_utf8(module).map_err(|_| profile_mismatch("textual LLVM UTF-8"))?;
    let marker = "\nmodule asm \".section ";
    let body_end = text
        .find(marker)
        .ok_or_else(|| profile_mismatch("identity section presence"))?;
    let body = &module[..body_end];
    let mut lines = text[body_end + 1..].split('\n').peekable();
    let mut decoded = Vec::with_capacity(SECTION_NAMES.len());
    for (index, section) in SECTION_NAMES.iter().enumerate() {
        if index != 0 && lines.peek() == Some(&"") {
            lines.next();
        }
        let header = format!("module asm \".section {section},\\22\\22,@progbits\"");
        if lines.next() != Some(header.as_str()) || lines.next() != Some("module asm \".balign 8\"")
        {
            return Err(profile_mismatch("identity section order/envelope"));
        }
        let mut bytes = Vec::new();
        while lines
            .peek()
            .is_some_and(|line| line.starts_with("module asm \".byte "))
        {
            let line = lines.next().expect("peeked line");
            let atoms = line
                .strip_prefix("module asm \".byte ")
                .and_then(|line| line.strip_suffix('"'))
                .ok_or_else(|| profile_mismatch("identity byte record"))?;
            let atoms: Vec<_> = atoms.split(", ").collect();
            if atoms.is_empty() || atoms.len() > 16 {
                return Err(profile_mismatch("identity byte record width"));
            }
            for atom in atoms {
                if atom.len() != 4 || !atom.starts_with("0x") {
                    return Err(profile_mismatch("identity byte atom"));
                }
                bytes.push(
                    u8::from_str_radix(&atom[2..], 16)
                        .map_err(|_| profile_mismatch("identity byte atom"))?,
                );
            }
        }
        if bytes.is_empty() {
            return Err(profile_mismatch("empty identity section"));
        }
        decoded.push(bytes);
    }
    if lines.any(|line| !line.is_empty()) {
        return Err(profile_mismatch("trailing module assembly"));
    }
    Ok((body, decoded))
}

fn calculate_exchange_identity(
    expected: MoeTop2V1DirectWorkerExpectationV1,
    bootstrap: &InertDecodedWorkerExchangeV2,
    replay: &InertDecodedWorkerExchangeV2,
) -> MoeTop2V1WorkerExchangeIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(EXCHANGE_IDENTITY_DOMAIN);
    digest.update(expected.handoff_sha256);
    digest.update(expected.module.sha256());
    digest.update(expected.module.byte_len().to_le_bytes());
    digest.update(expected.envelope_sha256);
    digest.update(expected.manifest_sha256);
    digest.update(bootstrap.request().canonical_bytes());
    digest.update(bootstrap.response().canonical_bytes());
    digest.update(replay.request().canonical_bytes());
    digest.update(replay.response().canonical_bytes());
    MoeTop2V1WorkerExchangeIdentityV1(digest.finalize().into())
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut decoded = [0; 32];
    for (index, byte) in decoded.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(decoded)
}

fn matches_hex(bytes: &[u8], expected: &str) -> bool {
    decode_hex_32(expected).is_some_and(|value| bytes == value)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn validate_identity_text(value: &str, field: &'static str) -> Result<(), MoeTop2V1WorkerErrorV1> {
    if value.is_empty()
        || value.len() > 160
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._+:=~-".contains(&byte))
    {
        return Err(profile_mismatch(field));
    }
    Ok(())
}

fn profile_mismatch(field: &'static str) -> MoeTop2V1WorkerErrorV1 {
    MoeTop2V1WorkerErrorV1::ProfileMismatch(field)
}
