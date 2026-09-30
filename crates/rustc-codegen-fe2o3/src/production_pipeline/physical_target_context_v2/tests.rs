use super::*;
use fe2o3_artifacts::{BlockSize, Dimensions, LaunchContract};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Module, Signature,
    Terminator, WorkgroupSize,
};
use fe2o3_mir_model::semantic_mir_v1::*;

fn source_function(name: &str) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let unit = SemanticTypeIdV1::from_index(0);
    let dimensions = SemanticWorkgroupDimensionsV1::new([256, 1, 1]).unwrap();
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([3; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([4; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([5; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([6; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([7; 32]),
        source,
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([8; 32]),
            unit,
            SemanticLocalRoleV1::Return,
            source,
        )],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([9; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(name.as_bytes().to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([10; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ))
}
fn source() -> Module {
    let mut module = Module::new("target_geometry_join");
    for name in ["first", "second"] {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            name,
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            name,
            name,
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(256, 1, 1));
        module.kernels.push(kernel);
    }
    module
}
fn geometries(
    module: &Module,
    grid: u32,
) -> Vec<crate::production_geometry_v1::ProductionGeometryV1> {
    let descriptor = LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
        Dimensions::new(grid, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    module
        .kernels
        .iter()
        .map(|kernel| {
            crate::production_geometry_v1::derive_production_geometry_v1(
                module,
                kernel.id.as_str(),
                &source_function(kernel.entry.as_str()),
                &descriptor,
                "gfx942:xnack-",
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn retained_target_context_uses_checked_descriptor_maximum_not_static_extent() {
    let source = source();
    let profile = fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942;
    let target = dialect_amdgcn::bind_production_target_v1(&source, profile).unwrap();
    for grid in [4, 5] {
        let geometry = geometries(&source, grid);
        assert!(geometry.iter().all(|row| row.max_grid() == [grid, 1, 1]));
        bind(&source, target.module(), profile, &geometry).unwrap();
    }
}

#[test]
fn retained_target_context_requires_exact_root_rank_workgroup_and_complete_geometry() {
    let source = source();
    let profile = fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942;
    let geometry = geometries(&source, 4);
    for mode in 0..6 {
        let mut target = source.clone();
        match mode {
            0 => {
                target.kernels.pop();
            }
            1 => target.kernels.swap(0, 1),
            2 => target.kernels[0].id = fe2o3_kernel_ir::KernelId::new("foreign"),
            3 => target.kernels[0].entry = fe2o3_kernel_ir::FunctionId::new("foreign"),
            4 => target.kernels[0].workgroup_size = Some(WorkgroupSize::new(128, 1, 1)),
            5 => {
                target.kernels[0].domain = LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                }
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            bind(&source, &target, profile, &geometry),
            Err(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure
            ))
        ));
    }
    assert!(matches!(
        bind(&source, &source, profile, &geometry[..1]),
        Err(ProductionPipelineError::Geometry(
            crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure
        ))
    ));
    let production = include_str!("../../production_pipeline.rs")
        .split_once("impl FormalMemoryAdmittedProductionCompilation {")
        .unwrap()
        .1
        .split_once("impl TargetLoweredProductionCompilation {")
        .unwrap()
        .0;
    assert!(
        production
            .find("semantic_entry.kernel_binding_identity()")
            .unwrap()
            < production.find("derive_production_geometry_v1(").unwrap()
    );
    assert!(
        production
            .find("let source_launch = typed_root.source_launch()")
            .unwrap()
            < production.find("derive_production_geometry_v1(").unwrap()
    );
    assert!(!production.contains("LaunchContract::new"));
    assert!(!production.contains(".or_else("));
}
