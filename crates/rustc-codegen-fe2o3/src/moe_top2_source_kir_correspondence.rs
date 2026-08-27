//! Private producer for an exact MoE source/FnAbi/MIR/KIR structural record.
//!
//! This child module is reachable only from the MoE rustc admission module.
//! Its producer accepts an opaque witness sealed after that path computes its
//! final authority. It serializes an opaque exact `FnAbi` identity plus a
//! bounded structural projection, a whole-module portable-MIR summary, and
//! aggregate canonical entries that collectively encode every current field
//! of the already validated MoE KIR and profile. It is diagnostic structural
//! evidence, not a MIR-to-KIR simulation or semantic refinement proof, and it
//! grants no downstream authority.

use std::error::Error;
use std::fmt;

use fe2o3_kernel_ir::{
    AddressSpace, MoeTop2ArgumentRoleV1, MoeTop2ArgumentShapeV1, MoeTop2CorrespondenceV1,
    MoeTop2FiniteInputPolicyV1, MoeTop2KernelIrV1, MoeTop2LayoutV1, MoeTop2OutputOwnershipPolicyV1,
    MoeTop2OverflowV1, MoeTop2ProfileV1, MoeTop2RoutingStepV1, MoeTop2TieBreakV1, ScalarType,
    SynchronizationScope, TargetCapability,
};
use sha2::{Digest as _, Sha256};

use super::validated_authority::ValidatedMoeTop2AuthorityV1;
use super::{ObservedMoeTop2FnAbiV1, RustcLoadedMoeTop2SourceV2};
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
    0x0e, 0x45, 0x70, 0xbd, 0x52, 0x86, 0x6d, 0xd2, 0x3b, 0x8b, 0x00, 0xd8, 0x39, 0x83, 0xaa, 0xdc,
    0x81, 0x8c, 0x77, 0x58, 0x0d, 0xe8, 0xf7, 0xf5, 0xe2, 0x98, 0x2e, 0x12, 0xa5, 0x7e, 0x20, 0xe2,
];
const PORTABLE_MIR_IDENTITY: [u8; 32] = [
    0xed, 0xef, 0xfa, 0x59, 0x72, 0x9d, 0xf7, 0x75, 0xae, 0x94, 0xd5, 0xd5, 0xeb, 0x11, 0x10, 0xb8,
    0xff, 0xd6, 0xbf, 0x07, 0xe9, 0x65, 0x9b, 0xa2, 0xa9, 0x6f, 0xc3, 0x7c, 0x97, 0x5d, 0x9b, 0x86,
];
const FN_ABI_IDENTITY: [u8; 32] = [
    0xdd, 0xc0, 0x17, 0x2c, 0xfc, 0x37, 0x01, 0x6c, 0x86, 0xbe, 0x2b, 0x57, 0x9c, 0x4c, 0x98, 0xb1,
    0x4f, 0x82, 0x3d, 0xd9, 0x37, 0x18, 0x16, 0xb6, 0x64, 0x8f, 0x1b, 0x8b, 0xd0, 0x61, 0xbd, 0x88,
];

// Filled from the exact live rustc admission. The text is deliberately
// readable: reviewers can see every checked structural input without
// reverse-engineering the final record digest.
pub(super) const MOE_TOP2_LIVE_STRUCTURAL_SNAPSHOT_V2: &str = concat!(
    "schema=moe-top2-private-structural-v2;",
    "source=0e4570bd52866dd23b8b00d83983aadc818c77580de8f7f5e2982e12a57e20e2;",
    "fnabi=ddc0172cfc37016c86be2b579c4c98b14f823dd9371816b6648f1b8bd061bd88:",
    "rust=1:variadic=0:fixed=8:unwind=1:ignored=1:result=0:",
    "args=[16:8:1:0:0,16:8:1:0:0,16:8:1:0:0,16:8:1:0:0,",
    "16:8:1:0:0,16:8:1:0:0,16:8:1:0:0,16:8:1:0:0];",
    "mir=edeffa59729df775ae94d5d5eb1110b8ffd6bf07e9659ba2a96fc37c975d9b86:",
    "functions=6:roots=1:helpers=5:blocks=118:statements=228:terminators=118:",
    "edges=139:imports=0:root-args=8:root-locals=176:assignments=226:calls=25:",
    "indexed=36:repeats=8:binops=0x0000ac05;",
    "kir=bdf19330357f898eb0267372e4115b33e4b60902c94b5b3b6e358f5703ee7eb0;",
    "profile=0e87d38ce9387a1f6a39abb9e98e327f7773c7eb87a9577696bc12871eb9c734;",
    "abi=f0abdc459360d1760e22c62f77830472ae9a3e151b5ec7323d56c5298dc87365;",
    "effects=4f7a7d0996535ee75ff22216c776666526caa93106a49c2e8cfb4956cb0f7716;",
    "routing=dc93201e71d4ba820f52bb44833f4592383b2023f67089a4cc1b73aae14f051b;",
    "compiler=4950c225e0cdbdce4e1230166984949970290dedc19e8dc4cd31f865f1625a4a;",
    "trusted=b51420b63c55408540054826b0450cb59af371eb240bcf8621646dc7deb6feb3;",
    "root=kernel::__fe2o3_host_kernel_v1_",
    "0d0504325353eb74b0c9ace47560290e2278a7cd7c20e3b1c6c70f4a7e37b1ab;",
    "authority=b2dfff3527e234212f34ffda81468e6710cba983550fa652711b9565759c2a28",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MoeFnAbiArgumentStructuralProjectionV2 {
    pub(super) size: u16,
    pub(super) alignment: u16,
    pub(super) pair_mode: bool,
    pub(super) first_pointee_bytes: u32,
    pub(super) second_pointee_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MoeFnAbiStructuralProjectionV2 {
    pub(super) identity: [u8; 32],
    pub(super) rust_calling_convention: bool,
    pub(super) c_variadic: bool,
    pub(super) fixed_count: u8,
    pub(super) can_unwind: bool,
    pub(super) result_ignored: bool,
    pub(super) result_size: u16,
    pub(super) arguments: [MoeFnAbiArgumentStructuralProjectionV2; 8],
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct SameSessionBindingV2 {
    compiler_semantics_identity: [u8; 32],
    trusted_definitions_identity: [u8; 32],
    root_instance_identity: String,
    source_authority_identity: [u8; 32],
}

const MEMBER_KERNEL_IR: u8 = 1 << 0;
const MEMBER_PROFILE: u8 = 1 << 1;
const MEMBER_ABI: u8 = 1 << 2;
const MEMBER_EFFECTS: u8 = 1 << 3;
const MEMBER_ROUTING: u8 = 1 << 4;

#[derive(Clone, Debug, Eq, PartialEq)]
struct StructuralClassifierCandidateV2 {
    source: Vec<u8>,
    source_identity: [u8; 32],
    fn_abi: MoeFnAbiStructuralProjectionV2,
    portable_mir: PortableMirSummaryV2,
    canonical: CanonicalKernelProfileV2,
    same_session: SameSessionBindingV2,
}

pub(super) struct SealedLiveMoeStructuralInputsV2<'a> {
    source: &'a RustcLoadedMoeTop2SourceV2,
    fn_abi: &'a ObservedMoeTop2FnAbiV1,
    portable_mir: &'a MirModule,
    portable_mir_identity: [u8; 32],
    ir: &'a MoeTop2KernelIrV1,
    profile: &'a MoeTop2ProfileV1,
    validated_authority: ValidatedMoeTop2AuthorityV1<'a>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedMoeSourceKirStructuralRecordV2 {
    identity: [u8; 32],
    source_identity: [u8; 32],
    fn_abi_identity: [u8; 32],
    portable_mir_identity: [u8; 32],
    kernel_ir_identity: [u8; 32],
    profile_identity: [u8; 32],
    compiler_semantics_identity: [u8; 32],
    trusted_definitions_identity: [u8; 32],
    root_instance_identity: String,
    source_authority_identity: [u8; 32],
    snapshot: String,
}

impl CheckedMoeSourceKirStructuralRecordV2 {
    pub(super) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(super) const fn source_identity(&self) -> [u8; 32] {
        self.source_identity
    }

    pub(super) const fn fn_abi_identity(&self) -> [u8; 32] {
        self.fn_abi_identity
    }

    pub(super) const fn portable_mir_identity(&self) -> [u8; 32] {
        self.portable_mir_identity
    }

    pub(super) const fn kernel_ir_identity(&self) -> [u8; 32] {
        self.kernel_ir_identity
    }

    pub(super) const fn profile_identity(&self) -> [u8; 32] {
        self.profile_identity
    }

    pub(super) const fn compiler_semantics_identity(&self) -> [u8; 32] {
        self.compiler_semantics_identity
    }

    pub(super) const fn trusted_definitions_identity(&self) -> [u8; 32] {
        self.trusted_definitions_identity
    }

    pub(super) fn root_instance_identity(&self) -> &str {
        &self.root_instance_identity
    }

    pub(super) const fn source_authority_identity(&self) -> [u8; 32] {
        self.source_authority_identity
    }

    pub(super) fn snapshot(&self) -> &str {
        &self.snapshot
    }

    pub(super) const fn proves_source_to_kir_semantic_refinement(&self) -> bool {
        false
    }

    pub(super) const fn proves_llvm_or_isa_refinement(&self) -> bool {
        false
    }

    pub(super) const fn proves_logical_to_machine_address_refinement(&self) -> bool {
        false
    }

    pub(super) const fn proves_ieee_fp32_or_ocml_semantics(&self) -> bool {
        false
    }

    pub(super) const fn proves_generalized_memory_safety_or_race_freedom(&self) -> bool {
        false
    }

    pub(super) const fn proves_gpu_execution(&self) -> bool {
        false
    }

    pub(super) const fn grants_artifact_authority(&self) -> bool {
        false
    }

    pub(super) const fn grants_load_authority(&self) -> bool {
        false
    }

    pub(super) const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MoeSourceKirProducerErrorV2 {
    MissingKernelRoot,
    CountOverflow(&'static str),
    Source,
    FnAbi,
    PortableMir,
    SameSession,
    Canonical(CanonicalClassifierErrorV2),
    SnapshotMismatch { actual: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CanonicalClassifierErrorV2 {
    DuplicateField {
        name: &'static str,
    },
    MissingField {
        name: &'static str,
    },
    UnexpectedField {
        name: &'static str,
    },
    FieldName {
        index: usize,
        expected: &'static str,
        actual: &'static str,
    },
    FieldOrder {
        name: &'static str,
        expected_index: usize,
        actual_index: usize,
    },
    Membership {
        name: &'static str,
        expected: u8,
        actual: u8,
    },
    Value {
        name: &'static str,
    },
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
            Self::SameSession => {
                formatter.write_str("structural inputs do not share the admitted rustc authority")
            }
            Self::Canonical(error) => write!(formatter, "canonical classifier rejected: {error}"),
            Self::SnapshotMismatch { actual } => write!(
                formatter,
                "live structural snapshot differs from its reviewed pin; observed {actual}"
            ),
        }
    }
}

impl Error for MoeSourceKirProducerErrorV2 {}

impl fmt::Display for CanonicalClassifierErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateField { name } => write!(formatter, "duplicate field `{name}`"),
            Self::MissingField { name } => write!(formatter, "missing field `{name}`"),
            Self::UnexpectedField { name } => write!(formatter, "unexpected field `{name}`"),
            Self::FieldName {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "field {index} name drifted: expected `{expected}`, found `{actual}`"
            ),
            Self::FieldOrder {
                name,
                expected_index,
                actual_index,
            } => write!(
                formatter,
                "field `{name}` moved from {expected_index} to {actual_index}"
            ),
            Self::Membership {
                name,
                expected,
                actual,
            } => write!(
                formatter,
                "field `{name}` membership drifted: expected {expected:#04x}, found {actual:#04x}"
            ),
            Self::Value { name } => write!(formatter, "field `{name}` value drifted"),
        }
    }
}

impl Error for CanonicalClassifierErrorV2 {}

pub(super) fn seal_authenticated_live_inputs_v2<'a>(
    source: &'a RustcLoadedMoeTop2SourceV2,
    fn_abi: &'a ObservedMoeTop2FnAbiV1,
    portable_mir: &'a MirModule,
    portable_mir_identity: [u8; 32],
    ir: &'a MoeTop2KernelIrV1,
    profile: &'a MoeTop2ProfileV1,
    validated_authority: ValidatedMoeTop2AuthorityV1<'a>,
) -> Result<SealedLiveMoeStructuralInputsV2<'a>, MoeSourceKirProducerErrorV2> {
    let authority = validated_authority.authority();
    if source.identity != authority.source_identity
        || fn_abi.identity != fn_abi.structural_projection.identity
        || fn_abi.identity != authority.fn_abi_identity
        || portable_mir_identity != authority.portable_mir_identity
        || authority.authority_identity == [0; 32]
    {
        return Err(MoeSourceKirProducerErrorV2::SameSession);
    }
    Ok(SealedLiveMoeStructuralInputsV2 {
        source,
        fn_abi,
        portable_mir,
        portable_mir_identity,
        ir,
        profile,
        validated_authority,
    })
}

pub(super) fn produce_checked_moe_source_kir_structural_record_v2(
    sealed: SealedLiveMoeStructuralInputsV2<'_>,
) -> Result<CheckedMoeSourceKirStructuralRecordV2, MoeSourceKirProducerErrorV2> {
    let authority = sealed.validated_authority.authority();
    let inputs = StructuralClassifierCandidateV2 {
        source: sealed.source.contents.as_bytes().to_vec(),
        source_identity: sealed.source.identity,
        fn_abi: sealed.fn_abi.structural_projection,
        portable_mir: summarize_portable_mir(sealed.portable_mir, sealed.portable_mir_identity)?,
        canonical: canonical_kernel_profile(sealed.ir, sealed.profile),
        same_session: SameSessionBindingV2 {
            compiler_semantics_identity: authority.compiler_semantics_identity,
            trusted_definitions_identity: authority.trusted_definitions_identity,
            root_instance_identity: authority.root_instance_identity.clone(),
            source_authority_identity: authority.authority_identity,
        },
    };
    check_structural_inputs(inputs)
}

pub(super) fn canonical_kernel_profile_identities_v2(
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
    inputs: StructuralClassifierCandidateV2,
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
    if inputs.same_session.compiler_semantics_identity == [0; 32]
        || inputs.same_session.trusted_definitions_identity == [0; 32]
        || inputs.same_session.root_instance_identity.is_empty()
        || inputs.same_session.source_authority_identity == [0; 32]
    {
        return Err(MoeSourceKirProducerErrorV2::SameSession);
    }
    classify_canonical_fields(&inputs.canonical).map_err(MoeSourceKirProducerErrorV2::Canonical)?;

    let snapshot = snapshot_text(&inputs);
    if snapshot != MOE_TOP2_LIVE_STRUCTURAL_SNAPSHOT_V2 {
        return Err(MoeSourceKirProducerErrorV2::SnapshotMismatch { actual: snapshot });
    }

    let fn_abi_bytes = canonical_fn_abi(&inputs.fn_abi);
    let mir_bytes = canonical_portable_mir(&inputs.portable_mir);
    let same_session_bytes = canonical_same_session(&inputs.same_session);
    let mut record = CanonicalWriter::new(RECORD_DOMAIN_V2);
    record.field(&inputs.source);
    record.field(&inputs.source_identity);
    record.field(&fn_abi_bytes);
    record.field(&mir_bytes);
    record.field(&inputs.canonical.canonical_table());
    record.field(&same_session_bytes);

    Ok(CheckedMoeSourceKirStructuralRecordV2 {
        identity: sha256(&record.finish()),
        source_identity: inputs.source_identity,
        fn_abi_identity: inputs.fn_abi.identity,
        portable_mir_identity: inputs.portable_mir.identity,
        kernel_ir_identity: inputs
            .canonical
            .identity(KERNEL_IR_DOMAIN_V2, MEMBER_KERNEL_IR),
        profile_identity: inputs.canonical.identity(PROFILE_DOMAIN_V2, MEMBER_PROFILE),
        compiler_semantics_identity: inputs.same_session.compiler_semantics_identity,
        trusted_definitions_identity: inputs.same_session.trusted_definitions_identity,
        root_instance_identity: inputs.same_session.root_instance_identity,
        source_authority_identity: inputs.same_session.source_authority_identity,
        snapshot,
    })
}

fn classify_canonical_fields(
    actual: &CanonicalKernelProfileV2,
) -> Result<(), CanonicalClassifierErrorV2> {
    let expected = canonical_kernel_profile(
        &fe2o3_kernel_ir::moe_top2_v1_kernel_ir(),
        &MoeTop2ProfileV1::exact_gfx942_xnack_minus_cov6(),
    );

    for (index, field) in actual.fields.iter().enumerate() {
        if actual.fields[..index]
            .iter()
            .any(|earlier| earlier.name == field.name)
        {
            return Err(CanonicalClassifierErrorV2::DuplicateField { name: field.name });
        }
    }
    if actual.fields.len() < expected.fields.len()
        && let Some(field) = expected.fields.iter().find(|expected_field| {
            !actual
                .fields
                .iter()
                .any(|actual_field| actual_field.name == expected_field.name)
        })
    {
        return Err(CanonicalClassifierErrorV2::MissingField { name: field.name });
    }
    if actual.fields.len() > expected.fields.len()
        && let Some(field) = actual.fields.iter().find(|actual_field| {
            !expected
                .fields
                .iter()
                .any(|expected_field| expected_field.name == actual_field.name)
        })
    {
        return Err(CanonicalClassifierErrorV2::UnexpectedField { name: field.name });
    }

    for (index, (expected_field, actual_field)) in
        expected.fields.iter().zip(&actual.fields).enumerate()
    {
        if actual_field.name != expected_field.name {
            if let Some(expected_index) = expected
                .fields
                .iter()
                .position(|field| field.name == actual_field.name)
            {
                return Err(CanonicalClassifierErrorV2::FieldOrder {
                    name: actual_field.name,
                    expected_index,
                    actual_index: index,
                });
            }
            return Err(CanonicalClassifierErrorV2::FieldName {
                index,
                expected: expected_field.name,
                actual: actual_field.name,
            });
        }
        if actual_field.memberships != expected_field.memberships {
            return Err(CanonicalClassifierErrorV2::Membership {
                name: actual_field.name,
                expected: expected_field.memberships,
                actual: actual_field.memberships,
            });
        }
        if actual_field.value != expected_field.value {
            return Err(CanonicalClassifierErrorV2::Value {
                name: actual_field.name,
            });
        }
    }
    Ok(())
}

fn fn_abi_is_exact(abi: &MoeFnAbiStructuralProjectionV2) -> bool {
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
                if matches!(statement.rvalue, Some(MirRvalueKind::Repeat { .. })) {
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

fn snapshot_text(inputs: &StructuralClassifierCandidateV2) -> String {
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
        "schema=moe-top2-private-structural-v2;source={};fnabi={}:rust={}:variadic={}:fixed={}:unwind={}:ignored={}:result={}:args=[{}];mir={}:functions={}:roots={}:helpers={}:blocks={}:statements={}:terminators={}:edges={}:imports={}:root-args={}:root-locals={}:assignments={}:calls={}:indexed={}:repeats={}:binops=0x{:08x};kir={};profile={};abi={};effects={};routing={};compiler={};trusted={};root={};authority={}",
        crate::encode_hex(&inputs.source_identity),
        crate::encode_hex(&abi.identity),
        u8::from(abi.rust_calling_convention),
        u8::from(abi.c_variadic),
        abi.fixed_count,
        u8::from(abi.can_unwind),
        u8::from(abi.result_ignored),
        abi.result_size,
        argument_text,
        crate::encode_hex(&mir.identity),
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
        crate::encode_hex(
            &inputs
                .canonical
                .identity(KERNEL_IR_DOMAIN_V2, MEMBER_KERNEL_IR),
        ),
        crate::encode_hex(&inputs.canonical.identity(PROFILE_DOMAIN_V2, MEMBER_PROFILE),),
        crate::encode_hex(
            &inputs
                .canonical
                .identity(ABI_PROJECTION_DOMAIN_V2, MEMBER_ABI),
        ),
        crate::encode_hex(
            &inputs
                .canonical
                .identity(EFFECTS_PROJECTION_DOMAIN_V2, MEMBER_EFFECTS),
        ),
        crate::encode_hex(
            &inputs
                .canonical
                .identity(ROUTING_PROJECTION_DOMAIN_V2, MEMBER_ROUTING),
        ),
        crate::encode_hex(&inputs.same_session.compiler_semantics_identity),
        crate::encode_hex(&inputs.same_session.trusted_definitions_identity),
        inputs.same_session.root_instance_identity,
        crate::encode_hex(&inputs.same_session.source_authority_identity),
    )
}

fn canonical_fn_abi(abi: &MoeFnAbiStructuralProjectionV2) -> Vec<u8> {
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

fn canonical_same_session(binding: &SameSessionBindingV2) -> Vec<u8> {
    let mut output = CanonicalWriter::new(b"fe2o3.moe-top2.same-rustc-authority.v2\0");
    output.field(&binding.compiler_semantics_identity);
    output.field(&binding.trusted_definitions_identity);
    output.text(&binding.root_instance_identity);
    output.field(&binding.source_authority_identity);
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

#[cfg(test)]
fn candidate_inputs_for_test() -> StructuralClassifierCandidateV2 {
    let authority = super::exact_authority_for_test();
    let source = include_bytes!("../../../examples/moe_top2_v1/src/kernel.rs").to_vec();
    let ir = fe2o3_kernel_ir::moe_top2_v1_kernel_ir();
    let profile = MoeTop2ProfileV1::exact_gfx942_xnack_minus_cov6();
    StructuralClassifierCandidateV2 {
        source,
        source_identity: SOURCE_IDENTITY,
        fn_abi: MoeFnAbiStructuralProjectionV2 {
            identity: FN_ABI_IDENTITY,
            rust_calling_convention: true,
            c_variadic: false,
            fixed_count: 8,
            can_unwind: true,
            result_ignored: true,
            result_size: 0,
            arguments: [MoeFnAbiArgumentStructuralProjectionV2 {
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
            block_count: 118,
            statement_count: 228,
            terminator_count: 118,
            edge_count: 139,
            external_import_count: 0,
            root_argument_count: 8,
            root_local_count: 176,
            assignment_count: 226,
            call_count: 25,
            indexed_place_count: 36,
            repeat_count: 8,
            binary_operation_mask: 0x0000_ac05,
        },
        canonical: canonical_kernel_profile(&ir, &profile),
        same_session: SameSessionBindingV2 {
            compiler_semantics_identity: authority.compiler_semantics_identity,
            trusted_definitions_identity: authority.trusted_definitions_identity,
            root_instance_identity: authority.root_instance_identity,
            source_authority_identity: authority.authority_identity,
        },
    }
}

#[cfg(test)]
pub(super) fn checked_record_for_test_authority(
    validated_authority: ValidatedMoeTop2AuthorityV1<'_>,
) -> CheckedMoeSourceKirStructuralRecordV2 {
    let authority = validated_authority.authority();
    let mut candidate = candidate_inputs_for_test();
    candidate.same_session = SameSessionBindingV2 {
        compiler_semantics_identity: authority.compiler_semantics_identity,
        trusted_definitions_identity: authority.trusted_definitions_identity,
        root_instance_identity: authority.root_instance_identity.clone(),
        source_authority_identity: authority.authority_identity,
    };
    check_structural_inputs(candidate)
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
    fn aggregate_canonical_entries_cover_all_current_kir_and_profile_fields() {
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
    fn canonical_classifier_reports_exact_schema_mutation_surfaces() {
        let mut renamed = candidate_inputs_for_test().canonical;
        renamed.fields[0].name = "kir.renamed-module-id";
        assert_eq!(
            classify_canonical_fields(&renamed),
            Err(CanonicalClassifierErrorV2::FieldName {
                index: 0,
                expected: "kir.module-id",
                actual: "kir.renamed-module-id",
            })
        );

        let mut reordered = candidate_inputs_for_test().canonical;
        reordered.fields.swap(0, 1);
        assert_eq!(
            classify_canonical_fields(&reordered),
            Err(CanonicalClassifierErrorV2::FieldOrder {
                name: "kir.function-id",
                expected_index: 1,
                actual_index: 0,
            })
        );

        let mut removed = candidate_inputs_for_test().canonical;
        removed.fields.remove(5);
        assert_eq!(
            classify_canonical_fields(&removed),
            Err(CanonicalClassifierErrorV2::MissingField { name: "kir.layout" })
        );

        let mut duplicated = candidate_inputs_for_test().canonical;
        duplicated.fields.push(duplicated.fields[4].clone());
        assert_eq!(
            classify_canonical_fields(&duplicated),
            Err(CanonicalClassifierErrorV2::DuplicateField { name: "kir.shape" })
        );

        let mut unexpected = candidate_inputs_for_test().canonical;
        unexpected.fields.push(CanonicalFieldV2 {
            name: "profile.unexpected",
            memberships: MEMBER_PROFILE,
            value: vec![0],
        });
        assert_eq!(
            classify_canonical_fields(&unexpected),
            Err(CanonicalClassifierErrorV2::UnexpectedField {
                name: "profile.unexpected",
            })
        );

        let mut membership = candidate_inputs_for_test().canonical;
        membership.fields[0].memberships |= MEMBER_PROFILE;
        assert_eq!(
            classify_canonical_fields(&membership),
            Err(CanonicalClassifierErrorV2::Membership {
                name: "kir.module-id",
                expected: MEMBER_KERNEL_IR,
                actual: MEMBER_KERNEL_IR | MEMBER_PROFILE,
            })
        );

        let mut value = candidate_inputs_for_test().canonical;
        value.fields[0].value.push(0);
        assert_eq!(
            classify_canonical_fields(&value),
            Err(CanonicalClassifierErrorV2::Value {
                name: "kir.module-id",
            })
        );
    }

    #[test]
    fn every_aggregate_entry_has_exact_membership_and_value_failures() {
        let field_count = candidate_inputs_for_test().canonical.fields.len();
        for index in 0..field_count {
            let mut value_mutation = candidate_inputs_for_test().canonical;
            let name = value_mutation.fields[index].name;
            value_mutation.fields[index].value.push(0);
            assert_eq!(
                classify_canonical_fields(&value_mutation),
                Err(CanonicalClassifierErrorV2::Value { name })
            );

            let mut membership_mutation = candidate_inputs_for_test().canonical;
            let expected = membership_mutation.fields[index].memberships;
            membership_mutation.fields[index].memberships ^= MEMBER_KERNEL_IR;
            assert_eq!(
                classify_canonical_fields(&membership_mutation),
                Err(CanonicalClassifierErrorV2::Membership {
                    name,
                    expected,
                    actual: expected ^ MEMBER_KERNEL_IR,
                })
            );
        }
    }

    #[test]
    fn bounded_post_admission_drift_reaches_the_snapshot_surface() {
        let mut candidate = candidate_inputs_for_test();
        candidate.portable_mir.assignment_count += 1;
        let actual = snapshot_text(&candidate);
        assert_eq!(
            check_structural_inputs(candidate),
            Err(MoeSourceKirProducerErrorV2::SnapshotMismatch { actual })
        );

        let mut candidate = candidate_inputs_for_test();
        candidate.same_session.compiler_semantics_identity[0] ^= 1;
        let actual = snapshot_text(&candidate);
        assert_eq!(
            check_structural_inputs(candidate),
            Err(MoeSourceKirProducerErrorV2::SnapshotMismatch { actual })
        );
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

        let mut fn_abi = candidate_inputs_for_test();
        fn_abi.fn_abi.arguments[0].alignment = 4;
        assert_eq!(
            check_structural_inputs(fn_abi),
            Err(MoeSourceKirProducerErrorV2::FnAbi)
        );

        let mut same_session = candidate_inputs_for_test();
        same_session.same_session.source_authority_identity = [0; 32];
        assert_eq!(
            check_structural_inputs(same_session),
            Err(MoeSourceKirProducerErrorV2::SameSession)
        );

        let mut canonical = candidate_inputs_for_test();
        canonical.canonical.fields[0].value.push(0);
        assert_eq!(
            check_structural_inputs(canonical),
            Err(MoeSourceKirProducerErrorV2::Canonical(
                CanonicalClassifierErrorV2::Value {
                    name: "kir.module-id",
                }
            ))
        );
    }

    #[test]
    fn checked_record_is_explicitly_inert() {
        let authority = super::super::exact_authority_for_test();
        let validated_authority = super::super::validated_authority::validate_authority(&authority)
            .expect("synthetic exact authority validates");
        let checked = checked_record_for_test_authority(validated_authority);
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
