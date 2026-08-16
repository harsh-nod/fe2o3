//! Private producer for an exact MoE source/FnAbi/MIR/KIR structural record.
//!
//! The record is produced only inside the already authenticated rustc path. It
//! mechanically serializes the observed `FnAbi`, a whole-module portable-MIR
//! summary, and every field of the already validated MoE KIR and profile. It
//! is diagnostic structural evidence, not a MIR-to-KIR simulation or a
//! semantic refinement proof, and it grants no downstream authority.

use std::error::Error;
use std::fmt;

use fe2o3_kernel_ir::{
    AddressSpace, MoeTop2ArgumentRoleV1, MoeTop2ArgumentShapeV1, MoeTop2CorrespondenceV1,
    MoeTop2FiniteInputPolicyV1, MoeTop2KernelIrV1, MoeTop2LayoutV1, MoeTop2OutputOwnershipPolicyV1,
    MoeTop2OverflowV1, MoeTop2ProfileV1, MoeTop2RoutingStepV1, MoeTop2TieBreakV1, ScalarType,
    SynchronizationScope, TargetCapability,
};
use sha2::{Digest as _, Sha256};

use crate::mir_import::{
    MirBinaryOp, MirFunctionKind, MirModule, MirOperandRef, MirPlaceRef, MirProjectionElem,
    MirRvalueKind, MirStatementKind, MirTerminatorKind,
};

const RECORD_DOMAIN_V2: &[u8] = b"fe2o3.moe-top2.private-live-structural-record.v2\0";
const CANONICAL_TABLE_DOMAIN_V2: &[u8] = b"fe2o3.moe-top2.canonical-field-table.v2\0";
const KERNEL_IR_DOMAIN_V2: &[u8] = b"fe2o3.moe-top2.canonical-kernel-ir.v2\0";
const PROFILE_DOMAIN_V2: &[u8] = b"fe2o3.moe-top2.canonical-profile.v2\0";
const ABI_PROJECTION_DOMAIN_V2: &[u8] = b"fe2o3.moe-top2.canonical-abi-projection.v2\0";
const EFFECTS_PROJECTION_DOMAIN_V2: &[u8] = b"fe2o3.moe-top2.canonical-effects-projection.v2\0";
const ROUTING_PROJECTION_DOMAIN_V2: &[u8] = b"fe2o3.moe-top2.canonical-routing-projection.v2\0";

const SOURCE_IDENTITY: [u8; 32] = [
    0xb7, 0x70, 0x16, 0xca, 0xa0, 0xc3, 0x70, 0x8e, 0x42, 0x0e, 0x58, 0x37, 0x12, 0xe6, 0x5e, 0x4e,
    0x64, 0x28, 0xdb, 0x7b, 0x4f, 0xea, 0xfd, 0x8d, 0x0a, 0x1d, 0x4b, 0xdc, 0x47, 0x5e, 0xf6, 0xff,
];
const PORTABLE_MIR_IDENTITY: [u8; 32] = [
    0x93, 0x4c, 0x22, 0x05, 0x97, 0x3e, 0x24, 0x21, 0x6d, 0x53, 0x7c, 0x5f, 0x89, 0xbc, 0x65, 0xd8,
    0xe1, 0x5d, 0xd6, 0x83, 0x76, 0xdc, 0xe4, 0x77, 0xd1, 0x76, 0x8e, 0x29, 0x36, 0xb4, 0xfc, 0x13,
];
const FN_ABI_IDENTITY: [u8; 32] = [
    0xf7, 0x96, 0x18, 0x0c, 0x59, 0x0c, 0xd8, 0x41, 0x25, 0x92, 0x1f, 0x2a, 0xae, 0xb8, 0x5a, 0xb1,
    0x3e, 0xf1, 0xb5, 0xc0, 0x50, 0x2c, 0x1b, 0x13, 0x16, 0xbf, 0x9a, 0x21, 0x14, 0xfd, 0x30, 0xf6,
];

// Filled from the exact live rustc admission. The text is deliberately
// readable: reviewers can see every checked structural input without
// reverse-engineering the final record digest.
pub(crate) const MOE_TOP2_LIVE_STRUCTURAL_SNAPSHOT_V2: &str = "schema=moe-top2-private-structural-v2;source=b77016caa0c3708e420e583712e65e4e6428db7b4feafd8d0a1d4bdc475ef6ff;fnabi=f796180c590cd84125921f2aaeb85ab13ef1b5c0502c1b1316bf9a2114fd30f6:rust=1:variadic=0:fixed=8:unwind=1:ignored=1:result=0:args=[16:8:1:0:0,16:8:1:0:0,16:8:1:0:0,16:8:1:0:0,16:8:1:0:0,16:8:1:0:0,16:8:1:0:0,16:8:1:0:0];mir=934c2205973e24216d537c5f89bc65d8e15dd68376dce477d1768e2936b4fc13:functions=6:roots=1:helpers=5:blocks=120:statements=222:terminators=120:edges=142:imports=0:root-args=8:root-locals=171:assignments=220:calls=27:indexed=36:repeats=8:binops=0x0000ec05;kir=bdf19330357f898eb0267372e4115b33e4b60902c94b5b3b6e358f5703ee7eb0;profile=deedd95c97b7bc3f146798f468c5eee6934a870d319fe441c6f0ec01f6a6afd8;abi=f0abdc459360d1760e22c62f77830472ae9a3e151b5ec7323d56c5298dc87365;effects=4f7a7d0996535ee75ff22216c776666526caa93106a49c2e8cfb4956cb0f7716;routing=dc93201e71d4ba820f52bb44833f4592383b2023f67089a4cc1b73aae14f051b";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MoeSourceKirFnAbiArgumentV1 {
    pub(crate) size: u16,
    pub(crate) alignment: u16,
    pub(crate) pair_mode: bool,
    pub(crate) first_pointee_bytes: u32,
    pub(crate) second_pointee_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MoeSourceKirFnAbiV1 {
    pub(crate) identity: [u8; 32],
    pub(crate) rust_calling_convention: bool,
    pub(crate) c_variadic: bool,
    pub(crate) fixed_count: u8,
    pub(crate) can_unwind: bool,
    pub(crate) result_ignored: bool,
    pub(crate) result_size: u16,
    pub(crate) arguments: [MoeSourceKirFnAbiArgumentV1; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PortableMirSummaryV2 {
    identity: [u8; 32],
    function_count: u32,
    kernel_root_count: u32,
    helper_count: u32,
    block_count: u32,
    statement_count: u32,
    terminator_count: u32,
    edge_count: u32,
    external_import_count: u32,
    root_argument_count: u32,
    root_local_count: u32,
    assignment_count: u32,
    call_count: u32,
    indexed_place_count: u32,
    repeat_count: u32,
    binary_operation_mask: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CanonicalKernelProfileV2 {
    fields: Vec<CanonicalFieldV2>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CanonicalFieldV2 {
    name: &'static str,
    memberships: u8,
    value: Vec<u8>,
}

const MEMBER_KERNEL_IR: u8 = 1 << 0;
const MEMBER_PROFILE: u8 = 1 << 1;
const MEMBER_ABI: u8 = 1 << 2;
const MEMBER_EFFECTS: u8 = 1 << 3;
const MEMBER_ROUTING: u8 = 1 << 4;

#[derive(Clone, Debug, Eq, PartialEq)]
struct StructuralInputsV2 {
    source: Vec<u8>,
    source_identity: [u8; 32],
    fn_abi: MoeSourceKirFnAbiV1,
    portable_mir: PortableMirSummaryV2,
    canonical: CanonicalKernelProfileV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedMoeSourceKirStructuralRecordV2 {
    identity: [u8; 32],
    source_identity: [u8; 32],
    fn_abi_identity: [u8; 32],
    portable_mir_identity: [u8; 32],
    kernel_ir_identity: [u8; 32],
    profile_identity: [u8; 32],
    snapshot: String,
}

impl CheckedMoeSourceKirStructuralRecordV2 {
    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(crate) const fn source_identity(&self) -> [u8; 32] {
        self.source_identity
    }

    pub(crate) const fn fn_abi_identity(&self) -> [u8; 32] {
        self.fn_abi_identity
    }

    pub(crate) const fn portable_mir_identity(&self) -> [u8; 32] {
        self.portable_mir_identity
    }

    pub(crate) const fn kernel_ir_identity(&self) -> [u8; 32] {
        self.kernel_ir_identity
    }

    pub(crate) const fn profile_identity(&self) -> [u8; 32] {
        self.profile_identity
    }

    pub(crate) fn snapshot(&self) -> &str {
        &self.snapshot
    }

    pub(crate) const fn proves_source_to_kir_semantic_refinement(&self) -> bool {
        false
    }

    pub(crate) const fn proves_llvm_or_isa_refinement(&self) -> bool {
        false
    }

    pub(crate) const fn proves_logical_to_machine_address_refinement(&self) -> bool {
        false
    }

    pub(crate) const fn proves_ieee_fp32_or_ocml_semantics(&self) -> bool {
        false
    }

    pub(crate) const fn proves_generalized_memory_safety_or_race_freedom(&self) -> bool {
        false
    }

    pub(crate) const fn proves_gpu_execution(&self) -> bool {
        false
    }

    pub(crate) const fn grants_artifact_authority(&self) -> bool {
        false
    }

    pub(crate) const fn grants_load_authority(&self) -> bool {
        false
    }

    pub(crate) const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MoeSourceKirProducerErrorV2 {
    MissingKernelRoot,
    CountOverflow(&'static str),
    Source,
    FnAbi,
    PortableMir,
    SnapshotMismatch { actual: String },
}

impl fmt::Display for MoeSourceKirProducerErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKernelRoot => {
                formatter.write_str("reachable portable MIR omitted the exact MoE kernel root")
            }
            Self::CountOverflow(field) => {
                write!(
                    formatter,
                    "reachable portable MIR {field} count overflowed u32"
                )
            }
            Self::Source => formatter.write_str("authenticated source observation drifted"),
            Self::FnAbi => formatter.write_str("authenticated rustc FnAbi observation drifted"),
            Self::PortableMir => {
                formatter.write_str("authenticated portable-MIR observation drifted")
            }
            Self::SnapshotMismatch { actual } => write!(
                formatter,
                "live structural snapshot differs from its reviewed pin; observed {actual}"
            ),
        }
    }
}

impl Error for MoeSourceKirProducerErrorV2 {}

pub(crate) fn produce_checked_moe_source_kir_structural_record_v2(
    source: &[u8],
    source_identity: [u8; 32],
    fn_abi: MoeSourceKirFnAbiV1,
    portable_mir: &MirModule,
    portable_mir_identity: [u8; 32],
    ir: &MoeTop2KernelIrV1,
    profile: &MoeTop2ProfileV1,
) -> Result<CheckedMoeSourceKirStructuralRecordV2, MoeSourceKirProducerErrorV2> {
    let inputs = StructuralInputsV2 {
        source: source.to_vec(),
        source_identity,
        fn_abi,
        portable_mir: summarize_portable_mir(portable_mir, portable_mir_identity)?,
        canonical: canonical_kernel_profile(ir, profile),
    };
    check_structural_inputs(inputs)
}

pub(crate) fn canonical_kernel_profile_identities_v2(
    ir: &MoeTop2KernelIrV1,
    profile: &MoeTop2ProfileV1,
) -> ([u8; 32], [u8; 32]) {
    let canonical = canonical_kernel_profile(ir, profile);
    (
        canonical.identity(KERNEL_IR_DOMAIN_V2, MEMBER_KERNEL_IR),
        canonical.identity(PROFILE_DOMAIN_V2, MEMBER_PROFILE),
    )
}

fn check_structural_inputs(
    inputs: StructuralInputsV2,
) -> Result<CheckedMoeSourceKirStructuralRecordV2, MoeSourceKirProducerErrorV2> {
    let actual_source_identity: [u8; 32] = Sha256::digest(&inputs.source).into();
    if inputs.source.is_empty()
        || inputs.source.len() > 16 * 1024
        || actual_source_identity != SOURCE_IDENTITY
        || inputs.source_identity != actual_source_identity
    {
        return Err(MoeSourceKirProducerErrorV2::Source);
    }
    if !fn_abi_is_exact(&inputs.fn_abi) {
        return Err(MoeSourceKirProducerErrorV2::FnAbi);
    }
    if !portable_mir_is_bounded(&inputs.portable_mir) {
        return Err(MoeSourceKirProducerErrorV2::PortableMir);
    }

    let snapshot = snapshot_text(&inputs);
    if snapshot != MOE_TOP2_LIVE_STRUCTURAL_SNAPSHOT_V2 {
        return Err(MoeSourceKirProducerErrorV2::SnapshotMismatch { actual: snapshot });
    }

    let fn_abi_bytes = canonical_fn_abi(&inputs.fn_abi);
    let mir_bytes = canonical_portable_mir(&inputs.portable_mir);
    let mut record = CanonicalWriter::new(RECORD_DOMAIN_V2);
    record.field(&inputs.source);
    record.field(&inputs.source_identity);
    record.field(&fn_abi_bytes);
    record.field(&mir_bytes);
    record.field(&inputs.canonical.canonical_table());

    Ok(CheckedMoeSourceKirStructuralRecordV2 {
        identity: sha256(&record.finish()),
        source_identity: inputs.source_identity,
        fn_abi_identity: inputs.fn_abi.identity,
        portable_mir_identity: inputs.portable_mir.identity,
        kernel_ir_identity: inputs
            .canonical
            .identity(KERNEL_IR_DOMAIN_V2, MEMBER_KERNEL_IR),
        profile_identity: inputs.canonical.identity(PROFILE_DOMAIN_V2, MEMBER_PROFILE),
        snapshot,
    })
}

fn fn_abi_is_exact(abi: &MoeSourceKirFnAbiV1) -> bool {
    abi.identity == FN_ABI_IDENTITY
        && abi.rust_calling_convention
        && !abi.c_variadic
        && abi.fixed_count == 8
        && abi.can_unwind
        && abi.result_ignored
        && abi.result_size == 0
        && abi.arguments.iter().all(|argument| {
            argument.size == 16
                && argument.alignment == 8
                && argument.pair_mode
                && argument.first_pointee_bytes == 0
                && argument.second_pointee_bytes == 0
        })
}

fn portable_mir_is_bounded(mir: &PortableMirSummaryV2) -> bool {
    mir.identity == PORTABLE_MIR_IDENTITY
        && (1..=32).contains(&mir.function_count)
        && mir.kernel_root_count == 1
        && mir.helper_count + mir.kernel_root_count == mir.function_count
        && (1..=4_096).contains(&mir.block_count)
        && (1..=16_384).contains(&mir.statement_count)
        && mir.terminator_count == mir.block_count
        && (1..=16_384).contains(&mir.edge_count)
        && mir.external_import_count == 0
        && mir.root_argument_count == 8
        && mir.root_local_count >= mir.root_argument_count
        && mir.assignment_count > 0
        && mir.call_count > 0
        && mir.indexed_place_count > 0
        && mir.repeat_count > 0
        && mir.binary_operation_mask != 0
}

fn summarize_portable_mir(
    module: &MirModule,
    identity: [u8; 32],
) -> Result<PortableMirSummaryV2, MoeSourceKirProducerErrorV2> {
    let roots = module
        .functions
        .iter()
        .filter(|function| function.kind == MirFunctionKind::KernelEntry)
        .collect::<Vec<_>>();
    let root = roots
        .first()
        .ok_or(MoeSourceKirProducerErrorV2::MissingKernelRoot)?;

    let mut summary = PortableMirSummaryV2 {
        identity,
        function_count: count(module.functions.len(), "function")?,
        kernel_root_count: count(roots.len(), "kernel root")?,
        helper_count: count(
            module
                .functions
                .iter()
                .filter(|function| function.kind == MirFunctionKind::InternalHelper)
                .count(),
            "helper",
        )?,
        block_count: 0,
        statement_count: 0,
        terminator_count: 0,
        edge_count: 0,
        external_import_count: 0,
        root_argument_count: count(root.arg_count, "root argument")?,
        root_local_count: count(root.local_count, "root local")?,
        assignment_count: 0,
        call_count: 0,
        indexed_place_count: 0,
        repeat_count: 0,
        binary_operation_mask: 0,
    };

    for function in &module.functions {
        add_count(&mut summary.block_count, function.blocks.len(), "block")?;
        for block in &function.blocks {
            add_count(
                &mut summary.statement_count,
                block.statements.len(),
                "statement",
            )?;
            for statement in &block.statements {
                if statement.kind == MirStatementKind::Assign {
                    add_count(&mut summary.assignment_count, 1, "assignment")?;
                }
                if statement.rvalue == Some(MirRvalueKind::Repeat) {
                    add_count(&mut summary.repeat_count, 1, "repeat")?;
                }
                if let Some(MirRvalueKind::Binary(operation)) = statement.rvalue {
                    summary.binary_operation_mask |= binary_operation_bit(operation);
                }
                add_count(
                    &mut summary.indexed_place_count,
                    count_statement_indexed_places(
                        statement.destination.as_ref(),
                        &statement.operands,
                    ),
                    "indexed place",
                )?;
            }

            let Some(terminator) = &block.terminator else {
                continue;
            };
            add_count(&mut summary.terminator_count, 1, "terminator")?;
            add_count(
                &mut summary.edge_count,
                terminator_edges(&terminator.kind),
                "edge",
            )?;
            match &terminator.kind {
                MirTerminatorKind::Call {
                    callee,
                    destination,
                    operands,
                    ..
                } => {
                    add_count(&mut summary.call_count, 1, "call")?;
                    add_count(
                        &mut summary.indexed_place_count,
                        count_statement_indexed_places(destination.as_ref(), operands),
                        "indexed place",
                    )?;
                    if callee
                        .as_ref()
                        .is_some_and(|value| value.external_import_evidence().is_some())
                    {
                        add_count(&mut summary.external_import_count, 1, "external import")?;
                    }
                }
                MirTerminatorKind::SwitchInt { discriminant, .. } => add_count(
                    &mut summary.indexed_place_count,
                    count_operand_indexed_places(discriminant),
                    "indexed place",
                )?,
                MirTerminatorKind::Assert { condition, .. } => add_count(
                    &mut summary.indexed_place_count,
                    count_operand_indexed_places(condition),
                    "indexed place",
                )?,
                MirTerminatorKind::Return
                | MirTerminatorKind::Unreachable
                | MirTerminatorKind::Goto { .. }
                | MirTerminatorKind::Drop { .. }
                | MirTerminatorKind::Other => {}
            }
        }
    }
    Ok(summary)
}

fn canonical_kernel_profile(
    ir: &MoeTop2KernelIrV1,
    profile: &MoeTop2ProfileV1,
) -> CanonicalKernelProfileV2 {
    let mut canonical = CanonicalKernelProfileV2 { fields: Vec::new() };
    canonical.push("kir.module-id", MEMBER_KERNEL_IR, ir.module_id.as_bytes());
    canonical.push(
        "kir.function-id",
        MEMBER_KERNEL_IR,
        ir.function_id.as_bytes(),
    );
    canonical.push(
        "kir.kernel-id",
        MEMBER_KERNEL_IR | MEMBER_ABI,
        ir.kernel_id.as_bytes(),
    );
    canonical.push(
        "kir.arguments",
        MEMBER_KERNEL_IR | MEMBER_ABI | MEMBER_EFFECTS,
        &encode_arguments(ir),
    );
    canonical.push(
        "kir.shape",
        MEMBER_KERNEL_IR | MEMBER_ROUTING,
        &encode_shape(ir),
    );
    canonical.push(
        "kir.layout",
        MEMBER_KERNEL_IR | MEMBER_ROUTING,
        &[layout_tag(ir.layout)],
    );
    canonical.push(
        "kir.finite-input",
        MEMBER_KERNEL_IR | MEMBER_EFFECTS | MEMBER_ROUTING,
        &[finite_input_tag(ir.finite_input)],
    );
    canonical.push(
        "kir.tie-break",
        MEMBER_KERNEL_IR | MEMBER_ROUTING,
        &[tie_break_tag(ir.tie_break)],
    );
    canonical.push(
        "kir.overflow",
        MEMBER_KERNEL_IR | MEMBER_ROUTING,
        &[overflow_tag(ir.overflow)],
    );
    canonical.push(
        "kir.routing-steps",
        MEMBER_KERNEL_IR | MEMBER_ROUTING,
        &encode_routing_steps(ir),
    );
    canonical.push(
        "kir.packing",
        MEMBER_KERNEL_IR | MEMBER_ROUTING,
        &encode_packing(ir),
    );
    canonical.push(
        "kir.ownership",
        MEMBER_KERNEL_IR | MEMBER_EFFECTS,
        &encode_ownership(ir),
    );

    canonical.push(
        "profile.source-sha256",
        MEMBER_PROFILE,
        &profile.source_sha256,
    );
    canonical.push("profile.namespace", MEMBER_PROFILE, &profile.namespace);
    canonical.push(
        "profile.target",
        MEMBER_PROFILE,
        &encode_target_capability(&profile.target),
    );
    canonical.push(
        "profile.code-object-version",
        MEMBER_PROFILE,
        &[profile.code_object_version],
    );
    canonical.push(
        "profile.wave-width",
        MEMBER_PROFILE,
        &profile.wave_width.lanes().to_le_bytes(),
    );
    canonical.push(
        "profile.workgroup-size",
        MEMBER_PROFILE,
        &encode_workgroup(profile.workgroup_size),
    );
    canonical.push("profile.grid", MEMBER_PROFILE, &encode_grid(profile.grid));
    canonical.push(
        "profile.correspondence",
        MEMBER_PROFILE,
        &[correspondence_tag(profile.correspondence)],
    );
    canonical.push(
        "profile.descriptor.logical-name",
        MEMBER_PROFILE | MEMBER_ABI,
        profile.descriptor.logical_name.as_bytes(),
    );
    canonical.push(
        "profile.descriptor.export-name",
        MEMBER_PROFILE | MEMBER_ABI,
        profile.descriptor.export_name.as_bytes(),
    );
    canonical.push(
        "profile.descriptor.symbol",
        MEMBER_PROFILE | MEMBER_ABI,
        profile.descriptor.descriptor_symbol.as_bytes(),
    );
    canonical.push(
        "profile.descriptor.code-object-version",
        MEMBER_PROFILE | MEMBER_ABI,
        &[profile.descriptor.code_object_version],
    );
    canonical.push(
        "profile.descriptor.explicit-kernarg-bytes",
        MEMBER_PROFILE | MEMBER_ABI,
        &profile.descriptor.explicit_kernarg_bytes.to_le_bytes(),
    );
    canonical.push(
        "profile.descriptor.complete-kernarg-bytes",
        MEMBER_PROFILE | MEMBER_ABI,
        &profile.descriptor.complete_kernarg_bytes.to_le_bytes(),
    );
    canonical.push(
        "profile.descriptor.workgroup-size",
        MEMBER_PROFILE | MEMBER_ABI,
        &encode_workgroup(profile.descriptor.workgroup_size),
    );
    canonical.push(
        "profile.descriptor.wave-width",
        MEMBER_PROFILE | MEMBER_ABI,
        &profile.descriptor.wave_width.lanes().to_le_bytes(),
    );
    canonical.push(
        "profile.descriptor.static-lds-bytes",
        MEMBER_PROFILE,
        &profile.descriptor.resources.static_lds_bytes.to_le_bytes(),
    );
    canonical.push(
        "profile.descriptor.required-dynamic-lds-bytes",
        MEMBER_PROFILE,
        &profile
            .descriptor
            .resources
            .required_dynamic_lds_bytes
            .to_le_bytes(),
    );
    canonical.push(
        "profile.descriptor.maximum-dynamic-lds-bytes",
        MEMBER_PROFILE,
        &profile
            .descriptor
            .resources
            .maximum_dynamic_lds_bytes
            .to_le_bytes(),
    );
    canonical
}

impl CanonicalKernelProfileV2 {
    fn push(&mut self, name: &'static str, memberships: u8, value: &[u8]) {
        debug_assert!(memberships != 0);
        debug_assert!(!self.fields.iter().any(|field| field.name == name));
        self.fields.push(CanonicalFieldV2 {
            name,
            memberships,
            value: value.to_vec(),
        });
    }

    fn identity(&self, domain: &[u8], member: u8) -> [u8; 32] {
        let mut output = CanonicalWriter::new(domain);
        for field in &self.fields {
            if field.memberships & member != 0 {
                output.text(field.name);
                output.field(&field.value);
            }
        }
        sha256(&output.finish())
    }

    fn canonical_table(&self) -> Vec<u8> {
        let mut output = CanonicalWriter::new(CANONICAL_TABLE_DOMAIN_V2);
        output.u32(self.fields.len() as u32);
        for field in &self.fields {
            output.text(field.name);
            output.u8(field.memberships);
            output.field(&field.value);
        }
        output.finish()
    }
}

fn encode_arguments(ir: &MoeTop2KernelIrV1) -> Vec<u8> {
    let mut output = CanonicalWriter::empty();
    output.u32(ir.arguments.len() as u32);
    for argument in ir.arguments {
        output.u8(argument_role_tag(argument.role));
        output.u8(argument_shape_tag(argument.shape));
        output.u8(scalar_type_tag(argument.scalar));
        output.u32(argument.offset);
        output.u32(argument.size);
        output.u32(argument.alignment);
    }
    output.finish()
}

fn encode_shape(ir: &MoeTop2KernelIrV1) -> Vec<u8> {
    let mut output = CanonicalWriter::empty();
    output.u8(ir.shape.tokens);
    output.u8(ir.shape.experts);
    output.u8(ir.shape.experts_per_token);
    output.u8(ir.shape.expert_capacity);
    output.u8(ir.shape.logits);
    output.u8(ir.shape.routes);
    output.finish()
}

fn encode_routing_steps(ir: &MoeTop2KernelIrV1) -> Vec<u8> {
    let mut output = CanonicalWriter::empty();
    output.u32(ir.routing.len() as u32);
    for step in ir.routing {
        output.u8(routing_step_tag(step));
    }
    output.finish()
}

fn encode_packing(ir: &MoeTop2KernelIrV1) -> Vec<u8> {
    let mut output = CanonicalWriter::empty();
    output.boolean(ir.packing.requested_counts_exact);
    output.boolean(ir.packing.admitted_is_requested_min_capacity);
    output.boolean(ir.packing.offsets_are_exclusive_scan);
    output.boolean(ir.packing.offsets_start_at_zero);
    output.boolean(ir.packing.accepted_slots_unique);
    output.boolean(ir.packing.accepted_slots_bounded_by_total_admitted);
    output.boolean(ir.packing.permutation_inverse_round_trip);
    output.boolean(ir.packing.dropped_slot_and_inverse_are_sentinel);
    output.boolean(ir.packing.unused_permutation_tail_is_sentinel);
    output.u32(ir.packing.sentinel);
    output.finish()
}

fn encode_ownership(ir: &MoeTop2KernelIrV1) -> Vec<u8> {
    let mut output = CanonicalWriter::empty();
    output.u8(ownership_policy_tag(ir.ownership.policy));
    output.u8(ir.ownership.physical_lanes);
    output.u8(ir.ownership.active_lanes);
    output.u32(ir.ownership.output_lengths.len() as u32);
    for length in ir.ownership.output_lengths {
        output.u8(length);
    }
    output.boolean(ir.ownership.every_output_element_written_once);
    output.boolean(ir.ownership.output_arguments_exclusive);
    output.boolean(ir.ownership.writes_in_bounds);
    output.finish()
}

fn encode_workgroup(size: fe2o3_kernel_ir::WorkgroupSize) -> Vec<u8> {
    let mut output = CanonicalWriter::empty();
    output.u32(size.x);
    output.u32(size.y);
    output.u32(size.z);
    output.finish()
}

fn encode_grid(grid: [u32; 3]) -> Vec<u8> {
    let mut output = CanonicalWriter::empty();
    for extent in grid {
        output.u32(extent);
    }
    output.finish()
}

fn encode_target_capability(target: &TargetCapability) -> Vec<u8> {
    let mut output = CanonicalWriter::empty();
    match target {
        TargetCapability::Float16 => output.u8(0),
        TargetCapability::BFloat16 => output.u8(1),
        TargetCapability::Float64 => output.u8(2),
        TargetCapability::Int64 => output.u8(3),
        TargetCapability::Subgroups => output.u8(4),
        TargetCapability::SubgroupSize(size) => {
            output.u8(5);
            output.u32(*size);
        }
        TargetCapability::WorkgroupMemory => output.u8(6),
        TargetCapability::WorkgroupBarrier => output.u8(7),
        TargetCapability::Atomic {
            width_bits,
            address_space,
            max_scope,
        } => {
            output.u8(8);
            output.u16(*width_bits);
            output.u8(address_space_tag(*address_space));
            output.u8(synchronization_scope_tag(*max_scope));
        }
        TargetCapability::DynamicWorkgroupMemory => output.u8(9),
        TargetCapability::Extension { namespace, name } => {
            output.u8(10);
            output.text(namespace);
            output.text(name);
        }
        TargetCapability::WaveWidth(width) => {
            output.u8(11);
            output.u32(width.lanes());
        }
    }
    output.finish()
}

fn snapshot_text(inputs: &StructuralInputsV2) -> String {
    let mir = inputs.portable_mir;
    let abi = inputs.fn_abi;
    let argument_text = abi
        .arguments
        .iter()
        .map(|argument| {
            format!(
                "{}:{}:{}:{}:{}",
                argument.size,
                argument.alignment,
                u8::from(argument.pair_mode),
                argument.first_pointee_bytes,
                argument.second_pointee_bytes
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "schema=moe-top2-private-structural-v2;source={};fnabi={}:rust={}:variadic={}:fixed={}:unwind={}:ignored={}:result={}:args=[{}];mir={}:functions={}:roots={}:helpers={}:blocks={}:statements={}:terminators={}:edges={}:imports={}:root-args={}:root-locals={}:assignments={}:calls={}:indexed={}:repeats={}:binops=0x{:08x};kir={};profile={};abi={};effects={};routing={}",
        hex(&inputs.source_identity),
        hex(&abi.identity),
        u8::from(abi.rust_calling_convention),
        u8::from(abi.c_variadic),
        abi.fixed_count,
        u8::from(abi.can_unwind),
        u8::from(abi.result_ignored),
        abi.result_size,
        argument_text,
        hex(&mir.identity),
        mir.function_count,
        mir.kernel_root_count,
        mir.helper_count,
        mir.block_count,
        mir.statement_count,
        mir.terminator_count,
        mir.edge_count,
        mir.external_import_count,
        mir.root_argument_count,
        mir.root_local_count,
        mir.assignment_count,
        mir.call_count,
        mir.indexed_place_count,
        mir.repeat_count,
        mir.binary_operation_mask,
        hex(&inputs
            .canonical
            .identity(KERNEL_IR_DOMAIN_V2, MEMBER_KERNEL_IR),),
        hex(&inputs.canonical.identity(PROFILE_DOMAIN_V2, MEMBER_PROFILE),),
        hex(&inputs
            .canonical
            .identity(ABI_PROJECTION_DOMAIN_V2, MEMBER_ABI),),
        hex(&inputs
            .canonical
            .identity(EFFECTS_PROJECTION_DOMAIN_V2, MEMBER_EFFECTS),),
        hex(&inputs
            .canonical
            .identity(ROUTING_PROJECTION_DOMAIN_V2, MEMBER_ROUTING),),
    )
}

fn canonical_fn_abi(abi: &MoeSourceKirFnAbiV1) -> Vec<u8> {
    let mut output = CanonicalWriter::new(b"fe2o3.moe-top2.observed-fnabi.v2\0");
    output.field(&abi.identity);
    output.boolean(abi.rust_calling_convention);
    output.boolean(abi.c_variadic);
    output.u8(abi.fixed_count);
    output.boolean(abi.can_unwind);
    output.boolean(abi.result_ignored);
    output.u16(abi.result_size);
    output.u32(abi.arguments.len() as u32);
    for argument in abi.arguments {
        output.u16(argument.size);
        output.u16(argument.alignment);
        output.boolean(argument.pair_mode);
        output.u32(argument.first_pointee_bytes);
        output.u32(argument.second_pointee_bytes);
    }
    output.finish()
}

fn canonical_portable_mir(mir: &PortableMirSummaryV2) -> Vec<u8> {
    let mut output = CanonicalWriter::new(b"fe2o3.moe-top2.portable-mir-summary.v2\0");
    output.field(&mir.identity);
    for value in [
        mir.function_count,
        mir.kernel_root_count,
        mir.helper_count,
        mir.block_count,
        mir.statement_count,
        mir.terminator_count,
        mir.edge_count,
        mir.external_import_count,
        mir.root_argument_count,
        mir.root_local_count,
        mir.assignment_count,
        mir.call_count,
        mir.indexed_place_count,
        mir.repeat_count,
        mir.binary_operation_mask,
    ] {
        output.u32(value);
    }
    output.finish()
}

struct CanonicalWriter {
    bytes: Vec<u8>,
}

impl CanonicalWriter {
    fn empty() -> Self {
        Self { bytes: Vec::new() }
    }

    fn new(domain: &[u8]) -> Self {
        let mut writer = Self::empty();
        writer.field(domain);
        writer
    }

    fn field(&mut self, bytes: &[u8]) {
        self.u64(bytes.len() as u64);
        self.bytes.extend_from_slice(bytes);
    }

    fn text(&mut self, value: &str) {
        self.field(value.as_bytes());
    }

    fn boolean(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

const fn argument_role_tag(role: MoeTop2ArgumentRoleV1) -> u8 {
    match role {
        MoeTop2ArgumentRoleV1::Logits => 0,
        MoeTop2ArgumentRoleV1::Top2Experts => 1,
        MoeTop2ArgumentRoleV1::RequestedCounts => 2,
        MoeTop2ArgumentRoleV1::AdmittedCounts => 3,
        MoeTop2ArgumentRoleV1::ExpertOffsets => 4,
        MoeTop2ArgumentRoleV1::RouteSlots => 5,
        MoeTop2ArgumentRoleV1::Permutation => 6,
        MoeTop2ArgumentRoleV1::Inverse => 7,
    }
}

const fn argument_shape_tag(shape: MoeTop2ArgumentShapeV1) -> u8 {
    match shape {
        MoeTop2ArgumentShapeV1::SharedReadOnlyContiguousF32x32 => 0,
        MoeTop2ArgumentShapeV1::LaneZeroOwnedReadWriteContiguousU32x16 => 1,
        MoeTop2ArgumentShapeV1::LaneZeroOwnedReadWriteContiguousU32x4 => 2,
        MoeTop2ArgumentShapeV1::LaneZeroOwnedReadWriteContiguousU32x5 => 3,
    }
}

const fn scalar_type_tag(value: ScalarType) -> u8 {
    match value {
        ScalarType::Bool => 0,
        ScalarType::I8 => 1,
        ScalarType::I16 => 2,
        ScalarType::I32 => 3,
        ScalarType::I64 => 4,
        ScalarType::I128 => 5,
        ScalarType::U8 => 6,
        ScalarType::U16 => 7,
        ScalarType::U32 => 8,
        ScalarType::U64 => 9,
        ScalarType::U128 => 10,
        ScalarType::Index => 11,
        ScalarType::F16 => 12,
        ScalarType::Bf16 => 13,
        ScalarType::F32 => 14,
        ScalarType::F64 => 15,
    }
}

const fn layout_tag(value: MoeTop2LayoutV1) -> u8 {
    match value {
        MoeTop2LayoutV1::TokenMajorLogitsAndTokenThenRankRoutes => 0,
        MoeTop2LayoutV1::ExpertMajorLogitsUnsupported => 1,
    }
}

const fn finite_input_tag(value: MoeTop2FiniteInputPolicyV1) -> u8 {
    match value {
        MoeTop2FiniteInputPolicyV1::AllLogitsFiniteOrTrapBeforeAnyOutputWrite => 0,
        MoeTop2FiniteInputPolicyV1::NonFiniteInputsUnsupported => 1,
    }
}

const fn tie_break_tag(value: MoeTop2TieBreakV1) -> u8 {
    match value {
        MoeTop2TieBreakV1::HigherFiniteF32ScoreThenLowerExpertId => 0,
        MoeTop2TieBreakV1::HigherExpertIdTieBreakUnsupported => 1,
    }
}

const fn overflow_tag(value: MoeTop2OverflowV1) -> u8 {
    match value {
        MoeTop2OverflowV1::StableRoutePrefixPerExpertDropAfterCapacity => 0,
        MoeTop2OverflowV1::ReplaceAcceptedRouteUnsupported => 1,
    }
}

const fn routing_step_tag(value: MoeTop2RoutingStepV1) -> u8 {
    match value {
        MoeTop2RoutingStepV1::ValidateExactExtentsAndFiniteInputsBeforeWrites => 0,
        MoeTop2RoutingStepV1::SelectDistinctTop2DescendingScoreLowerExpertTie => 1,
        MoeTop2RoutingStepV1::CountRequestedRoutesInTokenThenRankOrder => 2,
        MoeTop2RoutingStepV1::ClampAdmittedCountsToCapacityFour => 3,
        MoeTop2RoutingStepV1::ExclusiveScanAdmittedCountsInExpertOrder => 4,
        MoeTop2RoutingStepV1::InitializeSlotsPermutationAndInverseToSentinel => 5,
        MoeTop2RoutingStepV1::ComputeStableRankInIncreasingRouteOrder => 6,
        MoeTop2RoutingStepV1::AssignUniqueBoundedSlotFromExpertOffsetAndStableRank => 7,
        MoeTop2RoutingStepV1::EstablishPermutationAndInverseRoundTrip => 8,
        MoeTop2RoutingStepV1::CommitEveryOutputOnceFromLaneZero => 9,
    }
}

const fn ownership_policy_tag(value: MoeTop2OutputOwnershipPolicyV1) -> u8 {
    match value {
        MoeTop2OutputOwnershipPolicyV1::PhysicalLaneZeroOwnsAllOutputElementsOtherLanesInactive => {
            0
        }
    }
}

const fn correspondence_tag(value: MoeTop2CorrespondenceV1) -> u8 {
    match value {
        MoeTop2CorrespondenceV1::ReviewedExactSourceAndMirToCanonicalProfileNotRefinementProof => 0,
    }
}

const fn address_space_tag(value: AddressSpace) -> u8 {
    match value {
        AddressSpace::Private => 0,
        AddressSpace::Workgroup => 1,
        AddressSpace::Global => 2,
        AddressSpace::Constant => 3,
        AddressSpace::Generic => 4,
    }
}

const fn synchronization_scope_tag(value: SynchronizationScope) -> u8 {
    match value {
        SynchronizationScope::Invocation => 0,
        SynchronizationScope::Subgroup => 1,
        SynchronizationScope::Workgroup => 2,
        SynchronizationScope::Device => 3,
        SynchronizationScope::System => 4,
    }
}

const fn binary_operation_bit(operation: MirBinaryOp) -> u32 {
    1_u32
        << match operation {
            MirBinaryOp::Add => 0,
            MirBinaryOp::Sub => 1,
            MirBinaryOp::Mul => 2,
            MirBinaryOp::Div => 3,
            MirBinaryOp::Rem => 4,
            MirBinaryOp::BitXor => 5,
            MirBinaryOp::BitAnd => 6,
            MirBinaryOp::BitOr => 7,
            MirBinaryOp::Shl => 8,
            MirBinaryOp::Shr => 9,
            MirBinaryOp::Eq => 10,
            MirBinaryOp::Lt => 11,
            MirBinaryOp::Le => 12,
            MirBinaryOp::Ne => 13,
            MirBinaryOp::Ge => 14,
            MirBinaryOp::Gt => 15,
            MirBinaryOp::Cmp => 16,
            MirBinaryOp::Offset => 17,
            MirBinaryOp::AddUnchecked => 18,
            MirBinaryOp::SubUnchecked => 19,
            MirBinaryOp::MulUnchecked => 20,
            MirBinaryOp::ShlUnchecked => 21,
            MirBinaryOp::ShrUnchecked => 22,
            MirBinaryOp::AddWithOverflow => 23,
            MirBinaryOp::SubWithOverflow => 24,
            MirBinaryOp::MulWithOverflow => 25,
        }
}

fn count_statement_indexed_places(
    destination: Option<&MirPlaceRef>,
    operands: &[MirOperandRef],
) -> usize {
    destination.map_or(0, count_place_indexed_places)
        + operands
            .iter()
            .map(count_operand_indexed_places)
            .sum::<usize>()
}

fn count_operand_indexed_places(operand: &MirOperandRef) -> usize {
    match operand {
        MirOperandRef::Place(place) => count_place_indexed_places(place),
        MirOperandRef::Constant { .. } => 0,
    }
}

fn count_place_indexed_places(place: &MirPlaceRef) -> usize {
    place
        .projection
        .iter()
        .filter(|projection| {
            matches!(
                projection,
                MirProjectionElem::Index { .. } | MirProjectionElem::ConstantIndex { .. }
            )
        })
        .count()
}

fn terminator_edges(terminator: &MirTerminatorKind) -> usize {
    match terminator {
        MirTerminatorKind::Return | MirTerminatorKind::Unreachable | MirTerminatorKind::Other => 0,
        MirTerminatorKind::Goto { .. }
        | MirTerminatorKind::Assert { .. }
        | MirTerminatorKind::Drop { .. } => 1,
        MirTerminatorKind::SwitchInt { targets, .. } => targets.len() + 1,
        MirTerminatorKind::Call { target, .. } => usize::from(target.is_some()),
    }
}

fn count(value: usize, field: &'static str) -> Result<u32, MoeSourceKirProducerErrorV2> {
    u32::try_from(value).map_err(|_| MoeSourceKirProducerErrorV2::CountOverflow(field))
}

fn add_count(
    total: &mut u32,
    value: usize,
    field: &'static str,
) -> Result<(), MoeSourceKirProducerErrorV2> {
    *total = total
        .checked_add(count(value, field)?)
        .ok_or(MoeSourceKirProducerErrorV2::CountOverflow(field))?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
fn candidate_inputs_for_test() -> StructuralInputsV2 {
    let source = include_bytes!("../../../examples/moe_top2_v1/src/kernel.rs").to_vec();
    let ir = fe2o3_kernel_ir::moe_top2_v1_kernel_ir();
    let profile = MoeTop2ProfileV1::exact_gfx942_xnack_minus_cov6();
    StructuralInputsV2 {
        source,
        source_identity: SOURCE_IDENTITY,
        fn_abi: MoeSourceKirFnAbiV1 {
            identity: FN_ABI_IDENTITY,
            rust_calling_convention: true,
            c_variadic: false,
            fixed_count: 8,
            can_unwind: true,
            result_ignored: true,
            result_size: 0,
            arguments: [MoeSourceKirFnAbiArgumentV1 {
                size: 16,
                alignment: 8,
                pair_mode: true,
                first_pointee_bytes: 0,
                second_pointee_bytes: 0,
            }; 8],
        },
        portable_mir: PortableMirSummaryV2 {
            identity: PORTABLE_MIR_IDENTITY,
            function_count: 6,
            kernel_root_count: 1,
            helper_count: 5,
            block_count: 120,
            statement_count: 222,
            terminator_count: 120,
            edge_count: 142,
            external_import_count: 0,
            root_argument_count: 8,
            root_local_count: 171,
            assignment_count: 220,
            call_count: 27,
            indexed_place_count: 36,
            repeat_count: 8,
            binary_operation_mask: 0x0000_ec05,
        },
        canonical: canonical_kernel_profile(&ir, &profile),
    }
}

#[cfg(test)]
pub(crate) fn candidate_structural_record_for_test() -> CheckedMoeSourceKirStructuralRecordV2 {
    check_structural_inputs(candidate_inputs_for_test())
        .expect("pinned synthetic structural input matches the live summary")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_kir_identities_are_derived_and_domain_separated() {
        let inputs = candidate_inputs_for_test();
        let identities = [
            inputs
                .canonical
                .identity(KERNEL_IR_DOMAIN_V2, MEMBER_KERNEL_IR),
            inputs.canonical.identity(PROFILE_DOMAIN_V2, MEMBER_PROFILE),
            inputs
                .canonical
                .identity(ABI_PROJECTION_DOMAIN_V2, MEMBER_ABI),
            inputs
                .canonical
                .identity(EFFECTS_PROJECTION_DOMAIN_V2, MEMBER_EFFECTS),
            inputs
                .canonical
                .identity(ROUTING_PROJECTION_DOMAIN_V2, MEMBER_ROUTING),
        ];
        for (index, left) in identities.iter().enumerate() {
            assert_ne!(left, &[0; 32]);
            for right in &identities[index + 1..] {
                assert_ne!(left, right);
            }
        }
    }

    #[test]
    fn canonical_table_contains_each_actual_field_once() {
        let inputs = candidate_inputs_for_test();
        assert_eq!(inputs.canonical.fields.len(), 31);
        let unique_names = inputs
            .canonical
            .fields
            .iter()
            .map(|field| field.name)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique_names.len(), inputs.canonical.fields.len());
        assert!(
            inputs
                .canonical
                .fields
                .iter()
                .all(|field| field.memberships != 0)
        );
    }

    #[test]
    fn classifier_rejects_mutations_after_earlier_admission_gates() {
        let mutations: [fn(&mut StructuralInputsV2); 13] = [
            |inputs| inputs.fn_abi.arguments[0].alignment = 4,
            |inputs| inputs.portable_mir.function_count += 1,
            |inputs| inputs.portable_mir.block_count += 1,
            |inputs| inputs.portable_mir.statement_count += 1,
            |inputs| inputs.portable_mir.edge_count += 1,
            |inputs| inputs.portable_mir.assignment_count += 1,
            |inputs| inputs.portable_mir.call_count += 1,
            |inputs| inputs.portable_mir.indexed_place_count += 1,
            |inputs| inputs.canonical.fields[0].value.push(0),
            |inputs| inputs.canonical.fields[12].value.push(0),
            |inputs| {
                let field = inputs
                    .canonical
                    .fields
                    .iter_mut()
                    .find(|field| field.memberships & MEMBER_ABI != 0)
                    .unwrap();
                field.value.push(0);
            },
            |inputs| {
                let field = inputs
                    .canonical
                    .fields
                    .iter_mut()
                    .find(|field| field.memberships & MEMBER_EFFECTS != 0)
                    .unwrap();
                field.value.push(0);
            },
            |inputs| {
                let field = inputs
                    .canonical
                    .fields
                    .iter_mut()
                    .find(|field| field.memberships & MEMBER_ROUTING != 0)
                    .unwrap();
                field.value.push(0);
            },
        ];
        for mutate in mutations {
            let mut inputs = candidate_inputs_for_test();
            mutate(&mut inputs);
            assert!(check_structural_inputs(inputs).is_err());
        }

        let field_count = candidate_inputs_for_test().canonical.fields.len();
        for index in 0..field_count {
            let mut value_mutation = candidate_inputs_for_test();
            value_mutation.canonical.fields[index].value.push(0);
            assert!(check_structural_inputs(value_mutation).is_err());

            let mut membership_mutation = candidate_inputs_for_test();
            membership_mutation.canonical.fields[index].memberships ^= MEMBER_KERNEL_IR;
            assert!(check_structural_inputs(membership_mutation).is_err());
        }
    }

    #[test]
    fn source_and_portable_mir_bounds_fail_closed() {
        let mut source = candidate_inputs_for_test();
        source.source.push(b'\n');
        assert_eq!(
            check_structural_inputs(source),
            Err(MoeSourceKirProducerErrorV2::Source)
        );

        let mut imported = candidate_inputs_for_test();
        imported.portable_mir.external_import_count = 1;
        assert_eq!(
            check_structural_inputs(imported),
            Err(MoeSourceKirProducerErrorV2::PortableMir)
        );
    }

    #[test]
    fn checked_record_is_explicitly_inert() {
        let checked = candidate_structural_record_for_test();
        assert!(!checked.proves_source_to_kir_semantic_refinement());
        assert!(!checked.proves_llvm_or_isa_refinement());
        assert!(!checked.proves_logical_to_machine_address_refinement());
        assert!(!checked.proves_ieee_fp32_or_ocml_semantics());
        assert!(!checked.proves_generalized_memory_safety_or_race_freedom());
        assert!(!checked.proves_gpu_execution());
        assert!(!checked.grants_artifact_authority());
        assert!(!checked.grants_load_authority());
        assert!(!checked.grants_launch_authority());
    }
}
