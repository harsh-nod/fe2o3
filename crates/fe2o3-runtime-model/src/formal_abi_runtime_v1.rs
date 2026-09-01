//! First mechanically checked ABI-to-runtime-preparation profile.
//!
//! This module is an executable admission model for the production-shaped
//! `vecadd(&[f32], &[f32], DisjointSlice<f32>)` COV6 profile. Its corresponding
//! Verus theorem is `vecadd_runtime_preparation_refines_v1` in
//! `verus/formal_abi_runtime_v1.rs`.
//!
//! Admission ends at preparation. In particular, it proves no relationship
//! between Kernel IR and LLVM, no compiler or linker correctness, no HSA/KFD
//! driver behavior, and no firmware or machine execution result.

/// Verus theorem statement identity for the V1 formal vecadd preparation slice.
///
/// This is an opaque versioned statement identifier, not proof authority.
pub const FORMAL_VECADD_RUNTIME_REFINEMENT_STATEMENT_V1: [u8; 32] = [
    0x66, 0x65, 0x32, 0x6f, 0x33, 0x2d, 0x76, 0x65, 0x63, 0x61, 0x64, 0x64, 0x2d, 0x72, 0x75, 0x6e,
    0x74, 0x69, 0x6d, 0x65, 0x2d, 0x72, 0x65, 0x66, 0x69, 0x6e, 0x65, 0x2d, 0x76, 0x31, 0x00, 0x01,
];
pub const FORMAL_VECADD_RUNTIME_PROOF_SOURCE_SHA256_V1: [u8; 32] = [
    0x22, 0xb6, 0x45, 0x36, 0x0a, 0xbf, 0xcd, 0x16, 0x6f, 0x60, 0xa4, 0x09, 0xc1, 0x75, 0xdc, 0x31,
    0xb7, 0x43, 0xf4, 0x46, 0x98, 0x26, 0x13, 0x97, 0x94, 0x44, 0x15, 0xa1, 0xba, 0xe0, 0xba, 0x62,
];
pub const FORMAL_VECADD_RUNTIME_VERUS_SHA256_V1: [u8; 32] = [
    0xd9, 0x75, 0x01, 0xa8, 0x83, 0x93, 0x1d, 0x1d, 0x17, 0x3b, 0x1b, 0xf4, 0xb6, 0xcf, 0x4d, 0x97,
    0x3f, 0x16, 0xd1, 0x05, 0xdb, 0xcb, 0x46, 0x8e, 0x17, 0x7b, 0x52, 0xb2, 0x33, 0x16, 0x12, 0xd2,
];
pub const FORMAL_VECADD_RUNTIME_VERUS_CLOSURE_MANIFEST_SHA256_V1: [u8; 32] = [
    0xf0, 0x68, 0x83, 0xe4, 0xce, 0x46, 0x3b, 0xcb, 0x9a, 0x3c, 0x8f, 0x91, 0x10, 0x64, 0xac, 0x85,
    0x05, 0x4c, 0x78, 0x22, 0xdc, 0x33, 0x1d, 0xb1, 0xa7, 0x9f, 0x75, 0xf9, 0xe8, 0x87, 0x8b, 0x01,
];
pub const FORMAL_VECADD_RUNTIME_VERUS_TRANSCRIPT_SHA256_V1: [u8; 32] = [
    0x95, 0x4d, 0x80, 0xbe, 0x26, 0xb1, 0x62, 0x28, 0x01, 0xa7, 0xd1, 0x7e, 0xd7, 0x04, 0x04, 0xab,
    0x89, 0xd0, 0x96, 0x53, 0xc2, 0xef, 0xca, 0x08, 0x93, 0x32, 0x52, 0x5e, 0x77, 0x30, 0x02, 0x4a,
];

pub const FORMAL_VECADD_EXPLICIT_BYTES_V1: u64 = 48;
pub const FORMAL_VECADD_IMPLICIT_BYTES_V1: u64 = 256;
pub const FORMAL_VECADD_KERNARG_BYTES_V1: u64 = 304;
pub const FORMAL_VECADD_DESCRIPTOR_ALIGNMENT_V1: u32 = 8;
pub const FORMAL_VECADD_RUNTIME_ALIGNMENT_V1: u32 = 16;
pub const FORMAL_VECADD_WORKGROUP_V1: [u32; 3] = [256, 1, 1];
pub const FORMAL_VECADD_ARGUMENT_COUNT_V1: usize = 3;
pub const FORMAL_VECADD_COMPONENT_COUNT_V1: usize = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalVecaddAbiComponentKindV1 {
    SlicePointer,
    SliceLength,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalVecaddEffectV1 {
    SharedRead,
    ExclusiveWrite,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalVecaddArgumentOwnershipV1 {
    SharedBorrow,
    UniqueBorrow,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalVecaddAbiComponentV1 {
    pub argument_index: u8,
    pub kind: FormalVecaddAbiComponentKindV1,
    pub offset: u64,
    pub size: u64,
    pub alignment: u32,
    pub effect: FormalVecaddEffectV1,
    pub argument_ownership: FormalVecaddArgumentOwnershipV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalRuntimeResourceOwnerV1 {
    Caller,
    PreparedInvocation,
}

/// Exact allocation-relative region retained for one logical vecadd argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalVecaddResourceV1 {
    pub argument_index: u8,
    pub allocation_context: u64,
    pub allocation_identity: u64,
    pub allocation_base: u64,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub encoded_address: u64,
    pub element_count: u64,
    pub effect: FormalVecaddEffectV1,
    pub owner: FormalRuntimeResourceOwnerV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalRuntimePreparationPhaseV1 {
    Loaded,
    Prepared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalVecaddGeometryV1 {
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
    pub dynamic_group_bytes: u32,
    pub source_max_grid: [u32; 3],
    pub physical_max_grid: [u32; 3],
    pub source_max_flat_workgroup: u32,
    pub physical_max_flat_workgroup: u32,
    pub source_required_workgroup: [u32; 3],
    pub physical_required_workgroup: Option<[u32; 3]>,
    pub source_static_group_bytes: u32,
    pub physical_static_group_bytes: u64,
    pub physical_private_segment_bytes: u64,
}

/// Canonical, authority-free inputs projected from authenticated Worker V3 state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalVecaddRuntimeInputV1 {
    pub kernel_identity: [u8; 32],
    pub generated_host_contract_identity: [u8; 32],
    pub rust_layout_contract_identity: [u8; 32],
    pub rust_effect_contract_identity: [u8; 32],
    pub explicit_byte_len: u64,
    pub implicit_byte_offset: u64,
    pub implicit_byte_len: u64,
    pub physical_byte_len: u64,
    pub descriptor_alignment: u32,
    pub runtime_alignment: u32,
    pub components: [FormalVecaddAbiComponentV1; FORMAL_VECADD_COMPONENT_COUNT_V1],
    pub geometry: FormalVecaddGeometryV1,
    pub resources: [FormalVecaddResourceV1; FORMAL_VECADD_ARGUMENT_COUNT_V1],
    pub source_phase: FormalRuntimePreparationPhaseV1,
}

/// Scope of the mechanically checked statement retained by runtime admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalRuntimeRefinementScopeV1;

/// Exact authenticated proof-run coordinates carried by the host admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalRuntimeProofBindingV1 {
    pub statement_identity: [u8; 32],
    pub proof_source_sha256: [u8; 32],
    pub verus_executable_sha256: [u8; 32],
    pub verus_closure_manifest_sha256: [u8; 32],
    pub transcript_sha256: [u8; 32],
}

impl FormalRuntimeRefinementScopeV1 {
    pub const fn covers_canonical_kernarg_layout(self) -> bool {
        true
    }

    pub const fn covers_geometry_and_resource_preconditions(self) -> bool {
        true
    }

    pub const fn covers_argument_effect_identity(self) -> bool {
        true
    }

    pub const fn covers_preparation_ownership_transition(self) -> bool {
        true
    }

    pub const fn covers_llvm_lowering(self) -> bool {
        false
    }

    pub const fn covers_driver_or_hsa_execution(self) -> bool {
        false
    }

    pub const fn covers_machine_execution(self) -> bool {
        false
    }
}

/// Inert evidence that executable admission matched the proved V1 relation.
///
/// The value is deliberately not `Clone`: when embedded in a prepared host
/// invocation it mirrors the one-shot custody of the retained arguments. It
/// grants no load, launch, completion, or proof authority on its own.
#[derive(Debug)]
pub struct FormalVecaddRuntimePreparationEvidenceV1 {
    input: FormalVecaddRuntimeInputV1,
    prepared_resources: [FormalVecaddResourceV1; FORMAL_VECADD_ARGUMENT_COUNT_V1],
}

impl FormalVecaddRuntimePreparationEvidenceV1 {
    pub const fn theorem_statement_identity(&self) -> [u8; 32] {
        FORMAL_VECADD_RUNTIME_REFINEMENT_STATEMENT_V1
    }

    pub const fn scope(&self) -> FormalRuntimeRefinementScopeV1 {
        FormalRuntimeRefinementScopeV1
    }

    pub const fn proof_binding(&self) -> FormalRuntimeProofBindingV1 {
        FormalRuntimeProofBindingV1 {
            statement_identity: FORMAL_VECADD_RUNTIME_REFINEMENT_STATEMENT_V1,
            proof_source_sha256: FORMAL_VECADD_RUNTIME_PROOF_SOURCE_SHA256_V1,
            verus_executable_sha256: FORMAL_VECADD_RUNTIME_VERUS_SHA256_V1,
            verus_closure_manifest_sha256: FORMAL_VECADD_RUNTIME_VERUS_CLOSURE_MANIFEST_SHA256_V1,
            transcript_sha256: FORMAL_VECADD_RUNTIME_VERUS_TRANSCRIPT_SHA256_V1,
        }
    }

    pub const fn input(&self) -> &FormalVecaddRuntimeInputV1 {
        &self.input
    }

    pub const fn prepared_phase(&self) -> FormalRuntimePreparationPhaseV1 {
        FormalRuntimePreparationPhaseV1::Prepared
    }

    pub const fn prepared_resources(
        &self,
    ) -> &[FormalVecaddResourceV1; FORMAL_VECADD_ARGUMENT_COUNT_V1] {
        &self.prepared_resources
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FormalVecaddRuntimePreparationErrorV1 {
    ZeroIdentity(&'static str),
    KernargLayout,
    AbiComponent { index: usize },
    Geometry,
    Resource { index: usize },
    ResourceConflict { left: usize, right: usize },
    SourcePhase,
}

/// Admits the exact bounded vecadd profile and performs the proved ownership
/// projection from caller-held resources to prepared-invocation custody.
pub fn admit_formal_vecadd_runtime_preparation_v1(
    input: FormalVecaddRuntimeInputV1,
) -> Result<FormalVecaddRuntimePreparationEvidenceV1, FormalVecaddRuntimePreparationErrorV1> {
    for (identity, field) in [
        (input.kernel_identity, "kernel identity"),
        (
            input.generated_host_contract_identity,
            "generated host contract identity",
        ),
        (
            input.rust_layout_contract_identity,
            "Rust layout contract identity",
        ),
        (
            input.rust_effect_contract_identity,
            "Rust effect contract identity",
        ),
    ] {
        if identity == [0; 32] {
            return Err(FormalVecaddRuntimePreparationErrorV1::ZeroIdentity(field));
        }
    }
    if input.explicit_byte_len != FORMAL_VECADD_EXPLICIT_BYTES_V1
        || input.implicit_byte_offset != FORMAL_VECADD_EXPLICIT_BYTES_V1
        || input.implicit_byte_len != FORMAL_VECADD_IMPLICIT_BYTES_V1
        || input.physical_byte_len != FORMAL_VECADD_KERNARG_BYTES_V1
        || input.descriptor_alignment != FORMAL_VECADD_DESCRIPTOR_ALIGNMENT_V1
        || input.runtime_alignment != FORMAL_VECADD_RUNTIME_ALIGNMENT_V1
    {
        return Err(FormalVecaddRuntimePreparationErrorV1::KernargLayout);
    }

    for (index, component) in input.components.iter().copied().enumerate() {
        let argument_index = (index / 2) as u8;
        let expected_kind = if index % 2 == 0 {
            FormalVecaddAbiComponentKindV1::SlicePointer
        } else {
            FormalVecaddAbiComponentKindV1::SliceLength
        };
        let expected_effect = if argument_index < 2 {
            FormalVecaddEffectV1::SharedRead
        } else {
            FormalVecaddEffectV1::ExclusiveWrite
        };
        let expected_ownership = if argument_index < 2 {
            FormalVecaddArgumentOwnershipV1::SharedBorrow
        } else {
            FormalVecaddArgumentOwnershipV1::UniqueBorrow
        };
        if component.argument_index != argument_index
            || component.kind != expected_kind
            || component.offset != (index as u64) * 8
            || component.size != 8
            || component.alignment != 8
            || component.effect != expected_effect
            || component.argument_ownership != expected_ownership
        {
            return Err(FormalVecaddRuntimePreparationErrorV1::AbiComponent { index });
        }
    }

    validate_geometry(input.geometry, input.resources[2].element_count)?;
    for (index, resource) in input.resources.iter().copied().enumerate() {
        validate_resource(index, resource)?;
    }
    if input.resources[2].element_count > input.resources[0].element_count
        || input.resources[2].element_count > input.resources[1].element_count
    {
        return Err(FormalVecaddRuntimePreparationErrorV1::Resource { index: 2 });
    }
    for left in 0..input.resources.len() {
        for right in (left + 1)..input.resources.len() {
            if resources_conflict(input.resources[left], input.resources[right]) {
                return Err(FormalVecaddRuntimePreparationErrorV1::ResourceConflict {
                    left,
                    right,
                });
            }
        }
    }
    if input.source_phase != FormalRuntimePreparationPhaseV1::Loaded {
        return Err(FormalVecaddRuntimePreparationErrorV1::SourcePhase);
    }

    let mut prepared_resources = input.resources;
    for resource in &mut prepared_resources {
        resource.owner = FormalRuntimeResourceOwnerV1::PreparedInvocation;
    }
    Ok(FormalVecaddRuntimePreparationEvidenceV1 {
        input,
        prepared_resources,
    })
}

fn validate_geometry(
    geometry: FormalVecaddGeometryV1,
    output_elements: u64,
) -> Result<(), FormalVecaddRuntimePreparationErrorV1> {
    let expected_blocks = output_elements
        .checked_add(u64::from(FORMAL_VECADD_WORKGROUP_V1[0]) - 1)
        .map(|value| value / u64::from(FORMAL_VECADD_WORKGROUP_V1[0]))
        .and_then(|value| u32::try_from(value).ok());
    let covered = u64::from(geometry.grid[0]).checked_mul(u64::from(geometry.workgroup[0]));
    if output_elements == 0
        || expected_blocks != Some(geometry.grid[0])
        || geometry.grid[1..] != [1, 1]
        || geometry.workgroup != FORMAL_VECADD_WORKGROUP_V1
        || geometry.source_required_workgroup != FORMAL_VECADD_WORKGROUP_V1
        || geometry.physical_required_workgroup != Some(FORMAL_VECADD_WORKGROUP_V1)
        || geometry.dynamic_group_bytes != 0
        || geometry.source_static_group_bytes != 0
        || geometry.physical_static_group_bytes != 0
        || geometry
            .grid
            .into_iter()
            .zip(geometry.source_max_grid)
            .any(|(actual, maximum)| actual > maximum)
        || geometry
            .grid
            .into_iter()
            .zip(geometry.physical_max_grid)
            .any(|(actual, maximum)| actual > maximum)
        || geometry.source_max_flat_workgroup < FORMAL_VECADD_WORKGROUP_V1[0]
        || geometry.physical_max_flat_workgroup < FORMAL_VECADD_WORKGROUP_V1[0]
        || geometry.physical_private_segment_bytes != 0
        || covered.is_none_or(|covered| covered < output_elements)
    {
        return Err(FormalVecaddRuntimePreparationErrorV1::Geometry);
    }
    Ok(())
}

fn validate_resource(
    index: usize,
    resource: FormalVecaddResourceV1,
) -> Result<(), FormalVecaddRuntimePreparationErrorV1> {
    let expected_effect = if index < 2 {
        FormalVecaddEffectV1::SharedRead
    } else {
        FormalVecaddEffectV1::ExclusiveWrite
    };
    let expected_bytes = resource.element_count.checked_mul(4);
    let expected_address = resource.allocation_base.checked_add(resource.byte_offset);
    let allocation_end = resource.byte_offset.checked_add(resource.byte_len);
    if resource.argument_index != index as u8
        || resource.effect != expected_effect
        || resource.owner != FormalRuntimeResourceOwnerV1::Caller
        || resource.allocation_identity == 0
        || resource.encoded_address == 0
        || !resource.encoded_address.is_multiple_of(4)
        || expected_bytes != Some(resource.byte_len)
        || expected_address != Some(resource.encoded_address)
        || allocation_end.is_none()
    {
        return Err(FormalVecaddRuntimePreparationErrorV1::Resource { index });
    }
    Ok(())
}

fn resources_conflict(left: FormalVecaddResourceV1, right: FormalVecaddResourceV1) -> bool {
    if left.allocation_context != right.allocation_context
        || left.allocation_identity != right.allocation_identity
        || (left.effect == FormalVecaddEffectV1::SharedRead
            && right.effect == FormalVecaddEffectV1::SharedRead)
    {
        return false;
    }
    let Some(left_end) = left.byte_offset.checked_add(left.byte_len) else {
        return true;
    };
    let Some(right_end) = right.byte_offset.checked_add(right.byte_len) else {
        return true;
    };
    left.byte_offset < right_end && right.byte_offset < left_end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(index: usize) -> FormalVecaddAbiComponentV1 {
        FormalVecaddAbiComponentV1 {
            argument_index: (index / 2) as u8,
            kind: if index.is_multiple_of(2) {
                FormalVecaddAbiComponentKindV1::SlicePointer
            } else {
                FormalVecaddAbiComponentKindV1::SliceLength
            },
            offset: index as u64 * 8,
            size: 8,
            alignment: 8,
            effect: if index < 4 {
                FormalVecaddEffectV1::SharedRead
            } else {
                FormalVecaddEffectV1::ExclusiveWrite
            },
            argument_ownership: if index < 4 {
                FormalVecaddArgumentOwnershipV1::SharedBorrow
            } else {
                FormalVecaddArgumentOwnershipV1::UniqueBorrow
            },
        }
    }

    fn resource(index: usize, elements: u64) -> FormalVecaddResourceV1 {
        FormalVecaddResourceV1 {
            argument_index: index as u8,
            allocation_context: 7,
            allocation_identity: index as u64 + 1,
            allocation_base: 0x1_0000 + index as u64 * 0x1_0000,
            byte_offset: 0,
            byte_len: elements * 4,
            encoded_address: 0x1_0000 + index as u64 * 0x1_0000,
            element_count: elements,
            effect: if index < 2 {
                FormalVecaddEffectV1::SharedRead
            } else {
                FormalVecaddEffectV1::ExclusiveWrite
            },
            owner: FormalRuntimeResourceOwnerV1::Caller,
        }
    }

    fn valid_input() -> FormalVecaddRuntimeInputV1 {
        FormalVecaddRuntimeInputV1 {
            kernel_identity: [1; 32],
            generated_host_contract_identity: [2; 32],
            rust_layout_contract_identity: [3; 32],
            rust_effect_contract_identity: [4; 32],
            explicit_byte_len: 48,
            implicit_byte_offset: 48,
            implicit_byte_len: 256,
            physical_byte_len: 304,
            descriptor_alignment: 8,
            runtime_alignment: 16,
            components: core::array::from_fn(component),
            geometry: FormalVecaddGeometryV1 {
                grid: [4, 1, 1],
                workgroup: [256, 1, 1],
                dynamic_group_bytes: 0,
                source_max_grid: [65_535, 1, 1],
                physical_max_grid: [u32::MAX, u32::MAX, u32::MAX],
                source_max_flat_workgroup: 256,
                physical_max_flat_workgroup: 1_024,
                source_required_workgroup: [256, 1, 1],
                physical_required_workgroup: Some([256, 1, 1]),
                source_static_group_bytes: 0,
                physical_static_group_bytes: 0,
                physical_private_segment_bytes: 0,
            },
            resources: [resource(0, 1_024), resource(1, 1_024), resource(2, 1_024)],
            source_phase: FormalRuntimePreparationPhaseV1::Loaded,
        }
    }

    #[test]
    fn exact_profile_refines_to_prepared_custody_without_execution_claims() {
        let evidence = admit_formal_vecadd_runtime_preparation_v1(valid_input()).unwrap();
        assert_eq!(
            evidence.prepared_phase(),
            FormalRuntimePreparationPhaseV1::Prepared
        );
        assert!(
            evidence
                .prepared_resources()
                .iter()
                .all(|resource| resource.owner == FormalRuntimeResourceOwnerV1::PreparedInvocation)
        );
        assert!(evidence.scope().covers_canonical_kernarg_layout());
        assert_eq!(
            evidence.proof_binding().proof_source_sha256,
            FORMAL_VECADD_RUNTIME_PROOF_SOURCE_SHA256_V1
        );
        assert!(!evidence.scope().covers_llvm_lowering());
        assert!(!evidence.scope().covers_driver_or_hsa_execution());
        assert!(!evidence.scope().covers_machine_execution());
        assert!(!evidence.grants_runtime_authority());
    }

    #[test]
    fn negative_mutations_fail_closed() {
        let mut input = valid_input();
        input.components[4].offset = 24;
        assert_eq!(
            admit_formal_vecadd_runtime_preparation_v1(input).unwrap_err(),
            FormalVecaddRuntimePreparationErrorV1::AbiComponent { index: 4 }
        );

        let mut input = valid_input();
        input.geometry.grid[0] = 3;
        assert_eq!(
            admit_formal_vecadd_runtime_preparation_v1(input).unwrap_err(),
            FormalVecaddRuntimePreparationErrorV1::Geometry
        );

        let mut input = valid_input();
        input.resources[2].effect = FormalVecaddEffectV1::SharedRead;
        assert_eq!(
            admit_formal_vecadd_runtime_preparation_v1(input).unwrap_err(),
            FormalVecaddRuntimePreparationErrorV1::Resource { index: 2 }
        );

        let mut input = valid_input();
        input.resources[2].owner = FormalRuntimeResourceOwnerV1::PreparedInvocation;
        assert_eq!(
            admit_formal_vecadd_runtime_preparation_v1(input).unwrap_err(),
            FormalVecaddRuntimePreparationErrorV1::Resource { index: 2 }
        );

        let mut input = valid_input();
        input.resources[2].allocation_identity = input.resources[0].allocation_identity;
        input.resources[2].allocation_base = input.resources[0].allocation_base;
        input.resources[2].encoded_address = input.resources[0].encoded_address;
        assert_eq!(
            admit_formal_vecadd_runtime_preparation_v1(input).unwrap_err(),
            FormalVecaddRuntimePreparationErrorV1::ResourceConflict { left: 0, right: 2 }
        );
    }
}
