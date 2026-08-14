//! Canonical, inert Worker V2 load-envelope assembly for finalized COV6 output.
//!
//! Cargo assembles this structural envelope only from independently supplied sealed evidence and
//! exact raw and finalized snapshots. The envelope carries an inert durable publication claim; it
//! never carries a process-local lease and grants no load or launch authority. No genuine
//! proof/compiler authenticator exists at this boundary; downstream admission must supply one.

use std::fmt;

use fe2o3_artifact_transaction::{
    AttemptScopedHsacoPublicationOutcomeV1, AttemptScopedHsacoPublicationResultV1,
    BackendPublicationReceiptValidationErrorV1, BuildAttempt, DurableLinkPublicationPlanV1,
    DurablePublishedHsacoClaimV1, ProducerIdentity, UpstreamCodeObjectEvidenceIdentityV1,
    validate_backend_publication_receipt_v1,
};
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    ArtifactContainerV1, BlockSize, BundleIndexV1, CallerClaimedPackageIdentityV1, Capability,
    CodeObjectFormat, CodeObjectIdentity, CodeObjectPayload, CompilerIdentity,
    ContainerValidationError, DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DeclaredRustLayoutIdentity,
    DeclaredRustTypeIdentity, DigestAlgorithm, DigestBytes, Dimensions, DirectLinkBindingSourceV1,
    DirectLinkBundleEvidenceV1, Endianness, IdentityText, KernelEntry, LaunchContract,
    ManifestClaimDerivedLinkPublicationScopeV1, ManifestClaimDirectLinkPublicationBridgeV1,
    ManifestV1, Mutability, Name, PointerWidth, ProofRecordV1, ScalarType, ToolIdentity,
    TypeIdentity, ValidationError,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion, ExplicitValueKind, InspectedKernel,
};
use fe2o3_hsaco_finalize::{FinalizationError, inspect_finalized};
use fe2o3_kernel_descriptor::{AccessMode, AliasSemantics, CapabilityV1, OwnershipSemantics};
use fe2o3_worker_v2_bundle::{
    DescriptorLineageV1, EnvelopeValidationError, ExactRawHsacoV1, WorkerV2EnvelopeInputsV1,
    WorkerV2LoadEnvelopeV1,
};
use sha2::{Digest, Sha256};

const TARGET: &str = "gfx942:xnack-";
const TARGET_TRIPLE: &str = "amdgcn-amd-amdhsa";
const ALPHA_ZETA_KERNELS: [&str; 2] = ["alpha", "zeta"];
const SCALAR_GEMM_V1_KERNEL: &str = "scalar_gemm_v1";
const SCALAR_GEMM_V1_KERNEL_ID: [u8; 32] = [
    0x78, 0x9a, 0xde, 0xdf, 0xdc, 0x3b, 0xe1, 0xfb, 0x60, 0x51, 0x8d, 0xd2, 0xc7, 0x46, 0x0c, 0x3e,
    0xf8, 0xe6, 0xb9, 0x00, 0x52, 0x7d, 0x1b, 0xcb, 0x22, 0x89, 0xba, 0xa1, 0xe0, 0x14, 0x69, 0x3e,
];
const SCALAR_GEMM_V1_COMPILER: &str = "rustc-codegen-fe2o3";
const SCALAR_GEMM_V1_PRODUCER: &str = "rustc-codegen-fe2o3-worker-v2";
const SCALAR_GEMM_V1_PRODUCER_VERSION: &str = "typed-general-gfx942-cov6-v1";
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

/// Inert result of assembling one profile-checked container from one finalized publication
/// snapshot.
///
/// A bounded subset of publication and descriptor lineage is retained because
/// `ArtifactContainerV1` has no slots for it. The complete plan, receipt, raw
/// bytes, bundle/proof evidence, and currentness lease are not retained.
#[derive(Debug)]
#[cfg_attr(not(test), allow(dead_code))]
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
    descriptors: Vec<WorkerV2DescriptorKernelLineageV1>,
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
    Bundle(fe2o3_artifacts::BundleValidationError),
    Envelope(EnvelopeValidationError),
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
                formatter.write_str("Worker V2 artifact assembly has no supported kernel count")
            }
            Self::KernelSet => {
                formatter.write_str("Worker V2 artifact assembly has no supported kernel profile")
            }
            Self::DuplicateKernelField(field) => {
                write!(formatter, "duplicate descriptor kernel {field}")
            }
            Self::DescriptorModel(reason) => write!(
                formatter,
                "descriptor cannot be represented in manifest V1: {reason}"
            ),
            Self::ArtifactModel(error) => error.fmt(formatter),
            Self::Container(error) => error.fmt(formatter),
            Self::Bundle(error) => error.fmt(formatter),
            Self::Envelope(error) => error.fmt(formatter),
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
            Self::Bundle(error) => Some(error),
            Self::Envelope(error) => Some(error),
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
    block_size: [u32; 3],
    max_grid: [u32; 3],
    max_flat_workgroup_size: u32,
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

struct CanonicalContainerAssemblyV1 {
    container: ArtifactContainerV1,
    canonical_code_object_digest: [u8; 32],
    compiler_commit: [u8; 20],
    descriptors: Vec<WorkerV2DescriptorKernelLineageV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArtifactAssemblyProfileV1 {
    AlphaZeta,
    ScalarGemmV1,
}

impl ArtifactAssemblyProfileV1 {
    const fn kernel_count(self) -> usize {
        match self {
            Self::AlphaZeta => 2,
            Self::ScalarGemmV1 => 1,
        }
    }

    fn fields(self, entry: &str) -> Option<&'static [FrozenFieldV1]> {
        match (self, entry) {
            (Self::AlphaZeta, "alpha") => Some(&ALPHA_FIELDS),
            (Self::AlphaZeta, "zeta") => Some(&ZETA_FIELDS),
            (Self::ScalarGemmV1, SCALAR_GEMM_V1_KERNEL) => Some(&SCALAR_GEMM_V1_FIELDS),
            _ => None,
        }
    }

    fn explicit_size(self, entry: &str) -> Option<u32> {
        match (self, entry) {
            (Self::AlphaZeta, "alpha") => Some(40),
            (Self::AlphaZeta, "zeta") => Some(56),
            (Self::ScalarGemmV1, SCALAR_GEMM_V1_KERNEL) => Some(64),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum FrozenFieldKindV1 {
    F32,
    U32,
    SharedF32,
    ReadWriteDisjointF32,
}

impl FrozenFieldKindV1 {
    const fn descriptor_kind_tag(self) -> u8 {
        match self {
            Self::F32 | Self::U32 => 1,
            Self::SharedF32 => 2,
            Self::ReadWriteDisjointF32 => 3,
        }
    }

    const fn size(self) -> u16 {
        match self {
            Self::F32 | Self::U32 => 4,
            Self::SharedF32 | Self::ReadWriteDisjointF32 => 16,
        }
    }

    const fn alignment(self) -> u16 {
        match self {
            Self::F32 | Self::U32 => 4,
            Self::SharedF32 | Self::ReadWriteDisjointF32 => 8,
        }
    }

    const fn kind(self) -> AbiKind {
        match self {
            Self::F32 => AbiKind::Scalar(ScalarType::F32),
            Self::U32 => AbiKind::Scalar(ScalarType::U32),
            Self::SharedF32 | Self::ReadWriteDisjointF32 => AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
        }
    }

    const fn access(self) -> Access {
        match self {
            Self::F32 | Self::U32 => Access::ByValue,
            Self::SharedF32 => Access::ReadOnly,
            Self::ReadWriteDisjointF32 => Access::ReadWrite,
        }
    }

    const fn mutability(self) -> Mutability {
        match self {
            Self::ReadWriteDisjointF32 => Mutability::Mutable,
            Self::F32 | Self::U32 | Self::SharedF32 => Mutability::Immutable,
        }
    }

    const fn ownership(self) -> ArgumentOwnership {
        match self {
            Self::F32 | Self::U32 => ArgumentOwnership::ByValue,
            Self::SharedF32 => ArgumentOwnership::SharedBorrow,
            Self::ReadWriteDisjointF32 => ArgumentOwnership::UniqueBorrow,
        }
    }

    const fn alias(self) -> AliasClass {
        match self {
            Self::F32 | Self::U32 => AliasClass::Value,
            Self::SharedF32 => AliasClass::SharedReadOnly,
            Self::ReadWriteDisjointF32 => AliasClass::Exclusive,
        }
    }

    const fn descriptor_ownership(self) -> OwnershipSemantics {
        match self {
            Self::F32 | Self::U32 => OwnershipSemantics::ByValue,
            Self::SharedF32 => OwnershipSemantics::SharedBorrow,
            Self::ReadWriteDisjointF32 => OwnershipSemantics::UniqueBorrow,
        }
    }

    const fn descriptor_access(self) -> AccessMode {
        match self {
            Self::F32 | Self::U32 => AccessMode::ByValue,
            Self::SharedF32 => AccessMode::ReadOnly,
            Self::ReadWriteDisjointF32 => AccessMode::ReadWrite,
        }
    }

    const fn descriptor_alias(self) -> AliasSemantics {
        match self {
            Self::F32 | Self::U32 => AliasSemantics::Value,
            Self::SharedF32 => AliasSemantics::SharedReadOnly,
            Self::ReadWriteDisjointF32 => AliasSemantics::Exclusive,
        }
    }

    fn rust_type_identity(self) -> [u8; 32] {
        descriptor_identity(
            RUST_TYPE_DOMAIN,
            &[self.descriptor_kind_tag(), self.scalar_tag(), 0, 0],
        )
    }

    fn layout_identity(self) -> [u8; 32] {
        let mut descriptor = Vec::with_capacity(12);
        descriptor.push(self.descriptor_kind_tag());
        descriptor.push(self.scalar_tag());
        descriptor.extend_from_slice(&self.size().to_le_bytes());
        descriptor.extend_from_slice(&self.alignment().to_le_bytes());
        let reference = !matches!(self, Self::F32 | Self::U32);
        descriptor.push(u8::from(reference) * 8);
        descriptor.push(u8::from(reference) * 8);
        descriptor.extend_from_slice(&0_u16.to_le_bytes());
        descriptor.extend_from_slice(&0_u16.to_le_bytes());
        descriptor_identity(DEVICE_LAYOUT_DOMAIN, &descriptor)
    }

    const fn scalar_tag(self) -> u8 {
        match self {
            Self::U32 => 6,
            Self::F32 | Self::SharedF32 | Self::ReadWriteDisjointF32 => 10,
        }
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

const SCALAR_GEMM_V1_FIELDS: [FrozenFieldV1; 6] = [
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
        name: "c",
        offset: 32,
        kind: FrozenFieldKindV1::ReadWriteDisjointF32,
    },
    FrozenFieldV1 {
        name: "m",
        offset: 48,
        kind: FrozenFieldKindV1::U32,
    },
    FrozenFieldV1 {
        name: "n",
        offset: 52,
        kind: FrozenFieldKindV1::U32,
    },
    FrozenFieldV1 {
        name: "k",
        offset: 56,
        kind: FrozenFieldKindV1::U32,
    },
];

/// Prepares one deterministic two-entry container from typed Worker V2 publication evidence.
///
/// This internal structural adapter deliberately has no public container or serialization
/// accessor. Production publication wraps its result in the complete durable load envelope. Host
/// admission must separately reacquire a fresh process-local publication lease; the lease is never
/// serialized in the envelope.
pub(crate) fn prepare_worker_v2_artifact_container_v1(
    producer: &ProducerIdentity,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    publication: &AttemptScopedHsacoPublicationResultV1,
) -> Result<PreparedWorkerV2ArtifactContainerV1, WorkerV2ArtifactContainerAssemblyErrorV1> {
    prepare_worker_v2_artifact_container_from_parts_v1(
        producer,
        plan,
        upstream,
        publication.receipt(),
        publication.outcome(),
        publication.snapshot().artifact().bytes(),
    )
}

fn prepare_worker_v2_artifact_container_from_parts_v1(
    producer: &ProducerIdentity,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    receipt: fe2o3_artifact_transaction::BackendPublicationReceiptV1,
    outcome: AttemptScopedHsacoPublicationOutcomeV1,
    exact_finalized_hsaco: &[u8],
) -> Result<PreparedWorkerV2ArtifactContainerV1, WorkerV2ArtifactContainerAssemblyErrorV1> {
    validate_backend_publication_receipt_v1(producer, plan.attempt(), plan, upstream, receipt)
        .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::Receipt)?;
    let measured: [u8; 32] = Sha256::digest(exact_finalized_hsaco).into();
    if measured != receipt.finalized_output_identity() {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::FinalizedDigestMismatch);
    }
    let assembled = assemble_canonical_container_v1(exact_finalized_hsaco)?;

    Ok(PreparedWorkerV2ArtifactContainerV1 {
        container: assembled.container,
        attempt: plan.attempt(),
        outcome,
        raw_output_digest: *plan.linked_output().as_bytes(),
        finalized_output_digest: receipt.finalized_output_identity(),
        canonical_code_object_digest: assembled.canonical_code_object_digest,
        finalization_identity: *plan.finalization().as_bytes(),
        publication_identity: receipt.publication_identity(),
        upstream_evidence_identity: receipt.upstream_evidence_identity(),
        producer_receipt_identity: receipt.producer_identity(),
        compiler_commit: assembled.compiler_commit,
        descriptors: assembled.descriptors,
    })
}

fn assemble_canonical_container_v1(
    exact_finalized_hsaco: &[u8],
) -> Result<CanonicalContainerAssemblyV1, WorkerV2ArtifactContainerAssemblyErrorV1> {
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
    if table.code_object_version() != fe2o3_kernel_descriptor::CodeObjectVersion::V6
        || table.device_target().to_string() != TARGET
    {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
            "descriptor target or code-object version differs from the artifact",
        ));
    }
    let profile = profile_for_names(
        table
            .kernels()
            .iter()
            .map(|kernel| kernel.entry_name().as_str()),
    )?;
    if hsaco.kernels().len() != profile.kernel_count() {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::KernelCount);
    }

    let mut kernels = Vec::with_capacity(profile.kernel_count());
    for descriptor in table.kernels() {
        let entry = descriptor.entry_name().as_str();
        let fields = profile
            .fields(entry)
            .ok_or(WorkerV2ArtifactContainerAssemblyErrorV1::KernelSet)?;
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
        let expected_explicit = profile
            .explicit_size(entry)
            .ok_or(WorkerV2ArtifactContainerAssemblyErrorV1::KernelSet)?;
        validate_physical_profile(metadata, fields, expected_explicit, profile)?;

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
        let block_size = match launch.block_size() {
            fe2o3_kernel_descriptor::BlockSizeV1::Exact(dimensions) => {
                [dimensions.x(), dimensions.y(), dimensions.z()]
            }
            _ => {
                return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
                    "descriptor block-size constraint differs from the frozen profile",
                ));
            }
        };
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
            block_size,
            max_grid: [max_grid.x(), max_grid.y(), max_grid.z()],
            max_flat_workgroup_size: launch.max_flat_workgroup_size(),
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
    validate_profile_for(&table, profile)?;
    let (container, descriptors) = build_container(&table, exact_finalized_hsaco.to_vec())?;

    Ok(CanonicalContainerAssemblyV1 {
        container,
        canonical_code_object_digest: *inspection.digest().as_bytes(),
        compiler_commit: table.compiler_commit,
        descriptors,
    })
}

/// Derives the required-mode durable publication contract from the independently supplied capsule.
///
/// The resulting plan remains inert. Its identities are structural claims from the capsule, not
/// authenticated compiler or proof evidence.
pub(crate) fn derive_required_worker_v2_publication_plan_v1(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    exact_finalized_hsaco: &[u8],
    inputs: &WorkerV2EnvelopeInputsV1,
) -> Result<
    (
        DurableLinkPublicationPlanV1,
        UpstreamCodeObjectEvidenceIdentityV1,
    ),
    WorkerV2ArtifactContainerAssemblyErrorV1,
> {
    let assembled = assemble_canonical_container_v1(exact_finalized_hsaco)?;
    inputs
        .validate_against_container(&assembled.container)
        .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::Envelope)?;
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&assembled.container))
        .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::Bundle)?;
    let binding = &inputs.direct_link_evidence().bindings()[0];
    let source =
        DirectLinkBindingSourceV1::new(&assembled.container, binding.expectation().clone());
    let validated = inputs
        .direct_link_evidence()
        .validate_against(
            &bundle,
            &[&assembled.container],
            std::slice::from_ref(&source),
        )
        .map_err(|error| WorkerV2ArtifactContainerAssemblyErrorV1::Envelope(error.into()))?;
    let scope = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        CallerClaimedPackageIdentityV1::new(
            fe2o3_artifact_transaction::producer_package_identity_v1(producer),
        ),
        &validated,
        0,
        &assembled.container,
    )
    .map_err(|_| {
        WorkerV2ArtifactContainerAssemblyErrorV1::Envelope(
            EnvelopeValidationError::PublicationBridge,
        )
    })?;
    let bridge = ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
        attempt, scope, &validated, 0,
    )
    .map_err(|_| {
        WorkerV2ArtifactContainerAssemblyErrorV1::Envelope(
            EnvelopeValidationError::PublicationBridge,
        )
    })?;
    let plan = DurableLinkPublicationPlanV1::new(
        attempt,
        bridge
            .non_authoritative_diagnostics()
            .descriptive_scope_claim(),
        bridge.request_identity(),
        bridge.worker_identity(),
        bridge.response_identity(),
        bridge.linked_output_identity(),
        bridge.finalization_identity(),
        bridge.finalized_output_identity(),
        bridge.publication_identity(),
    );
    let evidence = inputs
        .direct_link_evidence()
        .digest(DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM);
    Ok((
        plan,
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes(*evidence.bytes().as_bytes()),
    ))
}

#[doc(hidden)]
#[allow(dead_code)] // Shared verbatim with the standalone measured-input fixture.
pub(crate) fn canonical_worker_v2_container_for_fixture_v1(
    exact_finalized_hsaco: &[u8],
) -> Result<ArtifactContainerV1, WorkerV2ArtifactContainerAssemblyErrorV1> {
    Ok(assemble_canonical_container_v1(exact_finalized_hsaco)?.container)
}

/// Assembles the canonical load envelope from exact, independently supplied evidence.
///
/// Cargo deliberately does not derive `direct_link` or `proofs` from descriptor digests. The
/// upstream producer must provide those sealed records, and the envelope constructor checks their
/// complete structural join against the finalized container and durable publication claim.
#[allow(dead_code)] // Fresh-publication counterpart to the production restart assembly path.
pub(crate) fn assemble_worker_v2_load_envelope_v1(
    producer: &ProducerIdentity,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    publication: &AttemptScopedHsacoPublicationResultV1,
    direct_link: DirectLinkBundleEvidenceV1,
    proofs: Vec<ProofRecordV1>,
    raw: ExactRawHsacoV1,
) -> Result<WorkerV2LoadEnvelopeV1, WorkerV2ArtifactContainerAssemblyErrorV1> {
    let descriptor = DescriptorLineageV1::new(
        inspect_finalized(publication.snapshot().artifact().bytes())
            .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::FinalizedHsaco)?
            .descriptor_table()
            .clone(),
    );
    let prepared = prepare_worker_v2_artifact_container_v1(producer, plan, upstream, publication)?;
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&prepared.container))
        .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::Bundle)?;
    WorkerV2LoadEnvelopeV1::new(
        prepared.container,
        bundle,
        direct_link,
        descriptor,
        proofs,
        raw,
        publication.published_claim().clone(),
    )
    .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::Envelope)
}

/// Deterministically assembles the complete expected envelope from durable publication state and
/// one exact canonical input capsule. The capsule is never replaced with descriptor-derived or
/// otherwise synthesized proof evidence.
pub(crate) fn assemble_recovered_worker_v2_load_envelope_v1(
    producer: &ProducerIdentity,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    exact_finalized_hsaco: &[u8],
    claim: DurablePublishedHsacoClaimV1,
    inputs: &WorkerV2EnvelopeInputsV1,
) -> Result<WorkerV2LoadEnvelopeV1, WorkerV2ArtifactContainerAssemblyErrorV1> {
    if claim.plan() != plan || claim.upstream_evidence() != upstream {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::Envelope(
            EnvelopeValidationError::PublicationClaimMismatch(
                fe2o3_worker_v2_bundle::PublicationClaimFieldV1::Publication,
            ),
        ));
    }
    let descriptor = DescriptorLineageV1::new(
        inspect_finalized(exact_finalized_hsaco)
            .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::FinalizedHsaco)?
            .descriptor_table()
            .clone(),
    );
    let prepared = prepare_worker_v2_artifact_container_from_parts_v1(
        producer,
        plan,
        upstream,
        claim.receipt(),
        AttemptScopedHsacoPublicationOutcomeV1::RecoveredCommittedPublication,
        exact_finalized_hsaco,
    )?;
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&prepared.container))
        .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::Bundle)?;
    WorkerV2LoadEnvelopeV1::new(
        prepared.container,
        bundle,
        inputs.direct_link_evidence().clone(),
        descriptor,
        inputs.proof_records().to_vec(),
        inputs.raw_hsaco().clone(),
        claim,
    )
    .map_err(WorkerV2ArtifactContainerAssemblyErrorV1::Envelope)
}

fn expected_components(field: FrozenFieldV1) -> Vec<(u32, u16, u16)> {
    match field.kind {
        FrozenFieldKindV1::F32 | FrozenFieldKindV1::U32 => vec![(field.offset, 4, 4)],
        FrozenFieldKindV1::SharedF32 | FrozenFieldKindV1::ReadWriteDisjointF32 => {
            vec![(field.offset, 8, 8), (field.offset + 8, 8, 8)]
        }
    }
}

fn validate_physical_profile(
    kernel: &InspectedKernel,
    fields: &[FrozenFieldV1],
    explicit_size: u32,
    profile: ArtifactAssemblyProfileV1,
) -> Result<(), WorkerV2ArtifactContainerAssemblyErrorV1> {
    if kernel.required_workgroup_size() != Some([256, 1, 1])
        || kernel.max_flat_workgroup_size() != 256
        || kernel.group_segment_fixed_size() != 0
        || kernel.private_segment_fixed_size() != 0
        || kernel.kernarg_segment_size() != u64::from(explicit_size) + 256
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
            FrozenFieldKindV1::F32 | FrozenFieldKindV1::U32 => vec![(
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
                        FrozenFieldKindV1::F32 | FrozenFieldKindV1::U32 => unreachable!(),
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
            || (profile == ArtifactAssemblyProfileV1::ScalarGemmV1
                && access.is_some()
                && argument.actual_access() != access)
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
    let profile = profile_for_names(
        table
            .kernels
            .iter()
            .map(|kernel| kernel.entry_name.as_str()),
    )?;
    validate_profile_for(table, profile)
}

fn validate_profile_for(
    table: &DescriptorTableAssemblyV1,
    profile: ArtifactAssemblyProfileV1,
) -> Result<(), WorkerV2ArtifactContainerAssemblyErrorV1> {
    if table.kernels.len() != profile.kernel_count() {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::KernelCount);
    }
    if profile == ArtifactAssemblyProfileV1::ScalarGemmV1
        && (table.compiler_name != SCALAR_GEMM_V1_COMPILER
            || table.compiler_release != env!("CARGO_PKG_VERSION")
            || table.compiler_commit != [0; 20]
            || table.producer_name != SCALAR_GEMM_V1_PRODUCER
            || table.producer_version != SCALAR_GEMM_V1_PRODUCER_VERSION)
    {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
            "scalar GEMM compiler or producer identity differs from the frozen profile",
        ));
    }
    reject_duplicate_kernel_fields(&table.kernels)?;
    for kernel in &table.kernels {
        validate_assembled_kernel_profile(kernel, profile)?;
    }
    Ok(())
}

fn profile_for_names<'a>(
    names: impl IntoIterator<Item = &'a str>,
) -> Result<ArtifactAssemblyProfileV1, WorkerV2ArtifactContainerAssemblyErrorV1> {
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort_unstable();
    if names == ALPHA_ZETA_KERNELS {
        Ok(ArtifactAssemblyProfileV1::AlphaZeta)
    } else if names == [SCALAR_GEMM_V1_KERNEL] {
        Ok(ArtifactAssemblyProfileV1::ScalarGemmV1)
    } else if matches!(names.len(), 1 | 2) {
        Err(WorkerV2ArtifactContainerAssemblyErrorV1::KernelSet)
    } else {
        Err(WorkerV2ArtifactContainerAssemblyErrorV1::KernelCount)
    }
}

fn validate_assembled_kernel_profile(
    kernel: &DescriptorKernelAssemblyV1,
    profile: ArtifactAssemblyProfileV1,
) -> Result<(), WorkerV2ArtifactContainerAssemblyErrorV1> {
    let expected = profile
        .fields(&kernel.entry_name)
        .ok_or(WorkerV2ArtifactContainerAssemblyErrorV1::KernelSet)?;
    let expected_explicit = profile
        .explicit_size(&kernel.entry_name)
        .ok_or(WorkerV2ArtifactContainerAssemblyErrorV1::KernelSet)?;
    if profile == ArtifactAssemblyProfileV1::ScalarGemmV1
        && (table_identity_is_not_scalar_gemm(kernel)
            || kernel.source_digest == [0; 32]
            || kernel.source_evidence_identity == [0; 32]
            || kernel.executable_digest == [0; 32]
            || kernel.executable_evidence_identity == [0; 32])
    {
        return Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(
            "scalar GEMM identity or evidence lineage differs from the frozen profile",
        ));
    }
    if kernel.logical_name != kernel.entry_name
        || kernel.descriptor_symbol != format!("{}.kd", kernel.entry_name)
        || kernel.capabilities != [Capability::AmdWave]
        || kernel.explicit_size != expected_explicit
        || kernel.kernarg_size != expected_explicit + 256
        || kernel.kernarg_alignment != 8
        || kernel.rank != 1
        || kernel.block_size != [256, 1, 1]
        || kernel.max_grid != [u32::MAX, 1, 1]
        || kernel.max_flat_workgroup_size != 256
        || kernel.static_shared_memory_bytes != 0
        || kernel.max_dynamic_shared_memory_bytes != 0
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

fn table_identity_is_not_scalar_gemm(kernel: &DescriptorKernelAssemblyV1) -> bool {
    kernel.kernel_id != SCALAR_GEMM_V1_KERNEL_ID
        || kernel.logical_name != SCALAR_GEMM_V1_KERNEL
        || kernel.entry_name != SCALAR_GEMM_V1_KERNEL
        || kernel.descriptor_symbol != "scalar_gemm_v1.kd"
}

fn build_container(
    table: &DescriptorTableAssemblyV1,
    exact_finalized_hsaco: Vec<u8>,
) -> Result<
    (ArtifactContainerV1, Vec<WorkerV2DescriptorKernelLineageV1>),
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
        AbiKind::Scalar(ScalarType::F32 | ScalarType::U32) => 4,
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
    use super::test_fixture::{
        ProfileMutation, ScalarProfileMutation, alpha_zeta_fixture, scalar_gemm_v1_fixture,
    };
    use super::*;
    use crate::worker_v2_restart::{
        ResumeMarkerStateV1, WorkerV2EnvelopePublicationOutcomeV1, WorkerV2PublicationKindV1,
        WorkerV2ResumeStoreV1, envelope_name, recover_worker_v2_intent_v1,
        restart_admission_commitment_with_inputs_v1,
    };
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    use fe2o3_artifact_transaction::recover_published_hsaco_claim_for_attempt_v1;
    use fe2o3_artifact_transaction::{
        AtomicPublicationIdentityV1, BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1,
        FinalizationIdentityV1, FinalizedOutputIdentityV1, KernelSetIdentityV1,
        LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1, PinnedWorkerIdentityV1,
        TargetIdentityV1, ValidatedResponseIdentityV1, WorkerV2PublicationIntentIdentityV1,
        begin_build_attempt, finish_build_attempt, persist_worker_v2_publication_intent_v1,
        producer_package_identity_v1, publish_exact_hsaco_evidence_for_attempt_v1,
    };
    use fe2o3_artifacts::{
        CallerClaimedPackageIdentityV1, ConfigurationEntry, DirectLinkBindingExpectationV1,
        DirectLinkBindingSourceV1, DirectLinkFfiClosureIdentityV1,
        DirectLinkFinalizationIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
        DirectLinkLinkedOutputIdentityV1, DirectLinkRequestIdentityV1,
        DirectLinkResponseIdentityV1, DirectLinkToolchainConfigurationIdentityV1,
        DirectLinkToolchainExecutableIdentityV1, DirectLinkToolchainIdentityV1,
        DirectLinkTransformationIdentityV1, DirectLinkWorkerConfigurationIdentityV1,
        DirectLinkWorkerExecutableIdentityV1, DirectLinkWorkerIdentityV1,
        ManifestClaimDerivedLinkPublicationScopeV1, ManifestClaimDirectLinkPublicationBridgeV1,
        MeasuredToolIdentity, PayloadDigest, ProofArtifactIdentity, ProofExecutionIdentity,
        ProofOutcome, ProofTargetIdentity, SourceContractIdentity, TrustedItem,
        VerificationModelIdentity,
    };
    use fe2o3_worker_v2_bundle::MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(1);
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const ENVELOPE_FAULT_HELPER_DIR_ENV: &str = "FE2O3_TEST_ENVELOPE_FAULT_HELPER_DIR";
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const ENVELOPE_FAULT_HELPER_ATTEMPT_ENV: &str = "FE2O3_TEST_ENVELOPE_FAULT_HELPER_ATTEMPT";

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

    fn payload_digest(bytes: [u8; 32]) -> PayloadDigest {
        PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes(bytes))
    }

    fn identity_text(value: &str) -> IdentityText {
        IdentityText::new(value).unwrap()
    }

    fn proof_record(kernel: &KernelEntry) -> ProofRecordV1 {
        proof_record_variant(kernel, None)
    }

    #[derive(Clone, Copy)]
    enum ProofMutation {
        Configuration,
        Execution,
        Outcome,
        TrustedItem,
    }

    fn proof_record_variant(
        kernel: &KernelEntry,
        mutation: Option<ProofMutation>,
    ) -> ProofRecordV1 {
        let tagged = |seed| payload_digest([seed; 32]);
        ProofRecordV1::new(
            ProofTargetIdentity::new(
                ProofArtifactIdentity::new(
                    payload_digest(*kernel.kernel_id().as_bytes()),
                    tagged(0x41),
                    payload_digest(*kernel.source_digest().as_bytes()),
                    tagged(0x42),
                    payload_digest(*kernel.executable_digest().as_bytes()),
                    tagged(0x43),
                    tagged(0x44),
                    tagged(0x45),
                ),
                SourceContractIdentity::new(
                    tagged(0x51),
                    tagged(0x52),
                    tagged(0x53),
                    tagged(0x54),
                    tagged(0x55),
                ),
            ),
            if matches!(mutation, Some(ProofMutation::Configuration)) {
                vec![ConfigurationEntry::new(
                    Name::new("substituted-config").unwrap(),
                    identity_text("enabled"),
                )]
            } else {
                vec![]
            },
            ProofExecutionIdentity::new(
                VerificationModelIdentity::new(
                    identity_text("test-model"),
                    tagged(if matches!(mutation, Some(ProofMutation::Execution)) {
                        0x69
                    } else {
                        0x61
                    }),
                ),
                MeasuredToolIdentity::new(
                    identity_text("test-verifier"),
                    identity_text("1"),
                    tagged(0x62),
                    tagged(0x63),
                ),
                MeasuredToolIdentity::new(
                    identity_text("test-solver"),
                    identity_text("1"),
                    tagged(0x64),
                    tagged(0x65),
                ),
                MeasuredToolIdentity::new(
                    identity_text("test-recorder"),
                    identity_text("1"),
                    tagged(0x66),
                    tagged(0x67),
                ),
                tagged(0x68),
            ),
            if matches!(mutation, Some(ProofMutation::Outcome)) {
                ProofOutcome::TimedOut
            } else {
                ProofOutcome::Failed
            },
            vec![],
            if matches!(mutation, Some(ProofMutation::TrustedItem)) {
                vec![TrustedItem::new(
                    Name::new("substituted-trust").unwrap(),
                    tagged(0x6a),
                )]
            } else {
                vec![]
            },
        )
        .unwrap()
    }

    fn substitute_same_receipt_proof(
        envelope: &WorkerV2LoadEnvelopeV1,
        mutation: ProofMutation,
    ) -> WorkerV2LoadEnvelopeV1 {
        let mut proofs = envelope.proof_records().to_vec();
        proofs[0] = proof_record_variant(
            &envelope.container().manifest().kernels()[0],
            Some(mutation),
        );
        WorkerV2LoadEnvelopeV1::new(
            ArtifactContainerV1::from_bytes(&envelope.container().to_bytes()).unwrap(),
            envelope.bundle_index().clone(),
            envelope.direct_link_evidence().clone(),
            envelope.descriptor_lineage().clone(),
            proofs,
            envelope.raw_hsaco().clone(),
            envelope.published_claim().clone(),
        )
        .unwrap()
    }

    fn canonical_envelope_fixture(
        directory: &TestDirectory,
    ) -> (
        ProducerIdentity,
        WorkerV2LoadEnvelopeV1,
        fe2o3_artifact_transaction::BackendPublicationReceiptV1,
    ) {
        canonical_envelope_fixture_for(directory, "alpha_zeta", "/workspace/envelope.rs")
    }

    fn canonical_envelope_fixture_for(
        directory: &TestDirectory,
        producer_name: &str,
        source: &str,
    ) -> (
        ProducerIdentity,
        WorkerV2LoadEnvelopeV1,
        fe2o3_artifact_transaction::BackendPublicationReceiptV1,
    ) {
        let fixture = alpha_zeta_fixture(ProfileMutation::None);
        canonical_envelope_fixture_for_bytes(directory, producer_name, source, fixture.bytes)
    }

    fn canonical_envelope_fixture_for_bytes(
        directory: &TestDirectory,
        producer_name: &str,
        source: &str,
        fixture_bytes: Vec<u8>,
    ) -> (
        ProducerIdentity,
        WorkerV2LoadEnvelopeV1,
        fe2o3_artifact_transaction::BackendPublicationReceiptV1,
    ) {
        let publisher = producer(producer_name, source);
        let first_attempt = begin(directory, &publisher, 0x11);
        let finalized: [u8; 32] = Sha256::digest(&fixture_bytes).into();
        let first_plan = plan(first_attempt, finalized, 0x21);
        let first_upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x31; 32]);
        let first_publication = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory.0,
            &publisher,
            first_attempt,
            first_plan,
            first_upstream,
            &fixture_bytes,
        )
        .unwrap();
        let container = prepare_worker_v2_artifact_container_v1(
            &publisher,
            first_plan,
            first_upstream,
            &first_publication,
        )
        .unwrap()
        .container;
        let stale_receipt = first_publication.receipt();
        drop(first_publication);
        finish_build_attempt(&directory.0, &publisher, first_attempt).unwrap();

        let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
        let identity = DigestAlgorithm::Sha256.calculate(&fixture_bytes);
        let tagged = |seed| payload_digest([seed; 32]);
        let expectation = DirectLinkBindingExpectationV1::new(
            DirectLinkRequestIdentityV1::new(tagged(0x71)),
            DirectLinkWorkerIdentityV1::new(
                identity_text("test-worker"),
                identity_text("1"),
                DirectLinkWorkerExecutableIdentityV1::new(tagged(0x72)),
                DirectLinkWorkerConfigurationIdentityV1::new(tagged(0x73)),
            ),
            DirectLinkToolchainIdentityV1::new(
                identity_text("test-llvm"),
                identity_text("1"),
                DirectLinkToolchainExecutableIdentityV1::new(tagged(0x74)),
                DirectLinkToolchainConfigurationIdentityV1::new(tagged(0x75)),
            ),
            DirectLinkResponseIdentityV1::new(tagged(0x76)),
            DirectLinkTransformationIdentityV1::new(
                DirectLinkLinkedOutputIdentityV1::new(identity),
                DirectLinkFinalizationIdentityV1::new(tagged(0x77)),
                DirectLinkFinalizedPayloadIdentityV1::new(identity),
            ),
            DirectLinkFfiClosureIdentityV1::new(tagged(0x78)),
        );
        let direct_link = DirectLinkBundleEvidenceV1::bind(
            &bundle,
            &[&container],
            &[DirectLinkBindingSourceV1::new(
                &container,
                expectation.clone(),
            )],
        )
        .unwrap();
        let validated = direct_link
            .validate_against(
                &bundle,
                &[&container],
                &[DirectLinkBindingSourceV1::new(&container, expectation)],
            )
            .unwrap();

        let attempt = begin(directory, &publisher, 0x41);
        let scope = ManifestClaimDerivedLinkPublicationScopeV1::derive(
            CallerClaimedPackageIdentityV1::new(producer_package_identity_v1(&publisher)),
            &validated,
            0,
            &container,
        )
        .unwrap();
        let bridge = ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt, scope, &validated, 0,
        )
        .unwrap();
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            bridge
                .non_authoritative_diagnostics()
                .descriptive_scope_claim(),
            bridge.request_identity(),
            bridge.worker_identity(),
            bridge.response_identity(),
            bridge.linked_output_identity(),
            bridge.finalization_identity(),
            bridge.finalized_output_identity(),
            bridge.publication_identity(),
        );
        let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes(
            Sha256::digest(direct_link.to_bytes()).into(),
        );
        persist_worker_v2_publication_intent_v1(
            &directory.0,
            &publisher,
            attempt,
            plan,
            upstream,
            &fixture_bytes,
        )
        .unwrap();
        let publication = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory.0,
            &publisher,
            attempt,
            plan,
            upstream,
            &fixture_bytes,
        )
        .unwrap();
        let proofs = container
            .manifest()
            .kernels()
            .iter()
            .map(proof_record)
            .collect();
        let envelope = assemble_worker_v2_load_envelope_v1(
            &publisher,
            plan,
            upstream,
            &publication,
            direct_link,
            proofs,
            ExactRawHsacoV1::from_bytes(fixture_bytes).unwrap(),
        )
        .unwrap();
        assert_eq!(envelope.published_claim().receipt(), publication.receipt());
        (publisher, envelope, stale_receipt)
    }

    fn stage_required_ready(
        store: &WorkerV2ResumeStoreV1,
        publisher: &ProducerIdentity,
        envelope: &WorkerV2LoadEnvelopeV1,
    ) -> (
        WorkerV2PublicationKindV1,
        BuildAttempt,
        fe2o3_artifact_transaction::BackendPublicationReceiptV1,
        WorkerV2PublicationIntentIdentityV1,
        ResumeMarkerStateV1,
    ) {
        let claim = envelope.published_claim();
        let publication = WorkerV2PublicationKindV1::FinalizedEnvelopeRequired;
        let inputs = WorkerV2EnvelopeInputsV1::new(
            envelope.direct_link_evidence().clone(),
            envelope.proof_records().to_vec(),
            envelope.raw_hsaco().clone(),
        )
        .unwrap();
        let attempt = claim.plan().attempt();
        store.persist_envelope_inputs(attempt, &inputs).unwrap();
        let admission = restart_admission_commitment_with_inputs_v1(
            publication,
            claim.plan(),
            claim.upstream_evidence(),
            envelope.finalized_payload(),
            Some(inputs.identity()),
        );
        let receipt = claim.receipt();
        store
            .persist_pending_with_envelope_inputs(
                publication,
                attempt,
                admission,
                Some(inputs.identity()),
            )
            .unwrap();
        let pending = store.load().unwrap().unwrap();
        let intent = recover_worker_v2_intent_v1(store, publisher, pending)
            .unwrap()
            .record()
            .identity();
        let ready = store.load().unwrap().unwrap();
        (publication, attempt, receipt, intent, ready)
    }

    fn envelope_inputs_path(directory: &TestDirectory) -> PathBuf {
        fs::read_dir(&directory.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                let name = path.file_name().unwrap().to_string_lossy();
                name.starts_with(".fe2o3-worker-v2-envelope-inputs-v1-")
                    && name.ends_with(".capsule")
            })
            .unwrap()
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn envelope_input_residue(directory: &TestDirectory) -> Vec<PathBuf> {
        fs::read_dir(&directory.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".fe2o3-worker-v2-envelope-inputs-v1-")
            })
            .collect()
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn envelope_publication_temp_residue(directory: &TestDirectory) -> Vec<PathBuf> {
        fs::read_dir(&directory.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                let name = path.file_name().unwrap().to_string_lossy();
                name.starts_with(".fe2o3-worker-v2-load-envelope-v1-")
                    && name.contains(".envelope.tmp-")
            })
            .collect()
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
            block_size: [256, 1, 1],
            max_grid: [u32::MAX, 1, 1],
            max_flat_workgroup_size: 256,
            static_shared_memory_bytes: 0,
            max_dynamic_shared_memory_bytes: 0,
            fields,
        }
    }

    fn scalar_gemm_kernel() -> DescriptorKernelAssemblyV1 {
        DescriptorKernelAssemblyV1 {
            kernel_id: SCALAR_GEMM_V1_KERNEL_ID,
            logical_name: SCALAR_GEMM_V1_KERNEL.to_owned(),
            entry_name: SCALAR_GEMM_V1_KERNEL.to_owned(),
            descriptor_symbol: "scalar_gemm_v1.kd".to_owned(),
            source_digest: [0x91; 32],
            source_evidence_identity: [0x92; 32],
            executable_digest: [0x93; 32],
            executable_evidence_identity: [0x94; 32],
            capabilities: vec![Capability::AmdWave],
            explicit_size: 64,
            kernarg_size: 320,
            kernarg_alignment: 8,
            rank: 1,
            block_size: [256, 1, 1],
            max_grid: [u32::MAX, 1, 1],
            max_flat_workgroup_size: 256,
            static_shared_memory_bytes: 0,
            max_dynamic_shared_memory_bytes: 0,
            fields: SCALAR_GEMM_V1_FIELDS.iter().copied().map(field).collect(),
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

    fn scalar_gemm_table() -> DescriptorTableAssemblyV1 {
        DescriptorTableAssemblyV1 {
            compiler_name: SCALAR_GEMM_V1_COMPILER.to_owned(),
            compiler_release: env!("CARGO_PKG_VERSION").to_owned(),
            compiler_commit: [0; 20],
            producer_name: SCALAR_GEMM_V1_PRODUCER.to_owned(),
            producer_version: SCALAR_GEMM_V1_PRODUCER_VERSION.to_owned(),
            kernels: vec![scalar_gemm_kernel()],
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
    fn finalized_scalar_gemm_publication_assembles_one_kernel_container_and_envelope() {
        let fixture = scalar_gemm_v1_fixture(ScalarProfileMutation::None);
        assert!(fixture.is_finalized);
        let directory = TestDirectory::new();
        let (_publisher, envelope, stale_receipt) = canonical_envelope_fixture_for_bytes(
            &directory,
            "scalar_gemm_v1",
            "/workspace/scalar_gemm_v1.rs",
            fixture.bytes.clone(),
        );
        let manifest = envelope.container().manifest();
        assert_eq!(manifest.target().triple().as_str(), TARGET_TRIPLE);
        assert_eq!(manifest.target().architecture().as_str(), TARGET);
        assert_eq!(manifest.target().capabilities(), &[Capability::AmdWave]);
        let [kernel] = manifest.kernels() else {
            panic!("scalar GEMM profile must assemble exactly one kernel");
        };
        assert_eq!(kernel.kernel_id().as_bytes(), &SCALAR_GEMM_V1_KERNEL_ID);
        assert_eq!(kernel.name().as_str(), SCALAR_GEMM_V1_KERNEL);
        assert_eq!(kernel.symbol().as_str(), SCALAR_GEMM_V1_KERNEL);
        assert_eq!(kernel.abi().size(), 64);
        assert_eq!(kernel.abi().alignment(), 8);
        assert_eq!(kernel.abi().fields().len(), 6);
        assert_eq!(
            kernel
                .abi()
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c", "m", "n", "k"]
        );
        assert_eq!(kernel.abi().fields()[2].access(), Access::ReadWrite);
        assert_eq!(
            kernel.abi().fields()[2].alias_class(),
            AliasClass::Exclusive
        );
        assert_eq!(kernel.launch().rank(), 1);
        assert_eq!(
            kernel.launch().block_size(),
            BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap())
        );
        assert_eq!(
            kernel.launch().max_grid(),
            Dimensions::new(u32::MAX, 1, 1).unwrap()
        );
        assert_eq!(envelope.finalized_payload(), fixture.bytes);
        let claim = envelope.published_claim();
        assert_eq!(
            DurablePublishedHsacoClaimV1::decode_canonical(&claim.encode_canonical().unwrap())
                .unwrap(),
            *claim
        );
        assert_ne!(envelope.published_claim().receipt(), stale_receipt);
        assert_eq!(
            claim.receipt().publication_identity(),
            *claim.plan().publication().as_bytes()
        );
        assert_eq!(
            claim.receipt().finalized_output_identity(),
            *claim.plan().finalized_output().as_bytes()
        );
        assert_eq!(
            claim.receipt().upstream_evidence_identity(),
            claim.upstream_evidence().as_bytes()
        );
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
    fn published_scalar_gemm_physical_profile_mutations_fail_closed() {
        for (index, mutation) in [
            ScalarProfileMutation::PhysicalKernargSize,
            ScalarProfileMutation::PhysicalOutputAccess,
            ScalarProfileMutation::PhysicalWorkgroup,
            ScalarProfileMutation::HsacoTarget,
        ]
        .into_iter()
        .enumerate()
        {
            let fixture = scalar_gemm_v1_fixture(mutation);
            let directory = TestDirectory::new();
            let seed = 0xb1_u8.wrapping_add(index as u8 * 8);
            let publisher = producer("scalar_gemm_v1", "/workspace/scalar_gemm_v1_mutated.rs");
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
            assert!(
                matches!(
                    error,
                    WorkerV2ArtifactContainerAssemblyErrorV1::FinalizedHsaco(_)
                        | WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(_)
                        | WorkerV2ArtifactContainerAssemblyErrorV1::Target
                ),
                "scalar physical mutation {mutation:?} unexpectedly produced {error:?}; finalized={}",
                fixture.is_finalized
            );
        }
    }

    #[test]
    fn scalar_gemm_descriptor_abi_launch_and_lineage_mutations_fail_closed() {
        let baseline = scalar_gemm_table();
        validate_profile(&baseline).unwrap();
        let mut mutations = Vec::new();

        macro_rules! mutate {
            ($label:literal, $body:expr) => {{
                let mut table = baseline.clone();
                $body(&mut table);
                mutations.push(($label, table));
            }};
        }

        mutate!("compiler name", |table: &mut DescriptorTableAssemblyV1| {
            table.compiler_name.push_str("-other")
        });
        mutate!(
            "compiler release",
            |table: &mut DescriptorTableAssemblyV1| table.compiler_release.push_str("-other")
        );
        mutate!(
            "compiler commit",
            |table: &mut DescriptorTableAssemblyV1| table.compiler_commit[0] = 1
        );
        mutate!("producer name", |table: &mut DescriptorTableAssemblyV1| {
            table.producer_name.push_str("-other")
        });
        mutate!(
            "producer version",
            |table: &mut DescriptorTableAssemblyV1| table.producer_version.push_str("-other")
        );
        mutate!("kernel ID", |table: &mut DescriptorTableAssemblyV1| {
            table.kernels[0].kernel_id[0] ^= 1
        });
        mutate!("logical name", |table: &mut DescriptorTableAssemblyV1| {
            table.kernels[0].logical_name.push_str("_other")
        });
        mutate!(
            "descriptor symbol",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0]
                .descriptor_symbol
                .push_str("_other")
        );
        mutate!("source digest", |table: &mut DescriptorTableAssemblyV1| {
            table.kernels[0].source_digest = [0; 32]
        });
        mutate!(
            "source identity",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].source_evidence_identity =
                [0; 32]
        );
        mutate!(
            "executable digest",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].executable_digest = [0; 32]
        );
        mutate!(
            "executable identity",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].executable_evidence_identity =
                [0; 32]
        );
        mutate!("capability", |table: &mut DescriptorTableAssemblyV1| table
            .kernels[0]
            .capabilities
            .clear());
        mutate!("explicit size", |table: &mut DescriptorTableAssemblyV1| {
            table.kernels[0].explicit_size = 60
        });
        mutate!(
            "total kernarg size",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].kernarg_size = 316
        );
        mutate!(
            "kernarg alignment",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].kernarg_alignment = 16
        );
        mutate!("launch rank", |table: &mut DescriptorTableAssemblyV1| {
            table.kernels[0].rank = 2
        });
        mutate!(
            "launch block size",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].block_size = [128, 1, 1]
        );
        mutate!("launch grid", |table: &mut DescriptorTableAssemblyV1| {
            table.kernels[0].max_grid = [u32::MAX, 2, 1]
        });
        mutate!(
            "max flat workgroup size",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].max_flat_workgroup_size = 128
        );
        mutate!(
            "static shared memory",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].static_shared_memory_bytes = 4
        );
        mutate!(
            "dynamic shared memory",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0]
                .max_dynamic_shared_memory_bytes = 4
        );
        mutate!("argument name", |table: &mut DescriptorTableAssemblyV1| {
            table.kernels[0].fields[2].name = "output".to_owned()
        });
        mutate!(
            "argument offset",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].fields[5].offset = 60
        );
        mutate!("argument kind", |table: &mut DescriptorTableAssemblyV1| {
            table.kernels[0].fields[3].kind = AbiKind::Scalar(ScalarType::F32)
        });
        mutate!(
            "argument access",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].fields[2].access =
                Access::WriteOnly
        );
        mutate!(
            "argument mutability",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].fields[2].mutability =
                Mutability::Immutable
        );
        mutate!(
            "argument ownership",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].fields[2].ownership =
                ArgumentOwnership::SharedBorrow
        );
        mutate!("argument alias", |table: &mut DescriptorTableAssemblyV1| {
            table.kernels[0].fields[2].alias = AliasClass::SharedReadOnly
        });
        mutate!(
            "rust type identity",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].fields[3].rust_type[0] ^= 1
        );
        mutate!(
            "layout identity",
            |table: &mut DescriptorTableAssemblyV1| table.kernels[0].fields[3].layout[0] ^= 1
        );

        for (label, mutation) in mutations {
            assert!(
                matches!(
                    validate_profile(&mutation),
                    Err(WorkerV2ArtifactContainerAssemblyErrorV1::DescriptorModel(_))
                        | Err(WorkerV2ArtifactContainerAssemblyErrorV1::KernelSet)
                ),
                "scalar descriptor mutation `{label}` was accepted"
            );
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

    #[test]
    fn canonical_envelope_store_is_create_new_recoverable_and_inert() {
        let directory = TestDirectory::new();
        let (publisher, envelope, stale_receipt) = canonical_envelope_fixture(&directory);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();

        assert_eq!(
            store.publish_load_envelope(&envelope).unwrap(),
            WorkerV2EnvelopePublicationOutcomeV1::Published
        );
        assert_eq!(
            store.publish_load_envelope(&envelope).unwrap(),
            WorkerV2EnvelopePublicationOutcomeV1::AlreadyPublished
        );
        let recovered = store
            .recover_load_envelope(envelope.published_claim().receipt())
            .unwrap();
        assert_eq!(recovered.to_bytes(), envelope.to_bytes());
        assert!(!recovered.grants_currentness_authority());
        assert!(!recovered.grants_load_authority());
        assert!(!recovered.grants_launch_authority());
        assert_ne!(
            stale_receipt.publication_identity(),
            recovered.published_claim().receipt().publication_identity()
        );
        assert!(store.recover_load_envelope(stale_receipt).is_err());
    }

    #[test]
    fn malformed_mutated_and_truncated_envelopes_are_never_replaced() {
        for case in ["mutated", "truncated"] {
            let directory = TestDirectory::new();
            let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
            let receipt = envelope.published_claim().receipt();
            let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
            store.publish_load_envelope(&envelope).unwrap();
            drop(store);

            let path = directory
                .0
                .join(envelope_name(receipt.publication_identity()));
            let mut bytes = fs::read(&path).unwrap();
            match case {
                "mutated" => bytes[0] ^= 1,
                "truncated" => {
                    bytes.truncate(bytes.len() - 1);
                }
                _ => unreachable!(),
            }
            fs::write(&path, &bytes).unwrap();

            let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
            assert!(store.recover_load_envelope(receipt).is_err(), "{case}");
            assert!(store.publish_load_envelope(&envelope).is_err(), "{case}");
            assert_eq!(fs::read(path).unwrap(), bytes, "{case}");
        }
    }

    #[test]
    fn envelope_store_rejects_cross_producer_publication_and_recovery() {
        let directory = TestDirectory::new();
        let (publisher, _, _) = canonical_envelope_fixture(&directory);
        let (other_publisher, other_envelope, _) = canonical_envelope_fixture_for(
            &directory,
            "other_alpha_zeta",
            "/workspace/other-envelope.rs",
        );
        let receipt = other_envelope.published_claim().receipt();

        let other_store = WorkerV2ResumeStoreV1::open(&directory.0, &other_publisher).unwrap();
        other_store.publish_load_envelope(&other_envelope).unwrap();
        drop(other_store);

        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        assert!(store.publish_load_envelope(&other_envelope).is_err());
        assert!(store.recover_load_envelope(receipt).is_err());
    }

    #[test]
    fn legacy_unowned_envelope_temp_is_conservatively_ignored() {
        let directory = TestDirectory::new();
        let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
        let stale_temp = directory
            .0
            .join(".fe2o3-worker-v2-load-envelope-v1-stale.envelope.tmp-1-1");
        fs::write(&stale_temp, envelope.to_bytes()).unwrap();
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();

        assert_eq!(
            store.publish_load_envelope(&envelope).unwrap(),
            WorkerV2EnvelopePublicationOutcomeV1::Published
        );
        assert!(stale_temp.exists(), "legacy temp has no package ownership");
        assert_eq!(
            store
                .recover_load_envelope(envelope.published_claim().receipt())
                .unwrap()
                .to_bytes(),
            envelope.to_bytes()
        );
    }

    #[test]
    fn required_completion_publishes_envelope_before_completed_marker() {
        let directory = TestDirectory::new();
        let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        let (publication, attempt, receipt, intent, _) =
            stage_required_ready(&store, &publisher, &envelope);
        assert!(
            store
                .persist_completed(publication, attempt, intent, receipt)
                .is_err()
        );
        assert!(
            store
                .persist_completed(
                    WorkerV2PublicationKindV1::Finalized,
                    attempt,
                    intent,
                    receipt,
                )
                .is_err(),
            "required Ready state must not downgrade to ordinary completion"
        );

        let completed = store
            .persist_envelope_and_completed(publication, attempt, intent, receipt, &envelope)
            .unwrap();
        assert_eq!(completed.envelope(), envelope.identity().as_bytes());
        assert_ne!(completed.envelope_inputs(), [0; 32]);
        assert_eq!(store.load().unwrap(), Some(completed));
        assert_eq!(
            store.recover_load_envelope(receipt).unwrap().to_bytes(),
            envelope.to_bytes()
        );
    }

    #[test]
    fn required_publication_plan_is_derived_from_the_exact_supplied_capsule() {
        let directory = TestDirectory::new();
        let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
        let claim = envelope.published_claim();
        let inputs = WorkerV2EnvelopeInputsV1::new(
            envelope.direct_link_evidence().clone(),
            envelope.proof_records().to_vec(),
            envelope.raw_hsaco().clone(),
        )
        .unwrap();
        let (plan, upstream) = derive_required_worker_v2_publication_plan_v1(
            &publisher,
            claim.plan().attempt(),
            envelope.finalized_payload(),
            &inputs,
        )
        .unwrap();
        assert_eq!(plan, claim.plan());
        assert_eq!(upstream, claim.upstream_evidence());
    }

    #[test]
    fn required_completed_recovery_rejects_a_missing_envelope() {
        let directory = TestDirectory::new();
        let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        let (publication, attempt, receipt, intent, _) =
            stage_required_ready(&store, &publisher, &envelope);
        let completed = store
            .persist_envelope_and_completed(publication, attempt, intent, receipt, &envelope)
            .unwrap();
        let path = fs::read_dir(&directory.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                let name = path.file_name().unwrap().to_string_lossy();
                name.starts_with(".fe2o3-worker-v2-load-envelope-v1-")
                    && name.ends_with(".envelope")
            })
            .unwrap();
        fs::remove_file(path).unwrap();

        assert!(store.recover_load_envelope(receipt).is_err());
        assert_eq!(store.load().unwrap(), Some(completed));
        assert!(completed.publication().requires_envelope());
    }

    #[test]
    fn required_ready_restart_recovers_a_durable_envelope() {
        let directory = TestDirectory::new();
        let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        let (publication, attempt, receipt, intent, ready) =
            stage_required_ready(&store, &publisher, &envelope);

        store.publish_load_envelope(&envelope).unwrap();
        assert_eq!(store.load().unwrap(), Some(ready));
        drop(store);

        let restarted = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        let completed = restarted
            .recover_envelope_and_completed(publication, attempt, intent, receipt)
            .unwrap();
        assert!(matches!(completed, ResumeMarkerStateV1::Completed { .. }));
        assert_eq!(restarted.load().unwrap(), Some(completed));
        assert_eq!(
            restarted.recover_load_envelope(receipt).unwrap().to_bytes(),
            envelope.to_bytes()
        );
    }

    #[test]
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn required_ready_restart_after_envelope_published_fault_completes() {
        let directory = TestDirectory::new();
        let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        let (publication, attempt, receipt, intent, ready) =
            stage_required_ready(&store, &publisher, &envelope);
        fs::write(
            directory.0.join("fault-envelope-input"),
            envelope.to_bytes(),
        )
        .unwrap();
        drop(store);

        let interrupted = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("worker_v2_artifact_container::tests::required_envelope_publication_fault_helper")
            .env(ENVELOPE_FAULT_HELPER_DIR_ENV, &directory.0)
            .env("FE2O3_TEST_WORKER_V2_FAULT_POINT_V1", "envelope-published")
            .status()
            .unwrap();
        assert_eq!(interrupted.code(), Some(86));

        let restarted = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        assert_eq!(restarted.load().unwrap(), Some(ready));
        let completed = restarted
            .recover_envelope_and_completed(publication, attempt, intent, receipt)
            .unwrap();
        assert!(matches!(completed, ResumeMarkerStateV1::Completed { .. }));
        assert_eq!(restarted.load().unwrap(), Some(completed));
    }

    #[test]
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn repeated_envelope_temp_synced_crashes_are_bounded_and_recoverable() {
        let directory = TestDirectory::new();
        let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        let (publication, attempt, receipt, intent, ready) =
            stage_required_ready(&store, &publisher, &envelope);
        fs::write(
            directory.0.join("fault-envelope-input"),
            envelope.to_bytes(),
        )
        .unwrap();
        drop(store);

        for cycle in 1..=3 {
            let interrupted = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg(
                    "worker_v2_artifact_container::tests::required_envelope_publication_fault_helper",
                )
                .env(ENVELOPE_FAULT_HELPER_DIR_ENV, &directory.0)
                .env(
                    "FE2O3_TEST_WORKER_V2_FAULT_POINT_V1",
                    "envelope-temp-synced",
                )
                .status()
                .unwrap();
            assert_eq!(interrupted.code(), Some(86), "crash cycle {cycle}");
            assert_eq!(
                envelope_publication_temp_residue(&directory).len(),
                1,
                "crash cycle {cycle} accumulated publication temps"
            );
        }

        let restarted = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        assert!(envelope_publication_temp_residue(&directory).is_empty());
        assert_eq!(restarted.load().unwrap(), Some(ready));
        let recovered_intent = recover_worker_v2_intent_v1(&restarted, &publisher, ready).unwrap();
        let inputs = restarted.recover_envelope_inputs(attempt).unwrap();
        let claim = recover_published_hsaco_claim_for_attempt_v1(
            &directory.0,
            &publisher,
            attempt,
            recovered_intent.record().plan(),
            recovered_intent.record().upstream_evidence(),
            receipt,
        )
        .unwrap();
        let expected = assemble_recovered_worker_v2_load_envelope_v1(
            &publisher,
            recovered_intent.record().plan(),
            recovered_intent.record().upstream_evidence(),
            recovered_intent.exact_output(),
            claim,
            &inputs,
        )
        .unwrap();
        let completed = restarted
            .persist_envelope_and_completed(publication, attempt, intent, receipt, &expected)
            .unwrap();
        assert!(matches!(completed, ResumeMarkerStateV1::Completed { .. }));
        assert_eq!(restarted.load().unwrap(), Some(completed));
        assert!(envelope_publication_temp_residue(&directory).is_empty());
        assert_eq!(
            restarted.recover_load_envelope(receipt).unwrap().to_bytes(),
            expected.to_bytes()
        );
    }

    #[test]
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn capsule_temp_crash_residue_is_scavenged_under_the_package_lock() {
        let directory = TestDirectory::new();
        let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
        let attempt = envelope.published_claim().plan().attempt();
        let inputs = WorkerV2EnvelopeInputsV1::new(
            envelope.direct_link_evidence().clone(),
            envelope.proof_records().to_vec(),
            envelope.raw_hsaco().clone(),
        )
        .unwrap();
        fs::write(directory.0.join("fault-capsule-input"), inputs.to_bytes()).unwrap();

        let interrupted = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("worker_v2_artifact_container::tests::capsule_temp_fault_helper")
            .env(ENVELOPE_FAULT_HELPER_DIR_ENV, &directory.0)
            .env(ENVELOPE_FAULT_HELPER_ATTEMPT_ENV, attempt.to_env_value())
            .env(
                "FE2O3_TEST_WORKER_V2_FAULT_POINT_V1",
                "envelope-inputs-temp-synced",
            )
            .status()
            .unwrap();
        assert_eq!(interrupted.code(), Some(86));
        assert_eq!(envelope_input_residue(&directory).len(), 1);

        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        assert_eq!(store.load().unwrap(), None);
        assert!(envelope_input_residue(&directory).is_empty());
    }

    #[test]
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn capsule_temp_fault_helper() {
        let (Some(directory), Some(attempt)) = (
            std::env::var_os(ENVELOPE_FAULT_HELPER_DIR_ENV),
            std::env::var(ENVELOPE_FAULT_HELPER_ATTEMPT_ENV).ok(),
        ) else {
            return;
        };
        let directory = PathBuf::from(directory);
        let publisher = producer("alpha_zeta", "/workspace/envelope.rs");
        let attempt = BuildAttempt::from_env_value(&attempt).unwrap();
        let inputs = WorkerV2EnvelopeInputsV1::from_bytes(
            &fs::read(directory.join("fault-capsule-input")).unwrap(),
        )
        .unwrap();
        let store = WorkerV2ResumeStoreV1::open(&directory, &publisher).unwrap();
        store.persist_envelope_inputs(attempt, &inputs).unwrap();
        panic!("envelope-inputs-temp-synced fault was not injected");
    }

    #[test]
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn required_envelope_publication_fault_helper() {
        let Some(directory) = std::env::var_os(ENVELOPE_FAULT_HELPER_DIR_ENV) else {
            return;
        };
        let directory = PathBuf::from(directory);
        let publisher = producer("alpha_zeta", "/workspace/envelope.rs");
        let envelope = WorkerV2LoadEnvelopeV1::from_bytes(
            &fs::read(directory.join("fault-envelope-input")).unwrap(),
        )
        .unwrap();
        let store = WorkerV2ResumeStoreV1::open(&directory, &publisher).unwrap();
        store.publish_load_envelope(&envelope).unwrap();
        panic!("envelope-published fault was not injected");
    }

    #[test]
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn required_restart_after_published_child_crash_reconstructs_from_durable_inputs() {
        let directory = TestDirectory::new();
        let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        let (publication, attempt, receipt, intent_identity, ready) =
            stage_required_ready(&store, &publisher, &envelope);
        drop(store);

        let interrupted = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("worker_v2_artifact_container::tests::required_published_fault_helper")
            .env(ENVELOPE_FAULT_HELPER_DIR_ENV, &directory.0)
            .env("FE2O3_TEST_WORKER_V2_FAULT_POINT_V1", "published")
            .status()
            .unwrap();
        assert_eq!(interrupted.code(), Some(86));

        let restarted = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        assert_eq!(restarted.load().unwrap(), Some(ready));
        let intent = recover_worker_v2_intent_v1(&restarted, &publisher, ready).unwrap();
        let inputs = restarted.recover_envelope_inputs(attempt).unwrap();
        let claim = recover_published_hsaco_claim_for_attempt_v1(
            &directory.0,
            &publisher,
            attempt,
            intent.record().plan(),
            intent.record().upstream_evidence(),
            receipt,
        )
        .unwrap();
        let expected = assemble_recovered_worker_v2_load_envelope_v1(
            &publisher,
            intent.record().plan(),
            intent.record().upstream_evidence(),
            intent.exact_output(),
            claim,
            &inputs,
        )
        .unwrap();
        let completed = restarted
            .persist_envelope_and_completed(
                publication,
                attempt,
                intent_identity,
                receipt,
                &expected,
            )
            .unwrap();
        assert_eq!(completed.envelope(), expected.identity().as_bytes());
    }

    #[test]
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn required_published_fault_helper() {
        let Some(directory) = std::env::var_os(ENVELOPE_FAULT_HELPER_DIR_ENV) else {
            return;
        };
        let directory = PathBuf::from(directory);
        let publisher = producer("alpha_zeta", "/workspace/envelope.rs");
        let store = WorkerV2ResumeStoreV1::open(&directory, &publisher).unwrap();
        let state = store.load().unwrap().unwrap();
        assert!(state.publication().requires_envelope());
        assert!(store.recover_envelope_inputs(state.attempt()).is_ok());
        crate::worker_v2_restart::injected_fault_point_v1("published");
        panic!("published fault was not injected");
    }

    #[test]
    fn required_ready_restart_rejects_missing_malformed_and_substituted_envelopes() {
        for case in ["missing", "malformed", "substituted"] {
            let directory = TestDirectory::new();
            let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
            let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
            let (publication, attempt, receipt, intent, ready) =
                stage_required_ready(&store, &publisher, &envelope);
            let expected_path = directory
                .0
                .join(envelope_name(receipt.publication_identity()));

            match case {
                "missing" => {}
                "malformed" => {
                    store.publish_load_envelope(&envelope).unwrap();
                    let mut bytes = fs::read(&expected_path).unwrap();
                    bytes[0] ^= 1;
                    fs::write(&expected_path, bytes).unwrap();
                }
                "substituted" => {
                    let other_directory = TestDirectory::new();
                    let (other_publisher, substituted, _) = canonical_envelope_fixture_for(
                        &other_directory,
                        "substituted_alpha_zeta",
                        "/workspace/substituted-envelope.rs",
                    );
                    let substituted_receipt = substituted.published_claim().receipt();
                    assert_ne!(substituted_receipt, receipt);
                    let other_store =
                        WorkerV2ResumeStoreV1::open(&other_directory.0, &other_publisher).unwrap();
                    other_store.publish_load_envelope(&substituted).unwrap();
                    drop(other_store);
                    fs::rename(
                        other_directory
                            .0
                            .join(envelope_name(substituted_receipt.publication_identity())),
                        &expected_path,
                    )
                    .unwrap();
                }
                _ => unreachable!(),
            }

            assert!(
                store
                    .recover_envelope_and_completed(publication, attempt, intent, receipt)
                    .is_err(),
                "{case} envelope unexpectedly completed the required marker"
            );
            assert_eq!(store.load().unwrap(), Some(ready), "{case}");
        }
    }

    #[test]
    fn required_ready_rejects_missing_malformed_symlink_oversized_and_substituted_capsules() {
        for case in [
            "missing",
            "malformed",
            "symlink",
            "oversized",
            "substituted",
        ] {
            let directory = TestDirectory::new();
            let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
            let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
            let (publication, attempt, receipt, intent, ready) =
                stage_required_ready(&store, &publisher, &envelope);
            store.publish_load_envelope(&envelope).unwrap();
            let path = envelope_inputs_path(&directory);
            match case {
                "missing" => fs::remove_file(&path).unwrap(),
                "malformed" => fs::write(&path, b"malformed").unwrap(),
                "symlink" => {
                    let target = directory.0.join("untrusted-capsule-target");
                    fs::write(&target, b"target").unwrap();
                    fs::remove_file(&path).unwrap();
                    symlink(target, &path).unwrap();
                }
                "oversized" => {
                    fs::remove_file(&path).unwrap();
                    let file = fs::File::create(&path).unwrap();
                    file.set_len((MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES + 1) as u64)
                        .unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                }
                "substituted" => {
                    let substituted =
                        substitute_same_receipt_proof(&envelope, ProofMutation::Execution);
                    let inputs = WorkerV2EnvelopeInputsV1::new(
                        substituted.direct_link_evidence().clone(),
                        substituted.proof_records().to_vec(),
                        substituted.raw_hsaco().clone(),
                    )
                    .unwrap();
                    fs::write(&path, inputs.to_bytes()).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                store
                    .recover_envelope_and_completed(publication, attempt, intent, receipt)
                    .is_err(),
                "{case} capsule unexpectedly completed the required marker"
            );
            assert_eq!(store.load().unwrap(), Some(ready), "{case}");
        }
    }

    #[test]
    fn required_completion_rejects_same_receipt_proof_substitution() {
        for mutation in [
            ProofMutation::Configuration,
            ProofMutation::Execution,
            ProofMutation::Outcome,
            ProofMutation::TrustedItem,
        ] {
            let directory = TestDirectory::new();
            let (publisher, envelope, _) = canonical_envelope_fixture(&directory);
            let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
            let (publication, attempt, receipt, intent, ready) =
                stage_required_ready(&store, &publisher, &envelope);
            let substituted = substitute_same_receipt_proof(&envelope, mutation);
            assert_eq!(substituted.published_claim().receipt(), receipt);
            assert_ne!(substituted.identity(), envelope.identity());
            store.publish_load_envelope(&substituted).unwrap();

            assert!(
                store
                    .persist_envelope_and_completed(
                        publication,
                        attempt,
                        intent,
                        receipt,
                        &envelope,
                    )
                    .is_err()
            );
            assert_eq!(store.load().unwrap(), Some(ready));
        }
    }
}
