//! Deterministic, inert `ArtifactContainerV1` assembly for finalized Worker V2 COV6 output.
//!
//! The artifact container wire does not carry durable Worker publication lineage, descriptor
//! symbols, code-object version, or the descriptor compiler commit. This adapter retains those
//! fields beside the canonical container. It does not publish the container and cannot prove that
//! its immutable publication snapshot is still current after assembly.

#![allow(dead_code)] // The prepared plan stays inert until publication can reacquire a current lease.

use std::fmt;

use fe2o3_artifact_transaction::{
    AttemptScopedHsacoPublicationOutcomeV1, BackendPublicationReceiptV1, BuildAttempt,
    DurableLinkPublicationPlanV1, LinkPublicationScopeV1, UpstreamCodeObjectEvidenceIdentityV1,
};
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    ArtifactContainerV1, BlockSize, CodeObjectFormat, CodeObjectIdentity, CodeObjectPayload,
    CompilerIdentity, ContainerValidationError, DeclaredRustLayoutIdentity,
    DeclaredRustTypeIdentity, DigestAlgorithm, DigestBytes, Dimensions, Endianness, IdentityText,
    KernelEntry, LaunchContract, ManifestV1, Mutability, Name, PointerWidth, ScalarType,
    ToolIdentity, TypeIdentity, ValidationError,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion, ExplicitValueKind, InspectedKernel,
};
use fe2o3_hsaco_finalize::{FinalizationError, inspect_finalized};
use sha2::{Digest, Sha256};

const TARGET: &str = "gfx942:xnack-";
const TARGET_TRIPLE: &str = "amdgcn-amd-amdhsa";
const REQUIRED_KERNELS: [&str; 2] = ["alpha", "zeta"];
const ATTEMPT_IDENTITY_DOMAIN: &[u8] = b"fe2o3.backend-receipt.attempt.v1\0";
const SCOPE_IDENTITY_DOMAIN: &[u8] = b"fe2o3.backend-receipt.scope.v1\0";
const PLAN_IDENTITY_DOMAIN: &[u8] = b"fe2o3.durable-link.complete-plan.v1\0";
const RUST_TYPE_DOMAIN: &[u8] = b"FE2O3/RUST-TYPE/V1\0";
const DEVICE_LAYOUT_DOMAIN: &[u8] = b"FE2O3/DEVICE-LAYOUT/V1\0";

/// Descriptor identity that the V1 container wire cannot retain by itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkerV2DescriptorKernelLineageV1 {
    kernel_id: DigestBytes,
    logical_name: String,
    entry_name: String,
    descriptor_symbol: String,
    source_evidence_identity: [u8; 32],
    executable_evidence_identity: [u8; 32],
}

impl WorkerV2DescriptorKernelLineageV1 {
    pub(crate) const fn kernel_id(&self) -> DigestBytes {
        self.kernel_id
    }

    pub(crate) fn logical_name(&self) -> &str {
        &self.logical_name
    }

    pub(crate) fn entry_name(&self) -> &str {
        &self.entry_name
    }

    pub(crate) fn descriptor_symbol(&self) -> &str {
        &self.descriptor_symbol
    }

    pub(crate) const fn source_evidence_identity(&self) -> [u8; 32] {
        self.source_evidence_identity
    }

    pub(crate) const fn executable_evidence_identity(&self) -> [u8; 32] {
        self.executable_evidence_identity
    }
}

/// Inert result of assembling the two-entry container from one finalized publication snapshot.
///
/// The complete publication plan and receipt fields are retained because `ArtifactContainerV1`
/// has no slots for them. This value is deliberately not a publication lease or currentness token.
#[derive(Debug)]
pub(crate) struct PreparedWorkerV2ArtifactContainerV1 {
    container: ArtifactContainerV1,
    attempt: BuildAttempt,
    outcome: AttemptScopedHsacoPublicationOutcomeV1,
    raw_output_digest: [u8; 32],
    finalized_output_digest: [u8; 32],
    canonical_code_object_digest: [u8; 32],
    finalization_identity: [u8; 32],
    publication_identity: [u8; 32],
    upstream_evidence_identity: [u8; 32],
    producer_receipt_identity: [u8; 32],
    compiler_commit: [u8; 20],
    descriptors: [WorkerV2DescriptorKernelLineageV1; 2],
}

impl PreparedWorkerV2ArtifactContainerV1 {
    pub(crate) const fn container(&self) -> &ArtifactContainerV1 {
        &self.container
    }

    pub(crate) fn to_container_bytes(&self) -> Vec<u8> {
        self.container.to_bytes()
    }

    pub(crate) const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub(crate) const fn outcome(&self) -> AttemptScopedHsacoPublicationOutcomeV1 {
        self.outcome
    }

    pub(crate) const fn raw_output_digest(&self) -> [u8; 32] {
        self.raw_output_digest
    }

    pub(crate) const fn finalized_output_digest(&self) -> [u8; 32] {
        self.finalized_output_digest
    }

    pub(crate) const fn canonical_code_object_digest(&self) -> [u8; 32] {
        self.canonical_code_object_digest
    }

    pub(crate) const fn finalization_identity(&self) -> [u8; 32] {
        self.finalization_identity
    }

    pub(crate) const fn publication_identity(&self) -> [u8; 32] {
        self.publication_identity
    }

    pub(crate) const fn upstream_evidence_identity(&self) -> [u8; 32] {
        self.upstream_evidence_identity
    }

    pub(crate) const fn producer_receipt_identity(&self) -> [u8; 32] {
        self.producer_receipt_identity
    }

    pub(crate) const fn compiler_commit(&self) -> [u8; 20] {
        self.compiler_commit
    }

    pub(crate) fn descriptors(&self) -> &[WorkerV2DescriptorKernelLineageV1; 2] {
        &self.descriptors
    }

    pub(crate) const fn grants_current_publication_authority(&self) -> bool {
        false
    }

    pub(crate) const fn grants_load_authority(&self) -> bool {
        false
    }

    pub(crate) const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) enum WorkerV2ArtifactContainerAssemblyErrorV1 {
    StaleAttempt,
    ReceiptSubstitution(&'static str),
    FinalizedDigestMismatch,
    FinalizedHsaco(FinalizationError),
    CodeObjectVersion,
    Target,
    KernelCount,
    KernelSet,
    DuplicateKernelField(&'static str),
    DescriptorModel(&'static str),
    ArtifactModel(ValidationError),
    Container(ContainerValidationError),
}

impl fmt::Display for WorkerV2ArtifactContainerAssemblyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleAttempt => formatter
                .write_str("Worker V2 artifact assembly rejected a stale publication attempt"),
            Self::ReceiptSubstitution(field) => write!(
                formatter,
                "Worker V2 publication receipt substituted {field}"
            ),
            Self::FinalizedDigestMismatch => formatter
                .write_str("finalized Worker V2 payload does not match its publication digest"),
            Self::FinalizedHsaco(error) => {
                write!(
                    formatter,
                    "finalized Worker V2 COV6 inspection failed: {error}"
                )
            }
            Self::CodeObjectVersion => {
                formatter.write_str("Worker V2 artifact assembly requires COV6")
            }
            Self::Target => write!(
                formatter,
                "Worker V2 artifact assembly requires target {TARGET}"
            ),
            Self::KernelCount => {
                formatter.write_str("Worker V2 artifact assembly requires exactly two kernels")
            }
            Self::KernelSet => formatter
                .write_str("Worker V2 artifact assembly requires exact alpha and zeta entries"),
            Self::DuplicateKernelField(field) => {
                write!(formatter, "duplicate descriptor kernel {field}")
            }
            Self::DescriptorModel(reason) => write!(
                formatter,
                "descriptor cannot be represented in manifest V1: {reason}"
            ),
            Self::ArtifactModel(error) => error.fmt(formatter),
            Self::Container(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WorkerV2ArtifactContainerAssemblyErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FinalizedHsaco(error) => Some(error),
            Self::ArtifactModel(error) => Some(error),
            Self::Container(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone)]
struct DescriptorTableAssemblyV1 {
    compiler_name: String,
    compiler_release: String,
    compiler_commit: [u8; 20],
    producer_name: String,
    producer_version: String,
    kernels: Vec<DescriptorKernelAssemblyV1>,
}

#[derive(Clone)]
struct DescriptorKernelAssemblyV1 {
    kernel_id: [u8; 32],
    logical_name: String,
    entry_name: String,
    descriptor_symbol: String,
    source_digest: [u8; 32],
    source_evidence_identity: [u8; 32],
    executable_digest: [u8; 32],
    executable_evidence_identity: [u8; 32],
    explicit_size: u32,
    kernarg_size: u32,
    kernarg_alignment: u32,
    rank: u8,
    max_grid: [u32; 3],
    static_shared_memory_bytes: u32,
    max_dynamic_shared_memory_bytes: u32,
    fields: Vec<DescriptorFieldAssemblyV1>,
}

#[derive(Clone)]
struct DescriptorFieldAssemblyV1 {
    name: String,
    offset: u32,
    kind: AbiKind,
    access: Access,
    mutability: Mutability,
    ownership: ArgumentOwnership,
    alias: AliasClass,
    rust_type: [u8; 32],
    layout: [u8; 32],
}

#[derive(Clone, Copy)]
enum FrozenFieldKindV1 {
    F32,
    SharedF32,
    WriteOnlyDisjointF32,
}

impl FrozenFieldKindV1 {
    const fn descriptor_kind_tag(self) -> u8 {
        match self {
            Self::F32 => 1,
            Self::SharedF32 => 2,
            Self::WriteOnlyDisjointF32 => 3,
        }
    }

    const fn size(self) -> u16 {
        match self {
            Self::F32 => 4,
            Self::SharedF32 | Self::WriteOnlyDisjointF32 => 16,
        }
    }

    const fn alignment(self) -> u16 {
        match self {
            Self::F32 => 4,
            Self::SharedF32 | Self::WriteOnlyDisjointF32 => 8,
        }
    }

    const fn kind(self) -> AbiKind {
        match self {
            Self::F32 => AbiKind::Scalar(ScalarType::F32),
            Self::SharedF32 | Self::WriteOnlyDisjointF32 => AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
        }
    }

    const fn access(self) -> Access {
        match self {
            Self::F32 => Access::ByValue,
            Self::SharedF32 => Access::ReadOnly,
            Self::WriteOnlyDisjointF32 => Access::WriteOnly,
        }
    }

    const fn mutability(self) -> Mutability {
        match self {
            Self::WriteOnlyDisjointF32 => Mutability::Mutable,
            Self::F32 | Self::SharedF32 => Mutability::Immutable,
        }
    }

    const fn ownership(self) -> ArgumentOwnership {
        match self {
            Self::F32 => ArgumentOwnership::ByValue,
            Self::SharedF32 => ArgumentOwnership::SharedBorrow,
            Self::WriteOnlyDisjointF32 => ArgumentOwnership::UniqueBorrow,
        }
    }

    const fn alias(self) -> AliasClass {
        match self {
            Self::F32 => AliasClass::Value,
            Self::SharedF32 => AliasClass::SharedReadOnly,
            Self::WriteOnlyDisjointF32 => AliasClass::Exclusive,
        }
    }

    fn rust_type_identity(self) -> [u8; 32] {
        descriptor_identity(RUST_TYPE_DOMAIN, &[self.descriptor_kind_tag(), 10, 0, 0])
    }

    fn layout_identity(self) -> [u8; 32] {
        let mut descriptor = Vec::with_capacity(12);
        descriptor.push(self.descriptor_kind_tag());
        descriptor.push(10);
        descriptor.extend_from_slice(&self.size().to_le_bytes());
        descriptor.extend_from_slice(&self.alignment().to_le_bytes());
        let reference = !matches!(self, Self::F32);
        descriptor.push(u8::from(reference) * 8);
        descriptor.push(u8::from(reference) * 8);
        descriptor.extend_from_slice(&0_u16.to_le_bytes());
        descriptor.extend_from_slice(&0_u16.to_le_bytes());
        descriptor_identity(DEVICE_LAYOUT_DOMAIN, &descriptor)
    }
}

#[derive(Clone, Copy)]
struct FrozenFieldV1 {
    name: &'static str,
    offset: u32,
    kind: FrozenFieldKindV1,
}

const ALPHA_FIELDS: [FrozenFieldV1; 3] = [
    FrozenFieldV1 {
        name: "scale",
        offset: 0,
        kind: FrozenFieldKindV1::F32,
    },
    FrozenFieldV1 {
        name: "input",
        offset: 8,
        kind: FrozenFieldKindV1::SharedF32,
    },
    FrozenFieldV1 {
        name: "output",
        offset: 24,
        kind: FrozenFieldKindV1::WriteOnlyDisjointF32,
    },
];

const ZETA_FIELDS: [FrozenFieldV1; 4] = [
    FrozenFieldV1 {
        name: "a",
        offset: 0,
        kind: FrozenFieldKindV1::SharedF32,
    },
    FrozenFieldV1 {
        name: "b",
        offset: 16,
        kind: FrozenFieldKindV1::SharedF32,
    },
    FrozenFieldV1 {
        name: "bias",
        offset: 32,
        kind: FrozenFieldKindV1::F32,
    },
    FrozenFieldV1 {
        name: "output",
        offset: 40,
        kind: FrozenFieldKindV1::WriteOnlyDisjointF32,
    },
];

/// Prepares one deterministic two-entry container from exact finalized Worker V2 bytes.
///
/// The caller must still retain or reacquire a durable current-publication lease before using the
/// result in a loader. The current artifact-transaction API cannot reacquire such a lease after a
/// receipt-only restart reconciliation, so this adapter intentionally returns inert evidence.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_worker_v2_artifact_container_v1(
    expected_attempt: BuildAttempt,
    expected_producer_receipt_identity: [u8; 32],
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    receipt: BackendPublicationReceiptV1,
    outcome: AttemptScopedHsacoPublicationOutcomeV1,
    exact_finalized_hsaco: &[u8],
) -> Result<PreparedWorkerV2ArtifactContainerV1, WorkerV2ArtifactContainerAssemblyErrorV1> {
    prepare_worker_v2_artifact_container_from_receipt_v1(
        expected_attempt,
        expected_producer_receipt_identity,
        plan,
        upstream,
        ReceiptFieldsV1::from(receipt),
        outcome,
        exact_finalized_hsaco,
    )
}

#[derive(Clone, Copy)]
struct ReceiptFieldsV1 {
    attempt: [u8; 32],
    producer: [u8; 32],
    scope: [u8; 32],
    plan: [u8; 32],
    upstream: [u8; 32],
    finalized: [u8; 32],
    publication: [u8; 32],
}

impl From<BackendPublicationReceiptV1> for ReceiptFieldsV1 {
    fn from(receipt: BackendPublicationReceiptV1) -> Self {
        Self {
            attempt: receipt.attempt_identity(),
            producer: receipt.producer_identity(),
            scope: receipt.scope_identity(),
            plan: receipt.plan_commitment(),
            upstream: receipt.upstream_evidence_identity(),
            finalized: receipt.finalized_output_identity(),
            publication: receipt.publication_identity(),
        }
    }
}

fn prepare_worker_v2_artifact_container_from_receipt_v1(
    expected_attempt: BuildAttempt,
    expected_producer_receipt_identity: [u8; 32],
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    receipt: ReceiptFieldsV1,
    outcome: AttemptScopedHsacoPublicationOutcomeV1,
    exact_finalized_hsaco: &[u8],
) -> Result<PreparedWorkerV2ArtifactContainerV1, WorkerV2ArtifactContainerAssemblyErrorV1> {
    validate_lineage(
        expected_attempt,
        expected_producer_receipt_identity,
        plan,
        upstream,
        receipt,
        exact_finalized_hsaco,
    )?;
    let inspection = inspect_finalized(exact_finalized_hsaco)
        .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::FinalizedHsaco)?;
    let hsaco = inspection.hsaco();
    if hsaco.code_object_version() != CodeObjectVersion::V6 {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::CodeObjectVersion);
    }
    if hsaco.target().to_string() != TARGET {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::Target);
    }
    let table = inspection.descriptor_table();
    if table.kernels().len() != 2 || hsaco.kernels().len() != 2 {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::KernelCount);
    }

    let mut kernels = Vec::with_capacity(2);
    for descriptor in table.kernels() {
        let entry = descriptor.entry_name().as_str();
        let fields: &[FrozenFieldV1] = match entry {
            "alpha" => &ALPHA_FIELDS,
            "zeta" => &ZETA_FIELDS,
            _ => return Err(WorkerV2ArtifactContainerAssemblyErrorV1::KernelSet),
        };
        if descriptor.logical_name().as_str() != entry
            || descriptor.descriptor_symbol().as_str() != format!("{entry}.kd")
            || !descriptor.capabilities().is_empty()
            || descriptor.arguments().len() != fields.len()
        {
            return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
                "kernel identity, capabilities, or argument count differs from the frozen alpha/zeta profile",
            ));
        }
        let metadata = hsaco
            .kernels()
            .iter()
            .find(|kernel| kernel.name() == entry)
            .ok_or(WorkerV2ArtifactContainerAssemblyErrorV1::KernelSet)?;
        validate_physical_profile(metadata, fields)?;

        let mut converted_fields = Vec::with_capacity(fields.len());
        for (source_index, (argument, expected)) in
            descriptor.arguments().iter().zip(fields).enumerate()
        {
            let components = argument.physical_components().collect::<Vec<_>>();
            let expected_components = expected_components(*expected);
            if usize::from(argument.source_index()) != source_index
                || argument.name().as_str() != expected.name
                || argument.source_type().as_bytes() != &expected.kind.rust_type_identity()
                || argument.device_layout().as_bytes() != &expected.kind.layout_identity()
                || components
                    .iter()
                    .map(|(_, offset, size, alignment)| (*offset, *size, *alignment))
                    .ne(expected_components.iter().copied())
            {
                return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
                    "logical or physical argument descriptor differs from the frozen alpha/zeta profile",
                ));
            }
            converted_fields.push(DescriptorFieldAssemblyV1 {
                name: expected.name.to_owned(),
                offset: expected.offset,
                kind: expected.kind.kind(),
                access: expected.kind.access(),
                mutability: expected.kind.mutability(),
                ownership: expected.kind.ownership(),
                alias: expected.kind.alias(),
                rust_type: expected.kind.rust_type_identity(),
                layout: expected.kind.layout_identity(),
            });
        }

        let layout = descriptor.abi_layout();
        let expected_explicit = if entry == "alpha" { 40 } else { 56 };
        let expected_kernarg = expected_explicit + 256;
        if layout.explicit_argument_size() != expected_explicit
            || layout.kernarg_segment_size() != expected_kernarg
            || layout.kernarg_segment_alignment() != 8
        {
            return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
                "COV6 kernarg size or alignment differs from the frozen alpha/zeta profile",
            ));
        }
        let launch = descriptor.launch();
        let max_grid = launch.max_grid();
        kernels.push(DescriptorKernelAssemblyV1 {
            kernel_id: *descriptor.kernel_id().as_bytes(),
            logical_name: descriptor.logical_name().as_str().to_owned(),
            entry_name: entry.to_owned(),
            descriptor_symbol: descriptor.descriptor_symbol().as_str().to_owned(),
            source_digest: *descriptor.source_evidence().digest().as_bytes(),
            source_evidence_identity: *descriptor.source_evidence().identity().as_bytes(),
            executable_digest: *descriptor.executable_ir_evidence().digest().as_bytes(),
            executable_evidence_identity: *descriptor
                .executable_ir_evidence()
                .identity()
                .as_bytes(),
            explicit_size: layout.explicit_argument_size(),
            kernarg_size: layout.kernarg_segment_size(),
            kernarg_alignment: layout.kernarg_segment_alignment(),
            rank: launch.rank(),
            max_grid: [max_grid.x(), max_grid.y(), max_grid.z()],
            static_shared_memory_bytes: launch.static_shared_memory_bytes(),
            max_dynamic_shared_memory_bytes: launch.max_dynamic_shared_memory_bytes(),
            fields: converted_fields,
        });
    }
    let table = DescriptorTableAssemblyV1 {
        compiler_name: table.compiler().name().as_str().to_owned(),
        compiler_release: table.compiler().release().as_str().to_owned(),
        compiler_commit: *table.compiler().commit(),
        producer_name: table.producer().name().as_str().to_owned(),
        producer_version: table.producer().version().as_str().to_owned(),
        kernels,
    };
    validate_profile(&table)?;
    let (container, descriptors) = build_container(&table, exact_finalized_hsaco.to_vec())?;

    Ok(PreparedWorkerV2ArtifactContainerV1 {
        container,
        attempt: expected_attempt,
        outcome,
        raw_output_digest: *plan.linked_output().as_bytes(),
        finalized_output_digest: receipt.finalized,
        canonical_code_object_digest: *inspection.digest().as_bytes(),
        finalization_identity: *plan.finalization().as_bytes(),
        publication_identity: receipt.publication,
        upstream_evidence_identity: receipt.upstream,
        producer_receipt_identity: receipt.producer,
        compiler_commit: table.compiler_commit,
        descriptors,
    })
}

fn expected_components(field: FrozenFieldV1) -> Vec<(u32, u16, u16)> {
    match field.kind {
        FrozenFieldKindV1::F32 => vec![(field.offset, 4, 4)],
        FrozenFieldKindV1::SharedF32 | FrozenFieldKindV1::WriteOnlyDisjointF32 => {
            vec![(field.offset, 8, 8), (field.offset + 8, 8, 8)]
        }
    }
}

fn validate_physical_profile(
    kernel: &InspectedKernel,
    fields: &[FrozenFieldV1],
) -> Result<(), WorkerV2ArtifactContainerAssemblyErrorV1> {
    if kernel.required_workgroup_size() != Some([256, 1, 1])
        || kernel.max_flat_workgroup_size() != 256
        || kernel.group_segment_fixed_size() != 0
    {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
            "physical launch metadata differs from the frozen alpha/zeta profile",
        ));
    }
    let expected = fields
        .iter()
        .flat_map(|field| match field.kind {
            FrozenFieldKindV1::F32 => vec![(
                u64::from(field.offset),
                4,
                ExplicitValueKind::ByValue,
                None,
                None,
            )],
            FrozenFieldKindV1::SharedF32 | FrozenFieldKindV1::WriteOnlyDisjointF32 => vec![
                (
                    u64::from(field.offset),
                    8,
                    ExplicitValueKind::GlobalBuffer,
                    Some(ArgumentAddressSpace::Global),
                    Some(match field.kind {
                        FrozenFieldKindV1::SharedF32 => ArgumentAccess::ReadOnly,
                        FrozenFieldKindV1::WriteOnlyDisjointF32 => ArgumentAccess::WriteOnly,
                        FrozenFieldKindV1::F32 => unreachable!(),
                    }),
                ),
                (
                    u64::from(field.offset + 8),
                    8,
                    ExplicitValueKind::ByValue,
                    None,
                    None,
                ),
            ],
        })
        .collect::<Vec<_>>();
    if kernel.explicit_arguments().len() != expected.len() {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
            "physical argument count differs from the frozen alpha/zeta profile",
        ));
    }
    for (argument, (offset, size, kind, address_space, access)) in
        kernel.explicit_arguments().iter().zip(expected)
    {
        if argument.offset() != offset
            || argument.size() != size
            || argument.value_kind() != kind
            || argument.address_space() != address_space
            || access.is_some_and(|expected| {
                argument.access().is_some_and(|actual| actual != expected)
                    || argument
                        .actual_access()
                        .is_some_and(|actual| actual != expected)
            })
        {
            return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
                "physical argument metadata differs from the frozen alpha/zeta profile",
            ));
        }
    }
    Ok(())
}

fn validate_lineage(
    expected_attempt: BuildAttempt,
    expected_producer_receipt_identity: [u8; 32],
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    receipt: ReceiptFieldsV1,
    exact_finalized_hsaco: &[u8],
) -> Result<(), WorkerV2ArtifactContainerAssemblyErrorV1> {
    if plan.attempt() != expected_attempt {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::StaleAttempt);
    }
    require_receipt(
        receipt.attempt == attempt_identity(expected_attempt),
        "attempt identity",
    )?;
    require_receipt(
        receipt.producer == expected_producer_receipt_identity,
        "producer identity",
    )?;
    require_receipt(
        receipt.scope == scope_identity(plan.scope()),
        "scope identity",
    )?;
    require_receipt(receipt.plan == plan_identity(plan), "plan commitment")?;
    require_receipt(
        receipt.upstream == upstream.as_bytes(),
        "upstream evidence identity",
    )?;
    require_receipt(
        receipt.finalized == *plan.finalized_output().as_bytes(),
        "finalized output identity",
    )?;
    require_receipt(
        receipt.publication == *plan.publication().as_bytes(),
        "publication identity",
    )?;
    let measured: [u8; 32] = Sha256::digest(exact_finalized_hsaco).into();
    if measured != receipt.finalized {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::FinalizedDigestMismatch);
    }
    Ok(())
}

fn require_receipt(
    condition: bool,
    field: &'static str,
) -> Result<(), WorkerV2ArtifactContainerAssemblyErrorV1> {
    if condition {
        Ok(())
    } else {
        Err(WorkerV2ArtifactContainerAssemblyErrorV1::ReceiptSubstitution(field))
    }
}

fn validate_profile(
    table: &DescriptorTableAssemblyV1,
) -> Result<(), WorkerV2ArtifactContainerAssemblyErrorV1> {
    if table.kernels.len() != 2 {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::KernelCount);
    }
    let mut names = table
        .kernels
        .iter()
        .map(|kernel| kernel.entry_name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    if names != REQUIRED_KERNELS {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::KernelSet);
    }
    reject_duplicate_kernel_fields(&table.kernels)
}

fn build_container(
    table: &DescriptorTableAssemblyV1,
    exact_finalized_hsaco: Vec<u8>,
) -> Result<
    (ArtifactContainerV1, [WorkerV2DescriptorKernelLineageV1; 2]),
    WorkerV2ArtifactContainerAssemblyErrorV1,
> {
    validate_profile(table)?;
    let payload = CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, exact_finalized_hsaco)
        .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::Container)?;
    let payload_digest = payload.digest().bytes();
    let code_object = CodeObjectIdentity::new(
        payload_digest,
        CodeObjectFormat::NativeExecutable,
        payload.bytes().len() as u64,
    )
    .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)?;
    let kernels = table
        .kernels
        .iter()
        .map(|descriptor| kernel_entry(descriptor, payload_digest))
        .collect::<Result<Vec<_>, _>>()?;
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text(&table.compiler_name)?, text(&table.compiler_release)?),
        ToolIdentity::new(text(&table.producer_name)?, text(&table.producer_version)?),
        fe2o3_artifacts::TargetIdentity::new(
            text(TARGET_TRIPLE)?,
            text(TARGET)?,
            PointerWidth::Bits64,
            Endianness::Little,
            vec![],
        )
        .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)?,
        vec![code_object],
        kernels,
    )
    .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)?;
    let container = ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload])
        .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::Container)?;

    let mut descriptors = table
        .kernels
        .iter()
        .map(descriptor_lineage)
        .collect::<Vec<_>>();
    descriptors.sort_unstable_by_key(WorkerV2DescriptorKernelLineageV1::kernel_id);
    let descriptors = descriptors
        .try_into()
        .map_err(|_| WorkerV2ArtifactContainerAssemblyErrorV1::KernelCount)?;
    Ok((container, descriptors))
}

fn reject_duplicate_kernel_fields(
    descriptors: &[DescriptorKernelAssemblyV1],
) -> Result<(), WorkerV2ArtifactContainerAssemblyErrorV1> {
    for (field, mut values) in [
        (
            "ID",
            descriptors
                .iter()
                .map(|kernel| kernel.kernel_id.as_slice())
                .collect::<Vec<_>>(),
        ),
        (
            "logical name",
            descriptors
                .iter()
                .map(|kernel| kernel.logical_name.as_bytes())
                .collect(),
        ),
        (
            "entry symbol",
            descriptors
                .iter()
                .map(|kernel| kernel.entry_name.as_bytes())
                .collect(),
        ),
        (
            "descriptor symbol",
            descriptors
                .iter()
                .map(|kernel| kernel.descriptor_symbol.as_bytes())
                .collect(),
        ),
    ] {
        values.sort_unstable();
        if values.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DuplicateKernelField(field));
        }
    }
    Ok(())
}

fn kernel_entry(
    descriptor: &DescriptorKernelAssemblyV1,
    payload_digest: DigestBytes,
) -> Result<KernelEntry, WorkerV2ArtifactContainerAssemblyErrorV1> {
    if descriptor.kernarg_size != descriptor.explicit_size + 256 {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
            "COV6 hidden argument span is not exactly 256 bytes",
        ));
    }
    let fields = descriptor
        .fields
        .iter()
        .map(abi_field)
        .collect::<Result<Vec<_>, _>>()?;
    let abi = AbiLayout::new(
        u64::from(descriptor.explicit_size),
        descriptor.kernarg_alignment,
        PointerWidth::Bits64,
        fields,
    )
    .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)?;
    let launch = LaunchContract::new(
        descriptor.rank,
        BlockSize::Exact(
            Dimensions::new(256, 1, 1)
                .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)?,
        ),
        Dimensions::new(
            descriptor.max_grid[0],
            descriptor.max_grid[1],
            descriptor.max_grid[2],
        )
        .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)?,
        descriptor.static_shared_memory_bytes,
        descriptor.max_dynamic_shared_memory_bytes,
    )
    .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)?;
    KernelEntry::new(
        DigestBytes::from_bytes(descriptor.kernel_id),
        name(&descriptor.logical_name)?,
        name(&descriptor.entry_name)?,
        DigestBytes::from_bytes(descriptor.source_digest),
        DigestBytes::from_bytes(descriptor.executable_digest),
        payload_digest,
        vec![],
        launch,
        abi,
    )
    .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)
}

fn abi_field(
    field: &DescriptorFieldAssemblyV1,
) -> Result<AbiField, WorkerV2ArtifactContainerAssemblyErrorV1> {
    let size = match field.kind {
        AbiKind::Scalar(ScalarType::F32) => 4,
        AbiKind::Slice {
            element_size: 4,
            element_alignment: 4,
        } => 16,
        _ => {
            return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
                "field is outside the frozen alpha/zeta ABI",
            ));
        }
    };
    let alignment = if size == 4 { 4 } else { 8 };
    let address_space = if field.access == Access::ByValue {
        AddressSpace::Value
    } else {
        AddressSpace::Global
    };
    AbiField::new(
        name(&field.name)?,
        u64::from(field.offset),
        size,
        alignment,
        field.kind,
        field.mutability,
        field.access,
        address_space,
        TypeIdentity::new(
            DeclaredRustTypeIdentity::from_untrusted_bytes(DigestBytes::from_bytes(
                field.rust_type,
            )),
            DeclaredRustLayoutIdentity::from_untrusted_bytes(DigestBytes::from_bytes(field.layout)),
        ),
        field.ownership,
        field.alias,
    )
    .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)
}

fn descriptor_lineage(
    descriptor: &DescriptorKernelAssemblyV1,
) -> WorkerV2DescriptorKernelLineageV1 {
    WorkerV2DescriptorKernelLineageV1 {
        kernel_id: DigestBytes::from_bytes(descriptor.kernel_id),
        logical_name: descriptor.logical_name.clone(),
        entry_name: descriptor.entry_name.clone(),
        descriptor_symbol: descriptor.descriptor_symbol.clone(),
        source_evidence_identity: descriptor.source_evidence_identity,
        executable_evidence_identity: descriptor.executable_evidence_identity,
    }
}

fn text(value: &str) -> Result<IdentityText, WorkerV2ArtifactContainerAssemblyErrorV1> {
    IdentityText::new(value).map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)
}

fn name(value: &str) -> Result<Name, WorkerV2ArtifactContainerAssemblyErrorV1> {
    Name::new(value).map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)
}

fn descriptor_identity(domain: &[u8], descriptor: &[u8]) -> [u8; 32] {
    hash_identity(domain, |digest| {
        digest.update((descriptor.len() as u64).to_le_bytes());
        digest.update(descriptor);
    })
}

fn attempt_identity(attempt: BuildAttempt) -> [u8; 32] {
    hash_identity(ATTEMPT_IDENTITY_DOMAIN, |digest| {
        digest.update(attempt.generation().to_le_bytes());
        digest.update(attempt.session().as_bytes());
        digest.update(attempt.invocation().as_bytes());
    })
}

fn scope_identity(scope: LinkPublicationScopeV1) -> [u8; 32] {
    hash_identity(SCOPE_IDENTITY_DOMAIN, |digest| {
        digest.update(scope.package().as_bytes());
        digest.update(scope.kernel_set().as_bytes());
        digest.update(scope.target().as_bytes());
    })
}

fn plan_identity(plan: DurableLinkPublicationPlanV1) -> [u8; 32] {
    hash_identity(PLAN_IDENTITY_DOMAIN, |digest| {
        let attempt = plan.attempt();
        digest.update(attempt.generation().to_le_bytes());
        digest.update(attempt.session().as_bytes());
        digest.update(attempt.invocation().as_bytes());
        let scope = plan.scope();
        digest.update(scope.package().as_bytes());
        digest.update(scope.kernel_set().as_bytes());
        digest.update(scope.target().as_bytes());
        digest.update(plan.request().as_bytes());
        digest.update(plan.worker().as_bytes());
        digest.update(plan.response().as_bytes());
        digest.update(plan.linked_output().as_bytes());
        digest.update(plan.finalization().as_bytes());
        digest.update(plan.finalized_output().as_bytes());
        digest.update(plan.publication().as_bytes());
    })
}

fn hash_identity(domain: &[u8], update: impl FnOnce(&mut Sha256)) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    update(&mut digest);
    digest.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifact_transaction::{
        AtomicPublicationIdentityV1, BuildSession, CanonicalLinkRequestIdentityV1,
        FinalizationIdentityV1, FinalizedOutputIdentityV1, KernelSetIdentityV1,
        LinkedOutputIdentityV1, PackageIdentityV1, PinnedWorkerIdentityV1, TargetIdentityV1,
        ValidatedResponseIdentityV1,
    };

    fn attempt(generation: u64) -> BuildAttempt {
        let session = format!("{generation:032x}");
        let invocation = format!("{:064x}", generation + 1);
        BuildAttempt::from_env_value(&format!("{generation}:{session}:{invocation}")).unwrap()
    }

    fn plan(attempt: BuildAttempt, finalized: [u8; 32], seed: u8) -> DurableLinkPublicationPlanV1 {
        DurableLinkPublicationPlanV1::new(
            attempt,
            LinkPublicationScopeV1::new(
                PackageIdentityV1::from_bytes([seed; 32]),
                KernelSetIdentityV1::from_bytes([seed.wrapping_add(1); 32]),
                TargetIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
            ),
            CanonicalLinkRequestIdentityV1::from_bytes([seed.wrapping_add(3); 32]),
            PinnedWorkerIdentityV1::from_bytes([seed.wrapping_add(4); 32]),
            ValidatedResponseIdentityV1::from_bytes([seed.wrapping_add(5); 32]),
            LinkedOutputIdentityV1::from_bytes([seed.wrapping_add(6); 32]),
            FinalizationIdentityV1::from_bytes([seed.wrapping_add(7); 32]),
            FinalizedOutputIdentityV1::from_bytes(finalized),
            AtomicPublicationIdentityV1::from_bytes([seed.wrapping_add(8); 32]),
        )
    }

    fn receipt_fields(
        plan: DurableLinkPublicationPlanV1,
        upstream: UpstreamCodeObjectEvidenceIdentityV1,
    ) -> ReceiptFieldsV1 {
        ReceiptFieldsV1 {
            attempt: attempt_identity(plan.attempt()),
            producer: [0xa0; 32],
            scope: scope_identity(plan.scope()),
            plan: plan_identity(plan),
            upstream: upstream.as_bytes(),
            finalized: *plan.finalized_output().as_bytes(),
            publication: *plan.publication().as_bytes(),
        }
    }

    fn field(spec: FrozenFieldV1) -> DescriptorFieldAssemblyV1 {
        DescriptorFieldAssemblyV1 {
            name: spec.name.to_owned(),
            offset: spec.offset,
            kind: spec.kind.kind(),
            access: spec.kind.access(),
            mutability: spec.kind.mutability(),
            ownership: spec.kind.ownership(),
            alias: spec.kind.alias(),
            rust_type: spec.kind.rust_type_identity(),
            layout: spec.kind.layout_identity(),
        }
    }

    fn kernel(name: &str, id: u8) -> DescriptorKernelAssemblyV1 {
        let (explicit_size, fields) = match name {
            "alpha" => (40, ALPHA_FIELDS.iter().copied().map(field).collect()),
            "zeta" => (56, ZETA_FIELDS.iter().copied().map(field).collect()),
            _ => (40, ALPHA_FIELDS.iter().copied().map(field).collect()),
        };
        DescriptorKernelAssemblyV1 {
            kernel_id: [id; 32],
            logical_name: name.to_owned(),
            entry_name: name.to_owned(),
            descriptor_symbol: format!("{name}.kd"),
            source_digest: [id.wrapping_add(1); 32],
            source_evidence_identity: [id.wrapping_add(2); 32],
            executable_digest: [id.wrapping_add(3); 32],
            executable_evidence_identity: [id.wrapping_add(4); 32],
            explicit_size,
            kernarg_size: explicit_size + 256,
            kernarg_alignment: 8,
            rank: 1,
            max_grid: [u32::MAX, 1, 1],
            static_shared_memory_bytes: 0,
            max_dynamic_shared_memory_bytes: 0,
            fields,
        }
    }

    fn table(kernels: Vec<DescriptorKernelAssemblyV1>) -> DescriptorTableAssemblyV1 {
        DescriptorTableAssemblyV1 {
            compiler_name: "rustc".to_owned(),
            compiler_release: "test".to_owned(),
            compiler_commit: [0x72; 20],
            producer_name: "rustc-codegen-fe2o3".to_owned(),
            producer_version: "test".to_owned(),
            kernels,
        }
    }

    #[test]
    fn deterministic_two_entry_container_is_independent_of_descriptor_input_order() {
        let payload = vec![0x5a; 4096];
        let (forward, forward_lineage) = build_container(
            &table(vec![kernel("alpha", 0x20), kernel("zeta", 0x10)]),
            payload.clone(),
        )
        .unwrap();
        let (reverse, reverse_lineage) = build_container(
            &table(vec![kernel("zeta", 0x10), kernel("alpha", 0x20)]),
            payload,
        )
        .unwrap();

        assert_eq!(forward.to_bytes(), reverse.to_bytes());
        assert_eq!(forward_lineage, reverse_lineage);
        assert_eq!(forward.payloads().len(), 1);
        assert_eq!(forward.manifest().code_objects().len(), 1);
        assert_eq!(forward.manifest().kernels().len(), 2);
        assert_eq!(
            forward
                .manifest()
                .kernels()
                .iter()
                .map(|kernel| kernel.name().as_str())
                .collect::<Vec<_>>(),
            ["zeta", "alpha"]
        );
        assert!(forward
            .manifest()
            .kernels()
            .iter()
            .all(|kernel| kernel.code_object_digest() == forward.payloads()[0].digest().bytes()));
    }

    #[test]
    fn duplicate_names_symbols_and_ids_fail_closed() {
        let cases = [
            vec![kernel("alpha", 0x20), kernel("alpha", 0x20)],
            {
                let mut zeta = kernel("zeta", 0x10);
                zeta.logical_name = "alpha".to_owned();
                vec![kernel("alpha", 0x20), zeta]
            },
            {
                let mut zeta = kernel("zeta", 0x10);
                zeta.descriptor_symbol = "alpha.kd".to_owned();
                vec![kernel("alpha", 0x20), zeta]
            },
            {
                let mut zeta = kernel("zeta", 0x20);
                zeta.kernel_id = [0x20; 32];
                vec![kernel("alpha", 0x20), zeta]
            },
        ];
        for kernels in cases {
            assert!(matches!(
                validate_profile(&table(kernels)),
                Err(WorkerV2ArtifactContainerAssemblyErrorV1::DuplicateKernelField(_))
                    | Err(WorkerV2ArtifactContainerAssemblyErrorV1::KernelSet)
            ));
        }
    }

    #[test]
    fn stale_attempt_and_every_public_receipt_binding_fail_closed() {
        let bytes = vec![0x66; 96];
        let finalized: [u8; 32] = Sha256::digest(&bytes).into();
        let current_attempt = attempt(7);
        let plan = plan(current_attempt, finalized, 0x41);
        let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x51; 32]);
        let valid = receipt_fields(plan, upstream);

        assert!(matches!(
            validate_lineage(attempt(8), valid.producer, plan, upstream, valid, &bytes),
            Err(WorkerV2ArtifactContainerAssemblyErrorV1::StaleAttempt)
        ));

        for field in [
            "attempt",
            "producer",
            "scope",
            "plan",
            "upstream",
            "finalized",
            "publication",
        ] {
            let mut substituted = valid;
            match field {
                "attempt" => substituted.attempt[0] ^= 1,
                "producer" => substituted.producer[0] ^= 1,
                "scope" => substituted.scope[0] ^= 1,
                "plan" => substituted.plan[0] ^= 1,
                "upstream" => substituted.upstream[0] ^= 1,
                "finalized" => substituted.finalized[0] ^= 1,
                "publication" => substituted.publication[0] ^= 1,
                _ => unreachable!(),
            }
            assert!(matches!(
                validate_lineage(
                    current_attempt,
                    valid.producer,
                    plan,
                    upstream,
                    substituted,
                    &bytes
                ),
                Err(WorkerV2ArtifactContainerAssemblyErrorV1::ReceiptSubstitution(_))
            ));
        }

        let mut changed_payload = bytes;
        changed_payload[0] ^= 1;
        assert!(matches!(
            validate_lineage(
                current_attempt,
                valid.producer,
                plan,
                upstream,
                valid,
                &changed_payload
            ),
            Err(WorkerV2ArtifactContainerAssemblyErrorV1::FinalizedDigestMismatch)
        ));
    }

    #[test]
    fn descriptor_or_payload_substitution_changes_or_rejects_the_container() {
        let baseline = table(vec![kernel("alpha", 0x20), kernel("zeta", 0x10)]);
        let bytes = vec![0x6b; 128];
        let first = build_container(&baseline, bytes.clone())
            .unwrap()
            .0
            .to_bytes();

        let mut substituted = baseline.clone();
        substituted.kernels[0].source_digest[0] ^= 1;
        let second = build_container(&substituted, bytes.clone())
            .unwrap()
            .0
            .to_bytes();
        assert_ne!(first, second);

        let mut payload = bytes;
        payload[0] ^= 1;
        let third = build_container(&baseline, payload).unwrap().0.to_bytes();
        assert_ne!(first, third);
    }

    #[test]
    fn recovery_outcomes_preserve_container_bytes_and_distinct_lineage() {
        let descriptor_table = table(vec![kernel("alpha", 0x20), kernel("zeta", 0x10)]);
        let payload = vec![0x6a; 128];
        let (container, descriptors) = build_container(&descriptor_table, payload.clone()).unwrap();
        let digest: [u8; 32] = Sha256::digest(&payload).into();
        let attempt = attempt(9);
        let plan = plan(attempt, digest, 0x61);
        let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x62; 32]);
        let receipt = receipt_fields(plan, upstream);

        let prepared = |outcome| PreparedWorkerV2ArtifactContainerV1 {
            container: ArtifactContainerV1::from_bytes(&container.to_bytes()).unwrap(),
            attempt,
            outcome,
            raw_output_digest: *plan.linked_output().as_bytes(),
            finalized_output_digest: receipt.finalized,
            canonical_code_object_digest: [0x71; 32],
            finalization_identity: *plan.finalization().as_bytes(),
            publication_identity: receipt.publication,
            upstream_evidence_identity: receipt.upstream,
            producer_receipt_identity: receipt.producer,
            compiler_commit: descriptor_table.compiler_commit,
            descriptors: descriptors.clone(),
        };
        let fresh = prepared(AttemptScopedHsacoPublicationOutcomeV1::Published);
        let recovered =
            prepared(AttemptScopedHsacoPublicationOutcomeV1::RecoveredCommittedPublication);

        assert_eq!(fresh.to_container_bytes(), recovered.to_container_bytes());
        assert_ne!(fresh.outcome(), recovered.outcome());
        assert_eq!(fresh.attempt(), attempt);
        assert_eq!(fresh.raw_output_digest(), *plan.linked_output().as_bytes());
        assert_eq!(fresh.finalized_output_digest(), digest);
        assert_eq!(fresh.descriptors(), recovered.descriptors());
        assert!(!fresh.grants_current_publication_authority());
        assert!(!fresh.grants_load_authority());
        assert!(!fresh.grants_launch_authority());
    }

    #[test]
    fn frozen_type_and_layout_identities_are_distinct_and_stable() {
        assert_ne!(
            FrozenFieldKindV1::F32.rust_type_identity(),
            FrozenFieldKindV1::SharedF32.rust_type_identity()
        );
        assert_ne!(
            FrozenFieldKindV1::SharedF32.rust_type_identity(),
            FrozenFieldKindV1::WriteOnlyDisjointF32.rust_type_identity()
        );
        assert_ne!(
            FrozenFieldKindV1::F32.layout_identity(),
            FrozenFieldKindV1::SharedF32.layout_identity()
        );
    }

    #[test]
    fn build_session_shape_used_by_fixture_is_managed() {
        let attempt = attempt(3);
        assert_ne!(attempt.session(), BuildSession::DIRECT);
        assert_ne!(attempt.invocation().as_bytes(), &[0; 32]);
    }
}
