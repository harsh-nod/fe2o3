//! Structural descriptor admission for the canonical direct-global tiled GEMM V1 ABI.

use std::{error::Error, fmt};

use crate::{
    AccessMode, AliasSemantics, BlockSizeV1, BuildEvidenceV1, CapabilityV1, CodeObjectVersion,
    DeviceDescriptorTableV1, KernelDescriptorDigest, KernelId, OwnershipSemantics,
    PhysicalAbiComponentKind, ScalarTypeV1,
};

pub const TILED_GEMM_V1_TARGET: &str = "gfx942:xnack-";
pub const TILED_GEMM_V1_ENTRY_NAME: &str = "tiled_gemm_v1";
pub const TILED_GEMM_V1_DESCRIPTOR_SYMBOL: &str = "tiled_gemm_v1.kd";
pub const TILED_GEMM_V1_WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];
pub const TILED_GEMM_V1_MAX_FLAT_WORKGROUP_SIZE: u32 = 64;
/// Explicit span of the direct-global four-slice ABI described by this policy.
pub const TILED_GEMM_V1_EXPLICIT_KERNARG_BYTES: u32 = 64;
pub const TILED_GEMM_V1_IMPLICIT_KERNARG_BYTES: u32 = 256;
pub const TILED_GEMM_V1_TOTAL_KERNARG_BYTES: u32 =
    TILED_GEMM_V1_EXPLICIT_KERNARG_BYTES + TILED_GEMM_V1_IMPLICIT_KERNARG_BYTES;

/// Separate fragment-level frontend evidence probe retained at the frontend boundary.
///
/// This is not the direct-global four-slice ABI and is intentionally rejected
/// by [`admit_tiled_gemm_v1_structural_descriptor_v1`].
pub const TILED_GEMM_FRAGMENT_FRONTEND_PROBE_V1_EXPLICIT_KERNARG_BYTES: u32 = 32;
pub const TILED_GEMM_FRAGMENT_FRONTEND_PROBE_V1_TOTAL_KERNARG_BYTES: u32 = 288;

const TILED_GEMM_V1_CAPABILITIES: [CapabilityV1; 4] = [
    CapabilityV1::Subgroup,
    CapabilityV1::MatrixMultiply,
    CapabilityV1::AmdWave,
    CapabilityV1::AmdMfma,
];

/// Exact declared provenance expected for one structural tiled GEMM profile.
///
/// These records remain unauthenticated V1 claims. Binding them here prevents
/// capability evidence from being silently omitted or substituted between the
/// caller's expected build and the descriptor, but does not establish origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TiledGemmV1StructuralDescriptorExpectationV1 {
    kernel_id: KernelId,
    source_evidence: BuildEvidenceV1,
    executable_ir_evidence: BuildEvidenceV1,
}

impl TiledGemmV1StructuralDescriptorExpectationV1 {
    pub fn new(
        kernel_id: KernelId,
        source_evidence: BuildEvidenceV1,
        executable_ir_evidence: BuildEvidenceV1,
    ) -> Result<Self, TiledGemmV1StructuralDescriptorErrorV1> {
        if kernel_id.as_bytes() == &[0; 32] {
            return Err(
                TiledGemmV1StructuralDescriptorErrorV1::InvalidExpectedProvenance(
                    "kernel identity is zero",
                ),
            );
        }
        for (field, evidence) in [
            ("source evidence", source_evidence),
            ("executable IR evidence", executable_ir_evidence),
        ] {
            if evidence.identity().as_bytes() == &[0; 32]
                || evidence.digest().as_bytes() == &[0; 32]
            {
                return Err(
                    TiledGemmV1StructuralDescriptorErrorV1::InvalidExpectedProvenance(field),
                );
            }
        }
        if source_evidence == executable_ir_evidence {
            return Err(
                TiledGemmV1StructuralDescriptorErrorV1::InvalidExpectedProvenance(
                    "source and executable IR evidence are identical",
                ),
            );
        }
        Ok(Self {
            kernel_id,
            source_evidence,
            executable_ir_evidence,
        })
    }

    pub const fn kernel_id(self) -> KernelId {
        self.kernel_id
    }

    pub const fn source_evidence(self) -> BuildEvidenceV1 {
        self.source_evidence
    }

    pub const fn executable_ir_evidence(self) -> BuildEvidenceV1 {
        self.executable_ir_evidence
    }

    pub const fn authenticates_evidence_origin(self) -> bool {
        false
    }
}

/// Sealed admission of the exact tiled GEMM V1 ABI and metadata declarations.
///
/// This value does not inspect the kernel body, authenticate code origin, or
/// establish that the machine code implements BF16 or MFMA semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedTiledGemmV1StructuralDescriptorV1 {
    kernel_id: KernelId,
    descriptor_digest: KernelDescriptorDigest,
    source_evidence: BuildEvidenceV1,
    executable_ir_evidence: BuildEvidenceV1,
}

impl AdmittedTiledGemmV1StructuralDescriptorV1 {
    pub const fn kernel_id(self) -> KernelId {
        self.kernel_id
    }

    pub const fn descriptor_digest(self) -> KernelDescriptorDigest {
        self.descriptor_digest
    }

    pub const fn source_evidence(self) -> BuildEvidenceV1 {
        self.source_evidence
    }

    pub const fn executable_ir_evidence(self) -> BuildEvidenceV1 {
        self.executable_ir_evidence
    }

    pub const fn workgroup_size(self) -> [u32; 3] {
        TILED_GEMM_V1_WORKGROUP_SIZE
    }

    pub const fn max_flat_workgroup_size(self) -> u32 {
        TILED_GEMM_V1_MAX_FLAT_WORKGROUP_SIZE
    }

    pub const fn explicit_kernarg_bytes(self) -> u32 {
        TILED_GEMM_V1_EXPLICIT_KERNARG_BYTES
    }

    pub const fn implicit_kernarg_bytes(self) -> u32 {
        TILED_GEMM_V1_IMPLICIT_KERNARG_BYTES
    }

    pub const fn total_kernarg_bytes(self) -> u32 {
        TILED_GEMM_V1_TOTAL_KERNARG_BYTES
    }

    pub const fn authenticates_evidence_origin(self) -> bool {
        false
    }

    pub const fn validates_kernel_body(self) -> bool {
        false
    }

    pub const fn proves_bf16_isa_semantics(self) -> bool {
        false
    }

    pub const fn proves_mfma_isa_semantics(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TiledGemmV1StructuralDescriptorErrorV1 {
    InvalidExpectedProvenance(&'static str),
    CodeObjectVersion,
    Target,
    KernelClosure,
    KernelIdentity,
    Symbol(&'static str),
    CapabilityProvenance,
    BuildEvidence(&'static str),
    KernargLayout,
    Launch(&'static str),
    Argument { index: usize, field: &'static str },
}

impl fmt::Display for TiledGemmV1StructuralDescriptorErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExpectedProvenance(field) => {
                write!(formatter, "invalid expected tiled GEMM provenance: {field}")
            }
            Self::CodeObjectVersion => formatter.write_str("tiled GEMM V1 requires COV6"),
            Self::Target => {
                write!(formatter, "tiled GEMM V1 requires target {TILED_GEMM_V1_TARGET}")
            }
            Self::KernelClosure => {
                formatter.write_str("tiled GEMM V1 requires exactly one descriptor kernel")
            }
            Self::KernelIdentity => {
                formatter.write_str("tiled GEMM V1 kernel identity differs from expected provenance")
            }
            Self::Symbol(field) => write!(formatter, "tiled GEMM V1 {field} drifted"),
            Self::CapabilityProvenance => formatter.write_str(
                "tiled GEMM V1 requires the exact subgroup, matrix, AMD-wave, and AMD-MFMA declaration set",
            ),
            Self::BuildEvidence(field) => {
                write!(formatter, "tiled GEMM V1 {field} differs from expected provenance")
            }
            Self::KernargLayout => {
                formatter.write_str(
                    "tiled GEMM V1 structural kernarg layout is not exactly 64 + 256 bytes",
                )
            }
            Self::Launch(field) => write!(formatter, "tiled GEMM V1 {field} launch policy drifted"),
            Self::Argument { index, field } => {
                write!(formatter, "tiled GEMM V1 argument {index} {field} drifted")
            }
        }
    }
}

impl Error for TiledGemmV1StructuralDescriptorErrorV1 {}

/// Admits only the exact direct-global tiled GEMM V1 structural descriptor profile.
///
/// The capability and evidence fields are declarations matched against caller
/// expectations. This function does not inspect executable instructions.
pub fn admit_tiled_gemm_v1_structural_descriptor_v1(
    table: &DeviceDescriptorTableV1,
    expected: TiledGemmV1StructuralDescriptorExpectationV1,
) -> Result<AdmittedTiledGemmV1StructuralDescriptorV1, TiledGemmV1StructuralDescriptorErrorV1> {
    if table.code_object_version() != CodeObjectVersion::V6 {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::CodeObjectVersion);
    }
    if table.device_target().to_string() != TILED_GEMM_V1_TARGET {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::Target);
    }
    let [kernel] = table.kernels() else {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::KernelClosure);
    };
    if kernel.kernel_id() != expected.kernel_id {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::KernelIdentity);
    }
    if kernel.logical_name().as_str() != TILED_GEMM_V1_ENTRY_NAME {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::Symbol(
            "logical name",
        ));
    }
    if kernel.entry_name().as_str() != TILED_GEMM_V1_ENTRY_NAME {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::Symbol("entry name"));
    }
    if kernel.descriptor_symbol().as_str() != TILED_GEMM_V1_DESCRIPTOR_SYMBOL {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::Symbol(
            "descriptor symbol",
        ));
    }
    if kernel.capabilities() != TILED_GEMM_V1_CAPABILITIES {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::CapabilityProvenance);
    }
    if kernel.source_evidence() != expected.source_evidence {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::BuildEvidence(
            "source evidence",
        ));
    }
    if kernel.executable_ir_evidence() != expected.executable_ir_evidence {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::BuildEvidence(
            "executable IR evidence",
        ));
    }

    let abi = kernel.abi_layout();
    if abi.explicit_argument_size() != TILED_GEMM_V1_EXPLICIT_KERNARG_BYTES
        || abi.kernarg_segment_size() != TILED_GEMM_V1_TOTAL_KERNARG_BYTES
        || abi.kernarg_segment_alignment() != 8
    {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::KernargLayout);
    }

    let launch = kernel.launch();
    let BlockSizeV1::Exact(block) = launch.block_size() else {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::Launch(
            "workgroup size",
        ));
    };
    if launch.rank() != 1
        || [block.x(), block.y(), block.z()] != TILED_GEMM_V1_WORKGROUP_SIZE
        || launch.max_flat_workgroup_size() != TILED_GEMM_V1_MAX_FLAT_WORKGROUP_SIZE
    {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::Launch(
            "workgroup size",
        ));
    }
    if launch.static_shared_memory_bytes() != 0 || launch.max_dynamic_shared_memory_bytes() != 0 {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::Launch("LDS"));
    }

    if kernel.arguments().len() != 4 {
        return Err(TiledGemmV1StructuralDescriptorErrorV1::Argument {
            index: kernel.arguments().len(),
            field: "count",
        });
    }
    for (index, argument) in kernel.arguments().iter().enumerate() {
        let scalar = if index < 2 {
            ScalarTypeV1::U16
        } else {
            ScalarTypeV1::F32
        };
        let expected_offset = u32::try_from(index).expect("bounded index") * 16;
        if argument.source_index() != u16::try_from(index).expect("bounded index") {
            return Err(argument_error(index, "source index"));
        }
        if argument.name().as_str() != ["a", "b", "c", "d"][index] {
            return Err(argument_error(index, "name"));
        }
        let (ownership, access, alias) = if index == 3 {
            (
                OwnershipSemantics::UniqueBorrow,
                AccessMode::ReadWrite,
                AliasSemantics::Exclusive,
            )
        } else {
            (
                OwnershipSemantics::SharedBorrow,
                AccessMode::ReadOnly,
                AliasSemantics::SharedReadOnly,
            )
        };
        if argument.ownership() != ownership
            || argument.access() != access
            || argument.alias() != alias
        {
            return Err(argument_error(index, "slice semantics"));
        }
        let components: Vec<_> = argument.physical_components().collect();
        if components
            != [
                (
                    PhysicalAbiComponentKind::GlobalPointer,
                    expected_offset,
                    8,
                    8,
                ),
                (
                    PhysicalAbiComponentKind::SliceLengthU64,
                    expected_offset + 8,
                    8,
                    8,
                ),
            ]
        {
            return Err(argument_error(index, "physical ABI"));
        }
        let source_scalar = table
            .type_records()
            .iter()
            .find(|record| record.identity() == argument.source_type())
            .map(|record| record.descriptor().scalar_type());
        let layout_scalar = table
            .layout_records()
            .iter()
            .find(|record| record.identity() == argument.device_layout())
            .map(|record| record.descriptor().scalar_type());
        if source_scalar != Some(scalar) || layout_scalar != Some(scalar) {
            return Err(argument_error(index, "type provenance"));
        }
    }

    Ok(AdmittedTiledGemmV1StructuralDescriptorV1 {
        kernel_id: kernel.kernel_id(),
        descriptor_digest: KernelDescriptorDigest::calculate(kernel),
        source_evidence: kernel.source_evidence(),
        executable_ir_evidence: kernel.executable_ir_evidence(),
    })
}

const fn argument_error(
    index: usize,
    field: &'static str,
) -> TiledGemmV1StructuralDescriptorErrorV1 {
    TiledGemmV1StructuralDescriptorErrorV1::Argument { index, field }
}
