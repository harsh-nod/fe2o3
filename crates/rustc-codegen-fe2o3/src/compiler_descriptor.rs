//! Workload-neutral rustc-derived descriptor input for production typed kernels.

use crate::collector::{CollectedFunction, TypedArgumentListV1};
use crate::kernel_ir_codegen::InertCompilerModuleTextV1;
use crate::rust_type_layout_v3::{
    GeneralTypedArgumentKindV3, GeneralTypedExtractError, extract_general_typed_kernel_v3,
};
use fe2o3_artifacts::{
    LaunchContract, RustLayoutEvidenceV1, TypeIdentity, derive_generated_host_contract_identity_v1,
};
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceErrorV1, CompilerDescriptorSourceV1, CompilerFfiEnvelopeV1,
};
use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CapabilityV1,
    CodeObjectVersion, CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1,
    KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1, ProducerIdentityV1,
    RUSTC_CODEGEN_FE2O3_COMPILER_NAME_V1, RUSTC_CODEGEN_FE2O3_PRODUCTION_V3_PRODUCER_NAME_V1,
    ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName, ValidationError,
};
use fe2o3_kernel_ir::{
    AMDGPU_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    BF16_F32_M16N16K16_CAPABILITY, MATRIX_CAPABILITY_NAMESPACE, Module,
    SCALED_FP4_E2M1_F32_M16N16K128_CAPABILITY, SCALED_FP4_E2M1_FP8_E4M3_F32_M16N16K128_CAPABILITY,
    SCALED_FP8_E4M3_F32_M16N16K128_CAPABILITY, TargetCapability, WaveWidth, WorkgroupSize,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1;
use reserved_fe2o3_symbols::{KernelBindingIdV1, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1};
use rustc_middle::ty::{TyCtxt, TyKind, TypingEnv};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

const ADMITTED_PRODUCTION_PROCESSORS: [&str; 2] = ["gfx942", "gfx950"];
#[cfg(test)]
const WORKGROUP_X: u32 = 256;

const SOURCE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-SOURCE-IDENTITY/V1\0";
const SOURCE_DIGEST_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-SOURCE-DIGEST/V1\0";
const IR_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-IR-IDENTITY/V1\0";
const IR_DIGEST_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-IR-DIGEST/V1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypedDescriptorRootV1 {
    logical_name: String,
    export_name: String,
    kernel_binding: KernelBindingIdV1,
    arguments: TypedArgumentListV1<TypedDescriptorArgumentV1>,
    explicit_argument_bytes: u32,
    kernarg_alignment_bytes: u32,
    source_launch: Option<LaunchContract>,
}

impl TypedDescriptorRootV1 {
    pub(crate) fn logical_name(&self) -> &str {
        &self.logical_name
    }

    pub(crate) const fn kernel_binding_bytes(&self) -> [u8; 32] {
        self.kernel_binding.as_bytes()
    }

    pub(crate) const fn source_launch(&self) -> Option<&LaunchContract> {
        self.source_launch.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DescriptorArgumentKindV1 {
    SharedSlice(ScalarTypeV1),
    DisjointSlice(ScalarTypeV1),
    GlobalMutPointer(ScalarTypeV1),
    Scalar(ScalarTypeV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TypedDescriptorArgumentV1 {
    name: String,
    kind: DescriptorArgumentKindV1,
    access: AccessMode,
    offset: u32,
    layout: RustLayoutEvidenceV1,
    semantic_type_identity: SemanticTypeIdentityV1,
}

/// Re-derives the complete generic typed evidence directly from rustc and the
/// independently authenticated frontend launch contract.
pub(crate) fn typed_descriptor_roots_from_production_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    functions: &[CollectedFunction<'tcx>],
) -> Result<Vec<TypedDescriptorRootV1>, CompilerDescriptorError> {
    functions
        .iter()
        .filter_map(|function| {
            function
                .generated_host_contract_identity
                .map(|generated_identity| {
                    if !function.is_kernel_entry() {
                        return Err(CompilerDescriptorError::TypedProfileOnNonKernel(
                            function.export_name.clone(),
                        ));
                    }
                    let logical_name = function.logical_name.clone().ok_or_else(|| {
                        CompilerDescriptorError::MissingTypedField {
                            kernel: function.export_name.clone(),
                            field: "logical name",
                        }
                    })?;
                    let kernel_binding = function.kernel_binding.ok_or_else(|| {
                        CompilerDescriptorError::MissingTypedField {
                            kernel: function.export_name.clone(),
                            field: "kernel binding",
                        }
                    })?;
                    let launch = crate::collector::rederive_general_typed_launch_for_descriptor_v1(
                        function.frontend_contract.as_ref(),
                        &function.export_name,
                    )
                    .map_err(|reason| {
                        CompilerDescriptorError::InvalidArgumentCollection {
                            kernel: function.export_name.clone(),
                            reason,
                        }
                    })?;
                    let contract = extract_general_typed_kernel_v3(tcx, function.instance, &launch)
                        .map_err(CompilerDescriptorError::GeneralRustLayout)?;
                    let instance_ty = function.instance.ty(tcx, TypingEnv::fully_monomorphized());
                    let (signature_def_id, signature_args) = match *instance_ty.kind() {
                        TyKind::FnDef(def_id, args) if def_id == function.instance.def_id() => {
                            (def_id, args)
                        }
                        _ => {
                            return Err(CompilerDescriptorError::InvalidArgumentCollection {
                                kernel: function.export_name.clone(),
                                reason: "typed descriptor lost its exact rustc function signature"
                                    .to_owned(),
                            });
                        }
                    };
                    let signature = tcx.instantiate_bound_regions_with_erased(
                        tcx.fn_sig(signature_def_id)
                            .instantiate(tcx, signature_args),
                    );
                    if signature.inputs().len() != contract.arguments().len() {
                        return Err(CompilerDescriptorError::InvalidArgumentCollection {
                            kernel: function.export_name.clone(),
                            reason: "typed descriptor argument/signature arity changed".to_owned(),
                        });
                    }
                    let derived = derive_generated_host_contract_identity_v1(
                        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                        kernel_binding.as_bytes(),
                        &logical_name,
                        &function.export_name,
                        contract.abi(),
                        contract.launch(),
                    );
                    if derived.as_bytes() != &generated_identity.as_bytes() {
                        return Err(CompilerDescriptorError::GeneratedHostContractMismatch(
                            function.export_name.clone(),
                        ));
                    }
                    let arguments = contract
                        .arguments()
                        .iter()
                        .zip(contract.abi().fields())
                        .zip(signature.inputs().iter().copied())
                        .enumerate()
                        .map(|(index, ((argument, field), source_ty))| {
                            Ok(TypedDescriptorArgumentV1 {
                                name: field.name().as_str().to_owned(),
                                kind: descriptor_argument_kind(argument.kind()),
                                access: match argument.kind() {
                                    GeneralTypedArgumentKindV3::Scalar(_) => AccessMode::ByValue,
                                    GeneralTypedArgumentKindV3::SharedSlice(_) => {
                                        AccessMode::ReadOnly
                                    }
                                    GeneralTypedArgumentKindV3::DisjointSlice(_)
                                    | GeneralTypedArgumentKindV3::GlobalMutPointer(_) => {
                                        AccessMode::ReadWrite
                                    }
                                },
                                offset: u32::try_from(field.offset()).map_err(|_| {
                                    CompilerDescriptorError::ArgumentOffsetOverflow {
                                        kernel: function.export_name.clone(),
                                        index,
                                    }
                                })?,
                                layout: argument.layout().clone(),
                                semantic_type_identity:
                                    crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(
                                        tcx, source_ty,
                                    ),
                            })
                        })
                        .collect::<Result<Vec<_>, CompilerDescriptorError>>()?;
                    let arguments = TypedArgumentListV1::new(arguments).map_err(|error| {
                        CompilerDescriptorError::InvalidArgumentCollection {
                            kernel: function.export_name.clone(),
                            reason: error.to_string(),
                        }
                    })?;
                    Ok(TypedDescriptorRootV1 {
                        logical_name,
                        export_name: function.export_name.clone(),
                        kernel_binding,
                        arguments,
                        explicit_argument_bytes: u32::try_from(contract.abi().size()).map_err(
                            |_| {
                                CompilerDescriptorError::ExplicitArgumentSizeOverflow(
                                    function.export_name.clone(),
                                )
                            },
                        )?,
                        kernarg_alignment_bytes: contract.abi().alignment(),
                        source_launch: Some(contract.launch().clone()),
                    })
                })
        })
        .collect()
}

fn descriptor_argument_kind(kind: GeneralTypedArgumentKindV3) -> DescriptorArgumentKindV1 {
    let scalar = descriptor_scalar(kind.scalar());
    match kind {
        GeneralTypedArgumentKindV3::Scalar(_) => DescriptorArgumentKindV1::Scalar(scalar),
        GeneralTypedArgumentKindV3::SharedSlice(_) => DescriptorArgumentKindV1::SharedSlice(scalar),
        GeneralTypedArgumentKindV3::DisjointSlice(_) => {
            DescriptorArgumentKindV1::DisjointSlice(scalar)
        }
        GeneralTypedArgumentKindV3::GlobalMutPointer(_) => {
            DescriptorArgumentKindV1::GlobalMutPointer(scalar)
        }
    }
}

fn descriptor_scalar(value: fe2o3_artifacts::RustScalarElementTypeV1) -> ScalarTypeV1 {
    use fe2o3_artifacts::RustScalarElementTypeV1 as RustScalar;
    match value {
        RustScalar::I8 => ScalarTypeV1::I8,
        RustScalar::U8 => ScalarTypeV1::U8,
        RustScalar::I16 => ScalarTypeV1::I16,
        RustScalar::U16 => ScalarTypeV1::U16,
        RustScalar::I32 => ScalarTypeV1::I32,
        RustScalar::U32 => ScalarTypeV1::U32,
        RustScalar::I64 => ScalarTypeV1::I64,
        RustScalar::U64 => ScalarTypeV1::U64,
        RustScalar::F32 => ScalarTypeV1::F32,
        RustScalar::F64 => ScalarTypeV1::F64,
        RustScalar::F16 => unreachable!("general typed V3 rejects f16"),
        _ => unreachable!("unknown scalar schema is not admitted by general typed V3"),
    }
}

/// Constructs a zero-digest descriptor source for a complete typed gfx942/COV6 module.
///
/// An all-raw module returns `None`. A mixed typed/raw module is rejected because publishing an
/// incomplete descriptor table would create a misleading kernel closure.
#[cfg(test)]
pub(crate) fn construct_compiler_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    module: &Module,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
) -> Result<Option<CompilerDescriptorSourceV1>, CompilerDescriptorError> {
    construct_compiler_descriptor_source_with_profile_v1(
        envelope,
        module,
        compiler_module,
        typed_roots,
        DescriptorConstructionProfileV1 {
            rank: 1,
            workgroup: [WORKGROUP_X, 1, 1],
            max_grid: [u32::MAX, 1, 1],
            max_flat_workgroup_size: WORKGROUP_X,
            static_shared_memory_bytes: 0,
            allow_exact_tiled_matrix: false,
            allow_workgroup_memory: false,
            producer_version: "typed-general-gfx942-cov6-v1",
        },
    )
}

/// Constructs the descriptor source for the single production pipeline from
/// evidence retained across every preceding typed stage.
///
/// This boundary accepts no caller-authored descriptor fields. The freshly
/// re-derived rustc root must agree with semantic MIR, target-bound Kernel IR,
/// and complete formal-memory admission before the existing canonical encoder
/// is allowed to emit source bytes.
pub(crate) fn construct_production_v1_compiler_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    module: &Module,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    formal: &fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    formal
        .verify_equivalence()
        .map_err(CompilerDescriptorError::ProductionFormalMemory)?;
    let target = envelope.target().to_string();
    let geometry =
        validate_production_v1_descriptor_evidence(module, typed_roots, formal, &target)?;
    let producer_version = match envelope.target().as_amd_target_id().processor() {
        "gfx942" => "production-v1-gfx942-cov6-v1",
        "gfx950" => "production-v1-gfx950-cov6-v1",
        _ => "production-v1-unsupported-target-v1",
    };
    construct_compiler_descriptor_source_with_profile_v1(
        envelope,
        module,
        compiler_module,
        typed_roots,
        DescriptorConstructionProfileV1 {
            rank: geometry.rank(),
            workgroup: geometry.workgroup(),
            max_grid: geometry.max_grid(),
            max_flat_workgroup_size: geometry.max_flat_workgroup_size(),
            static_shared_memory_bytes: geometry.static_shared_memory_bytes(),
            allow_exact_tiled_matrix: geometry.allow_exact_tiled_matrix(),
            allow_workgroup_memory: geometry.allow_workgroup_memory(),
            producer_version,
        },
    )?
    .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
        "complete typed descriptor closure",
    ))
}

fn validate_production_v1_descriptor_evidence(
    module: &Module,
    typed_roots: &[TypedDescriptorRootV1],
    formal: &fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
    device_target: &str,
) -> Result<crate::production_geometry_v1::ProductionGeometryV1, CompilerDescriptorError> {
    use fe2o3_artifacts::{RustDisjointIndexSpaceV1, RustSourceTypeShapeV1};
    use fe2o3_kernel_ir::{
        AccessMode as KirAccessMode, AddressSpace, FormalMemoryAccessKind, FormalParameterKind,
    };
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1, SemanticDisjointIndexSpaceV1,
    };

    let semantic = formal.semantic_kir().semantic().semantic();
    let [root] = typed_roots else {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "one complete typed root",
        ));
    };
    let [semantic_root] = semantic.roots() else {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "one semantic root",
        ));
    };
    let semantic_function = semantic
        .functions()
        .get(semantic_root.index() as usize)
        .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
            "semantic root function",
        ))?;
    let [kernel] = module.kernels.as_slice() else {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "one target-bound kernel",
        ));
    };
    let entry = module
        .functions
        .iter()
        .find(|function| function.id == kernel.entry)
        .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
            "target-bound kernel entry",
        ))?;
    let body = entry
        .body
        .as_ref()
        .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
            "defined target-bound entry",
        ))?;
    if kernel.id.as_str() != root.export_name
        || entry.signature.parameters.len() != root.arguments.len()
        || body.parameters.len() != root.arguments.len()
        || formal.obligations().kernel() != &kernel.id
        || formal.obligations().entry() != &kernel.entry
    {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "target/formal kernel closure",
        ));
    }
    let source_launch =
        root.source_launch()
            .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
                "authenticated source launch",
            ))?;
    let geometry = crate::production_geometry_v1::derive_production_geometry_v1(
        module,
        semantic_function,
        source_launch,
        device_target,
    )
    .map_err(CompilerDescriptorError::ProductionGeometry)?;

    for (index, (root_argument, kernel_type)) in root
        .arguments
        .as_slice()
        .iter()
        .zip(&entry.signature.parameters)
        .enumerate()
    {
        let exact_kernel_type =
            production_descriptor_argument_matches_kernel_type_v1(root_argument.kind, kernel_type);
        if !exact_kernel_type {
            return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "typed descriptor/Kernel IR argument correspondence",
            ));
        }
        if matches!(
            root_argument.kind,
            DescriptorArgumentKindV1::DisjointSlice(_)
        ) {
            let semantic_type_id = *semantic_function
                .abi()
                .source_input_types()
                .get(index)
                .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
                    "pre-ranked semantic argument identity",
                ))?;
            let RustSourceTypeShapeV1::DisjointSlice { index_space, .. } =
                root_argument.layout.rust_type().source_type()
            else {
                return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                    "typed disjoint source identity",
                ));
            };
            let expected_mapping = match index_space {
                RustDisjointIndexSpaceV1::Index1D => SemanticDisjointIndexSpaceV1::Index1d,
                RustDisjointIndexSpaceV1::ShiftedIndex1D { offset } => {
                    SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset }
                }
                RustDisjointIndexSpaceV1::GridExclusive => {
                    SemanticDisjointIndexSpaceV1::GridExclusive
                }
                RustDisjointIndexSpaceV1::BlockedIndex1D {
                    lanes_per_block,
                    elements_per_lane,
                } => SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                    lanes_per_block: lanes_per_block.get(),
                    elements_per_lane: elements_per_lane.get(),
                },
                RustDisjointIndexSpaceV1::Tiled2DIndex1D {
                    lanes_per_tile,
                    tile_rows,
                    tile_columns,
                    elements_per_lane,
                } => SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                    lanes_per_tile: lanes_per_tile.get(),
                    tile_rows: tile_rows.get(),
                    tile_columns: tile_columns.get(),
                    elements_per_lane: elements_per_lane.get(),
                },
                RustDisjointIndexSpaceV1::RowStriped2DIndex1D {
                    lanes_per_row,
                    elements_per_lane,
                } => SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
                    lanes_per_row: lanes_per_row.get(),
                    elements_per_lane: elements_per_lane.get(),
                },
                _ => {
                    return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                        "supported disjoint mapping identity",
                    ));
                }
            };
            let mappings = semantic
                .callables()
                .iter()
                .filter_map(|callable| match callable {
                    SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } => match operation {
                        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                            disjoint_slice,
                            ..
                        } if *disjoint_slice == semantic_type_id => {
                            Some(SemanticDisjointIndexSpaceV1::Index1d)
                        }
                        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                            disjoint_slice,
                            index_space,
                            ..
                        } if *disjoint_slice == semantic_type_id => Some(*index_space),
                        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                            disjoint_slice,
                            ..
                        } if *disjoint_slice == semantic_type_id => {
                            Some(SemanticDisjointIndexSpaceV1::GridExclusive)
                        }
                        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                            disjoint_slice,
                            index_space,
                            ..
                        } if *disjoint_slice == semantic_type_id => Some(*index_space),
                        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
                            disjoint_slice,
                            index_space,
                            ..
                        }
                        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut {
                            disjoint_slice,
                            index_space,
                            ..
                        } if *disjoint_slice == semantic_type_id => Some(*index_space),
                        _ => None,
                    },
                    _ => None,
                });
            let mut found = false;
            for mapping in mappings {
                found = true;
                if mapping != expected_mapping {
                    return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                        "semantic/artifact disjoint mapping identity",
                    ));
                }
            }
            if !found {
                return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                    "semantic disjoint accessor identity",
                ));
            }
        }
    }

    let expected_allocations = root
        .arguments
        .as_slice()
        .iter()
        .enumerate()
        .filter(|(_, argument)| !matches!(argument.kind, DescriptorArgumentKindV1::Scalar(_)))
        .collect::<Vec<_>>();
    if formal.obligations().allocations().len() != expected_allocations.len()
        || !formal.obligations().inter_invocation_conflicts().is_empty()
    {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "closed formal allocation/race obligations",
        ));
    }
    for (index, argument) in expected_allocations {
        let allocation = formal
            .obligations()
            .allocations()
            .iter()
            .find(|allocation| allocation.identity().parameter_index() as usize == index)
            .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
                "formal allocation parameter",
            ))?;
        let expected_access = match argument.kind {
            DescriptorArgumentKindV1::SharedSlice(_) => KirAccessMode::ReadOnly,
            DescriptorArgumentKindV1::DisjointSlice(_) => KirAccessMode::ReadWrite,
            DescriptorArgumentKindV1::GlobalMutPointer(_) => KirAccessMode::ReadWrite,
            DescriptorArgumentKindV1::Scalar(_) => unreachable!(),
        };
        let expected_kind = match argument.kind {
            DescriptorArgumentKindV1::SharedSlice(_)
            | DescriptorArgumentKindV1::DisjointSlice(_) => FormalParameterKind::Slice,
            DescriptorArgumentKindV1::GlobalMutPointer(_) => FormalParameterKind::Pointer,
            DescriptorArgumentKindV1::Scalar(_) => unreachable!(),
        };
        if allocation.value() != body.parameters[index]
            || allocation.kind() != expected_kind
            || allocation.address_space() != AddressSpace::Global
            || allocation.access() != expected_access
        {
            return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "formal allocation ownership",
            ));
        }
    }
    if !formal
        .obligations()
        .runtime_alias_requirements()
        .iter()
        .all(|requirement| {
            rust_ownership_discharges_runtime_alias_v1(
                requirement.left().parameter_index() as usize,
                requirement.right().parameter_index() as usize,
                root.arguments.as_slice(),
            )
        })
    {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "formal alias obligation not discharged by Rust ownership",
        ));
    }
    for access in formal.obligations().accesses() {
        let index = access.allocation().parameter_index() as usize;
        let Some(argument) = root.arguments.as_slice().get(index) else {
            return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "formal access parameter",
            ));
        };
        if access.address_space() != AddressSpace::Global
            || (access.kind() == FormalMemoryAccessKind::Write
                && matches!(argument.kind, DescriptorArgumentKindV1::SharedSlice(_)))
        {
            return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "formal access mode",
            ));
        }
    }
    Ok(geometry)
}

fn production_descriptor_argument_matches_kernel_type_v1(
    descriptor: DescriptorArgumentKindV1,
    kernel_type: &fe2o3_kernel_ir::Type,
) -> bool {
    use fe2o3_kernel_ir::{AccessMode as KirAccessMode, AddressSpace, Type as KirType};

    match (descriptor, kernel_type) {
        (DescriptorArgumentKindV1::Scalar(scalar), KirType::Scalar(actual)) => {
            descriptor_scalar_to_kernel_ir(scalar) == Some(*actual)
        }
        (DescriptorArgumentKindV1::SharedSlice(scalar), KirType::Slice(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.access == KirAccessMode::ReadOnly
                && actual.element.as_scalar() == descriptor_scalar_to_kernel_ir(scalar)
        }
        (DescriptorArgumentKindV1::DisjointSlice(scalar), KirType::Slice(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.access == KirAccessMode::ReadWrite
                && actual.element.as_scalar() == descriptor_scalar_to_kernel_ir(scalar)
        }
        (DescriptorArgumentKindV1::GlobalMutPointer(scalar), KirType::Pointer(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.access == KirAccessMode::ReadWrite
                && actual.pointee.as_scalar() == descriptor_scalar_to_kernel_ir(scalar)
        }
        _ => false,
    }
}

fn require_production_descriptor_argument_semantic_type_v1(
    descriptor: &TypedDescriptorArgumentV1,
    semantic_identity: SemanticTypeIdentityV1,
) -> Result<(), CompilerDescriptorError> {
    if descriptor.semantic_type_identity != semantic_identity {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "rustc semantic argument type identity",
        ));
    }
    Ok(())
}

pub(crate) fn validate_production_v1_semantic_ownership_evidence(
    typed_roots: &[TypedDescriptorRootV1],
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
) -> Result<(), CompilerDescriptorError> {
    let semantic_roots = semantic
        .roots()
        .iter()
        .map(|semantic_root| {
            let function = semantic
                .functions()
                .get(semantic_root.index() as usize)
                .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
                    "semantic root function",
                ))?;
            let entry = function.kernel_entry().ok_or(
                CompilerDescriptorError::ProductionDescriptorMismatch("semantic kernel entry"),
            )?;
            Ok((function, *entry.kernel_binding_identity().as_bytes()))
        })
        .collect::<Result<Vec<_>, CompilerDescriptorError>>()?;
    let typed_bindings = typed_roots
        .iter()
        .map(|root| root.kernel_binding.as_bytes())
        .collect::<Vec<_>>();
    let semantic_bindings = semantic_roots
        .iter()
        .map(|(_, binding)| *binding)
        .collect::<Vec<_>>();
    let matched_semantic_roots = match_exact_production_root_roster_v1(
        typed_bindings.as_slice(),
        semantic_bindings.as_slice(),
    )?;

    for (root, semantic_index) in typed_roots.iter().zip(matched_semantic_roots) {
        validate_production_v1_semantic_root_ownership_evidence(
            root,
            semantic,
            semantic_roots[semantic_index].0,
        )?;
    }
    Ok(())
}

fn match_exact_production_root_roster_v1(
    typed_bindings: &[[u8; 32]],
    semantic_bindings: &[[u8; 32]],
) -> Result<Vec<usize>, CompilerDescriptorError> {
    if typed_bindings.is_empty() || typed_bindings.len() != semantic_bindings.len() {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "complete typed/semantic root roster",
        ));
    }

    let mut semantic_indices = BTreeMap::new();
    for (index, binding) in semantic_bindings.iter().copied().enumerate() {
        if semantic_indices.insert(binding, index).is_some() {
            return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "unique semantic kernel binding roster",
            ));
        }
    }

    let mut matched = Vec::with_capacity(typed_bindings.len());
    for binding in typed_bindings {
        let Some(index) = semantic_indices.remove(binding) else {
            return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "exact typed/semantic kernel binding roster",
            ));
        };
        matched.push(index);
    }
    if !semantic_indices.is_empty() {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "exact typed/semantic kernel binding roster",
        ));
    }
    Ok(matched)
}

fn validate_production_v1_semantic_root_ownership_evidence(
    root: &TypedDescriptorRootV1,
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    function: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1,
) -> Result<(), CompilerDescriptorError> {
    use fe2o3_artifacts::RustcAbiClassV1;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiPassModeV1, SemanticSourceArgumentOwnershipV1,
    };

    let entry =
        function
            .kernel_entry()
            .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
                "semantic kernel entry",
            ))?;
    let export = std::str::from_utf8(entry.export_symbol().as_bytes())
        .map_err(|_| CompilerDescriptorError::ProductionDescriptorMismatch("UTF-8 export"))?;
    if export != root.export_name
        || entry.kernel_binding_identity().as_bytes() != &root.kernel_binding.as_bytes()
        || function.abi().source_input_types().len() != root.arguments.len()
        || function.abi().adjusted_arguments().len() != root.arguments.len()
        || function.abi().source_argument_ownership().len() != root.arguments.len()
    {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "semantic source identity/ABI/ownership closure",
        ));
    }

    for (index, ((argument, semantic_type), semantic_abi)) in root
        .arguments
        .as_slice()
        .iter()
        .zip(function.abi().source_input_types())
        .zip(function.abi().adjusted_arguments())
        .enumerate()
    {
        let semantic_type = semantic.types().get(semantic_type.index() as usize).ok_or(
            CompilerDescriptorError::ProductionDescriptorMismatch("semantic argument type"),
        )?;
        let expected_ownership = match argument.kind {
            DescriptorArgumentKindV1::Scalar(_) => SemanticSourceArgumentOwnershipV1::ByValue,
            DescriptorArgumentKindV1::SharedSlice(_) => {
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            }
            DescriptorArgumentKindV1::DisjointSlice(_) => {
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner
            }
            DescriptorArgumentKindV1::GlobalMutPointer(_) => {
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner
            }
        };
        require_production_descriptor_argument_semantic_type_v1(
            argument,
            semantic_type.identity(),
        )?;
        let exact_abi_mode = matches!(
            (argument.layout.abi_class(), semantic_abi.mode()),
            (RustcAbiClassV1::Scalar, SemanticAbiPassModeV1::Direct(_))
                | (
                    RustcAbiClassV1::ScalarPair,
                    SemanticAbiPassModeV1::Pair { .. }
                )
        );
        if semantic_type.layout().size_bytes() != Some(argument.layout.size())
            || semantic_type.layout().alignment_bytes()
                != u64::from(argument.layout.abi_alignment())
            || semantic_abi.ty() != function.abi().source_input_types()[index]
            || function.abi().source_argument_ownership()[index] != expected_ownership
            || !exact_abi_mode
        {
            return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "rustc semantic argument layout/ownership",
            ));
        }
    }
    Ok(())
}

fn rust_ownership_discharges_runtime_alias_v1(
    left: usize,
    right: usize,
    arguments: &[TypedDescriptorArgumentV1],
) -> bool {
    if left == right {
        return false;
    }
    let Some(left) = arguments.get(left).map(|argument| argument.kind) else {
        return false;
    };
    let Some(right) = arguments.get(right).map(|argument| argument.kind) else {
        return false;
    };
    matches!(left, DescriptorArgumentKindV1::DisjointSlice(_))
        || matches!(left, DescriptorArgumentKindV1::GlobalMutPointer(_))
        || matches!(right, DescriptorArgumentKindV1::DisjointSlice(_))
        || matches!(right, DescriptorArgumentKindV1::GlobalMutPointer(_))
}

fn descriptor_scalar_to_kernel_ir(scalar: ScalarTypeV1) -> Option<fe2o3_kernel_ir::ScalarType> {
    use fe2o3_kernel_ir::ScalarType as KirScalar;
    Some(match scalar {
        ScalarTypeV1::I8 => KirScalar::I8,
        ScalarTypeV1::U8 => KirScalar::U8,
        ScalarTypeV1::I16 => KirScalar::I16,
        ScalarTypeV1::U16 => KirScalar::U16,
        ScalarTypeV1::I32 => KirScalar::I32,
        ScalarTypeV1::U32 => KirScalar::U32,
        ScalarTypeV1::I64 => KirScalar::I64,
        ScalarTypeV1::U64 => KirScalar::U64,
        ScalarTypeV1::F16 => KirScalar::F16,
        ScalarTypeV1::F32 => KirScalar::F32,
        ScalarTypeV1::F64 => KirScalar::F64,
    })
}

struct DescriptorConstructionProfileV1 {
    rank: u8,
    workgroup: [u32; 3],
    max_grid: [u32; 3],
    max_flat_workgroup_size: u32,
    static_shared_memory_bytes: u32,
    allow_exact_tiled_matrix: bool,
    allow_workgroup_memory: bool,
    producer_version: &'static str,
}

fn construct_compiler_descriptor_source_with_profile_v1(
    envelope: &CompilerFfiEnvelopeV1,
    module: &Module,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    profile: DescriptorConstructionProfileV1,
) -> Result<Option<CompilerDescriptorSourceV1>, CompilerDescriptorError> {
    if typed_roots.is_empty() {
        return Ok(None);
    }
    if typed_roots.len() != module.kernels.len() {
        return Err(CompilerDescriptorError::IncompleteTypedKernelClosure {
            typed: typed_roots.len(),
            total: module.kernels.len(),
        });
    }
    if !ADMITTED_PRODUCTION_PROCESSORS.contains(&envelope.target().as_amd_target_id().processor()) {
        return Err(CompilerDescriptorError::UnsupportedTarget(
            envelope.target().to_string(),
        ));
    }
    if envelope.code_object_version() != CodeObjectVersion::V6 {
        return Err(CompilerDescriptorError::UnsupportedCodeObjectVersion(
            envelope.code_object_version(),
        ));
    }

    let descriptor_kinds = typed_roots
        .iter()
        .flat_map(|root| {
            root.arguments
                .as_slice()
                .iter()
                .map(|argument| argument.kind)
        })
        .collect::<BTreeSet<_>>();
    let mut source_types = Vec::with_capacity(descriptor_kinds.len());
    let mut device_layouts = Vec::with_capacity(descriptor_kinds.len());
    let mut descriptor_indexes = BTreeMap::new();
    for kind in descriptor_kinds {
        let index = source_types.len();
        descriptor_indexes.insert(kind, index);
        let (source, layout) = descriptor_records(kind);
        source_types.push(source);
        device_layouts.push(layout);
    }

    let module_capabilities = descriptor_capabilities(
        module,
        profile.allow_exact_tiled_matrix,
        profile.allow_workgroup_memory,
    )?;
    let mut seen_exports = BTreeSet::new();
    let mut kernels = Vec::with_capacity(typed_roots.len());
    for root in typed_roots {
        if !seen_exports.insert(root.export_name.as_str()) {
            return Err(CompilerDescriptorError::DuplicateTypedKernel(
                root.export_name.clone(),
            ));
        }
        let kernel = module
            .kernels
            .iter()
            .find(|kernel| kernel.id.as_str() == root.export_name)
            .ok_or_else(|| CompilerDescriptorError::MissingTypedKernel(root.export_name.clone()))?;
        if kernel.workgroup_size
            != Some(WorkgroupSize::new(
                profile.workgroup[0],
                profile.workgroup[1],
                profile.workgroup[2],
            ))
        {
            return Err(CompilerDescriptorError::UnexpectedWorkgroupSize {
                kernel: root.export_name.clone(),
                expected: profile.workgroup,
            });
        }

        let arguments = root
            .arguments
            .as_slice()
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                let descriptor_index = descriptor_indexes[&argument.kind];
                let source_type = &source_types[descriptor_index];
                let device_layout = &device_layouts[descriptor_index];
                let source_index = u16::try_from(index).map_err(|_| {
                    CompilerDescriptorError::ArgumentIndexOverflow {
                        kernel: root.export_name.clone(),
                        index,
                    }
                })?;
                let name = ValidName::new(argument.name.clone())?;
                match argument.kind {
                    DescriptorArgumentKindV1::Scalar(_) => LogicalArgumentV1::scalar(
                        source_index,
                        name,
                        source_type,
                        device_layout,
                        argument.offset,
                    ),
                    DescriptorArgumentKindV1::SharedSlice(_) => LogicalArgumentV1::shared_slice(
                        source_index,
                        name,
                        source_type,
                        device_layout,
                        argument.offset,
                    ),
                    DescriptorArgumentKindV1::DisjointSlice(_) => {
                        LogicalArgumentV1::disjoint_slice(
                            source_index,
                            name,
                            source_type,
                            device_layout,
                            argument.access,
                            argument.offset,
                        )
                    }
                    DescriptorArgumentKindV1::GlobalMutPointer(_) => {
                        LogicalArgumentV1::global_mut_pointer(
                            source_index,
                            name,
                            source_type,
                            device_layout,
                            argument.offset,
                        )
                    }
                }
                .map_err(CompilerDescriptorError::Validation)
            })
            .collect::<Result<Vec<_>, CompilerDescriptorError>>()?;
        let source_evidence = source_evidence(root);
        let ir_evidence = ir_evidence(envelope, compiler_module, root);
        let kernarg_segment_bytes =
            root.explicit_argument_bytes
                .checked_add(256)
                .ok_or_else(|| {
                    CompilerDescriptorError::KernargSizeOverflow(root.export_name.clone())
                })?;
        kernels.push(KernelDescriptorV1::new(
            KernelId::from_bytes(root.kernel_binding.as_bytes()),
            ValidName::new(root.logical_name.clone())?,
            ValidName::new(root.export_name.clone())?,
            ValidName::new(format!("{}.kd", root.export_name))?,
            source_evidence,
            ir_evidence,
            module_capabilities.clone(),
            KernelAbiLayoutV1::new(
                root.explicit_argument_bytes,
                kernarg_segment_bytes,
                root.kernarg_alignment_bytes,
            )?,
            LaunchConstraintsV1::new(
                profile.rank,
                BlockSizeV1::Exact(DimensionsV1::new(
                    profile.workgroup[0],
                    profile.workgroup[1],
                    profile.workgroup[2],
                )?),
                DimensionsV1::new(
                    profile.max_grid[0],
                    profile.max_grid[1],
                    profile.max_grid[2],
                )?,
                profile.max_flat_workgroup_size,
                profile.static_shared_memory_bytes,
                0,
            )?,
            arguments,
        )?);
    }

    let producer_version = profile.producer_version;
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            Text::new(RUSTC_CODEGEN_FE2O3_COMPILER_NAME_V1)?,
            Text::new(env!("CARGO_PKG_VERSION"))?,
            [0; 20],
        ),
        ProducerIdentityV1::new(
            Text::new(RUSTC_CODEGEN_FE2O3_PRODUCTION_V3_PRODUCER_NAME_V1)?,
            Text::new(producer_version)?,
        ),
        envelope.target(),
        source_types,
        device_layouts,
        kernels,
    )?;
    CompilerDescriptorSourceV1::new(table)
        .map(Some)
        .map_err(CompilerDescriptorError::Source)
}

fn descriptor_records(
    kind: DescriptorArgumentKindV1,
) -> (SourceTypeRecordV1, DeviceLayoutRecordV1) {
    match kind {
        DescriptorArgumentKindV1::Scalar(scalar) => (
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(scalar)),
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(scalar)),
        ),
        DescriptorArgumentKindV1::SharedSlice(scalar) => (
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(scalar)),
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(scalar)),
        ),
        DescriptorArgumentKindV1::DisjointSlice(scalar) => (
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(scalar)),
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(scalar)),
        ),
        DescriptorArgumentKindV1::GlobalMutPointer(scalar) => (
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::global_mut_pointer(scalar)),
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::global_mut_pointer(scalar)),
        ),
    }
}

fn descriptor_capabilities(
    module: &Module,
    allow_exact_tiled_matrix: bool,
    allow_workgroup_memory: bool,
) -> Result<Vec<CapabilityV1>, CompilerDescriptorError> {
    let mut result = BTreeSet::new();
    let mut effective = module.effective_capabilities();
    effective.extend(
        module
            .kernels
            .iter()
            .flat_map(|kernel| kernel.required_capabilities.iter().cloned()),
    );
    effective.extend(
        module
            .functions
            .iter()
            .flat_map(|function| function.effective_capabilities()),
    );
    let has_exact_diagnostic_target = effective.iter().any(|capability| {
        matches!(
            capability,
            TargetCapability::Extension { namespace, name }
                if namespace == fe2o3_kernel_ir::AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
                    && matches!(
                        name.as_str(),
                        fe2o3_kernel_ir::AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
                            | fe2o3_kernel_ir::AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME
                    )
        )
    });
    for capability in effective {
        match capability {
            TargetCapability::Int64 => {}
            TargetCapability::Extension { namespace, name }
                if namespace == fe2o3_kernel_ir::AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
                    && matches!(
                        name.as_str(),
                        fe2o3_kernel_ir::AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
                            | fe2o3_kernel_ir::AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME
                    ) =>
            {
                // Exact target binding is represented by the descriptor table's
                // device target, not as an executable kernel capability.
            }
            TargetCapability::Extension { namespace, name }
                if namespace == fe2o3_kernel_ir::AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE
                    && name == fe2o3_kernel_ir::AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME =>
            {
                // The closed diagnostic contract is authenticated by Kernel IR
                // and target lowering. Descriptor V1 has no diagnostic tag.
            }
            TargetCapability::Subgroups | TargetCapability::SubgroupSize(64) => {
                result.insert(CapabilityV1::Subgroup);
                result.insert(CapabilityV1::AmdWave);
            }
            TargetCapability::WaveWidth(WaveWidth::Wave64) => {
                result.insert(CapabilityV1::AmdWave);
            }
            TargetCapability::WorkgroupMemory | TargetCapability::WorkgroupBarrier
                if allow_workgroup_memory =>
            {
                result.insert(CapabilityV1::WorkgroupMemory);
            }
            TargetCapability::Atomic { .. } => {
                result.insert(CapabilityV1::Atomics);
            }
            TargetCapability::BFloat16 => {
                // Descriptor construction has already admitted an exact gfx942
                // or gfx950 target. BF16 remains bound in retained Kernel IR and
                // executable evidence; descriptor V1 has no scalar-format tag.
            }
            TargetCapability::Extension { namespace, name }
                if allow_exact_tiled_matrix
                    && namespace == MATRIX_CAPABILITY_NAMESPACE
                    && matches!(
                        name.as_str(),
                        BF16_F32_M16N16K16_CAPABILITY
                            | SCALED_FP4_E2M1_F32_M16N16K128_CAPABILITY
                            | SCALED_FP8_E4M3_F32_M16N16K128_CAPABILITY
                            | SCALED_FP4_E2M1_FP8_E4M3_F32_M16N16K128_CAPABILITY
                    ) =>
            {
                result.insert(CapabilityV1::MatrixMultiply);
                result.insert(CapabilityV1::AmdMfma);
            }
            TargetCapability::Extension { namespace, name }
                if allow_workgroup_memory
                    && namespace == MATRIX_CAPABILITY_NAMESPACE
                    && name == fe2o3_kernel_ir::LDS_TILE_16X16_XOR4_CAPABILITY =>
            {
                result.insert(CapabilityV1::WorkgroupMemory);
            }
            TargetCapability::Extension { namespace, name }
                if has_exact_diagnostic_target
                    && namespace == AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
                    && name == AMDGPU_DIAGNOSTICS_CAPABILITY_NAME =>
            {
                // Diagnostics are lowered by the exact target backend and do
                // not add a kernel descriptor launch or ABI capability.
            }
            unsupported => {
                return Err(CompilerDescriptorError::UnsupportedCapability(format!(
                    "{unsupported:?}"
                )));
            }
        }
    }
    Ok(result.into_iter().collect())
}

fn source_evidence(root: &TypedDescriptorRootV1) -> BuildEvidenceV1 {
    let binding = root.kernel_binding.as_bytes();
    let mut identity_frames = vec![
        binding.as_slice(),
        root.logical_name.as_bytes(),
        root.export_name.as_bytes(),
    ];
    let identity_bytes = root
        .arguments
        .as_slice()
        .iter()
        .map(|argument| type_identity_bytes(argument.layout.type_identity()))
        .collect::<Vec<_>>();
    for bytes in &identity_bytes {
        identity_frames.push(bytes.as_slice());
    }
    let canonical_layouts = root
        .arguments
        .as_slice()
        .iter()
        .map(|argument| argument.layout.canonical_bytes())
        .collect::<Vec<_>>();
    let digest_frames = canonical_layouts
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(domain_hash(
            SOURCE_IDENTITY_DOMAIN_V1,
            &identity_frames,
        )),
        EvidenceDigest::from_sha256_bytes(domain_hash(SOURCE_DIGEST_DOMAIN_V1, &digest_frames)),
    )
}

fn ir_evidence(
    envelope: &CompilerFfiEnvelopeV1,
    module: &InertCompilerModuleTextV1,
    root: &TypedDescriptorRootV1,
) -> BuildEvidenceV1 {
    let binding = root.kernel_binding.as_bytes();
    let envelope_identity = envelope.identity().as_bytes();
    let target = envelope.target().to_string();
    let identity = domain_hash(
        IR_IDENTITY_DOMAIN_V1,
        &[
            binding.as_slice(),
            envelope_identity.as_slice(),
            target.as_bytes(),
            root.export_name.as_bytes(),
        ],
    );
    let digest = domain_hash(
        IR_DIGEST_DOMAIN_V1,
        &[
            envelope.canonical_bytes(),
            module.llvm_ir().as_bytes(),
            root.export_name.as_bytes(),
        ],
    );
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(identity),
        EvidenceDigest::from_sha256_bytes(digest),
    )
}

fn type_identity_bytes(identity: TypeIdentity) -> [u8; 64] {
    let mut bytes = [0_u8; 64];
    bytes[..32].copy_from_slice(identity.rust_type().bytes().as_bytes());
    bytes[32..].copy_from_slice(identity.layout().bytes().as_bytes());
    bytes
}

fn domain_hash(domain: &[u8], frames: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((frames.len() as u64).to_le_bytes());
    for frame in frames {
        hasher.update((frame.len() as u64).to_le_bytes());
        hasher.update(frame);
    }
    hasher.finalize().into()
}

#[derive(Debug)]
pub(crate) enum CompilerDescriptorError {
    TypedProfileOnNonKernel(String),
    MissingTypedField { kernel: String, field: &'static str },
    InvalidArgumentCollection { kernel: String, reason: String },
    GeneralRustLayout(GeneralTypedExtractError),
    GeneratedHostContractMismatch(String),
    ArgumentOffsetOverflow { kernel: String, index: usize },
    ArgumentIndexOverflow { kernel: String, index: usize },
    ExplicitArgumentSizeOverflow(String),
    KernargSizeOverflow(String),
    IncompleteTypedKernelClosure { typed: usize, total: usize },
    UnsupportedTarget(String),
    UnsupportedCodeObjectVersion(CodeObjectVersion),
    DuplicateTypedKernel(String),
    MissingTypedKernel(String),
    UnexpectedWorkgroupSize { kernel: String, expected: [u32; 3] },
    ProductionFormalMemory(fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1),
    ProductionGeometry(crate::production_geometry_v1::ProductionGeometryErrorV1),
    ProductionDescriptorMismatch(&'static str),
    UnsupportedCapability(String),
    Validation(ValidationError),
    Source(CompilerDescriptorSourceErrorV1),
}

impl From<ValidationError> for CompilerDescriptorError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}

impl fmt::Display for CompilerDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypedProfileOnNonKernel(kernel) => {
                write!(
                    formatter,
                    "typed descriptor profile is attached to non-kernel `{kernel}`"
                )
            }
            Self::MissingTypedField { kernel, field } => {
                write!(formatter, "typed kernel `{kernel}` has no {field}")
            }
            Self::InvalidArgumentCollection { kernel, reason } => {
                write!(
                    formatter,
                    "typed kernel `{kernel}` has invalid arguments: {reason}"
                )
            }
            Self::GeneralRustLayout(error) => {
                write!(formatter, "general rustc layout extraction failed: {error}")
            }
            Self::GeneratedHostContractMismatch(kernel) => write!(
                formatter,
                "typed kernel `{kernel}` generated host-contract identity does not match fresh rustc evidence"
            ),
            Self::ArgumentOffsetOverflow { kernel, index } => write!(
                formatter,
                "typed kernel `{kernel}` argument {index} offset exceeds u32"
            ),
            Self::ArgumentIndexOverflow { kernel, index } => write!(
                formatter,
                "typed kernel `{kernel}` argument index {index} exceeds u16"
            ),
            Self::ExplicitArgumentSizeOverflow(kernel) => write!(
                formatter,
                "typed kernel `{kernel}` explicit argument size exceeds u32"
            ),
            Self::KernargSizeOverflow(kernel) => write!(
                formatter,
                "typed kernel `{kernel}` COV6 kernarg size overflows u32"
            ),
            Self::IncompleteTypedKernelClosure { typed, total } => write!(
                formatter,
                "typed descriptor closure has {typed} typed kernel(s) for {total} module kernel(s)"
            ),
            Self::UnsupportedTarget(target) => {
                write!(
                    formatter,
                    "typed descriptor source currently requires gfx942 or gfx950, found {target}"
                )
            }
            Self::UnsupportedCodeObjectVersion(version) => write!(
                formatter,
                "typed descriptor source currently requires code object V6, found {version:?}"
            ),
            Self::DuplicateTypedKernel(kernel) => {
                write!(formatter, "duplicate typed descriptor kernel `{kernel}`")
            }
            Self::MissingTypedKernel(kernel) => {
                write!(
                    formatter,
                    "typed descriptor kernel `{kernel}` is absent from kernel IR"
                )
            }
            Self::UnexpectedWorkgroupSize { kernel, expected } => write!(
                formatter,
                "typed descriptor kernel `{kernel}` does not have the exact {expected:?} workgroup"
            ),
            Self::ProductionFormalMemory(error) => {
                write!(
                    formatter,
                    "production formal-memory evidence failed: {error}"
                )
            }
            Self::ProductionGeometry(error) => {
                write!(formatter, "production geometry evidence failed: {error}")
            }
            Self::ProductionDescriptorMismatch(field) => write!(
                formatter,
                "production descriptor evidence has an internal {field} mismatch"
            ),
            Self::UnsupportedCapability(capability) => write!(
                formatter,
                "typed descriptor cannot represent capability {capability}"
            ),
            Self::Validation(error) => write!(formatter, "invalid typed descriptor: {error}"),
            Self::Source(error) => write!(formatter, "invalid compiler descriptor source: {error}"),
        }
    }
}

impl std::error::Error for CompilerDescriptorError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel_ir_codegen::{
        bind_compiler_descriptor_source_v1, construct_inert_compiler_module_text_v1,
    };
    use fe2o3_artifacts::{
        PointerWidth, RustDisjointIndexSpaceV1, RustPhysicalComponentKindV1,
        RustPhysicalComponentV1, RustPointerMutabilityV1, RustScalarElementTypeV1,
        RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
    };
    use fe2o3_compiler_ffi::{
        CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1,
        CompilerFfiSourceOwnerV1, DeviceTargetV1,
    };
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Signature, Terminator,
    };
    use reserved_fe2o3_symbols::{
        DeviceFfiContractFieldsV1, DeviceFfiDirectionV1, derive_device_ffi_contract_id_v1,
    };

    const TEST_ABI: &str = "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]";

    fn envelope(version: CodeObjectVersion) -> CompilerFfiEnvelopeV1 {
        let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
        let version_number = match version {
            CodeObjectVersion::V4 => 4,
            CodeObjectVersion::V5 => 5,
            CodeObjectVersion::V6 => 6,
        };
        let semantic_identity = [0x55; 32];
        let semantic_text = semantic_identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let id = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: DeviceFfiDirectionV1::Import.tag(),
            symbol: "external_test",
            calling_convention: "C",
            code_object_version: version_number,
            target: "gfx942:xnack-",
            physical_abi: TEST_ABI,
            effects: "read_global",
            semantic_identity: &semantic_text,
        });
        let contract = CompilerFfiContractV1::new(
            id,
            DeviceFfiDirectionV1::Import,
            CompilerFfiLinkRoleV1::RequiresExternalDefinition,
            target,
            version,
            CompilerFfiSourceOwnerV1::new(
                "descriptor_test",
                "descriptor_test::external_test",
                [0x44; 16],
                "_RNvCs1234_descriptor_test13external_test",
            )
            .unwrap(),
            "external_test",
            TEST_ABI,
            "read_global",
            semantic_identity,
        )
        .unwrap();
        let mut builder = CompilerFfiEnvelopeBuilderV1::new(target, version, 1).unwrap();
        builder.push(contract).unwrap();
        builder.finish().unwrap()
    }

    fn layout(disjoint: bool) -> RustLayoutEvidenceV1 {
        let (shape, mutability) = if disjoint {
            (
                RustSourceTypeShapeV1::disjoint_slice(
                    RustScalarElementTypeV1::F32,
                    RustDisjointIndexSpaceV1::Index1D,
                ),
                RustPointerMutabilityV1::Mut,
            )
        } else {
            (
                RustSourceTypeShapeV1::shared_slice(RustScalarElementTypeV1::F32),
                RustPointerMutabilityV1::Const,
            )
        };
        RustLayoutEvidenceV1::new(
            RustTypeEvidenceV1::new(shape),
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            vec![
                RustPhysicalComponentV1::new(
                    0,
                    8,
                    8,
                    RustPhysicalComponentKindV1::Pointer {
                        mutability,
                        pointee: RustScalarElementTypeV1::F32,
                    },
                )
                .unwrap(),
                RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
            ],
        )
        .unwrap()
    }

    fn root_with_layouts(binding: u8, layouts: Vec<RustLayoutEvidenceV1>) -> TypedDescriptorRootV1 {
        let arguments = layouts
            .into_iter()
            .enumerate()
            .map(|(index, layout)| {
                let disjoint = matches!(
                    layout.rust_type().source_type(),
                    RustSourceTypeShapeV1::DisjointSlice { .. }
                );
                TypedDescriptorArgumentV1 {
                    name: format!("arg{index}"),
                    kind: if disjoint {
                        DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32)
                    } else {
                        DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32)
                    },
                    access: if disjoint {
                        AccessMode::WriteOnly
                    } else {
                        AccessMode::ReadOnly
                    },
                    offset: u32::try_from(index * 16).unwrap(),
                    layout,
                    semantic_type_identity: SemanticTypeIdentityV1::from_sha256(
                        [u8::try_from(index).unwrap(); 32],
                    ),
                }
            })
            .collect::<Vec<_>>();
        let explicit_argument_bytes = u32::try_from(arguments.len() * 16).unwrap();
        TypedDescriptorRootV1 {
            logical_name: "fixture".to_owned(),
            export_name: "kernel".to_owned(),
            kernel_binding: KernelBindingIdV1::from_bytes([binding; 32]),
            arguments: TypedArgumentListV1::new(arguments).unwrap(),
            explicit_argument_bytes,
            kernarg_alignment_bytes: 8,
            source_launch: None,
        }
    }

    fn root(binding: u8) -> TypedDescriptorRootV1 {
        root_with_layouts(binding, vec![layout(false), layout(false), layout(true)])
    }

    fn scalar_layout() -> RustLayoutEvidenceV1 {
        RustLayoutEvidenceV1::new(
            RustTypeEvidenceV1::new(RustSourceTypeShapeV1::scalar(RustScalarElementTypeV1::F32)),
            RustcAbiClassV1::Scalar,
            PointerWidth::Bits64,
            4,
            4,
            vec![
                RustPhysicalComponentV1::new(
                    0,
                    4,
                    4,
                    RustPhysicalComponentKindV1::Scalar {
                        scalar: RustScalarElementTypeV1::F32,
                    },
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn global_mut_pointer_layout() -> RustLayoutEvidenceV1 {
        RustLayoutEvidenceV1::new(
            RustTypeEvidenceV1::new(RustSourceTypeShapeV1::global_mut_pointer(
                RustScalarElementTypeV1::F32,
            )),
            RustcAbiClassV1::Scalar,
            PointerWidth::Bits64,
            8,
            8,
            vec![
                RustPhysicalComponentV1::new(
                    0,
                    8,
                    8,
                    RustPhysicalComponentKindV1::Pointer {
                        mutability: RustPointerMutabilityV1::Mut,
                        pointee: RustScalarElementTypeV1::F32,
                    },
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn descriptor_argument(
        index: usize,
        kind: DescriptorArgumentKindV1,
        offset: u32,
    ) -> TypedDescriptorArgumentV1 {
        let (layout, access) = match kind {
            DescriptorArgumentKindV1::Scalar(_) => (scalar_layout(), AccessMode::ByValue),
            DescriptorArgumentKindV1::SharedSlice(_) => (layout(false), AccessMode::ReadOnly),
            DescriptorArgumentKindV1::DisjointSlice(_) => (layout(true), AccessMode::ReadWrite),
            DescriptorArgumentKindV1::GlobalMutPointer(_) => {
                (global_mut_pointer_layout(), AccessMode::ReadWrite)
            }
        };
        TypedDescriptorArgumentV1 {
            name: format!("arg{index}"),
            kind,
            access,
            offset,
            layout,
            semantic_type_identity: SemanticTypeIdentityV1::from_sha256(
                [u8::try_from(index).unwrap(); 32],
            ),
        }
    }

    fn named_descriptor_argument(
        name: &str,
        index: usize,
        kind: DescriptorArgumentKindV1,
        offset: u32,
    ) -> TypedDescriptorArgumentV1 {
        let mut argument = descriptor_argument(index, kind, offset);
        argument.name = name.to_owned();
        argument
    }

    fn general_root(
        logical_name: &str,
        binding: u8,
        explicit_argument_bytes: u32,
        arguments: Vec<TypedDescriptorArgumentV1>,
    ) -> TypedDescriptorRootV1 {
        TypedDescriptorRootV1 {
            logical_name: logical_name.to_owned(),
            export_name: logical_name.to_owned(),
            kernel_binding: KernelBindingIdV1::from_bytes([binding; 32]),
            arguments: TypedArgumentListV1::new(arguments).unwrap(),
            explicit_argument_bytes,
            kernarg_alignment_bytes: 8,
            source_launch: None,
        }
    }

    fn alpha_root() -> TypedDescriptorRootV1 {
        general_root(
            "alpha",
            0x61,
            40,
            vec![
                named_descriptor_argument(
                    "scale",
                    0,
                    DescriptorArgumentKindV1::Scalar(ScalarTypeV1::F32),
                    0,
                ),
                named_descriptor_argument(
                    "input",
                    1,
                    DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                    8,
                ),
                named_descriptor_argument(
                    "output",
                    2,
                    DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32),
                    24,
                ),
            ],
        )
    }

    fn zeta_root() -> TypedDescriptorRootV1 {
        general_root(
            "zeta",
            0x7a,
            56,
            vec![
                named_descriptor_argument(
                    "a",
                    0,
                    DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                    0,
                ),
                named_descriptor_argument(
                    "b",
                    1,
                    DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                    16,
                ),
                named_descriptor_argument(
                    "bias",
                    2,
                    DescriptorArgumentKindV1::Scalar(ScalarTypeV1::F32),
                    32,
                ),
                named_descriptor_argument(
                    "output",
                    3,
                    DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32),
                    40,
                ),
            ],
        )
    }

    #[test]
    fn rust_ownership_only_discharges_aliases_involving_exclusive_arguments() {
        let arguments = vec![
            descriptor_argument(
                0,
                DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                0,
            ),
            descriptor_argument(
                1,
                DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                16,
            ),
            descriptor_argument(
                2,
                DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32),
                32,
            ),
            descriptor_argument(3, DescriptorArgumentKindV1::Scalar(ScalarTypeV1::F32), 48),
            descriptor_argument(
                4,
                DescriptorArgumentKindV1::GlobalMutPointer(ScalarTypeV1::F32),
                56,
            ),
        ];

        assert!(!rust_ownership_discharges_runtime_alias_v1(
            0, 1, &arguments
        ));
        assert!(rust_ownership_discharges_runtime_alias_v1(0, 2, &arguments));
        assert!(rust_ownership_discharges_runtime_alias_v1(2, 1, &arguments));
        assert!(!rust_ownership_discharges_runtime_alias_v1(
            0, 3, &arguments
        ));
        assert!(rust_ownership_discharges_runtime_alias_v1(0, 4, &arguments));
        assert!(rust_ownership_discharges_runtime_alias_v1(4, 1, &arguments));
        assert!(!rust_ownership_discharges_runtime_alias_v1(
            2, 2, &arguments
        ));
        assert!(!rust_ownership_discharges_runtime_alias_v1(
            4, 4, &arguments
        ));
        assert!(!rust_ownership_discharges_runtime_alias_v1(
            2, 8, &arguments
        ));
    }

    #[test]
    fn production_descriptor_requires_exact_global_mut_pointer_kernel_type() {
        use fe2o3_kernel_ir::{
            AccessMode as KirAccessMode, AddressSpace, ScalarType as KirScalar, Type as KirType,
        };

        let descriptor = DescriptorArgumentKindV1::GlobalMutPointer(ScalarTypeV1::U32);
        let exact = KirType::pointer(
            KirType::Scalar(KirScalar::U32),
            AddressSpace::Global,
            KirAccessMode::ReadWrite,
        );
        assert!(production_descriptor_argument_matches_kernel_type_v1(
            descriptor, &exact
        ));

        for hostile in [
            KirType::pointer(
                KirType::Scalar(KirScalar::I32),
                AddressSpace::Global,
                KirAccessMode::ReadWrite,
            ),
            KirType::pointer(
                KirType::Scalar(KirScalar::U32),
                AddressSpace::Workgroup,
                KirAccessMode::ReadWrite,
            ),
            KirType::pointer(
                KirType::Scalar(KirScalar::U32),
                AddressSpace::Global,
                KirAccessMode::ReadOnly,
            ),
            KirType::slice(
                KirType::Scalar(KirScalar::U32),
                AddressSpace::Global,
                KirAccessMode::ReadWrite,
            ),
        ] {
            assert!(!production_descriptor_argument_matches_kernel_type_v1(
                descriptor, &hostile
            ));
        }
    }

    fn module() -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = Function::kernel_entry(
            "kernel_impl",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        );
        let mut kernel = Kernel::new(
            "kernel",
            "kernel_impl",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(256, 1, 1));
        let mut module = Module::new("typed_descriptor_test");
        module.functions.push(entry);
        module.kernels.push(kernel);
        module
            .required_capabilities
            .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
        module
    }

    #[test]
    fn descriptor_admits_target_neutral_diagnostics_only_with_exact_target_binding() {
        let diagnostic = TargetCapability::Extension {
            namespace: AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
            name: AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
        };
        let mut unbound = Module::new("diagnostic-without-target");
        unbound.required_capabilities.insert(diagnostic.clone());
        assert!(matches!(
            descriptor_capabilities(&unbound, false, false),
            Err(CompilerDescriptorError::UnsupportedCapability(_))
        ));

        for target in [
            fe2o3_kernel_ir::AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
            fe2o3_kernel_ir::AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME,
        ] {
            let mut bound = unbound.clone();
            bound
                .required_capabilities
                .insert(TargetCapability::Extension {
                    namespace: fe2o3_kernel_ir::AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
                    name: target.to_owned(),
                });
            assert_eq!(descriptor_capabilities(&bound, false, false).unwrap(), []);
        }
    }

    fn module_for(exports: &[&str]) -> Module {
        let mut module = Module::new("typed_descriptor_test");
        for export in exports {
            let implementation = format!("{export}_impl");
            let mut block = BasicBlock::new(BlockId(0));
            block.terminator = Some(Terminator::Return { values: vec![] });
            module.functions.push(Function::kernel_entry(
                implementation.clone(),
                Signature::new(vec![], vec![]),
                vec![],
                vec![block],
            ));
            let mut kernel = Kernel::new(
                *export,
                implementation,
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            );
            kernel.workgroup_size = Some(WorkgroupSize::new(256, 1, 1));
            module.kernels.push(kernel);
        }
        module
            .required_capabilities
            .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
        module
    }

    #[test]
    fn constructs_workload_neutral_gfx942_descriptor() {
        let envelope = envelope(CodeObjectVersion::V6);
        let module = module();
        let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
        let source =
            construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[root(0x42)])
                .unwrap()
                .unwrap();
        let table = source.table();
        assert_eq!(table.code_object_version(), CodeObjectVersion::V6);
        assert_eq!(table.device_target(), envelope.target());
        assert_eq!(table.canonical_code_object_digest().as_bytes(), &[0; 32]);
        assert_eq!(table.type_records().len(), 2);
        assert_eq!(table.layout_records().len(), 2);
        assert_eq!(table.kernels().len(), 1);
        let kernel = &table.kernels()[0];
        assert_eq!(kernel.kernel_id().as_bytes(), &[0x42; 32]);
        assert_eq!(kernel.logical_name().as_str(), "fixture");
        assert_eq!(kernel.entry_name().as_str(), "kernel");
        assert_eq!(kernel.descriptor_symbol().as_str(), "kernel.kd");
        assert_eq!(kernel.abi_layout().explicit_argument_size(), 48);
        assert_eq!(kernel.abi_layout().kernarg_segment_size(), 304);
        assert_eq!(kernel.abi_layout().kernarg_segment_alignment(), 8);
        assert_eq!(kernel.arguments().len(), 3);
        assert_eq!(kernel.arguments()[2].access(), AccessMode::WriteOnly);
        assert_eq!(kernel.capabilities(), &[CapabilityV1::AmdWave]);
        assert!(!source.authenticates_compiler_origin());
        assert!(!source.grants_link_authority());
        assert!(!source.grants_load_authority());
        assert!(!source.grants_launch_authority());

        let source_identity = source.identity();
        let bound = bind_compiler_descriptor_source_v1(llvm, &source).unwrap();
        assert_eq!(bound.descriptor_source_identity(), Some(source_identity));
        assert!(bound.llvm_ir().contains(".section .fe2o3.kd.v1"));
    }

    #[test]
    fn profile_constructor_preserves_rank_xyz_grid_and_flat_geometry() {
        for (rank, workgroup, max_grid, domain) in [
            (
                1,
                [128, 1, 1],
                [17, 1, 1],
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            ),
            (
                2,
                [16, 16, 1],
                [17, 19, 1],
                LaunchDomain::D2 {
                    x: LaunchExtent::Dynamic,
                    y: LaunchExtent::Dynamic,
                },
            ),
            (
                3,
                [4, 4, 4],
                [17, 19, 23],
                LaunchDomain::D3 {
                    x: LaunchExtent::Dynamic,
                    y: LaunchExtent::Dynamic,
                    z: LaunchExtent::Dynamic,
                },
            ),
        ] {
            let envelope = envelope(CodeObjectVersion::V6);
            let mut module = module();
            module.kernels[0].domain = domain;
            module.kernels[0].workgroup_size =
                Some(WorkgroupSize::new(workgroup[0], workgroup[1], workgroup[2]));
            let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
            let flat = workgroup.into_iter().product::<u32>();
            let source = construct_compiler_descriptor_source_with_profile_v1(
                &envelope,
                &module,
                &llvm,
                &[root(0x42)],
                DescriptorConstructionProfileV1 {
                    rank,
                    workgroup,
                    max_grid,
                    max_flat_workgroup_size: flat,
                    static_shared_memory_bytes: 0,
                    allow_exact_tiled_matrix: false,
                    allow_workgroup_memory: false,
                    producer_version: "geometry-test-v1",
                },
            )
            .unwrap()
            .unwrap();
            let launch = source.table().kernels()[0].launch();
            assert_eq!(launch.rank(), rank);
            let BlockSizeV1::Exact(block) = launch.block_size() else {
                panic!("production profile must emit an exact block");
            };
            assert_eq!([block.x(), block.y(), block.z()], workgroup);
            let grid = launch.max_grid();
            assert_eq!([grid.x(), grid.y(), grid.z()], max_grid);
            assert_eq!(launch.max_flat_workgroup_size(), flat);
        }
    }

    #[test]
    fn constructs_two_differing_generic_v3_descriptors() {
        let envelope = envelope(CodeObjectVersion::V6);
        let module = module_for(&["alpha", "zeta"]);
        let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
        let source = construct_compiler_descriptor_source_v1(
            &envelope,
            &module,
            &llvm,
            &[alpha_root(), zeta_root()],
        )
        .unwrap()
        .unwrap();
        let kernels = source.table().kernels();
        assert_eq!(kernels.len(), 2);
        assert_eq!(kernels[0].entry_name().as_str(), "alpha");
        assert_eq!(kernels[0].abi_layout().explicit_argument_size(), 40);
        assert_eq!(kernels[0].abi_layout().kernarg_segment_size(), 296);
        assert_eq!(kernels[0].arguments().len(), 3);
        assert_eq!(
            kernels[0]
                .arguments()
                .iter()
                .map(|argument| argument.name().as_str())
                .collect::<Vec<_>>(),
            ["scale", "input", "output"]
        );
        assert_eq!(
            kernels[0]
                .arguments()
                .iter()
                .map(|argument| {
                    argument
                        .physical_components()
                        .next()
                        .expect("argument has a physical component")
                        .1
                })
                .collect::<Vec<_>>(),
            [0, 8, 24]
        );
        assert_eq!(kernels[0].arguments()[0].access(), AccessMode::ByValue);
        assert_eq!(kernels[0].arguments()[2].access(), AccessMode::ReadWrite);
        assert_eq!(kernels[1].entry_name().as_str(), "zeta");
        assert_eq!(kernels[1].abi_layout().explicit_argument_size(), 56);
        assert_eq!(kernels[1].abi_layout().kernarg_segment_size(), 312);
        assert_eq!(kernels[1].arguments().len(), 4);
        assert_eq!(
            kernels[1]
                .arguments()
                .iter()
                .map(|argument| argument.name().as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "bias", "output"]
        );
        assert_eq!(
            kernels[1]
                .arguments()
                .iter()
                .map(|argument| {
                    argument
                        .physical_components()
                        .next()
                        .expect("argument has a physical component")
                        .1
                })
                .collect::<Vec<_>>(),
            [0, 16, 32, 40]
        );
        assert_eq!(kernels[1].arguments()[2].access(), AccessMode::ByValue);
        assert_eq!(kernels[1].arguments()[3].access(), AccessMode::ReadWrite);
    }

    #[test]
    fn general_v3_contract_field_names_are_identity_bound_and_lookalikes_stay_positional() {
        let positional_alpha_arguments = || {
            vec![
                descriptor_argument(0, DescriptorArgumentKindV1::Scalar(ScalarTypeV1::F32), 0),
                descriptor_argument(
                    1,
                    DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                    8,
                ),
                descriptor_argument(
                    2,
                    DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32),
                    24,
                ),
            ]
        };
        let envelope = envelope(CodeObjectVersion::V6);
        let alpha_module = module_for(&["alpha"]);
        let alpha_llvm = construct_inert_compiler_module_text_v1(&alpha_module).unwrap();
        let exact = construct_compiler_descriptor_source_v1(
            &envelope,
            &alpha_module,
            &alpha_llvm,
            &[alpha_root()],
        )
        .unwrap()
        .unwrap();
        let positional = construct_compiler_descriptor_source_v1(
            &envelope,
            &alpha_module,
            &alpha_llvm,
            &[general_root(
                "alpha",
                0x61,
                40,
                positional_alpha_arguments(),
            )],
        )
        .unwrap()
        .unwrap();
        assert_ne!(exact.identity(), positional.identity());

        let lookalike_module = module_for(&["alpha_lookalike"]);
        let lookalike_llvm = construct_inert_compiler_module_text_v1(&lookalike_module).unwrap();
        let lookalike = construct_compiler_descriptor_source_v1(
            &envelope,
            &lookalike_module,
            &lookalike_llvm,
            &[general_root(
                "alpha_lookalike",
                0x62,
                40,
                positional_alpha_arguments(),
            )],
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            lookalike.table().kernels()[0]
                .arguments()
                .iter()
                .map(|argument| argument.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2"]
        );
    }

    #[test]
    fn synthetic_generic_roots_retain_distinct_argument_counts() {
        let envelope = envelope(CodeObjectVersion::V6);
        let module = module();
        let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
        for (root, expected) in [
            (root_with_layouts(1, vec![layout(false)]), 1),
            (root_with_layouts(2, vec![layout(false), layout(true)]), 2),
        ] {
            let source =
                construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[root])
                    .unwrap()
                    .unwrap();
            assert_eq!(source.table().kernels()[0].arguments().len(), expected);
        }
    }

    #[test]
    fn rejects_partial_closures_wrong_cov_and_unrepresentable_capabilities() {
        let mut two_kernels = module();
        let llvm = construct_inert_compiler_module_text_v1(&two_kernels).unwrap();
        let mut second = two_kernels.kernels[0].clone();
        second.id = "other".into();
        two_kernels.kernels.push(second);
        assert!(matches!(
            construct_compiler_descriptor_source_v1(
                &envelope(CodeObjectVersion::V6),
                &two_kernels,
                &llvm,
                &[root(1)],
            ),
            Err(CompilerDescriptorError::IncompleteTypedKernelClosure { typed: 1, total: 2 })
        ));

        let one = module();
        let llvm = construct_inert_compiler_module_text_v1(&one).unwrap();
        assert!(matches!(
            construct_compiler_descriptor_source_v1(
                &envelope(CodeObjectVersion::V5),
                &one,
                &llvm,
                &[root(1)],
            ),
            Err(CompilerDescriptorError::UnsupportedCodeObjectVersion(
                CodeObjectVersion::V5
            ))
        ));

        let mut unsupported = module();
        let llvm = construct_inert_compiler_module_text_v1(&unsupported).unwrap();
        unsupported
            .required_capabilities
            .insert(TargetCapability::Float64);
        assert!(matches!(
            construct_compiler_descriptor_source_v1(
                &envelope(CodeObjectVersion::V6),
                &unsupported,
                &llvm,
                &[root(1)],
            ),
            Err(CompilerDescriptorError::UnsupportedCapability(_))
        ));
    }

    #[test]
    fn exact_kernel_ir_atomic_requirement_projects_to_descriptor_atomics() {
        use fe2o3_kernel_ir::{AddressSpace, SynchronizationScope};

        let mut module = Module::new("atomic_descriptor_capability_test");
        module
            .required_capabilities
            .insert(TargetCapability::Atomic {
                width_bits: 32,
                address_space: AddressSpace::Global,
                max_scope: SynchronizationScope::System,
            });

        assert_eq!(
            descriptor_capabilities(&module, false, false).unwrap(),
            vec![CapabilityV1::Atomics]
        );

        module
            .required_capabilities
            .insert(TargetCapability::Float64);
        assert!(matches!(
            descriptor_capabilities(&module, false, false),
            Err(CompilerDescriptorError::UnsupportedCapability(_))
        ));
    }

    #[test]
    fn exact_target_implies_bfloat16_without_matrix_authority() {
        let mut module = Module::new("bfloat16_descriptor_capability_test");
        module
            .required_capabilities
            .insert(TargetCapability::BFloat16);

        assert!(
            descriptor_capabilities(&module, false, false)
                .unwrap()
                .is_empty()
        );
        assert!(
            descriptor_capabilities(&module, true, false)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn bfloat16_does_not_bypass_exact_matrix_extension_admission() {
        let mut module = Module::new("bfloat16_matrix_descriptor_capability_test");
        module
            .required_capabilities
            .insert(TargetCapability::BFloat16);
        module
            .required_capabilities
            .insert(TargetCapability::Extension {
                namespace: MATRIX_CAPABILITY_NAMESPACE.to_owned(),
                name: BF16_F32_M16N16K16_CAPABILITY.to_owned(),
            });

        assert!(matches!(
            descriptor_capabilities(&module, false, false),
            Err(CompilerDescriptorError::UnsupportedCapability(_))
        ));
        assert_eq!(
            descriptor_capabilities(&module, true, false).unwrap(),
            vec![CapabilityV1::MatrixMultiply, CapabilityV1::AmdMfma]
        );
    }

    #[test]
    fn descriptor_capabilities_admit_only_the_exact_gfx942_diagnostic_contract() {
        let exact = TargetCapability::Extension {
            namespace: fe2o3_kernel_ir::AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
            name: fe2o3_kernel_ir::AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
        };
        let mut module = Module::new("diagnostic_descriptor_capability_test");
        module.required_capabilities.insert(exact.clone());
        assert!(
            descriptor_capabilities(&module, false, false)
                .unwrap()
                .is_empty()
        );

        for hostile in [
            TargetCapability::Extension {
                namespace: "fe2o3.amdgpu.lookalike".to_owned(),
                name: fe2o3_kernel_ir::AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
            },
            TargetCapability::Extension {
                namespace: fe2o3_kernel_ir::AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE
                    .to_owned(),
                name: "diagnostics.gfx942.v2".to_owned(),
            },
        ] {
            module.required_capabilities.clear();
            module.required_capabilities.insert(hostile);
            assert!(matches!(
                descriptor_capabilities(&module, false, false),
                Err(CompilerDescriptorError::UnsupportedCapability(_))
            ));
        }
    }

    #[test]
    fn raw_modules_stay_unbound_and_exact_inputs_change_evidence() {
        let envelope = envelope(CodeObjectVersion::V6);
        let module = module();
        let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
        assert!(
            construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[])
                .unwrap()
                .is_none()
        );

        let first = construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[root(1)])
            .unwrap()
            .unwrap();
        let second = construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[root(2)])
            .unwrap()
            .unwrap();
        assert_ne!(first.identity(), second.identity());
        assert_ne!(
            first.table().kernels()[0].source_evidence(),
            second.table().kernels()[0].source_evidence()
        );
        assert_ne!(
            first.table().kernels()[0].executable_ir_evidence(),
            second.table().kernels()[0].executable_ir_evidence()
        );
    }

    #[test]
    fn production_semantic_root_roster_matches_by_binding_not_root_order() {
        let alpha = [0xa1; 32];
        let zeta = [0x7a; 32];

        assert_eq!(
            match_exact_production_root_roster_v1(&[alpha], &[alpha]).unwrap(),
            vec![0],
        );
        assert_eq!(
            match_exact_production_root_roster_v1(&[alpha, zeta], &[zeta, alpha]).unwrap(),
            vec![1, 0],
        );
    }

    #[test]
    fn production_semantic_root_roster_rejects_missing_duplicate_and_substituted_bindings() {
        let alpha = [0xa1; 32];
        let zeta = [0x7a; 32];
        let substituted = [0xfe; 32];

        assert!(matches!(
            match_exact_production_root_roster_v1(&[alpha, zeta], &[alpha]),
            Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "complete typed/semantic root roster"
            ))
        ));
        assert!(matches!(
            match_exact_production_root_roster_v1(&[alpha, zeta], &[alpha, alpha]),
            Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "unique semantic kernel binding roster"
            ))
        ));
        assert!(matches!(
            match_exact_production_root_roster_v1(&[alpha, alpha], &[alpha, zeta]),
            Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "exact typed/semantic kernel binding roster"
            ))
        ));
        assert!(matches!(
            match_exact_production_root_roster_v1(&[alpha, substituted], &[alpha, zeta]),
            Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "exact typed/semantic kernel binding roster"
            ))
        ));
    }

    #[test]
    fn semantic_type_identity_substitution_fails_with_identical_abi_evidence() {
        let exact = descriptor_argument(
            0,
            DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32),
            0,
        );
        let expected = exact.semantic_type_identity;
        require_production_descriptor_argument_semantic_type_v1(&exact, expected).unwrap();

        let mut substituted = exact.clone();
        substituted.semantic_type_identity = SemanticTypeIdentityV1::from_sha256([0xfe; 32]);
        assert_eq!(substituted.name, exact.name);
        assert_eq!(substituted.kind, exact.kind);
        assert_eq!(substituted.access, exact.access);
        assert_eq!(substituted.offset, exact.offset);
        assert_eq!(substituted.layout, exact.layout);
        assert!(matches!(
            require_production_descriptor_argument_semantic_type_v1(&substituted, expected),
            Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "rustc semantic argument type identity"
            ))
        ));
    }

    #[test]
    fn semantic_type_identity_is_bound_to_argument_order() {
        let first = descriptor_argument(0, DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U32), 0);
        let second = descriptor_argument(1, DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U32), 4);

        assert!(
            require_production_descriptor_argument_semantic_type_v1(
                &first,
                second.semantic_type_identity,
            )
            .is_err()
        );
        assert!(
            require_production_descriptor_argument_semantic_type_v1(
                &second,
                first.semantic_type_identity,
            )
            .is_err()
        );
    }
}
