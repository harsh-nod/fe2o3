//! Deterministic, inert `ArtifactContainerV1` assembly for finalized Worker V2 COV6 output.
//!
//! The artifact container wire does not carry durable Worker publication lineage, descriptor
//! symbols, code-object version, or the descriptor compiler commit. This adapter retains those
//! fields beside the canonical container. It does not publish the container and cannot prove that
//! its immutable publication snapshot is still current after assembly.

use std::fmt;

use fe2o3_artifact_transaction::{
    AttemptScopedHsacoPublicationOutcomeV1, AttemptScopedHsacoPublicationResultV1,
    BackendPublicationReceiptValidationErrorV1, BuildAttempt, DurableLinkPublicationPlanV1,
    ProducerIdentity, UpstreamCodeObjectEvidenceIdentityV1,
    validate_backend_publication_receipt_v1,
};
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    ArtifactContainerV1, BlockSize, Capability, CodeObjectFormat, CodeObjectIdentity,
    CodeObjectPayload, CompilerIdentity, ContainerValidationError, DeclaredRustLayoutIdentity,
    DeclaredRustTypeIdentity, DigestAlgorithm, DigestBytes, Dimensions, Endianness, IdentityText,
    KernelEntry, LaunchContract, ManifestV1, Mutability, Name, PointerWidth, ScalarType,
    ToolIdentity, TypeIdentity, ValidationError,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion, ExplicitValueKind, InspectedKernel,
};
use fe2o3_hsaco_finalize::{FinalizationError, inspect_finalized};
use fe2o3_kernel_descriptor::{AccessMode, AliasSemantics, CapabilityV1, OwnershipSemantics};
use sha2::{Digest, Sha256};

const TARGET: &str = "gfx942:xnack-";
const TARGET_TRIPLE: &str = "amdgcn-amd-amdhsa";
const REQUIRED_KERNELS: [&str; 2] = ["alpha", "zeta"];
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

#[derive(Debug)]
#[non_exhaustive]
pub(crate) enum WorkerV2ArtifactContainerAssemblyErrorV1 {
    Receipt(BackendPublicationReceiptValidationErrorV1),
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
            Self::Receipt(error) => error.fmt(formatter),
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
            Self::Receipt(error) => Some(error),
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
    capabilities: Vec<Capability>,
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
    ReadWriteDisjointF32,
}

impl FrozenFieldKindV1 {
    const fn descriptor_kind_tag(self) -> u8 {
        match self {
            Self::F32 => 1,
            Self::SharedF32 => 2,
            Self::ReadWriteDisjointF32 => 3,
        }
    }

    const fn size(self) -> u16 {
        match self {
            Self::F32 => 4,
            Self::SharedF32 | Self::ReadWriteDisjointF32 => 16,
        }
    }

    const fn alignment(self) -> u16 {
        match self {
            Self::F32 => 4,
            Self::SharedF32 | Self::ReadWriteDisjointF32 => 8,
        }
    }

    const fn kind(self) -> AbiKind {
        match self {
            Self::F32 => AbiKind::Scalar(ScalarType::F32),
            Self::SharedF32 | Self::ReadWriteDisjointF32 => AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
        }
    }

    const fn access(self) -> Access {
        match self {
            Self::F32 => Access::ByValue,
            Self::SharedF32 => Access::ReadOnly,
            Self::ReadWriteDisjointF32 => Access::ReadWrite,
        }
    }

    const fn mutability(self) -> Mutability {
        match self {
            Self::ReadWriteDisjointF32 => Mutability::Mutable,
            Self::F32 | Self::SharedF32 => Mutability::Immutable,
        }
    }

    const fn ownership(self) -> ArgumentOwnership {
        match self {
            Self::F32 => ArgumentOwnership::ByValue,
            Self::SharedF32 => ArgumentOwnership::SharedBorrow,
            Self::ReadWriteDisjointF32 => ArgumentOwnership::UniqueBorrow,
        }
    }

    const fn alias(self) -> AliasClass {
        match self {
            Self::F32 => AliasClass::Value,
            Self::SharedF32 => AliasClass::SharedReadOnly,
            Self::ReadWriteDisjointF32 => AliasClass::Exclusive,
        }
    }

    const fn descriptor_ownership(self) -> OwnershipSemantics {
        match self {
            Self::F32 => OwnershipSemantics::ByValue,
            Self::SharedF32 => OwnershipSemantics::SharedBorrow,
            Self::ReadWriteDisjointF32 => OwnershipSemantics::UniqueBorrow,
        }
    }

    const fn descriptor_access(self) -> AccessMode {
        match self {
            Self::F32 => AccessMode::ByValue,
            Self::SharedF32 => AccessMode::ReadOnly,
            Self::ReadWriteDisjointF32 => AccessMode::ReadWrite,
        }
    }

    const fn descriptor_alias(self) -> AliasSemantics {
        match self {
            Self::F32 => AliasSemantics::Value,
            Self::SharedF32 => AliasSemantics::SharedReadOnly,
            Self::ReadWriteDisjointF32 => AliasSemantics::Exclusive,
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
        kind: FrozenFieldKindV1::ReadWriteDisjointF32,
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
        kind: FrozenFieldKindV1::ReadWriteDisjointF32,
    },
];

/// Prepares one deterministic two-entry container from typed Worker V2 publication evidence.
///
/// This test-only adapter deliberately has no container or serialization accessor. A production
/// caller must first define a durable envelope that binds all retained lineage to the container and
/// carries the publication lease into host admission.
pub(crate) fn prepare_worker_v2_artifact_container_v1(
    producer: &ProducerIdentity,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    publication: &AttemptScopedHsacoPublicationResultV1,
) -> Result<PreparedWorkerV2ArtifactContainerV1, WorkerV2ArtifactContainerAssemblyErrorV1> {
    let receipt = publication.receipt();
    validate_backend_publication_receipt_v1(producer, plan.attempt(), plan, upstream, receipt)
        .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::Receipt)?;
    let exact_finalized_hsaco = publication.snapshot().artifact().bytes();
    let measured: [u8; 32] = Sha256::digest(exact_finalized_hsaco).into();
    if measured != receipt.finalized_output_identity() {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::FinalizedDigestMismatch);
    }
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
            || descriptor.capabilities() != [CapabilityV1::AmdWave]
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
                || argument.ownership() != expected.kind.descriptor_ownership()
                || argument.access() != expected.kind.descriptor_access()
                || argument.alias() != expected.kind.descriptor_alias()
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
            capabilities: descriptor
                .capabilities()
                .iter()
                .copied()
                .map(manifest_capability)
                .collect::<Result<Vec<_>, _>>()?,
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
        attempt: plan.attempt(),
        outcome: publication.outcome(),
        raw_output_digest: *plan.linked_output().as_bytes(),
        finalized_output_digest: receipt.finalized_output_identity(),
        canonical_code_object_digest: *inspection.digest().as_bytes(),
        finalization_identity: *plan.finalization().as_bytes(),
        publication_identity: receipt.publication_identity(),
        upstream_evidence_identity: receipt.upstream_evidence_identity(),
        producer_receipt_identity: receipt.producer_identity(),
        compiler_commit: table.compiler_commit,
        descriptors,
    })
}

fn expected_components(field: FrozenFieldV1) -> Vec<(u32, u16, u16)> {
    match field.kind {
        FrozenFieldKindV1::F32 => vec![(field.offset, 4, 4)],
        FrozenFieldKindV1::SharedF32 | FrozenFieldKindV1::ReadWriteDisjointF32 => {
            vec![(field.offset, 8, 8), (field.offset + 8, 8, 8)]
        }
    }
}

fn validate_physical_profile(
    kernel: &InspectedKernel,
    fields: &[FrozenFieldV1],
) -> Result<(), WorkerV2ArtifactContainerAssemblyErrorV1> {
    let explicit_size = fields
        .iter()
        .map(|field| u64::from(field.offset) + u64::from(field.kind.size()))
        .max()
        .unwrap_or(0);
    if kernel.required_workgroup_size() != Some([256, 1, 1])
        || kernel.max_flat_workgroup_size() != 256
        || kernel.group_segment_fixed_size() != 0
        || kernel.private_segment_fixed_size() != 0
        || kernel.kernarg_segment_size() != explicit_size + 256
        || kernel.kernarg_segment_alignment() != 8
        || kernel.wavefront_size() != 64
        || kernel.uses_dynamic_stack()
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
            FrozenFieldKindV1::SharedF32 | FrozenFieldKindV1::ReadWriteDisjointF32 => vec![
                (
                    u64::from(field.offset),
                    8,
                    ExplicitValueKind::GlobalBuffer,
                    Some(ArgumentAddressSpace::Global),
                    Some(match field.kind {
                        FrozenFieldKindV1::SharedF32 => ArgumentAccess::ReadOnly,
                        FrozenFieldKindV1::ReadWriteDisjointF32 => ArgumentAccess::ReadWrite,
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
            || argument.access() != access
            || access.is_some_and(|expected| {
                argument
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
    reject_duplicate_kernel_fields(&table.kernels)?;
    for kernel in &table.kernels {
        validate_assembled_kernel_profile(kernel)?;
    }
    Ok(())
}

fn validate_assembled_kernel_profile(
    kernel: &DescriptorKernelAssemblyV1,
) -> Result<(), WorkerV2ArtifactContainerAssemblyErrorV1> {
    let expected = match kernel.entry_name.as_str() {
        "alpha" => ALPHA_FIELDS.as_slice(),
        "zeta" => ZETA_FIELDS.as_slice(),
        _ => return Err(WorkerV2ArtifactContainerAssemblyErrorV1::KernelSet),
    };
    if kernel.logical_name != kernel.entry_name
        || kernel.descriptor_symbol != format!("{}.kd", kernel.entry_name)
        || kernel.capabilities != [Capability::AmdWave]
        || kernel.fields.len() != expected.len()
    {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
            "assembled kernel identity, capabilities, or argument count differs from the frozen alpha/zeta profile",
        ));
    }
    for (field, expected) in kernel.fields.iter().zip(expected) {
        if field.name != expected.name
            || field.offset != expected.offset
            || field.kind != expected.kind.kind()
            || field.access != expected.kind.access()
            || field.mutability != expected.kind.mutability()
            || field.ownership != expected.kind.ownership()
            || field.alias != expected.kind.alias()
            || field.rust_type != expected.kind.rust_type_identity()
            || field.layout != expected.kind.layout_identity()
        {
            return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
                "assembled argument semantics differ from the frozen alpha/zeta profile",
            ));
        }
    }
    Ok(())
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
    let mut target_capabilities = table
        .kernels
        .iter()
        .flat_map(|kernel| kernel.capabilities.iter().copied())
        .collect::<Vec<_>>();
    target_capabilities.sort_unstable();
    target_capabilities.dedup();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text(&table.compiler_name)?, text(&table.compiler_release)?),
        ToolIdentity::new(text(&table.producer_name)?, text(&table.producer_version)?),
        fe2o3_artifacts::TargetIdentity::new(
            text(TARGET_TRIPLE)?,
            text(TARGET)?,
            PointerWidth::Bits64,
            Endianness::Little,
            target_capabilities,
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
    descriptors.sort_unstable_by_key(|descriptor| descriptor.kernel_id);
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
        descriptor.capabilities.clone(),
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

fn manifest_capability(
    capability: CapabilityV1,
) -> Result<Capability, WorkerV2ArtifactContainerAssemblyErrorV1> {
    match capability {
        CapabilityV1::AmdWave => Ok(Capability::AmdWave),
        _ => Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
            "kernel capability differs from the frozen alpha/zeta profile",
        )),
    }
}

fn text(value: &str) -> Result<IdentityText, WorkerV2ArtifactContainerAssemblyErrorV1> {
    IdentityText::new(value).map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)
}

fn name(value: &str) -> Result<Name, WorkerV2ArtifactContainerAssemblyErrorV1> {
    Name::new(value).map_err(WorkerV2ArtifactContainerAssemblyErrorV1::ArtifactModel)
}

fn descriptor_identity(domain: &[u8], descriptor: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((descriptor.len() as u64).to_le_bytes());
    digest.update(descriptor);
    digest.finalize().into()
}

#[cfg(test)]
#[path = "worker_v2_artifact_container_test_fixture.rs"]
mod test_fixture;

#[cfg(test)]
mod tests {
    use super::test_fixture::{ProfileMutation, alpha_zeta_fixture};
    use super::*;
    use fe2o3_artifact_transaction::{
        AtomicPublicationIdentityV1, BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1,
        FinalizationIdentityV1, FinalizedOutputIdentityV1, KernelSetIdentityV1,
        LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1, PinnedWorkerIdentityV1,
        TargetIdentityV1, ValidatedResponseIdentityV1, begin_build_attempt,
        publish_exact_hsaco_evidence_for_attempt_v1,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "cargo-fe2o3-artifact-container-{}-{}",
                std::process::id(),
                NEXT_TEST.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn producer(name: &str, source: &str) -> ProducerIdentity {
        ProducerIdentity::from_codegen(name, Some(Path::new(source))).unwrap()
    }

    fn begin(directory: &TestDirectory, producer: &ProducerIdentity, seed: u8) -> BuildAttempt {
        begin_build_attempt(
            &directory.0,
            producer,
            BuildInvocation::from_bytes([seed; 32]),
            BuildSession::from_bytes([seed.wrapping_add(1); 16]),
        )
        .unwrap()
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
            capabilities: vec![Capability::AmdWave],
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
            forward.manifest().target().capabilities(),
            &[Capability::AmdWave]
        );
        assert!(
            forward
                .manifest()
                .kernels()
                .iter()
                .all(|kernel| kernel.required_capabilities() == [Capability::AmdWave])
        );
        assert!(
            forward.manifest().kernels().iter().all(|kernel| kernel
                .abi()
                .fields()
                .last()
                .unwrap()
                .access()
                == Access::ReadWrite)
        );
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
    fn typed_publication_path_binds_producer_plan_outcome_and_snapshot_bytes() {
        let directory = TestDirectory::new();
        let publisher = producer("alpha_zeta", "/workspace/alpha_zeta.rs");
        let attempt = begin(&directory, &publisher, 0x31);
        let bytes = b"not an hsaco";
        let finalized: [u8; 32] = Sha256::digest(bytes).into();
        let plan = plan(attempt, finalized, 0x41);
        let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x51; 32]);
        let publication = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory.0,
            &publisher,
            attempt,
            plan,
            upstream,
            bytes,
        )
        .unwrap();

        assert!(matches!(
            prepare_worker_v2_artifact_container_v1(&publisher, plan, upstream, &publication),
            Err(WorkerV2ArtifactContainerAssemblyErrorV1::FinalizedHsaco(_))
        ));
        let substituted = producer("alpha_zeta", "/workspace/substituted.rs");
        assert!(matches!(
            prepare_worker_v2_artifact_container_v1(&substituted, plan, upstream, &publication),
            Err(WorkerV2ArtifactContainerAssemblyErrorV1::Receipt(
                BackendPublicationReceiptValidationErrorV1::ProducerIdentityMismatch
            ))
        ));
        assert_eq!(
            publication.outcome(),
            AttemptScopedHsacoPublicationOutcomeV1::Published
        );
    }

    #[test]
    fn finalized_alpha_zeta_publication_assembles_complete_bound_container() {
        let fixture = alpha_zeta_fixture(ProfileMutation::None);
        assert!(fixture.is_finalized);
        let directory = TestDirectory::new();
        let publisher = producer("alpha_zeta", "/workspace/alpha_zeta.rs");
        let attempt = begin(&directory, &publisher, 0x61);
        let finalized: [u8; 32] = Sha256::digest(&fixture.bytes).into();
        let plan = plan(attempt, finalized, 0x71);
        let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x81; 32]);
        let publication = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory.0,
            &publisher,
            attempt,
            plan,
            upstream,
            &fixture.bytes,
        )
        .unwrap();

        let prepared =
            prepare_worker_v2_artifact_container_v1(&publisher, plan, upstream, &publication)
                .unwrap();
        let manifest = prepared.container.manifest();
        assert_eq!(manifest.target().capabilities(), &[Capability::AmdWave]);
        assert_eq!(manifest.kernels().len(), 2);
        assert!(
            manifest
                .kernels()
                .iter()
                .all(|kernel| kernel.required_capabilities() == [Capability::AmdWave])
        );
        assert_eq!(
            manifest
                .kernels()
                .iter()
                .map(|kernel| kernel.name().as_str())
                .collect::<Vec<_>>(),
            ["alpha", "zeta"]
        );
        for kernel in manifest.kernels() {
            let output = kernel.abi().fields().last().unwrap();
            assert_eq!(output.name().as_str(), "output");
            assert_eq!(output.access(), Access::ReadWrite);
            assert_eq!(output.mutability(), Mutability::Mutable);
            assert_eq!(output.ownership(), ArgumentOwnership::UniqueBorrow);
            assert_eq!(output.alias_class(), AliasClass::Exclusive);
        }
        assert_eq!(prepared.attempt, attempt);
        assert_eq!(
            prepared.outcome,
            AttemptScopedHsacoPublicationOutcomeV1::Published
        );
        assert_eq!(prepared.raw_output_digest, *plan.linked_output().as_bytes());
        assert_eq!(prepared.finalized_output_digest, finalized);
        assert_eq!(
            prepared.canonical_code_object_digest,
            *inspect_finalized(&fixture.bytes)
                .unwrap()
                .digest()
                .as_bytes()
        );
        assert_eq!(
            prepared.finalization_identity,
            *plan.finalization().as_bytes()
        );
        assert_eq!(
            prepared.publication_identity,
            publication.receipt().publication_identity()
        );
        assert_eq!(prepared.upstream_evidence_identity, upstream.as_bytes());
        assert_eq!(
            prepared.producer_receipt_identity,
            publication.receipt().producer_identity()
        );
        assert_eq!(prepared.compiler_commit, [0x31; 20]);
        assert_eq!(prepared.descriptors.len(), 2);
        for (descriptor, name, symbol, id, source_identity, executable_identity) in [
            (
                &prepared.descriptors[0],
                "alpha",
                "alpha.kd",
                0x61,
                0x11,
                0x13,
            ),
            (
                &prepared.descriptors[1],
                "zeta",
                "zeta.kd",
                0x7a,
                0x21,
                0x23,
            ),
        ] {
            assert_eq!(descriptor.kernel_id.as_bytes(), &[id; 32]);
            assert_eq!(descriptor.logical_name, name);
            assert_eq!(descriptor.entry_name, name);
            assert_eq!(descriptor.descriptor_symbol, symbol);
            assert_eq!(descriptor.source_evidence_identity, [source_identity; 32]);
            assert_eq!(
                descriptor.executable_evidence_identity,
                [executable_identity; 32]
            );
        }
    }

    #[test]
    fn published_profile_mutations_fail_closed_at_the_public_assembly_boundary() {
        for (index, mutation) in [
            ProfileMutation::MissingCapability,
            ProfileMutation::WriteOnlyAccess,
            ProfileMutation::SharedOwnership,
            ProfileMutation::SharedAlias,
        ]
        .into_iter()
        .enumerate()
        {
            let fixture = alpha_zeta_fixture(mutation);
            let directory = TestDirectory::new();
            let seed = 0x91_u8.wrapping_add(index as u8 * 16);
            let publisher = producer("alpha_zeta", "/workspace/alpha_zeta_mutated.rs");
            let attempt = begin(&directory, &publisher, seed);
            let finalized: [u8; 32] = Sha256::digest(&fixture.bytes).into();
            let plan = plan(attempt, finalized, seed.wrapping_add(2));
            let upstream =
                UpstreamCodeObjectEvidenceIdentityV1::from_bytes([seed.wrapping_add(3); 32]);
            let publication = publish_exact_hsaco_evidence_for_attempt_v1(
                &directory.0,
                &publisher,
                attempt,
                plan,
                upstream,
                &fixture.bytes,
            )
            .unwrap();

            let error =
                prepare_worker_v2_artifact_container_v1(&publisher, plan, upstream, &publication)
                    .unwrap_err();
            if fixture.is_finalized {
                assert!(matches!(
                    error,
                    WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(_)
                ));
            } else {
                assert!(matches!(
                    error,
                    WorkerV2ArtifactContainerAssemblyErrorV1::FinalizedHsaco(_)
                ));
            }
        }
    }

    #[test]
    fn capability_access_ownership_and_alias_mutations_fail_closed() {
        let baseline = table(vec![kernel("alpha", 0x20), kernel("zeta", 0x10)]);

        let mut capability = baseline.clone();
        capability.kernels[0].capabilities.clear();
        let mut access = baseline.clone();
        access.kernels[0].fields[2].access = Access::WriteOnly;
        let mut ownership = baseline.clone();
        ownership.kernels[0].fields[2].ownership = ArgumentOwnership::SharedBorrow;
        let mut alias = baseline.clone();
        alias.kernels[0].fields[2].alias = AliasClass::SharedReadOnly;

        for mutation in [capability, access, ownership, alias] {
            assert!(matches!(
                validate_profile(&mutation),
                Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(_))
            ));
        }
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
    fn frozen_type_and_layout_identities_are_distinct_and_stable() {
        assert_ne!(
            FrozenFieldKindV1::F32.rust_type_identity(),
            FrozenFieldKindV1::SharedF32.rust_type_identity()
        );
        assert_ne!(
            FrozenFieldKindV1::SharedF32.rust_type_identity(),
            FrozenFieldKindV1::ReadWriteDisjointF32.rust_type_identity()
        );
        assert_ne!(
            FrozenFieldKindV1::F32.layout_identity(),
            FrozenFieldKindV1::SharedF32.layout_identity()
        );
    }
}
