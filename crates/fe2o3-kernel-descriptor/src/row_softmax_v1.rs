//! Structural descriptor admission for the fixed one-row row-softmax V1 ABI.

use std::{error::Error, fmt};

use crate::{
    AccessMode, AliasSemantics, BlockSizeV1, BuildEvidenceV1, CapabilityV1, CodeObjectVersion,
    DeviceDescriptorTableV1, KernelDescriptorDigest, KernelId, OwnershipSemantics,
    PhysicalAbiComponentKind, ScalarTypeV1,
};

pub const ROW_SOFTMAX_V1_TARGET: &str = "gfx942:xnack-";
pub const ROW_SOFTMAX_V1_ENTRY_NAME: &str = "row_softmax_v1";
pub const ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL: &str = "row_softmax_v1.kd";
/// Intended host-side row length, outside structural artifact evidence.
///
/// V1 descriptor bytes carry runtime length fields but not their values. No
/// admitted descriptor accessor exposes this constant as an observed fact.
pub const ROW_SOFTMAX_V1_INTENDED_HOST_ROW_ELEMENTS: u64 = 64;
pub const ROW_SOFTMAX_V1_WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];
pub const ROW_SOFTMAX_V1_MAX_GRID_SIZE: [u32; 3] = [1, 1, 1];
pub const ROW_SOFTMAX_V1_MAX_FLAT_WORKGROUP_SIZE: u32 = 64;
pub const ROW_SOFTMAX_V1_EXPLICIT_KERNARG_BYTES: u32 = 32;
pub const ROW_SOFTMAX_V1_IMPLICIT_KERNARG_BYTES: u32 = 256;
pub const ROW_SOFTMAX_V1_TOTAL_KERNARG_BYTES: u32 =
    ROW_SOFTMAX_V1_EXPLICIT_KERNARG_BYTES + ROW_SOFTMAX_V1_IMPLICIT_KERNARG_BYTES;

const ROW_SOFTMAX_V1_CAPABILITIES: [CapabilityV1; 2] =
    [CapabilityV1::Subgroup, CapabilityV1::AmdWave];

/// Exact declared provenance expected for one structural row-softmax profile.
///
/// V1 evidence records are caller-provided declarations. Matching them does not
/// authenticate their source or establish a compiler-to-artifact chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1StructuralDescriptorExpectationV1 {
    kernel_id: KernelId,
    source_evidence: BuildEvidenceV1,
    executable_ir_evidence: BuildEvidenceV1,
}

impl RowSoftmaxV1StructuralDescriptorExpectationV1 {
    pub fn new(
        kernel_id: KernelId,
        source_evidence: BuildEvidenceV1,
        executable_ir_evidence: BuildEvidenceV1,
    ) -> Result<Self, RowSoftmaxV1StructuralDescriptorErrorV1> {
        if kernel_id.as_bytes() == &[0; 32] {
            return Err(
                RowSoftmaxV1StructuralDescriptorErrorV1::InvalidExpectedProvenance(
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
                    RowSoftmaxV1StructuralDescriptorErrorV1::InvalidExpectedProvenance(field),
                );
            }
        }
        if source_evidence == executable_ir_evidence {
            return Err(
                RowSoftmaxV1StructuralDescriptorErrorV1::InvalidExpectedProvenance(
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

/// Sealed admission of row-softmax V1 declarations and physical ABI facts.
///
/// The 64-element shape is a host-side profile. The admitted ABI contains two
/// runtime lengths, but neither the descriptor table nor HSACO metadata records
/// their values. This type therefore cannot prove that either slice has length 64.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedRowSoftmaxV1StructuralDescriptorV1 {
    kernel_id: KernelId,
    descriptor_digest: KernelDescriptorDigest,
    source_evidence: BuildEvidenceV1,
    executable_ir_evidence: BuildEvidenceV1,
}

impl AdmittedRowSoftmaxV1StructuralDescriptorV1 {
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
        ROW_SOFTMAX_V1_WORKGROUP_SIZE
    }

    pub const fn max_grid_size(self) -> [u32; 3] {
        ROW_SOFTMAX_V1_MAX_GRID_SIZE
    }

    pub const fn max_flat_workgroup_size(self) -> u32 {
        ROW_SOFTMAX_V1_MAX_FLAT_WORKGROUP_SIZE
    }

    pub const fn explicit_kernarg_bytes(self) -> u32 {
        ROW_SOFTMAX_V1_EXPLICIT_KERNARG_BYTES
    }

    pub const fn implicit_kernarg_bytes(self) -> u32 {
        ROW_SOFTMAX_V1_IMPLICIT_KERNARG_BYTES
    }

    pub const fn total_kernarg_bytes(self) -> u32 {
        ROW_SOFTMAX_V1_TOTAL_KERNARG_BYTES
    }

    pub const fn authenticates_evidence_origin(self) -> bool {
        false
    }

    pub const fn validates_runtime_slice_lengths(self) -> bool {
        false
    }

    pub const fn validates_kernel_body(self) -> bool {
        false
    }

    pub const fn proves_functional_softmax(self) -> bool {
        false
    }

    pub const fn proves_exp_implementation(self) -> bool {
        false
    }

    pub const fn proves_numerical_contract(self) -> bool {
        false
    }

    pub const fn proves_race_freedom(self) -> bool {
        false
    }

    pub const fn proves_verus_verification(self) -> bool {
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

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RowSoftmaxV1StructuralDescriptorErrorV1 {
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

impl fmt::Display for RowSoftmaxV1StructuralDescriptorErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExpectedProvenance(field) => {
                write!(
                    formatter,
                    "invalid expected row-softmax provenance: {field}"
                )
            }
            Self::CodeObjectVersion => formatter.write_str("row-softmax V1 requires COV6"),
            Self::Target => write!(
                formatter,
                "row-softmax V1 requires target {ROW_SOFTMAX_V1_TARGET}"
            ),
            Self::KernelClosure => {
                formatter.write_str("row-softmax V1 requires exactly one descriptor kernel")
            }
            Self::KernelIdentity => formatter
                .write_str("row-softmax V1 kernel identity differs from expected provenance"),
            Self::Symbol(field) => write!(formatter, "row-softmax V1 {field} drifted"),
            Self::CapabilityProvenance => formatter.write_str(
                "row-softmax V1 requires the exact subgroup and AMD-wave declaration set",
            ),
            Self::BuildEvidence(field) => write!(
                formatter,
                "row-softmax V1 {field} differs from expected provenance"
            ),
            Self::KernargLayout => formatter.write_str(
                "row-softmax V1 structural kernarg layout is not exactly 32 + 256 bytes",
            ),
            Self::Launch(field) => {
                write!(formatter, "row-softmax V1 {field} launch policy drifted")
            }
            Self::Argument { index, field } => {
                write!(formatter, "row-softmax V1 argument {index} {field} drifted")
            }
        }
    }
}

impl Error for RowSoftmaxV1StructuralDescriptorErrorV1 {}

/// Admits only the exact row-softmax V1 structural descriptor profile.
///
/// Capability and evidence fields remain declarations. This function checks
/// no executable instruction and sees no runtime kernarg values.
pub fn admit_row_softmax_v1_structural_descriptor_v1(
    table: &DeviceDescriptorTableV1,
    expected: RowSoftmaxV1StructuralDescriptorExpectationV1,
) -> Result<AdmittedRowSoftmaxV1StructuralDescriptorV1, RowSoftmaxV1StructuralDescriptorErrorV1> {
    if table.code_object_version() != CodeObjectVersion::V6 {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::CodeObjectVersion);
    }
    if table.device_target().to_string() != ROW_SOFTMAX_V1_TARGET {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::Target);
    }
    let [kernel] = table.kernels() else {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::KernelClosure);
    };
    if kernel.kernel_id() != expected.kernel_id {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::KernelIdentity);
    }
    if kernel.logical_name().as_str() != ROW_SOFTMAX_V1_ENTRY_NAME {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::Symbol(
            "logical name",
        ));
    }
    if kernel.entry_name().as_str() != ROW_SOFTMAX_V1_ENTRY_NAME {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::Symbol(
            "entry name",
        ));
    }
    if kernel.descriptor_symbol().as_str() != ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::Symbol(
            "descriptor symbol",
        ));
    }
    if kernel.capabilities() != ROW_SOFTMAX_V1_CAPABILITIES {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::CapabilityProvenance);
    }
    if kernel.source_evidence() != expected.source_evidence {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::BuildEvidence(
            "source evidence",
        ));
    }
    if kernel.executable_ir_evidence() != expected.executable_ir_evidence {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::BuildEvidence(
            "executable IR evidence",
        ));
    }

    let abi = kernel.abi_layout();
    if abi.explicit_argument_size() != ROW_SOFTMAX_V1_EXPLICIT_KERNARG_BYTES
        || abi.kernarg_segment_size() != ROW_SOFTMAX_V1_TOTAL_KERNARG_BYTES
        || abi.kernarg_segment_alignment() != 8
    {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::KernargLayout);
    }

    let launch = kernel.launch();
    let BlockSizeV1::Exact(block) = launch.block_size() else {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::Launch(
            "workgroup size",
        ));
    };
    if launch.rank() != 1
        || [block.x(), block.y(), block.z()] != ROW_SOFTMAX_V1_WORKGROUP_SIZE
        || launch.max_flat_workgroup_size() != ROW_SOFTMAX_V1_MAX_FLAT_WORKGROUP_SIZE
    {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::Launch(
            "workgroup size",
        ));
    }
    let max_grid = launch.max_grid();
    if [max_grid.x(), max_grid.y(), max_grid.z()] != ROW_SOFTMAX_V1_MAX_GRID_SIZE {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::Launch(
            "maximum grid size",
        ));
    }
    if launch.static_shared_memory_bytes() != 0 || launch.max_dynamic_shared_memory_bytes() != 0 {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::Launch("LDS"));
    }

    if kernel.arguments().len() != 2 {
        return Err(RowSoftmaxV1StructuralDescriptorErrorV1::Argument {
            index: kernel.arguments().len(),
            field: "count",
        });
    }
    for (index, argument) in kernel.arguments().iter().enumerate() {
        let expected_offset = u32::try_from(index).expect("bounded index") * 16;
        if argument.source_index() != u16::try_from(index).expect("bounded index") {
            return Err(argument_error(index, "source index"));
        }
        if argument.name().as_str() != ["input", "output"][index] {
            return Err(argument_error(index, "name"));
        }
        let (ownership, access, alias) = if index == 0 {
            (
                OwnershipSemantics::SharedBorrow,
                AccessMode::ReadOnly,
                AliasSemantics::SharedReadOnly,
            )
        } else {
            (
                OwnershipSemantics::UniqueBorrow,
                AccessMode::ReadWrite,
                AliasSemantics::Exclusive,
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
        if source_scalar != Some(ScalarTypeV1::F32) || layout_scalar != Some(ScalarTypeV1::F32) {
            return Err(argument_error(index, "type provenance"));
        }
    }

    Ok(AdmittedRowSoftmaxV1StructuralDescriptorV1 {
        kernel_id: kernel.kernel_id(),
        descriptor_digest: KernelDescriptorDigest::calculate(kernel),
        source_evidence: kernel.source_evidence(),
        executable_ir_evidence: kernel.executable_ir_evidence(),
    })
}

const fn argument_error(
    index: usize,
    field: &'static str,
) -> RowSoftmaxV1StructuralDescriptorErrorV1 {
    RowSoftmaxV1StructuralDescriptorErrorV1::Argument { index, field }
}
