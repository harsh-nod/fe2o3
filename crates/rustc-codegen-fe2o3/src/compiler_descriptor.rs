//! Rustc-derived descriptor input for the first typed Worker V2 profile.

use crate::collector::{CollectedFunction, TypedArgumentListV1, TypedKernelProfile};
use crate::kernel_ir_codegen::InertCompilerModuleTextV1;
use crate::rust_type_layout::{ExtractError, extract_exact_typed_vecadd_layout};
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
    AccessMode, AliasSemantics, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest,
    CapabilityV1, CodeObjectVersion, CompilerIdentityV1, DeviceDescriptorTableV1,
    DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
    KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1,
    OwnershipSemantics, PhysicalAbiComponentKind, ProducerIdentityV1, ScalarTypeV1,
    SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName, ValidationError,
};
use fe2o3_kernel_ir::{
    BF16_F32_M16N16K16_CAPABILITY, BasicBlock, BlockId, Function, Kernel, LaunchDomain,
    LaunchExtent, MATRIX_CAPABILITY_NAMESPACE, Module, Signature, TargetCapability, Terminator,
    WaveWidth, WorkgroupSize,
};
use reserved_fe2o3_symbols::{
    CrateBindingIdV1, KernelBindingIdV1, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
    derive_kernel_binding_id_v1,
};
use rustc_middle::ty::TyCtxt;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

const GFX942_PROCESSOR: &str = "gfx942";
const EXPLICIT_ARGUMENT_BYTES: u32 = 48;
const KERNARG_ALIGNMENT_BYTES: u32 = 8;
const WORKGROUP_X: u32 = 256;

const SOURCE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-SOURCE-IDENTITY/V1\0";
const SOURCE_DIGEST_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-SOURCE-DIGEST/V1\0";
const IR_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-IR-IDENTITY/V1\0";
const IR_DIGEST_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-IR-DIGEST/V1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypedDescriptorRootV1 {
    logical_name: String,
    export_name: String,
    profile: TypedKernelProfile,
    kernel_binding: KernelBindingIdV1,
    arguments: TypedArgumentListV1<TypedDescriptorArgumentV1>,
    explicit_argument_bytes: u32,
    kernarg_alignment_bytes: u32,
    source_launch: Option<LaunchContract>,
}

impl TypedDescriptorRootV1 {
    pub(crate) fn general_v3_semantic_identity(
        &self,
    ) -> Option<(
        KernelBindingIdV1,
        reserved_fe2o3_symbols::GeneratedHostContractIdV3,
    )> {
        match self.profile {
            TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
                generated_host_contract_identity,
            } => Some((self.kernel_binding, generated_host_contract_identity)),
            TypedKernelProfile::VecAddRustcLayoutV2 => None,
        }
    }

    pub(crate) const fn source_launch(&self) -> Option<&LaunchContract> {
        self.source_launch.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DescriptorArgumentKindV1 {
    SharedSlice(ScalarTypeV1),
    DisjointSlice(ScalarTypeV1),
    Scalar(ScalarTypeV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TypedDescriptorArgumentV1 {
    name: String,
    kind: DescriptorArgumentKindV1,
    access: AccessMode,
    offset: u32,
    layout: RustLayoutEvidenceV1,
}

/// Re-extracts exact rustc layout evidence instead of trusting retained identity bytes alone.
pub(crate) fn typed_descriptor_roots_from_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    functions: &[CollectedFunction<'tcx>],
) -> Result<Vec<TypedDescriptorRootV1>, CompilerDescriptorError> {
    typed_descriptor_roots_from_collection_with_policy(tcx, functions, true)
}

/// Production collection intentionally does not retain the legacy layout
/// identity cache. Re-derive the complete evidence directly from rustc and
/// the independently authenticated frontend launch contract.
pub(crate) fn typed_descriptor_roots_from_production_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    functions: &[CollectedFunction<'tcx>],
) -> Result<Vec<TypedDescriptorRootV1>, CompilerDescriptorError> {
    typed_descriptor_roots_from_collection_with_policy(tcx, functions, false)
}

fn typed_descriptor_roots_from_collection_with_policy<'tcx>(
    tcx: TyCtxt<'tcx>,
    functions: &[CollectedFunction<'tcx>],
    require_retained_evidence: bool,
) -> Result<Vec<TypedDescriptorRootV1>, CompilerDescriptorError> {
    functions
        .iter()
        .filter_map(|function| {
            function.typed_profile.map(|profile| {
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
                let (arguments, explicit_argument_bytes, kernarg_alignment_bytes, source_launch) =
                    match profile {
                        TypedKernelProfile::VecAddRustcLayoutV2 => {
                            let [input_a, input_b, output] =
                                extract_exact_typed_vecadd_layout(tcx, function.instance)
                                    .map_err(CompilerDescriptorError::RustLayout)?;
                            (
                                TypedArgumentListV1::new(vec![
                                    TypedDescriptorArgumentV1 {
                                        name: "input_a".to_owned(),
                                        kind: DescriptorArgumentKindV1::SharedSlice(
                                            ScalarTypeV1::F32,
                                        ),
                                        access: AccessMode::ReadOnly,
                                        offset: 0,
                                        layout: input_a,
                                    },
                                    TypedDescriptorArgumentV1 {
                                        name: "input_b".to_owned(),
                                        kind: DescriptorArgumentKindV1::SharedSlice(
                                            ScalarTypeV1::F32,
                                        ),
                                        access: AccessMode::ReadOnly,
                                        offset: 16,
                                        layout: input_b,
                                    },
                                    TypedDescriptorArgumentV1 {
                                        name: "output".to_owned(),
                                        kind: DescriptorArgumentKindV1::DisjointSlice(
                                            ScalarTypeV1::F32,
                                        ),
                                        access: AccessMode::WriteOnly,
                                        offset: 32,
                                        layout: output,
                                    },
                                ]),
                                EXPLICIT_ARGUMENT_BYTES,
                                KERNARG_ALIGNMENT_BYTES,
                                None,
                            )
                        }
                        TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
                            generated_host_contract_identity,
                        } => {
                            let launch = match function.general_typed_contract.as_ref() {
                            Some(retained) => retained.launch().clone(),
                            None if require_retained_evidence => {
                                return Err(CompilerDescriptorError::MissingTypedField {
                                    kernel: function.export_name.clone(),
                                    field: "general rustc contract",
                                });
                            }
                            None => {
                                crate::collector::rederive_general_typed_launch_for_descriptor_v1(
                                    function.frontend_contract.as_ref(),
                                    &function.export_name,
                                )
                                .map_err(|reason| {
                                    CompilerDescriptorError::InvalidArgumentCollection {
                                        kernel: function.export_name.clone(),
                                        reason,
                                    }
                                })?
                            }
                        };
                            let contract = extract_general_typed_kernel_v3(
                                tcx,
                                function.instance,
                                &logical_name,
                                &function.export_name,
                                &launch,
                            )
                            .map_err(CompilerDescriptorError::GeneralRustLayout)?;
                            if function
                                .general_typed_contract
                                .as_ref()
                                .is_some_and(|retained| retained != &contract)
                            {
                                return Err(
                                    CompilerDescriptorError::RetainedGeneralContractMismatch(
                                        function.export_name.clone(),
                                    ),
                                );
                            }
                            let derived = derive_generated_host_contract_identity_v1(
                                MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                                kernel_binding.as_bytes(),
                                &logical_name,
                                &function.export_name,
                                contract.abi(),
                                contract.launch(),
                            );
                            if derived.as_bytes() != &generated_host_contract_identity.as_bytes() {
                                return Err(
                                    CompilerDescriptorError::GeneratedHostContractMismatch(
                                        function.export_name.clone(),
                                    ),
                                );
                            }
                            let fields = contract.abi().fields();
                            let arguments = contract
                                .arguments()
                                .iter()
                                .zip(fields)
                                .enumerate()
                                .map(|(index, (argument, field))| {
                                    Ok(TypedDescriptorArgumentV1 {
                                        name: field.name().as_str().to_owned(),
                                        kind: descriptor_argument_kind(argument.kind()),
                                        access: match argument.kind() {
                                            GeneralTypedArgumentKindV3::Scalar(_) => {
                                                AccessMode::ByValue
                                            }
                                            GeneralTypedArgumentKindV3::SharedSlice(_) => {
                                                AccessMode::ReadOnly
                                            }
                                            GeneralTypedArgumentKindV3::DisjointSlice(_) => {
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
                                    })
                                })
                                .collect::<Result<Vec<_>, CompilerDescriptorError>>()?;
                            (
                                TypedArgumentListV1::new(arguments),
                                u32::try_from(contract.abi().size()).map_err(|_| {
                                    CompilerDescriptorError::ExplicitArgumentSizeOverflow(
                                        function.export_name.clone(),
                                    )
                                })?,
                                contract.abi().alignment(),
                                Some(contract.launch().clone()),
                            )
                        }
                    };
                let arguments = arguments.map_err(|error| {
                    CompilerDescriptorError::InvalidArgumentCollection {
                        kernel: function.export_name.clone(),
                        reason: error.to_string(),
                    }
                })?;
                validate_profile_argument_count(profile, &function.export_name, arguments.len())?;
                match function.typed_layout_identities.as_ref() {
                    Some(retained) if retained.len() != arguments.len() => {
                        return Err(
                            CompilerDescriptorError::RetainedLayoutArgumentCountMismatch {
                                kernel: function.export_name.clone(),
                                retained: retained.len(),
                                rederived: arguments.len(),
                            },
                        );
                    }
                    Some(retained)
                        if !retained.as_slice().iter().copied().eq(arguments
                            .as_slice()
                            .iter()
                            .map(|argument| argument.layout.type_identity())) =>
                    {
                        return Err(CompilerDescriptorError::RetainedLayoutIdentityMismatch(
                            function.export_name.clone(),
                        ));
                    }
                    None if require_retained_evidence => {
                        return Err(CompilerDescriptorError::MissingTypedField {
                            kernel: function.export_name.clone(),
                            field: "rustc layout identities",
                        });
                    }
                    Some(_) | None => {}
                }
                Ok(TypedDescriptorRootV1 {
                    logical_name,
                    export_name: function.export_name.clone(),
                    profile,
                    kernel_binding,
                    arguments,
                    explicit_argument_bytes,
                    kernarg_alignment_bytes,
                    source_launch,
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
    let geometry = validate_production_v1_descriptor_evidence(module, typed_roots, formal)?;
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
            producer_version: "production-v1-gfx942-cov6-v1",
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
) -> Result<crate::production_geometry_v1::ProductionGeometryV1, CompilerDescriptorError> {
    use fe2o3_artifacts::{RustDisjointIndexSpaceV1, RustSourceTypeShapeV1};
    use fe2o3_kernel_ir::{
        AccessMode as KirAccessMode, AddressSpace, FormalMemoryAccessKind, FormalParameterKind,
        Type as KirType,
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
    )
    .map_err(CompilerDescriptorError::ProductionGeometry)?;

    for (index, (root_argument, kernel_type)) in root
        .arguments
        .as_slice()
        .iter()
        .zip(&entry.signature.parameters)
        .enumerate()
    {
        let exact_kernel_type = match (root_argument.kind, kernel_type) {
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
            _ => false,
        };
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
            DescriptorArgumentKindV1::Scalar(_) => unreachable!(),
        };
        if allocation.value() != body.parameters[index]
            || allocation.kind() != FormalParameterKind::Slice
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

pub(crate) fn validate_production_v1_semantic_ownership_evidence(
    typed_roots: &[TypedDescriptorRootV1],
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
) -> Result<(), CompilerDescriptorError> {
    use fe2o3_artifacts::RustcAbiClassV1;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiPassModeV1, SemanticSourceArgumentOwnershipV1,
    };

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
    let function = semantic
        .functions()
        .get(semantic_root.index() as usize)
        .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
            "semantic root function",
        ))?;
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
        };
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
        || matches!(right, DescriptorArgumentKindV1::DisjointSlice(_))
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

/// Constructs the source-authenticated one-wave tiled GEMM descriptor input.
///
/// The existing generic/scalar path remains fixed at WG256. This entry point
/// admits WG64 and matrix capabilities only for the exact canonical tiled GEMM
/// module; it does not relax descriptor policy for any caller-supplied graph.
pub(crate) fn construct_tiled_gemm_v1_compiler_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    module: &Module,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
) -> Result<Option<CompilerDescriptorSourceV1>, CompilerDescriptorError> {
    if module != &fe2o3_kernel_ir::tiled_gemm_v1_module() {
        return Err(CompilerDescriptorError::NonCanonicalTiledGemmModule);
    }
    construct_compiler_descriptor_source_with_profile_v1(
        envelope,
        module,
        compiler_module,
        typed_roots,
        DescriptorConstructionProfileV1 {
            rank: 1,
            workgroup: [fe2o3_kernel_ir::TILED_GEMM_V1_LANES, 1, 1],
            max_grid: [1, 1, 1],
            max_flat_workgroup_size: fe2o3_kernel_ir::TILED_GEMM_V1_LANES,
            static_shared_memory_bytes: 0,
            allow_exact_tiled_matrix: true,
            allow_workgroup_memory: false,
            producer_version: "typed-tiled-gemm-gfx942-cov6-v1",
        },
    )
}

/// Constructs the exact source-corresponded LDS Slice 1 descriptor input.
///
/// The attributed Rust export and canonical Kernel IR entry intentionally have
/// different names. Rustc authenticates the former before this function
/// projects its already re-derived typed ABI onto the latter. The descriptor
/// records the compiler-owned 1024-byte static LDS requirement; the attributed
/// frontend contract remains a distinct zero-user-shared-memory record.
pub(crate) fn construct_tiled_gemm_lds_slice1_compiler_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    module: &Module,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
) -> Result<Option<CompilerDescriptorSourceV1>, CompilerDescriptorError> {
    if module != &fe2o3_kernel_ir::tiled_gemm_lds_v1_module() {
        return Err(CompilerDescriptorError::NonCanonicalTiledGemmLdsSlice1Module);
    }
    if compiler_module.descriptor_source_identity().is_some() {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch("pre-section LLVM"),
        );
    }
    let [source_root] = typed_roots else {
        return Err(CompilerDescriptorError::IncompleteTypedKernelClosure {
            typed: typed_roots.len(),
            total: module.kernels.len(),
        });
    };
    if source_root.logical_name
        != crate::collected_tiled_gemm_lds_slice1_v1::LDS_SLICE1_KERNEL_EXPORT_V1
        || source_root.export_name
            != crate::collected_tiled_gemm_lds_slice1_v1::LDS_SLICE1_KERNEL_EXPORT_V1
    {
        return Err(CompilerDescriptorError::UnexpectedAttributedSourceKernel(
            source_root.export_name.clone(),
        ));
    }

    let mut projected_root = source_root.clone();
    projected_root.export_name = fe2o3_kernel_ir::TILED_GEMM_LDS_V1_KERNEL_ID.to_owned();
    let source = construct_compiler_descriptor_source_with_profile_v1(
        envelope,
        module,
        compiler_module,
        &[projected_root],
        tiled_gemm_lds_slice1_descriptor_profile_v1(),
    )?
    .ok_or(CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch("descriptor presence"))?;
    validate_tiled_gemm_lds_slice1_compiler_descriptor_source_v1(
        &source,
        envelope,
        compiler_module,
        source_root,
    )?;
    Ok(Some(source))
}

/// Constructs the exact two-slice WG64 row-softmax descriptor source.
///
/// The generic descriptor path remains WG256. This profile is reachable only
/// for the private canonical row-softmax graph selected by rustc admission.
pub(crate) fn construct_row_softmax_v1_compiler_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    module: &Module,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
) -> Result<Option<CompilerDescriptorSourceV1>, CompilerDescriptorError> {
    if module != &crate::collected_row_softmax_v1::canonical_row_softmax_v1_module() {
        return Err(CompilerDescriptorError::NonCanonicalRowSoftmaxModule);
    }
    construct_compiler_descriptor_source_with_profile_v1(
        envelope,
        module,
        compiler_module,
        typed_roots,
        DescriptorConstructionProfileV1 {
            rank: 1,
            workgroup: [
                crate::collected_row_softmax_v1::ROW_SOFTMAX_ELEMENTS_V1,
                1,
                1,
            ],
            max_grid: [1, 1, 1],
            max_flat_workgroup_size: crate::collected_row_softmax_v1::ROW_SOFTMAX_ELEMENTS_V1,
            static_shared_memory_bytes: 0,
            allow_exact_tiled_matrix: false,
            allow_workgroup_memory: false,
            producer_version: "typed-row-softmax-gfx942-cov6-v1",
        },
    )
}

/// Constructs descriptor source only for the authenticated fixed Flash profile.
/// The semantic sidecar is rechecked before projecting the freshly extracted
/// rustc typed root onto the exact WG64/COV6 descriptor schema.
pub(crate) fn construct_flash_attention_v1_compiler_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    ir: &fe2o3_kernel_ir::FlashAttentionKernelIrV1,
    profile: &fe2o3_kernel_ir::FlashAttentionProfileV1,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    fe2o3_kernel_ir::verify_flash_attention_v1(ir, profile)
        .map_err(|_| CompilerDescriptorError::NonCanonicalFlashAttentionProfile)?;
    let [root] = typed_roots else {
        return Err(CompilerDescriptorError::IncompleteTypedKernelClosure {
            typed: typed_roots.len(),
            total: 1,
        });
    };
    let expected_kinds = [
        DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
        DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
        DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
        DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32),
    ];
    let exact_root = root.logical_name == fe2o3_kernel_ir::FLASH_ATTENTION_V1_KERNEL_ID
        && root.export_name == fe2o3_kernel_ir::FLASH_ATTENTION_V1_KERNEL_ID
        && root.explicit_argument_bytes
            == fe2o3_kernel_ir::FLASH_ATTENTION_V1_EXPLICIT_KERNARG_BYTES
        && root.kernarg_alignment_bytes == 8
        && root.arguments.len() == 4
        && root
            .arguments
            .as_slice()
            .iter()
            .zip(expected_kinds)
            .enumerate()
            .all(|(index, (argument, kind))| {
                argument.name == format!("arg{index}")
                    && argument.kind == kind
                    && argument.offset == (index as u32) * 16
                    && argument.access
                        == if index < 3 {
                            AccessMode::ReadOnly
                        } else {
                            AccessMode::ReadWrite
                        }
            });
    if !exact_root {
        return Err(CompilerDescriptorError::FlashAttentionDescriptorMismatch(
            "typed root/ABI/ownership",
        ));
    }

    construct_compiler_descriptor_source_with_profile_v1(
        envelope,
        &flash_attention_descriptor_module_v1(),
        compiler_module,
        typed_roots,
        DescriptorConstructionProfileV1 {
            rank: 1,
            workgroup: [64, 1, 1],
            max_grid: [1, 1, 1],
            max_flat_workgroup_size: 64,
            static_shared_memory_bytes: 0,
            allow_exact_tiled_matrix: false,
            allow_workgroup_memory: false,
            producer_version: "typed-flash-attention-gfx942-cov6-v1",
        },
    )?
    .ok_or(CompilerDescriptorError::FlashAttentionDescriptorMismatch(
        "descriptor presence",
    ))
}

fn flash_attention_descriptor_module_v1() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: Vec::new() });
    let entry = Function::kernel_entry(
        fe2o3_kernel_ir::FLASH_ATTENTION_V1_FUNCTION_ID,
        Signature::new(Vec::new(), Vec::new()),
        Vec::new(),
        vec![block],
    );
    let mut kernel = Kernel::new(
        fe2o3_kernel_ir::FLASH_ATTENTION_V1_KERNEL_ID,
        fe2o3_kernel_ir::FLASH_ATTENTION_V1_FUNCTION_ID,
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new(fe2o3_kernel_ir::FLASH_ATTENTION_V1_MODULE_ID);
    module.required_capabilities = [
        fe2o3_kernel_ir::gfx942_xnack_minus_target_capability(),
        TargetCapability::Subgroups,
        TargetCapability::WaveWidth(WaveWidth::Wave64),
    ]
    .into_iter()
    .collect();
    module.functions.push(entry);
    module.kernels.push(kernel);
    module
}

/// Builds the descriptor only for the consumed exact T8/E4/K2/C4 MoE profile.
pub(crate) fn construct_moe_top2_v1_compiler_descriptor_source_v1(
    parts: &crate::collected_moe_top2_v1::AuthenticatedMoeTop2WorkerPartsV1,
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    fe2o3_kernel_ir::verify_moe_top2_v1(&parts.ir, &parts.profile)
        .map_err(|_| CompilerDescriptorError::NonCanonicalMoeTop2Module)?;
    if envelope.target().to_string() != crate::collected_moe_top2_v1::EXACT_MOE_TOP2_TARGET_V1
        || envelope.code_object_version() != CodeObjectVersion::V6
        || compiler_module.kernel_entries() != [fe2o3_kernel_ir::MOE_TOP2_V1_KERNEL_ID]
        || !compiler_module.device_definitions().is_empty()
        || !compiler_module.device_ffi_exports().is_empty()
        || !compiler_module.external_declarations().is_empty()
        || compiler_module.descriptor_source_identity().is_some()
    {
        return Err(CompilerDescriptorError::NonCanonicalMoeTop2Module);
    }

    let shared_f32 =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let disjoint_u32 =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let shared_f32_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let disjoint_u32_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
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
    let mut arguments = Vec::with_capacity(names.len());
    arguments.push(LogicalArgumentV1::shared_slice(
        0,
        ValidName::new(names[0])?,
        &shared_f32,
        &shared_f32_layout,
        0,
    )?);
    for (index, name) in names.iter().enumerate().skip(1) {
        arguments.push(LogicalArgumentV1::disjoint_slice(
            u16::try_from(index).expect("eight MoE arguments fit u16"),
            ValidName::new(*name)?,
            &disjoint_u32,
            &disjoint_u32_layout,
            AccessMode::ReadWrite,
            u32::try_from(index * 16).expect("fixed MoE offset fits u32"),
        )?);
    }

    let compiler_binding = CrateBindingIdV1::from_bytes(parts.compiler_crate_binding);
    let kernel_binding = derive_kernel_binding_id_v1(
        compiler_binding,
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        fe2o3_kernel_ir::MOE_TOP2_V1_KERNEL_ID,
        fe2o3_kernel_ir::MOE_TOP2_V1_KERNEL_ID,
    );
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes(kernel_binding.as_bytes()),
        ValidName::new(fe2o3_kernel_ir::MOE_TOP2_V1_KERNEL_ID)?,
        ValidName::new(fe2o3_kernel_ir::MOE_TOP2_V1_KERNEL_ID)?,
        ValidName::new(fe2o3_kernel_ir::MOE_TOP2_V1_DESCRIPTOR_SYMBOL)?,
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes(parts.source_identity),
            EvidenceDigest::from_sha256_bytes(parts.source_authority_identity),
        ),
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes(parts.canonical_ir_identity),
            EvidenceDigest::from_sha256_bytes(
                Sha256::digest(compiler_module.llvm_ir().as_bytes()).into(),
            ),
        ),
        vec![CapabilityV1::AmdWave],
        KernelAbiLayoutV1::new(
            fe2o3_kernel_ir::MOE_TOP2_V1_EXPLICIT_KERNARG_BYTES,
            fe2o3_kernel_ir::MOE_TOP2_V1_COMPLETE_COV6_KERNARG_BYTES,
            8,
        )?,
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1)?),
            DimensionsV1::new(1, 1, 1)?,
            64,
            0,
            0,
        )?,
        arguments,
    )?;
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3")?,
            Text::new("1.96.0-nightly")?,
            [
                0x55, 0xe8, 0x6c, 0x99, 0x68, 0x09, 0x90, 0x2e, 0x8b, 0xba, 0xd5, 0x12, 0xcf, 0xb4,
                0xd2, 0xc1, 0x8b, 0xe4, 0x46, 0xd9,
            ],
        ),
        ProducerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3-worker-v2")?,
            Text::new("typed-moe-top2-t8-e4-k2-c4-gfx942-cov6-v1")?,
        ),
        envelope.target(),
        vec![shared_f32, disjoint_u32],
        vec![shared_f32_layout, disjoint_u32_layout],
        vec![kernel],
    )?;
    CompilerDescriptorSourceV1::new(table).map_err(CompilerDescriptorError::Source)
}

#[derive(Clone, Copy)]
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

const fn tiled_gemm_lds_slice1_descriptor_profile_v1() -> DescriptorConstructionProfileV1 {
    DescriptorConstructionProfileV1 {
        rank: 1,
        workgroup: [fe2o3_kernel_ir::TILED_GEMM_LDS_V1_LANES, 1, 1],
        max_grid: [1, 1, 1],
        max_flat_workgroup_size: fe2o3_kernel_ir::TILED_GEMM_LDS_V1_LANES,
        static_shared_memory_bytes: fe2o3_kernel_ir::TILED_GEMM_LDS_V1_STATIC_LDS_BYTES,
        allow_exact_tiled_matrix: true,
        allow_workgroup_memory: true,
        producer_version: "typed-tiled-gemm-lds-slice1-gfx942-cov6-v1",
    }
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
    if envelope.target().as_amd_target_id().processor() != GFX942_PROCESSOR {
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
        validate_profile_argument_count(root.profile, &root.export_name, root.arguments.len())?;
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

    let producer_version = if typed_roots
        .iter()
        .all(|root| root.profile == TypedKernelProfile::VecAddRustcLayoutV2)
    {
        "typed-vecadd-gfx942-cov6-v1"
    } else {
        profile.producer_version
    };
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3")?,
            Text::new(env!("CARGO_PKG_VERSION"))?,
            [0; 20],
        ),
        ProducerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3-worker-v2")?,
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

fn validate_tiled_gemm_lds_slice1_compiler_descriptor_source_v1(
    source: &CompilerDescriptorSourceV1,
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    authenticated_root: &TypedDescriptorRootV1,
) -> Result<(), CompilerDescriptorError> {
    if compiler_module.descriptor_source_identity().is_some() {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch("pre-section LLVM"),
        );
    }
    let [kernel] = source.table().kernels() else {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch("one-kernel closure"),
        );
    };
    let table = source.table();
    if table.device_target().to_string()
        != crate::collected_tiled_gemm_v1::EXACT_TILED_GEMM_TARGET_V1
        || table.code_object_version() != CodeObjectVersion::V6
    {
        return Err(CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch("target/COV6"));
    }
    if *kernel.kernel_id().as_bytes() != authenticated_root.kernel_binding.as_bytes() {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch("kernel binding"),
        );
    }
    let canonical_entry = fe2o3_kernel_ir::TILED_GEMM_LDS_V1_KERNEL_ID;
    if kernel.logical_name().as_str() != authenticated_root.logical_name
        || kernel.entry_name().as_str() != canonical_entry
        || kernel.descriptor_symbol().as_str() != format!("{canonical_entry}.kd")
    {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch("kernel projection"),
        );
    }

    let abi = kernel.abi_layout();
    let launch = kernel.launch();
    if abi.explicit_argument_size() != 48
        || abi.kernarg_segment_size() != 304
        || abi.kernarg_segment_alignment() != 8
        || launch.rank() != 1
        || launch.block_size() != BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1)?)
        || launch.max_grid() != DimensionsV1::new(1, 1, 1)?
        || launch.max_flat_workgroup_size() != 64
        || launch.static_shared_memory_bytes()
            != fe2o3_kernel_ir::TILED_GEMM_LDS_V1_STATIC_LDS_BYTES
        || launch.max_dynamic_shared_memory_bytes() != 0
        || kernel.capabilities()
            != [
                CapabilityV1::Subgroup,
                CapabilityV1::WorkgroupMemory,
                CapabilityV1::MatrixMultiply,
                CapabilityV1::AmdWave,
                CapabilityV1::AmdMfma,
            ]
    {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch(
                "ABI/launch/capability profile",
            ),
        );
    }

    let [a, b, c] = kernel.arguments() else {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch("argument count"),
        );
    };
    for (index, argument) in [a, b, c].into_iter().enumerate() {
        let (source_descriptor, layout_descriptor, pointer_offset) = match index {
            0 => (
                SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U16),
                DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U16),
                0,
            ),
            1 => (
                SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U16),
                DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U16),
                16,
            ),
            2 => (
                SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32),
                DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32),
                32,
            ),
            _ => unreachable!("exact LDS Slice 1 descriptor has three arguments"),
        };
        let exact_source = table.type_records().iter().any(|record| {
            record.identity() == argument.source_type() && record.descriptor() == &source_descriptor
        });
        let exact_layout = table.layout_records().iter().any(|record| {
            record.identity() == argument.device_layout()
                && record.descriptor() == &layout_descriptor
        });
        if argument.source_index() != index as u16 || !exact_source || !exact_layout {
            return Err(
                CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch(
                    "argument source/layout provenance",
                ),
            );
        }
        let exact_role = if index < 2 {
            argument.name().as_str() == format!("arg{index}")
                && argument.access() == AccessMode::ReadOnly
                && argument.ownership() == OwnershipSemantics::SharedBorrow
                && argument.alias() == AliasSemantics::SharedReadOnly
        } else {
            argument.name().as_str() == "arg2"
                && argument.access() == AccessMode::ReadWrite
                && argument.ownership() == OwnershipSemantics::UniqueBorrow
                && argument.alias() == AliasSemantics::Exclusive
        };
        if !exact_role {
            return Err(
                CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch(
                    "argument name/access/ownership",
                ),
            );
        }
        if argument.physical_components().collect::<Vec<_>>()
            != [
                (
                    PhysicalAbiComponentKind::GlobalPointer,
                    pointer_offset,
                    8,
                    8,
                ),
                (
                    PhysicalAbiComponentKind::SliceLengthU64,
                    pointer_offset + 8,
                    8,
                    8,
                ),
            ]
        {
            return Err(
                CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch(
                    "argument physical ABI",
                ),
            );
        }
    }

    if kernel.source_evidence()
        != recompute_projected_source_evidence_v1(authenticated_root, canonical_entry)
    {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch("source evidence"),
        );
    }
    if kernel.executable_ir_evidence()
        != recompute_projected_ir_evidence_v1(
            envelope,
            compiler_module,
            authenticated_root,
            canonical_entry,
        )
    {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch("executable IR evidence"),
        );
    }
    Ok(())
}

/// Rechecks that the exact pre-section LLVM body is the body committed by the
/// source-authenticated descriptor before Worker V2 sections are appended.
pub(crate) fn validate_tiled_gemm_lds_slice1_compiler_module_evidence_v1(
    source: &CompilerDescriptorSourceV1,
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
) -> Result<(), CompilerDescriptorError> {
    if compiler_module.descriptor_source_identity().is_some()
        || envelope.target().to_string()
            != crate::collected_tiled_gemm_v1::EXACT_TILED_GEMM_TARGET_V1
        || envelope.code_object_version() != CodeObjectVersion::V6
    {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch(
                "pre-section LLVM envelope",
            ),
        );
    }
    let [kernel] = source.table().kernels() else {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch(
                "pre-section LLVM kernel closure",
            ),
        );
    };
    let expected = recompute_projected_ir_evidence_from_binding_v1(
        envelope,
        compiler_module,
        kernel.kernel_id().as_bytes(),
        fe2o3_kernel_ir::TILED_GEMM_LDS_V1_KERNEL_ID,
    );
    if kernel.executable_ir_evidence() != expected {
        return Err(
            CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch(
                "pre-section LLVM evidence",
            ),
        );
    }
    Ok(())
}

fn recompute_projected_source_evidence_v1(
    authenticated_root: &TypedDescriptorRootV1,
    canonical_entry: &str,
) -> BuildEvidenceV1 {
    let binding = authenticated_root.kernel_binding.as_bytes();
    let mut identity_frames = vec![
        binding.as_slice(),
        authenticated_root.logical_name.as_bytes(),
        canonical_entry.as_bytes(),
    ];
    let identity_bytes = authenticated_root
        .arguments
        .as_slice()
        .iter()
        .map(|argument| type_identity_bytes(argument.layout.type_identity()))
        .collect::<Vec<_>>();
    for bytes in &identity_bytes {
        identity_frames.push(bytes.as_slice());
    }
    let canonical_layouts = authenticated_root
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

fn recompute_projected_ir_evidence_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    authenticated_root: &TypedDescriptorRootV1,
    canonical_entry: &str,
) -> BuildEvidenceV1 {
    recompute_projected_ir_evidence_from_binding_v1(
        envelope,
        compiler_module,
        &authenticated_root.kernel_binding.as_bytes(),
        canonical_entry,
    )
}

fn recompute_projected_ir_evidence_from_binding_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    binding: &[u8; 32],
    canonical_entry: &str,
) -> BuildEvidenceV1 {
    let envelope_identity = envelope.identity().as_bytes();
    let target = envelope.target().to_string();
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(domain_hash(
            IR_IDENTITY_DOMAIN_V1,
            &[
                binding.as_slice(),
                envelope_identity.as_slice(),
                target.as_bytes(),
                canonical_entry.as_bytes(),
            ],
        )),
        EvidenceDigest::from_sha256_bytes(domain_hash(
            IR_DIGEST_DOMAIN_V1,
            &[
                envelope.canonical_bytes(),
                compiler_module.llvm_ir().as_bytes(),
                canonical_entry.as_bytes(),
            ],
        )),
    )
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
    for capability in effective {
        match capability {
            TargetCapability::Int64 => {}
            TargetCapability::Extension { namespace, name }
                if namespace == fe2o3_kernel_ir::AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
                    && name
                        == fe2o3_kernel_ir::AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME =>
            {
                // Exact target binding is represented by the descriptor table's
                // device target, not as an executable kernel capability.
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
            TargetCapability::BFloat16 if allow_exact_tiled_matrix => {
                result.insert(CapabilityV1::MatrixMultiply);
            }
            TargetCapability::Extension { namespace, name }
                if allow_exact_tiled_matrix
                    && namespace == MATRIX_CAPABILITY_NAMESPACE
                    && name == BF16_F32_M16N16K16_CAPABILITY =>
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

fn validate_profile_argument_count(
    profile: TypedKernelProfile,
    kernel: &str,
    actual: usize,
) -> Result<(), CompilerDescriptorError> {
    if let Some(expected) = profile.expected_argument_count()
        && actual != expected
    {
        return Err(CompilerDescriptorError::TypedProfileArgumentCountMismatch {
            kernel: kernel.to_owned(),
            expected,
            actual,
        });
    }
    if !profile.accepts_argument_count(actual) {
        return Err(CompilerDescriptorError::InvalidArgumentCollection {
            kernel: kernel.to_owned(),
            reason: format!("unsupported general typed argument count {actual}"),
        });
    }
    Ok(())
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
    MissingTypedField {
        kernel: String,
        field: &'static str,
    },
    InvalidArgumentCollection {
        kernel: String,
        reason: String,
    },
    TypedProfileArgumentCountMismatch {
        kernel: String,
        expected: usize,
        actual: usize,
    },
    RetainedLayoutArgumentCountMismatch {
        kernel: String,
        retained: usize,
        rederived: usize,
    },
    RustLayout(ExtractError),
    GeneralRustLayout(GeneralTypedExtractError),
    RetainedLayoutIdentityMismatch(String),
    RetainedGeneralContractMismatch(String),
    GeneratedHostContractMismatch(String),
    ArgumentOffsetOverflow {
        kernel: String,
        index: usize,
    },
    ArgumentIndexOverflow {
        kernel: String,
        index: usize,
    },
    ExplicitArgumentSizeOverflow(String),
    KernargSizeOverflow(String),
    IncompleteTypedKernelClosure {
        typed: usize,
        total: usize,
    },
    UnsupportedTarget(String),
    UnsupportedCodeObjectVersion(CodeObjectVersion),
    DuplicateTypedKernel(String),
    MissingTypedKernel(String),
    UnexpectedWorkgroupSize {
        kernel: String,
        expected: [u32; 3],
    },
    NonCanonicalTiledGemmModule,
    NonCanonicalTiledGemmLdsSlice1Module,
    UnexpectedAttributedSourceKernel(String),
    TiledGemmLdsSlice1DescriptorMismatch(&'static str),
    NonCanonicalRowSoftmaxModule,
    NonCanonicalFlashAttentionProfile,
    FlashAttentionDescriptorMismatch(&'static str),
    NonCanonicalMoeTop2Module,
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
            Self::TypedProfileArgumentCountMismatch {
                kernel,
                expected,
                actual,
            } => write!(
                formatter,
                "typed kernel `{kernel}` profile requires {expected} argument(s), found {actual}"
            ),
            Self::RetainedLayoutArgumentCountMismatch {
                kernel,
                retained,
                rederived,
            } => write!(
                formatter,
                "typed kernel `{kernel}` retained {retained} layout identity/identities but rustc rederived {rederived}"
            ),
            Self::RustLayout(error) => write!(formatter, "rustc layout extraction failed: {error}"),
            Self::GeneralRustLayout(error) => {
                write!(formatter, "general rustc layout extraction failed: {error}")
            }
            Self::RetainedLayoutIdentityMismatch(kernel) => write!(
                formatter,
                "typed kernel `{kernel}` retained layout identities do not match fresh rustc evidence"
            ),
            Self::RetainedGeneralContractMismatch(kernel) => write!(
                formatter,
                "typed kernel `{kernel}` retained general contract does not match fresh rustc evidence"
            ),
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
                    "typed descriptor source currently requires gfx942, found {target}"
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
            Self::NonCanonicalTiledGemmModule => formatter.write_str(
                "tiled GEMM descriptor construction requires the exact canonical tiled_gemm_v1 module",
            ),
            Self::NonCanonicalTiledGemmLdsSlice1Module => formatter.write_str(
                "LDS Slice 1 descriptor construction requires the exact canonical tiled_gemm_lds_v1 module",
            ),
            Self::UnexpectedAttributedSourceKernel(kernel) => write!(
                formatter,
                "LDS Slice 1 descriptor construction requires the exact attributed source kernel, found `{kernel}`"
            ),
            Self::TiledGemmLdsSlice1DescriptorMismatch(field) => write!(
                formatter,
                "LDS Slice 1 compiler descriptor has an internal {field} mismatch"
            ),
            Self::NonCanonicalRowSoftmaxModule => formatter.write_str(
                "row-softmax descriptor construction requires the exact canonical row_softmax_v1 module",
            ),
            Self::NonCanonicalFlashAttentionProfile => formatter.write_str(
                "FlashAttention descriptor construction requires the exact authenticated B1/H1/N8/D16 profile",
            ),
            Self::FlashAttentionDescriptorMismatch(field) => write!(
                formatter,
                "FlashAttention compiler descriptor has an internal {field} mismatch"
            ),
            Self::NonCanonicalMoeTop2Module => formatter.write_str(
                "MoE descriptor construction requires the exact authenticated T8/E4/K2/C4 module",
            ),
            Self::ProductionFormalMemory(error) => {
                write!(formatter, "production formal-memory evidence failed: {error}")
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
pub(crate) fn scalar_gemm_v1_descriptor_source_for_test() -> CompilerDescriptorSourceV1 {
    use fe2o3_artifacts::{
        PointerWidth, RustDisjointIndexSpaceV1, RustLayoutEvidenceV1, RustPhysicalComponentKindV1,
        RustPhysicalComponentV1, RustPointerMutabilityV1, RustScalarElementTypeV1,
        RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
    };
    use reserved_fe2o3_symbols::GeneratedHostContractIdV3;

    fn layout(kind: GeneralTypedArgumentKindV3) -> RustLayoutEvidenceV1 {
        let (shape, abi, size, alignment, components) = match kind {
            GeneralTypedArgumentKindV3::SharedSlice(element) => (
                RustSourceTypeShapeV1::shared_slice(element),
                RustcAbiClassV1::ScalarPair,
                16,
                8,
                vec![
                    RustPhysicalComponentV1::new(
                        0,
                        8,
                        8,
                        RustPhysicalComponentKindV1::Pointer {
                            mutability: RustPointerMutabilityV1::Const,
                            pointee: element,
                        },
                    )
                    .unwrap(),
                    RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize)
                        .unwrap(),
                ],
            ),
            GeneralTypedArgumentKindV3::DisjointSlice(element) => (
                RustSourceTypeShapeV1::disjoint_slice(element, RustDisjointIndexSpaceV1::Index1D),
                RustcAbiClassV1::ScalarPair,
                16,
                8,
                vec![
                    RustPhysicalComponentV1::new(
                        0,
                        8,
                        8,
                        RustPhysicalComponentKindV1::Pointer {
                            mutability: RustPointerMutabilityV1::Mut,
                            pointee: element,
                        },
                    )
                    .unwrap(),
                    RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize)
                        .unwrap(),
                ],
            ),
            GeneralTypedArgumentKindV3::Scalar(element) => (
                RustSourceTypeShapeV1::scalar(element),
                RustcAbiClassV1::Scalar,
                4,
                4,
                vec![
                    RustPhysicalComponentV1::new(
                        0,
                        4,
                        4,
                        RustPhysicalComponentKindV1::Scalar { scalar: element },
                    )
                    .unwrap(),
                ],
            ),
        };
        RustLayoutEvidenceV1::new(
            RustTypeEvidenceV1::new(shape),
            abi,
            PointerWidth::Bits64,
            size,
            alignment,
            components,
        )
        .unwrap()
    }

    let kinds = [
        GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
        GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
        GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
        GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U32),
        GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U32),
        GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U32),
    ];
    let names = ["a", "b", "c", "m", "n", "k"];
    let offsets = [0, 16, 32, 48, 52, 56];
    let arguments = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| TypedDescriptorArgumentV1 {
            name: names[index].to_owned(),
            kind: descriptor_argument_kind(kind),
            access: match kind {
                GeneralTypedArgumentKindV3::Scalar(_) => AccessMode::ByValue,
                GeneralTypedArgumentKindV3::SharedSlice(_) => AccessMode::ReadOnly,
                GeneralTypedArgumentKindV3::DisjointSlice(_) => AccessMode::ReadWrite,
            },
            offset: offsets[index],
            layout: layout(kind),
        })
        .collect::<Vec<_>>();
    let root = TypedDescriptorRootV1 {
        logical_name: "scalar_gemm_v1".to_owned(),
        export_name: "scalar_gemm_v1".to_owned(),
        profile: TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
            generated_host_contract_identity: GeneratedHostContractIdV3::from_bytes([0x55; 32]),
        },
        kernel_binding: KernelBindingIdV1::from_bytes([
            0x78, 0x9a, 0xde, 0xdf, 0xdc, 0x3b, 0xe1, 0xfb, 0x60, 0x51, 0x8d, 0xd2, 0xc7, 0x46,
            0x0c, 0x3e, 0xf8, 0xe6, 0xb9, 0x00, 0x52, 0x7d, 0x1b, 0xcb, 0x22, 0x89, 0xba, 0xa1,
            0xe0, 0x14, 0x69, 0x3e,
        ]),
        arguments: TypedArgumentListV1::new(arguments).unwrap(),
        explicit_argument_bytes: 64,
        kernarg_alignment_bytes: 8,
        source_launch: None,
    };
    let module = fe2o3_kernel_ir::scalar_gemm_v1_module();
    let compiler_module =
        crate::kernel_ir_codegen::construct_inert_scalar_gemm_v1_module_text(&module).unwrap();
    let target = fe2o3_compiler_ffi::DeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .unwrap();
    construct_compiler_descriptor_source_v1(&envelope, &module, &compiler_module, &[root])
        .unwrap()
        .unwrap()
}

#[cfg(test)]
pub(crate) fn tiled_gemm_v1_descriptor_source_for_test() -> CompilerDescriptorSourceV1 {
    use fe2o3_artifacts::{
        PointerWidth, RustDisjointIndexSpaceV1, RustLayoutEvidenceV1, RustPhysicalComponentKindV1,
        RustPhysicalComponentV1, RustPointerMutabilityV1, RustScalarElementTypeV1,
        RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
    };
    use reserved_fe2o3_symbols::GeneratedHostContractIdV3;

    fn layout(kind: GeneralTypedArgumentKindV3) -> RustLayoutEvidenceV1 {
        let (element, disjoint) = match kind {
            GeneralTypedArgumentKindV3::SharedSlice(element) => (element, false),
            GeneralTypedArgumentKindV3::DisjointSlice(element) => (element, true),
            GeneralTypedArgumentKindV3::Scalar(_) => unreachable!("tiled profile has no scalar"),
        };
        let shape = if disjoint {
            RustSourceTypeShapeV1::disjoint_slice(element, RustDisjointIndexSpaceV1::Index1D)
        } else {
            RustSourceTypeShapeV1::shared_slice(element)
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
                        mutability: if disjoint {
                            RustPointerMutabilityV1::Mut
                        } else {
                            RustPointerMutabilityV1::Const
                        },
                        pointee: element,
                    },
                )
                .unwrap(),
                RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
            ],
        )
        .unwrap()
    }

    let kinds = [
        GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::U16),
        GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::U16),
        GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
        GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
    ];
    let names = ["a", "b", "c", "d"];
    let arguments = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| TypedDescriptorArgumentV1 {
            name: names[index].to_owned(),
            kind: descriptor_argument_kind(kind),
            access: match kind {
                GeneralTypedArgumentKindV3::SharedSlice(_) => AccessMode::ReadOnly,
                GeneralTypedArgumentKindV3::DisjointSlice(_) => AccessMode::ReadWrite,
                GeneralTypedArgumentKindV3::Scalar(_) => unreachable!(),
            },
            offset: (index as u32) * 16,
            layout: layout(kind),
        })
        .collect::<Vec<_>>();
    let root = TypedDescriptorRootV1 {
        logical_name: "tiled_gemm_v1".to_owned(),
        export_name: "tiled_gemm_v1".to_owned(),
        profile: TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
            generated_host_contract_identity: GeneratedHostContractIdV3::from_bytes([0x66; 32]),
        },
        kernel_binding: KernelBindingIdV1::from_bytes([0x74; 32]),
        arguments: TypedArgumentListV1::new(arguments).unwrap(),
        explicit_argument_bytes: 64,
        kernarg_alignment_bytes: 8,
        source_launch: None,
    };
    let module = fe2o3_kernel_ir::tiled_gemm_v1_module();
    let compiler_module =
        crate::kernel_ir_codegen::construct_inert_tiled_gemm_v1_module_text(&module).unwrap();
    let target = fe2o3_compiler_ffi::DeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .unwrap();
    construct_tiled_gemm_v1_compiler_descriptor_source_v1(
        &envelope,
        &module,
        &compiler_module,
        &[root],
    )
    .unwrap()
    .unwrap()
}

#[cfg(test)]
fn tiled_gemm_lds_slice1_descriptor_inputs_for_test() -> (
    CompilerFfiEnvelopeV1,
    Module,
    InertCompilerModuleTextV1,
    TypedDescriptorRootV1,
) {
    use fe2o3_artifacts::{
        PointerWidth, RustDisjointIndexSpaceV1, RustLayoutEvidenceV1, RustPhysicalComponentKindV1,
        RustPhysicalComponentV1, RustPointerMutabilityV1, RustScalarElementTypeV1,
        RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
    };
    use reserved_fe2o3_symbols::GeneratedHostContractIdV3;

    fn layout(kind: GeneralTypedArgumentKindV3) -> RustLayoutEvidenceV1 {
        let (element, disjoint) = match kind {
            GeneralTypedArgumentKindV3::SharedSlice(element) => (element, false),
            GeneralTypedArgumentKindV3::DisjointSlice(element) => (element, true),
            GeneralTypedArgumentKindV3::Scalar(_) => {
                unreachable!("LDS Slice 1 profile has no scalar")
            }
        };
        let shape = if disjoint {
            RustSourceTypeShapeV1::disjoint_slice(element, RustDisjointIndexSpaceV1::Index1D)
        } else {
            RustSourceTypeShapeV1::shared_slice(element)
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
                        mutability: if disjoint {
                            RustPointerMutabilityV1::Mut
                        } else {
                            RustPointerMutabilityV1::Const
                        },
                        pointee: element,
                    },
                )
                .unwrap(),
                RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
            ],
        )
        .unwrap()
    }

    let kinds = [
        GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::U16),
        GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::U16),
        GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
    ];
    let arguments = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| TypedDescriptorArgumentV1 {
            name: format!("arg{index}"),
            kind: descriptor_argument_kind(kind),
            access: match kind {
                GeneralTypedArgumentKindV3::SharedSlice(_) => AccessMode::ReadOnly,
                GeneralTypedArgumentKindV3::DisjointSlice(_) => AccessMode::ReadWrite,
                GeneralTypedArgumentKindV3::Scalar(_) => unreachable!(),
            },
            offset: (index as u32) * 16,
            layout: layout(kind),
        })
        .collect::<Vec<_>>();
    let source_name =
        crate::collected_tiled_gemm_lds_slice1_v1::LDS_SLICE1_KERNEL_EXPORT_V1.to_owned();
    let root = TypedDescriptorRootV1 {
        logical_name: source_name.clone(),
        export_name: source_name,
        profile: TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
            generated_host_contract_identity: GeneratedHostContractIdV3::from_bytes([0x6c; 32]),
        },
        kernel_binding: KernelBindingIdV1::from_bytes([0x4c; 32]),
        arguments: TypedArgumentListV1::new(arguments).unwrap(),
        explicit_argument_bytes: 48,
        kernarg_alignment_bytes: 8,
        source_launch: None,
    };
    let module = fe2o3_kernel_ir::tiled_gemm_lds_v1_module();
    let compiler_module =
        crate::kernel_ir_codegen::construct_inert_tiled_gemm_lds_slice1_module_text(&module)
            .unwrap();
    let target = fe2o3_compiler_ffi::DeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .unwrap();
    (envelope, module, compiler_module, root)
}

#[cfg(test)]
pub(crate) fn tiled_gemm_lds_slice1_descriptor_source_for_test() -> CompilerDescriptorSourceV1 {
    let (envelope, module, compiler_module, root) =
        tiled_gemm_lds_slice1_descriptor_inputs_for_test();
    construct_tiled_gemm_lds_slice1_compiler_descriptor_source_v1(
        &envelope,
        &module,
        &compiler_module,
        &[root],
    )
    .unwrap()
    .unwrap()
}

#[cfg(test)]
pub(crate) fn row_softmax_v1_descriptor_source_for_test() -> CompilerDescriptorSourceV1 {
    use fe2o3_artifacts::{
        PointerWidth, RustDisjointIndexSpaceV1, RustLayoutEvidenceV1, RustPhysicalComponentKindV1,
        RustPhysicalComponentV1, RustPointerMutabilityV1, RustScalarElementTypeV1,
        RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
    };
    use reserved_fe2o3_symbols::GeneratedHostContractIdV3;

    fn layout(kind: GeneralTypedArgumentKindV3) -> RustLayoutEvidenceV1 {
        let (element, disjoint) = match kind {
            GeneralTypedArgumentKindV3::SharedSlice(element) => (element, false),
            GeneralTypedArgumentKindV3::DisjointSlice(element) => (element, true),
            GeneralTypedArgumentKindV3::Scalar(_) => {
                unreachable!("row-softmax profile has no scalar")
            }
        };
        let shape = if disjoint {
            RustSourceTypeShapeV1::disjoint_slice(element, RustDisjointIndexSpaceV1::Index1D)
        } else {
            RustSourceTypeShapeV1::shared_slice(element)
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
                        mutability: if disjoint {
                            RustPointerMutabilityV1::Mut
                        } else {
                            RustPointerMutabilityV1::Const
                        },
                        pointee: element,
                    },
                )
                .unwrap(),
                RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
            ],
        )
        .unwrap()
    }

    let kinds = [
        GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
        GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
    ];
    let names = ["input", "output"];
    let arguments = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| TypedDescriptorArgumentV1 {
            name: names[index].to_owned(),
            kind: descriptor_argument_kind(kind),
            access: match kind {
                GeneralTypedArgumentKindV3::SharedSlice(_) => AccessMode::ReadOnly,
                GeneralTypedArgumentKindV3::DisjointSlice(_) => AccessMode::ReadWrite,
                GeneralTypedArgumentKindV3::Scalar(_) => unreachable!(),
            },
            offset: (index as u32) * 16,
            layout: layout(kind),
        })
        .collect::<Vec<_>>();
    let root = TypedDescriptorRootV1 {
        logical_name: "row_softmax_v1".to_owned(),
        export_name: "row_softmax_v1".to_owned(),
        profile: TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
            generated_host_contract_identity: GeneratedHostContractIdV3::from_bytes([0x73; 32]),
        },
        kernel_binding: KernelBindingIdV1::from_bytes([0x72; 32]),
        arguments: TypedArgumentListV1::new(arguments).unwrap(),
        explicit_argument_bytes: 32,
        kernarg_alignment_bytes: 8,
        source_launch: None,
    };
    let module = crate::collected_row_softmax_v1::canonical_row_softmax_v1_module();
    let compiler_module =
        crate::kernel_ir_codegen::construct_inert_row_softmax_v1_module_text(&module).unwrap();
    let envelope = crate::worker_v2_producer::construct_row_softmax_v1_compiler_envelope(
        crate::collected_row_softmax_v1::exponential_boundary_commitment(),
    )
    .unwrap();
    construct_row_softmax_v1_compiler_descriptor_source_v1(
        &envelope,
        &module,
        &compiler_module,
        &[root],
    )
    .unwrap()
    .unwrap()
}

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
        DeviceFfiContractFieldsV1, DeviceFfiDirectionV1, GeneratedHostContractIdV3,
        derive_device_ffi_contract_id_v1,
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
                    name: if index == 0 {
                        "input_a".to_owned()
                    } else if index == 1 {
                        "input_b".to_owned()
                    } else {
                        "output".to_owned()
                    },
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
                }
            })
            .collect();
        TypedDescriptorRootV1 {
            logical_name: "add".to_owned(),
            export_name: "vecadd".to_owned(),
            profile: TypedKernelProfile::VecAddRustcLayoutV2,
            kernel_binding: KernelBindingIdV1::from_bytes([binding; 32]),
            arguments: TypedArgumentListV1::new(arguments).unwrap(),
            explicit_argument_bytes: 48,
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

    fn descriptor_argument(
        index: usize,
        kind: DescriptorArgumentKindV1,
        offset: u32,
    ) -> TypedDescriptorArgumentV1 {
        let (layout, access) = match kind {
            DescriptorArgumentKindV1::Scalar(_) => (scalar_layout(), AccessMode::ByValue),
            DescriptorArgumentKindV1::SharedSlice(_) => (layout(false), AccessMode::ReadOnly),
            DescriptorArgumentKindV1::DisjointSlice(_) => (layout(true), AccessMode::ReadWrite),
        };
        TypedDescriptorArgumentV1 {
            name: format!("arg{index}"),
            kind,
            access,
            offset,
            layout,
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
            profile: TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
                generated_host_contract_identity: GeneratedHostContractIdV3::from_bytes(
                    [binding; 32],
                ),
            },
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
    fn rust_ownership_only_discharges_aliases_involving_disjoint_slices() {
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
        ];

        assert!(!rust_ownership_discharges_runtime_alias_v1(
            0, 1, &arguments
        ));
        assert!(rust_ownership_discharges_runtime_alias_v1(0, 2, &arguments));
        assert!(rust_ownership_discharges_runtime_alias_v1(2, 1, &arguments));
        assert!(!rust_ownership_discharges_runtime_alias_v1(
            0, 3, &arguments
        ));
        assert!(!rust_ownership_discharges_runtime_alias_v1(
            2, 2, &arguments
        ));
        assert!(!rust_ownership_discharges_runtime_alias_v1(
            2, 8, &arguments
        ));
    }

    fn module() -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = Function::kernel_entry(
            "vecadd_impl",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        );
        let mut kernel = Kernel::new(
            "vecadd",
            "vecadd_impl",
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
    fn constructs_exact_gfx942_cov6_vecadd_descriptor() {
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
        assert_eq!(kernel.logical_name().as_str(), "add");
        assert_eq!(kernel.entry_name().as_str(), "vecadd");
        assert_eq!(kernel.descriptor_symbol().as_str(), "vecadd.kd");
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
    fn exact_vecadd_descriptor_bytes_match_the_v2_compatibility_golden() {
        let envelope = envelope(CodeObjectVersion::V6);
        let module = module();
        let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
        let source =
            construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[root(0x42)])
                .unwrap()
                .unwrap();
        let digest: [u8; 32] = Sha256::digest(source.canonical_bytes()).into();
        let actual = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            actual,
            "92d88cdc6a13474ac5a988bb2f3afa196985c4454343a9886d80c068442445ba"
        );
    }

    #[test]
    fn constructs_two_differing_general_v3_descriptors_and_mixed_v2_v3() {
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

        let mixed_module = module_for(&["vecadd", "alpha"]);
        let mixed_llvm = construct_inert_compiler_module_text_v1(&mixed_module).unwrap();
        let mixed = construct_compiler_descriptor_source_v1(
            &envelope,
            &mixed_module,
            &mixed_llvm,
            &[root(0x42), alpha_root()],
        )
        .unwrap()
        .unwrap();
        assert_eq!(mixed.table().kernels().len(), 2);
        assert_eq!(
            mixed.table().kernels()[0].arguments()[2].access(),
            AccessMode::WriteOnly
        );
        assert_eq!(
            mixed.table().kernels()[1].arguments()[2].access(),
            AccessMode::ReadWrite
        );
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
    fn semantic_witness_plans_select_only_general_v3_roots_in_binding_order() {
        assert!(
            crate::semantic_witness::plans_from_descriptor_roots(&[root(0x42)])
                .unwrap()
                .is_empty()
        );

        let plans = crate::semantic_witness::plans_from_descriptor_roots(&[
            zeta_root(),
            root(0x42),
            alpha_root(),
        ])
        .unwrap();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].kernel_binding().as_bytes(), [0x61; 32]);
        assert_eq!(plans[1].kernel_binding().as_bytes(), [0x7a; 32]);
        assert_ne!(plans[0].payload(), plans[1].payload());
    }

    #[test]
    fn synthetic_roots_retain_distinct_argument_counts_but_vecadd_rejects_them() {
        let one = root_with_layouts(1, vec![layout(false)]);
        let two = root_with_layouts(2, vec![layout(false), layout(true)]);
        assert_eq!(one.arguments.len(), 1);
        assert_eq!(two.arguments.len(), 2);

        let envelope = envelope(CodeObjectVersion::V6);
        let module = module();
        let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
        for (root, actual) in [(one, 1), (two, 2)] {
            assert!(matches!(
                construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[root]),
                Err(CompilerDescriptorError::TypedProfileArgumentCountMismatch {
                    expected: 3,
                    actual: found,
                    ..
                }) if found == actual
            ));
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
    fn exact_lds_slice1_descriptor_projects_source_name_and_binds_static_resources() {
        let source = tiled_gemm_lds_slice1_descriptor_source_for_test();
        let table = source.table();
        assert_eq!(table.device_target().to_string(), "gfx942:xnack-");
        assert_eq!(table.code_object_version(), CodeObjectVersion::V6);
        let [kernel] = table.kernels() else {
            panic!("exact LDS descriptor must contain one kernel");
        };
        assert_eq!(kernel.logical_name().as_str(), "tiled_gemm_lds_slice1");
        assert_eq!(kernel.entry_name().as_str(), "tiled_gemm_lds_v1");
        assert_eq!(kernel.descriptor_symbol().as_str(), "tiled_gemm_lds_v1.kd");
        assert_eq!(kernel.abi_layout().explicit_argument_size(), 48);
        assert_eq!(kernel.abi_layout().kernarg_segment_size(), 304);
        assert_eq!(kernel.abi_layout().kernarg_segment_alignment(), 8);
        assert_eq!(
            kernel.launch().block_size(),
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap())
        );
        assert_eq!(
            kernel.launch().max_grid(),
            DimensionsV1::new(1, 1, 1).unwrap()
        );
        assert_eq!(kernel.launch().static_shared_memory_bytes(), 1024);
        assert_eq!(kernel.launch().max_dynamic_shared_memory_bytes(), 0);
        assert_eq!(
            kernel.capabilities(),
            &[
                CapabilityV1::Subgroup,
                CapabilityV1::WorkgroupMemory,
                CapabilityV1::MatrixMultiply,
                CapabilityV1::AmdWave,
                CapabilityV1::AmdMfma,
            ]
        );
        assert_eq!(
            kernel
                .arguments()
                .iter()
                .map(|argument| argument.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2"]
        );
        assert_eq!(kernel.arguments()[0].access(), AccessMode::ReadOnly);
        assert_eq!(kernel.arguments()[1].access(), AccessMode::ReadOnly);
        assert_eq!(kernel.arguments()[2].access(), AccessMode::ReadWrite);

        let (_, module, compiler_module, root) = tiled_gemm_lds_slice1_descriptor_inputs_for_test();
        let wrong_target = DeviceTargetV1::parse("gfx942:xnack+").unwrap();
        let wrong_envelope = CompilerFfiEnvelopeV1::for_module_without_device_ffi(
            wrong_target,
            CodeObjectVersion::V6,
        )
        .unwrap();
        assert!(matches!(
            construct_tiled_gemm_lds_slice1_compiler_descriptor_source_v1(
                &wrong_envelope,
                &module,
                &compiler_module,
                &[root],
            ),
            Err(CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch("target/COV6"))
        ));
    }

    #[test]
    fn exact_row_softmax_descriptor_is_two_slices_cov6_wg64_and_288_bytes() {
        let source = row_softmax_v1_descriptor_source_for_test();
        let kernels = source.table().kernels();
        assert_eq!(kernels.len(), 1);
        let kernel = &kernels[0];
        assert_eq!(kernel.entry_name().as_str(), "row_softmax_v1");
        assert_eq!(kernel.descriptor_symbol().as_str(), "row_softmax_v1.kd");
        assert_eq!(kernel.abi_layout().explicit_argument_size(), 32);
        assert_eq!(kernel.abi_layout().kernarg_segment_size(), 288);
        assert_eq!(kernel.abi_layout().kernarg_segment_alignment(), 8);
        assert_eq!(kernel.arguments().len(), 2);
        assert_eq!(kernel.arguments()[0].access(), AccessMode::ReadOnly);
        assert_eq!(kernel.arguments()[1].access(), AccessMode::ReadWrite);
        assert_eq!(
            kernel.launch().block_size(),
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap())
        );
        assert_eq!(
            kernel.launch().max_grid(),
            DimensionsV1::new(1, 1, 1).unwrap()
        );
        assert_eq!(kernel.launch().max_flat_workgroup_size(), 64);

        let exact_module = crate::collected_row_softmax_v1::canonical_row_softmax_v1_module();
        let exact_llvm =
            crate::kernel_ir_codegen::construct_inert_row_softmax_v1_module_text(&exact_module)
                .unwrap();
        let envelope = crate::worker_v2_producer::construct_row_softmax_v1_compiler_envelope(
            crate::collected_row_softmax_v1::exponential_boundary_commitment(),
        )
        .unwrap();
        let mut substituted = exact_module;
        substituted.id = "fe2o3::row_softmax_v1_substitution".into();
        assert!(matches!(
            construct_row_softmax_v1_compiler_descriptor_source_v1(
                &envelope,
                &substituted,
                &exact_llvm,
                &[],
            ),
            Err(CompilerDescriptorError::NonCanonicalRowSoftmaxModule)
        ));
    }
}
