//! Evidence-derived launch geometry and workgroup-resource accounting.

use std::{collections::BTreeSet, fmt};

use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifacts::{BlockSize, LaunchContract};
use fe2o3_kernel_ir::{
    AddressSpace, FunctionId, LaunchExtent, Module, OperationKind, ScalarType, TargetCapability,
    Type, WorkgroupMemoryExtent,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1;

use crate::production_target_v1::PRODUCTION_TARGET_V1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionGeometryV1 {
    rank: u8,
    workgroup: [u32; 3],
    max_grid: [u32; 3],
    max_flat_workgroup_size: u32,
    static_shared_memory_bytes: u32,
    allow_exact_tiled_matrix: bool,
    allow_workgroup_memory: bool,
}

impl ProductionGeometryV1 {
    pub(crate) const fn rank(self) -> u8 {
        self.rank
    }

    pub(crate) const fn workgroup(self) -> [u32; 3] {
        self.workgroup
    }

    pub(crate) const fn max_grid(self) -> [u32; 3] {
        self.max_grid
    }

    pub(crate) const fn max_flat_workgroup_size(self) -> u32 {
        self.max_flat_workgroup_size
    }

    pub(crate) const fn static_shared_memory_bytes(self) -> u32 {
        self.static_shared_memory_bytes
    }

    pub(crate) const fn allow_exact_tiled_matrix(self) -> bool {
        self.allow_exact_tiled_matrix
    }

    pub(crate) const fn allow_workgroup_memory(self) -> bool {
        self.allow_workgroup_memory
    }
}

pub(crate) fn derive_production_geometry_v1(
    module: &Module,
    semantic_function: &SemanticFunctionDeclV1,
    source_launch: &LaunchContract,
) -> Result<ProductionGeometryV1, ProductionGeometryErrorV1> {
    let entry = semantic_function
        .kernel_entry()
        .ok_or(ProductionGeometryErrorV1::MissingSemanticKernelEntry)?;
    let semantic_launch = entry
        .source_contract()
        .launch()
        .ok_or(ProductionGeometryErrorV1::MissingSourceWorkgroup)?;
    derive_production_geometry_from_launch_v1(
        module,
        semantic_launch.required(),
        semantic_launch.maximum(),
        source_launch,
    )
}

fn derive_production_geometry_from_launch_v1(
    module: &Module,
    required: Option<fe2o3_mir_model::semantic_mir_v1::SemanticWorkgroupDimensionsV1>,
    maximum: Option<fe2o3_mir_model::semantic_mir_v1::SemanticWorkgroupDimensionsV1>,
    source_launch: &LaunchContract,
) -> Result<ProductionGeometryV1, ProductionGeometryErrorV1> {
    let required = required.ok_or(ProductionGeometryErrorV1::DynamicSourceWorkgroup)?;
    if maximum.is_some_and(|maximum| maximum != required) {
        return Err(ProductionGeometryErrorV1::DynamicSourceWorkgroup);
    }
    let workgroup = required.as_array();
    if workgroup.contains(&0) {
        return Err(ProductionGeometryErrorV1::ZeroWorkgroupAxis);
    }
    let rank = source_launch.rank();

    let BlockSize::Exact(source_block) = source_launch.block_size() else {
        return Err(ProductionGeometryErrorV1::NonExactDescriptorWorkgroup);
    };
    let source_workgroup = [source_block.x(), source_block.y(), source_block.z()];
    if source_workgroup != workgroup {
        return Err(ProductionGeometryErrorV1::SourceGeometryMismatch {
            semantic: workgroup,
            descriptor: source_workgroup,
        });
    }
    if source_launch.max_dynamic_shared_memory_bytes() != 0 {
        return Err(ProductionGeometryErrorV1::DynamicWorkgroupMemory);
    }
    let source_grid = source_launch.max_grid();
    let max_grid = [source_grid.x(), source_grid.y(), source_grid.z()];

    let [kernel] = module.kernels.as_slice() else {
        return Err(ProductionGeometryErrorV1::KernelClosure);
    };
    let kir_workgroup = kernel
        .workgroup_size
        .ok_or(ProductionGeometryErrorV1::MissingKirWorkgroup)?;
    let kir_workgroup = [kir_workgroup.x, kir_workgroup.y, kir_workgroup.z];
    if kir_workgroup != workgroup {
        return Err(ProductionGeometryErrorV1::KirWorkgroupMismatch {
            source: workgroup,
            kir: kir_workgroup,
        });
    }
    if kernel.domain.rank() != rank {
        return Err(ProductionGeometryErrorV1::KirRankMismatch {
            source: rank,
            kir: kernel.domain.rank(),
        });
    }
    validate_static_launch_extents(&kernel.domain, workgroup, max_grid)?;

    let max_flat_workgroup_size = workgroup
        .into_iter()
        .try_fold(1_u32, u32::checked_mul)
        .ok_or(ProductionGeometryErrorV1::ArithmeticOverflow(
            "flat workgroup size",
        ))?;
    let target = AmdTargetId::parse(PRODUCTION_TARGET_V1)
        .map_err(|_| ProductionGeometryErrorV1::MissingTargetCapabilities)?
        .capabilities()
        .map_err(|_| ProductionGeometryErrorV1::MissingTargetCapabilities)?;
    validate_target_workgroup(workgroup, target.workgroup_limits())?;

    let static_shared_memory_bytes = static_workgroup_memory_bytes(module)?;
    if source_launch.static_shared_memory_bytes() != static_shared_memory_bytes {
        return Err(ProductionGeometryErrorV1::StaticWorkgroupMemoryMismatch {
            source: source_launch.static_shared_memory_bytes(),
            kir: static_shared_memory_bytes,
        });
    }
    let max_lds = target.max_lds_bytes_per_workgroup();
    if static_shared_memory_bytes > max_lds {
        return Err(ProductionGeometryErrorV1::StaticWorkgroupMemoryLimit {
            actual: static_shared_memory_bytes,
            maximum: max_lds,
        });
    }
    let effective = reachable_effective_capabilities(module)?;
    if effective.contains(&TargetCapability::DynamicWorkgroupMemory) {
        return Err(ProductionGeometryErrorV1::DynamicWorkgroupMemory);
    }
    let allow_workgroup_memory = effective.contains(&TargetCapability::WorkgroupMemory);
    if static_shared_memory_bytes != 0 && !allow_workgroup_memory {
        return Err(ProductionGeometryErrorV1::MissingWorkgroupMemoryCapability);
    }
    let allow_exact_tiled_matrix = effective.contains(&TargetCapability::Extension {
        namespace: fe2o3_kernel_ir::MATRIX_CAPABILITY_NAMESPACE.to_owned(),
        name: fe2o3_kernel_ir::BF16_F32_M16N16K16_CAPABILITY.to_owned(),
    });

    Ok(ProductionGeometryV1 {
        rank,
        workgroup,
        max_grid,
        max_flat_workgroup_size,
        static_shared_memory_bytes,
        allow_exact_tiled_matrix,
        allow_workgroup_memory,
    })
}

fn validate_target_workgroup(
    workgroup: [u32; 3],
    limits: Option<fe2o3_amd_target::WorkgroupLimits>,
) -> Result<(), ProductionGeometryErrorV1> {
    let limits = limits.ok_or(ProductionGeometryErrorV1::MissingTargetCapabilities)?;
    if !limits.supports_dimensions(workgroup[0], workgroup[1], workgroup[2]) {
        return Err(ProductionGeometryErrorV1::UnsupportedTargetWorkgroup(
            workgroup,
        ));
    }
    Ok(())
}

fn validate_static_launch_extents(
    launch: &fe2o3_kernel_ir::LaunchDomain,
    workgroup: [u32; 3],
    max_grid: [u32; 3],
) -> Result<(), ProductionGeometryErrorV1> {
    if workgroup.contains(&0) {
        return Err(ProductionGeometryErrorV1::ZeroWorkgroupAxis);
    }
    for (axis, extent) in launch.extents().enumerate() {
        if let LaunchExtent::Static(global_items) = extent {
            let quotient = global_items / workgroup[axis];
            let blocks = quotient
                .checked_add(u32::from(global_items % workgroup[axis] != 0))
                .ok_or(ProductionGeometryErrorV1::ArithmeticOverflow(
                    "static launch extent",
                ))?;
            if blocks > max_grid[axis] {
                return Err(ProductionGeometryErrorV1::StaticGridMismatch {
                    axis,
                    kir_blocks: blocks,
                    descriptor_blocks: max_grid[axis],
                });
            }
        }
    }
    Ok(())
}

fn static_workgroup_memory_bytes(module: &Module) -> Result<u32, ProductionGeometryErrorV1> {
    let reachable = reachable_function_ids(module)?;
    let mut total = 0_u32;
    for operation in module
        .functions
        .iter()
        .filter(|function| reachable.contains(&function.id))
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        let allocation = match &operation.kind {
            OperationKind::WorkgroupMemory(memory) => {
                let WorkgroupMemoryExtent::Static(elements) = memory.extent else {
                    return Err(ProductionGeometryErrorV1::DynamicWorkgroupMemory);
                };
                Some((&memory.element, elements, memory.alignment))
            }
            OperationKind::Alloca {
                element,
                count,
                address_space: AddressSpace::Workgroup,
                alignment,
            } => {
                if count.is_some() {
                    return Err(ProductionGeometryErrorV1::DynamicWorkgroupMemory);
                }
                Some((element, 1, *alignment))
            }
            _ => None,
        };
        let Some((element, elements, alignment)) = allocation else {
            continue;
        };
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(ProductionGeometryErrorV1::InvalidWorkgroupAlignment(
                alignment,
            ));
        }
        total = total
            .checked_add(alignment - 1)
            .map(|value| value & !(alignment - 1))
            .ok_or(ProductionGeometryErrorV1::ArithmeticOverflow(
                "workgroup allocation alignment",
            ))?;
        let bytes = type_size_bytes(element)?.checked_mul(elements).ok_or(
            ProductionGeometryErrorV1::ArithmeticOverflow("workgroup allocation size"),
        )?;
        total = total
            .checked_add(bytes)
            .ok_or(ProductionGeometryErrorV1::ArithmeticOverflow(
                "static workgroup memory",
            ))?;
    }
    Ok(total)
}

fn reachable_function_ids(
    module: &Module,
) -> Result<BTreeSet<FunctionId>, ProductionGeometryErrorV1> {
    let [kernel] = module.kernels.as_slice() else {
        return Err(ProductionGeometryErrorV1::KernelClosure);
    };
    let functions = module
        .functions
        .iter()
        .map(|function| (&function.id, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut reachable = BTreeSet::new();
    let mut pending = vec![kernel.entry.clone()];
    while let Some(function_id) = pending.pop() {
        if !reachable.insert(function_id.clone()) {
            continue;
        }
        let function = functions
            .get(&function_id)
            .copied()
            .ok_or(ProductionGeometryErrorV1::IncompleteKirCallClosure)?;
        if let Some(body) = &function.body {
            pending.extend(body.blocks.iter().flat_map(|block| {
                block.operations.iter().filter_map(|operation| {
                    if let OperationKind::Call { callee, .. } = &operation.kind {
                        Some(callee.clone())
                    } else {
                        None
                    }
                })
            }));
        }
    }
    Ok(reachable)
}

fn reachable_effective_capabilities(
    module: &Module,
) -> Result<BTreeSet<TargetCapability>, ProductionGeometryErrorV1> {
    let reachable = reachable_function_ids(module)?;
    Ok(module
        .required_capabilities
        .iter()
        .cloned()
        .chain(
            module
                .functions
                .iter()
                .filter(|function| reachable.contains(&function.id))
                .flat_map(|function| function.effective_capabilities()),
        )
        .collect())
}

fn type_size_bytes(value: &Type) -> Result<u32, ProductionGeometryErrorV1> {
    match value {
        Type::Scalar(ScalarType::Bool | ScalarType::I8 | ScalarType::U8) => Ok(1),
        Type::Scalar(ScalarType::I16 | ScalarType::U16 | ScalarType::F16 | ScalarType::Bf16) => {
            Ok(2)
        }
        Type::Scalar(ScalarType::I32 | ScalarType::U32 | ScalarType::F32) => Ok(4),
        Type::Scalar(ScalarType::I64 | ScalarType::U64 | ScalarType::Index | ScalarType::F64)
        | Type::Pointer(_) => Ok(8),
        Type::Scalar(ScalarType::I128 | ScalarType::U128) => Ok(16),
        Type::Unit | Type::Slice(_) => Err(ProductionGeometryErrorV1::UnsizedWorkgroupType),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionGeometryErrorV1 {
    MissingSemanticKernelEntry,
    MissingSourceWorkgroup,
    ZeroWorkgroupAxis,
    DynamicSourceWorkgroup,
    NonExactDescriptorWorkgroup,
    SourceGeometryMismatch {
        semantic: [u32; 3],
        descriptor: [u32; 3],
    },
    KernelClosure,
    IncompleteKirCallClosure,
    MissingKirWorkgroup,
    KirWorkgroupMismatch {
        source: [u32; 3],
        kir: [u32; 3],
    },
    KirRankMismatch {
        source: u8,
        kir: u8,
    },
    StaticGridMismatch {
        axis: usize,
        kir_blocks: u32,
        descriptor_blocks: u32,
    },
    MissingTargetCapabilities,
    UnsupportedTargetWorkgroup([u32; 3]),
    DynamicWorkgroupMemory,
    InvalidWorkgroupAlignment(u32),
    UnsizedWorkgroupType,
    ArithmeticOverflow(&'static str),
    StaticWorkgroupMemoryLimit {
        actual: u32,
        maximum: u32,
    },
    StaticWorkgroupMemoryMismatch {
        source: u32,
        kir: u32,
    },
    MissingWorkgroupMemoryCapability,
}

impl fmt::Display for ProductionGeometryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSemanticKernelEntry => {
                formatter.write_str("semantic root has no authenticated kernel entry")
            }
            Self::MissingSourceWorkgroup => {
                formatter.write_str("source contract has no launch bounds")
            }
            Self::ZeroWorkgroupAxis => formatter.write_str("source workgroup contains a zero axis"),
            Self::DynamicSourceWorkgroup => formatter
                .write_str("production launch requires identical exact source workgroup bounds"),
            Self::NonExactDescriptorWorkgroup => {
                formatter.write_str("authenticated descriptor source has no exact workgroup")
            }
            Self::SourceGeometryMismatch {
                semantic,
                descriptor,
            } => write!(
                formatter,
                "semantic workgroup {semantic:?} disagrees with authenticated descriptor source {descriptor:?}",
            ),
            Self::KernelClosure => {
                formatter.write_str("target-bound KIR must contain exactly one kernel")
            }
            Self::IncompleteKirCallClosure => {
                formatter.write_str("target-bound KIR call closure references a missing function")
            }
            Self::MissingKirWorkgroup => {
                formatter.write_str("target-bound KIR has no exact workgroup size")
            }
            Self::KirWorkgroupMismatch { source, kir } => write!(
                formatter,
                "target-bound KIR workgroup {kir:?} disagrees with source workgroup {source:?}",
            ),
            Self::KirRankMismatch { source, kir } => write!(
                formatter,
                "target-bound KIR launch rank {kir} disagrees with source rank {source}",
            ),
            Self::StaticGridMismatch {
                axis,
                kir_blocks,
                descriptor_blocks,
            } => write!(
                formatter,
                "static KIR launch axis {axis} requires {kir_blocks} block(s), but source descriptor allows {descriptor_blocks}",
            ),
            Self::MissingTargetCapabilities => {
                formatter.write_str("gfx942 target resource capabilities are unavailable")
            }
            Self::UnsupportedTargetWorkgroup(workgroup) => write!(
                formatter,
                "workgroup {workgroup:?} exceeds reviewed gfx942 limits",
            ),
            Self::DynamicWorkgroupMemory => {
                formatter.write_str("production-v1 does not support dynamic workgroup memory")
            }
            Self::InvalidWorkgroupAlignment(alignment) => write!(
                formatter,
                "workgroup allocation alignment {alignment} is not a nonzero power of two",
            ),
            Self::UnsizedWorkgroupType => {
                formatter.write_str("workgroup allocation has no static target size")
            }
            Self::ArithmeticOverflow(resource) => {
                write!(
                    formatter,
                    "{resource} overflows the production resource model"
                )
            }
            Self::StaticWorkgroupMemoryLimit { actual, maximum } => write!(
                formatter,
                "static workgroup memory requires {actual} bytes, exceeding gfx942 limit {maximum}",
            ),
            Self::StaticWorkgroupMemoryMismatch { source, kir } => write!(
                formatter,
                "source launch claims {source} static workgroup-memory bytes, but reachable KIR requires {kir}",
            ),
            Self::MissingWorkgroupMemoryCapability => formatter.write_str(
                "static workgroup allocation is missing its KIR workgroup-memory capability",
            ),
        }
    }
}

impl std::error::Error for ProductionGeometryErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifacts::Dimensions;
    use fe2o3_kernel_ir::{
        AccessMode, BasicBlock, BlockId, Function, Kernel, LaunchDomain, Operation, Signature,
        Terminator, ValueDef, ValueId, WorkgroupMemory, WorkgroupSize,
    };
    use fe2o3_mir_model::semantic_mir_v1::SemanticWorkgroupDimensionsV1;

    fn launch(rank: u8, workgroup: [u32; 3], static_lds: u32, dynamic_lds: u32) -> LaunchContract {
        let max_grid = match rank {
            1 => [u32::MAX, 1, 1],
            2 => [u32::MAX, u32::MAX, 1],
            3 => [u32::MAX, u32::from(u16::MAX), u32::from(u16::MAX)],
            _ => unreachable!(),
        };
        launch_with_grid(rank, workgroup, max_grid, static_lds, dynamic_lds)
    }

    fn launch_with_grid(
        rank: u8,
        workgroup: [u32; 3],
        max_grid: [u32; 3],
        static_lds: u32,
        dynamic_lds: u32,
    ) -> LaunchContract {
        LaunchContract::new(
            rank,
            BlockSize::Exact(Dimensions::new(workgroup[0], workgroup[1], workgroup[2]).unwrap()),
            Dimensions::new(max_grid[0], max_grid[1], max_grid[2]).unwrap(),
            static_lds,
            dynamic_lds,
        )
        .unwrap()
    }

    fn memory_helper(id: &str, memory: WorkgroupMemory) -> Function {
        let pointer = Type::pointer(
            memory.element.clone(),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        );
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(0), pointer)],
            OperationKind::WorkgroupMemory(memory),
        ));
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        Function::internal_helper(
            id,
            Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![block],
        )
    }

    fn call(module: &mut Module, callee: &str) {
        module.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::new(
                Vec::new(),
                OperationKind::Call {
                    callee: callee.into(),
                    arguments: Vec::new(),
                },
            ));
    }

    fn module(rank: u8, workgroup: [u32; 3], memory: Option<WorkgroupMemory>) -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        if let Some(memory) = memory {
            let pointer = Type::pointer(
                memory.element.clone(),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            );
            block.operations.push(Operation::new(
                vec![ValueDef::new(ValueId(0), pointer)],
                OperationKind::WorkgroupMemory(memory),
            ));
        }
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        let entry = Function::kernel_entry(
            "entry",
            Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![block],
        );
        let launch = match rank {
            1 => LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
            2 => LaunchDomain::D2 {
                x: LaunchExtent::Dynamic,
                y: LaunchExtent::Dynamic,
            },
            3 => LaunchDomain::D3 {
                x: LaunchExtent::Dynamic,
                y: LaunchExtent::Dynamic,
                z: LaunchExtent::Dynamic,
            },
            _ => unreachable!(),
        };
        let mut kernel = Kernel::new("kernel", "entry", launch);
        kernel.workgroup_size = Some(WorkgroupSize::new(workgroup[0], workgroup[1], workgroup[2]));
        let mut module = Module::new("geometry_test");
        module.functions.push(entry);
        module.kernels.push(kernel);
        module
    }

    fn derive(
        module: &Module,
        rank: u8,
        workgroup: [u32; 3],
    ) -> Result<ProductionGeometryV1, ProductionGeometryErrorV1> {
        let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
        derive_production_geometry_from_launch_v1(
            module,
            Some(dimensions),
            Some(dimensions),
            &launch(rank, workgroup, 0, 0),
        )
    }

    #[test]
    fn derives_one_two_three_dimensional_and_multi_wave_geometry() {
        for (rank, workgroup) in [
            (1, [64, 1, 1]),
            (1, [128, 1, 1]),
            (2, [64, 1, 1]),
            (2, [8, 8, 1]),
            (2, [16, 16, 1]),
            (3, [64, 1, 1]),
            (3, [4, 4, 4]),
        ] {
            let geometry = derive(&module(rank, workgroup, None), rank, workgroup).unwrap();
            assert_eq!(geometry.rank(), rank);
            assert_eq!(geometry.workgroup(), workgroup);
            assert_eq!(
                geometry.max_flat_workgroup_size(),
                workgroup.into_iter().product::<u32>()
            );
            assert_eq!(geometry.static_shared_memory_bytes(), 0);
        }
    }

    #[test]
    fn derives_checked_static_lds_bytes_and_workgroup_capability() {
        let memory = WorkgroupMemory {
            element: Type::Scalar(ScalarType::U32),
            extent: WorkgroupMemoryExtent::Static(256),
            alignment: 16,
        };
        let workgroup = [128, 1, 1];
        let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
        let geometry = derive_production_geometry_from_launch_v1(
            &module(1, workgroup, Some(memory)),
            Some(dimensions),
            Some(dimensions),
            &launch(1, workgroup, 1024, 0),
        )
        .unwrap();
        assert_eq!(geometry.static_shared_memory_bytes(), 1024);
        assert!(geometry.allow_workgroup_memory());
    }

    #[test]
    fn barrier_capability_does_not_authorize_workgroup_memory() {
        let workgroup = [64, 1, 1];
        let mut module = module(1, workgroup, None);
        module
            .required_capabilities
            .insert(TargetCapability::WorkgroupBarrier);
        let geometry = derive(&module, 1, workgroup).unwrap();
        assert!(!geometry.allow_workgroup_memory());
    }

    #[test]
    fn exact_matrix_permission_requires_the_exact_extension() {
        let workgroup = [64, 1, 1];
        let mut bf16_only = module(1, workgroup, None);
        bf16_only
            .required_capabilities
            .insert(TargetCapability::BFloat16);
        assert!(
            !derive(&bf16_only, 1, workgroup)
                .unwrap()
                .allow_exact_tiled_matrix()
        );

        let mut wrong_extension = module(1, workgroup, None);
        wrong_extension
            .required_capabilities
            .insert(TargetCapability::Extension {
                namespace: fe2o3_kernel_ir::MATRIX_CAPABILITY_NAMESPACE.to_owned(),
                name: "bf16_f32_m32n32k8".to_owned(),
            });
        assert!(
            !derive(&wrong_extension, 1, workgroup)
                .unwrap()
                .allow_exact_tiled_matrix()
        );

        let mut exact = module(1, workgroup, None);
        exact
            .required_capabilities
            .insert(TargetCapability::Extension {
                namespace: fe2o3_kernel_ir::MATRIX_CAPABILITY_NAMESPACE.to_owned(),
                name: fe2o3_kernel_ir::BF16_F32_M16N16K16_CAPABILITY.to_owned(),
            });
        assert!(
            derive(&exact, 1, workgroup)
                .unwrap()
                .allow_exact_tiled_matrix()
        );
    }

    #[test]
    fn reachable_helper_resources_are_counted_and_unreachable_resources_are_excluded() {
        let workgroup = [64, 1, 1];
        let static_memory = WorkgroupMemory {
            element: Type::Scalar(ScalarType::U32),
            extent: WorkgroupMemoryExtent::Static(256),
            alignment: 16,
        };
        let mut reachable = module(1, workgroup, None);
        call(&mut reachable, "helper");
        reachable
            .functions
            .push(memory_helper("helper", static_memory.clone()));
        let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
        let geometry = derive_production_geometry_from_launch_v1(
            &reachable,
            Some(dimensions),
            Some(dimensions),
            &launch(1, workgroup, 1024, 0),
        )
        .unwrap();
        assert_eq!(geometry.static_shared_memory_bytes(), 1024);

        let dynamic_memory = WorkgroupMemory {
            element: Type::Scalar(ScalarType::U32),
            extent: WorkgroupMemoryExtent::Dynamic,
            alignment: 4,
        };
        let mut unreachable = module(1, workgroup, None);
        unreachable
            .functions
            .push(memory_helper("unused", dynamic_memory));
        assert_eq!(
            derive(&unreachable, 1, workgroup)
                .unwrap()
                .static_shared_memory_bytes(),
            0
        );
    }

    #[test]
    fn reachable_call_closure_is_cycle_safe_and_rejects_missing_callees() {
        let workgroup = [64, 1, 1];
        let mut cyclic = module(1, workgroup, None);
        call(&mut cyclic, "helper");
        let mut helper = memory_helper(
            "helper",
            WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::Static(1),
                alignment: 4,
            },
        );
        helper.body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::new(
                Vec::new(),
                OperationKind::Call {
                    callee: "helper".into(),
                    arguments: Vec::new(),
                },
            ));
        cyclic.functions.push(helper);
        let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
        assert_eq!(
            derive_production_geometry_from_launch_v1(
                &cyclic,
                Some(dimensions),
                Some(dimensions),
                &launch(1, workgroup, 4, 0),
            )
            .unwrap()
            .static_shared_memory_bytes(),
            4
        );

        let mut missing = module(1, workgroup, None);
        call(&mut missing, "absent");
        assert_eq!(
            derive(&missing, 1, workgroup),
            Err(ProductionGeometryErrorV1::IncompleteKirCallClosure)
        );
    }

    #[test]
    fn target_limits_and_invalid_lds_alignment_fail_closed() {
        let oversized = [1025, 1, 1];
        let target = AmdTargetId::parse(PRODUCTION_TARGET_V1)
            .unwrap()
            .capabilities()
            .unwrap();
        assert_eq!(
            validate_target_workgroup(oversized, target.workgroup_limits()),
            Err(ProductionGeometryErrorV1::UnsupportedTargetWorkgroup(
                oversized
            ))
        );

        let memory = WorkgroupMemory {
            element: Type::Scalar(ScalarType::U32),
            extent: WorkgroupMemoryExtent::Static(1),
            alignment: 3,
        };
        assert_eq!(
            derive(&module(1, [64, 1, 1], Some(memory)), 1, [64, 1, 1]),
            Err(ProductionGeometryErrorV1::InvalidWorkgroupAlignment(3))
        );
    }

    #[test]
    fn static_grid_rounding_accepts_limits_and_rejects_excess_blocks() {
        let workgroup = [64, 1, 1];
        let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
        let mut module = module(1, workgroup, None);
        module.kernels[0].domain = LaunchDomain::D1 {
            x: LaunchExtent::Static(129),
        };
        let geometry = derive_production_geometry_from_launch_v1(
            &module,
            Some(dimensions),
            Some(dimensions),
            &launch_with_grid(1, workgroup, [4, 1, 1], 0, 0),
        )
        .unwrap();
        assert_eq!(geometry.max_grid(), [4, 1, 1]);
        assert_eq!(
            derive_production_geometry_from_launch_v1(
                &module,
                Some(dimensions),
                Some(dimensions),
                &launch_with_grid(1, workgroup, [2, 1, 1], 0, 0),
            ),
            Err(ProductionGeometryErrorV1::StaticGridMismatch {
                axis: 0,
                kir_blocks: 3,
                descriptor_blocks: 2,
            })
        );

        module.kernels[0].domain = LaunchDomain::D1 {
            x: LaunchExtent::Static(u32::MAX),
        };
        derive_production_geometry_from_launch_v1(
            &module,
            Some(dimensions),
            Some(dimensions),
            &launch_with_grid(1, workgroup, [u32::MAX, 1, 1], 0, 0),
        )
        .unwrap();
    }

    #[test]
    fn rejects_missing_mismatched_and_wrong_rank_kir_geometry() {
        let workgroup = [8, 8, 1];
        let mut missing = module(2, workgroup, None);
        missing.kernels[0].workgroup_size = None;
        assert_eq!(
            derive(&missing, 2, workgroup),
            Err(ProductionGeometryErrorV1::MissingKirWorkgroup)
        );

        let mut mismatch = module(2, workgroup, None);
        mismatch.kernels[0].workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        assert!(matches!(
            derive(&mismatch, 2, workgroup),
            Err(ProductionGeometryErrorV1::KirWorkgroupMismatch { .. })
        ));

        let mut wrong_rank = module(2, workgroup, None);
        wrong_rank.kernels[0].domain = LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        };
        assert_eq!(
            derive(&wrong_rank, 2, workgroup),
            Err(ProductionGeometryErrorV1::KirRankMismatch { source: 2, kir: 1 })
        );
    }

    #[test]
    fn rejects_dynamic_and_overflowing_workgroup_memory() {
        for extent in [
            WorkgroupMemoryExtent::Dynamic,
            WorkgroupMemoryExtent::DynamicAtLeast(1),
        ] {
            let memory = WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent,
                alignment: 4,
            };
            assert_eq!(
                derive(&module(1, [64, 1, 1], Some(memory)), 1, [64, 1, 1]),
                Err(ProductionGeometryErrorV1::DynamicWorkgroupMemory)
            );
        }

        let overflow = WorkgroupMemory {
            element: Type::Scalar(ScalarType::U128),
            extent: WorkgroupMemoryExtent::Static(u32::MAX),
            alignment: 16,
        };
        assert_eq!(
            derive(&module(1, [64, 1, 1], Some(overflow)), 1, [64, 1, 1],),
            Err(ProductionGeometryErrorV1::ArithmeticOverflow(
                "workgroup allocation size"
            ))
        );
    }

    #[test]
    fn rejects_dynamic_descriptor_lds_and_nonidentical_source_bounds() {
        let workgroup = [64, 1, 1];
        let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
        assert_eq!(
            derive_production_geometry_from_launch_v1(
                &module(1, workgroup, None),
                Some(dimensions),
                Some(dimensions),
                &launch(1, workgroup, 0, 1),
            ),
            Err(ProductionGeometryErrorV1::DynamicWorkgroupMemory)
        );
        let maximum = SemanticWorkgroupDimensionsV1::new([128, 1, 1]).unwrap();
        assert_eq!(
            derive_production_geometry_from_launch_v1(
                &module(1, workgroup, None),
                Some(dimensions),
                Some(maximum),
                &launch(1, workgroup, 0, 0),
            ),
            Err(ProductionGeometryErrorV1::DynamicSourceWorkgroup)
        );

        for block_size in [
            BlockSize::Any,
            BlockSize::AtMost(Dimensions::new(64, 1, 1).unwrap()),
        ] {
            let nonexact = LaunchContract::new(
                1,
                block_size,
                Dimensions::new(u32::MAX, 1, 1).unwrap(),
                0,
                0,
            )
            .unwrap();
            assert_eq!(
                derive_production_geometry_from_launch_v1(
                    &module(1, workgroup, None),
                    Some(dimensions),
                    Some(dimensions),
                    &nonexact,
                ),
                Err(ProductionGeometryErrorV1::NonExactDescriptorWorkgroup)
            );
        }
    }

    #[test]
    fn static_extent_check_rejects_zero_axis_before_rounding() {
        assert_eq!(
            validate_static_launch_extents(
                &LaunchDomain::D1 {
                    x: LaunchExtent::Static(1),
                },
                [0, 1, 1],
                [1, 1, 1],
            ),
            Err(ProductionGeometryErrorV1::ZeroWorkgroupAxis)
        );
    }

    #[test]
    fn rejects_source_and_kir_static_lds_mismatch() {
        let workgroup = [64, 1, 1];
        let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
        let memory = WorkgroupMemory {
            element: Type::Scalar(ScalarType::U32),
            extent: WorkgroupMemoryExtent::Static(256),
            alignment: 16,
        };
        assert_eq!(
            derive_production_geometry_from_launch_v1(
                &module(1, workgroup, Some(memory)),
                Some(dimensions),
                Some(dimensions),
                &launch(1, workgroup, 0, 0),
            ),
            Err(ProductionGeometryErrorV1::StaticWorkgroupMemoryMismatch {
                source: 0,
                kir: 1024,
            })
        );
    }
}
