fn semantic_kernel_metadata_v1(
    symbol: &str,
    entry_function: &Function,
    required_workgroup: Option<[u32; 3]>,
    launch_rank: u8,
    authenticated_launch: Option<RetainedRankedLaunchRootV1>,
) -> Result<Kernel, ProductionSemanticKirErrorV1> {
    let dimensions = required_workgroup;
    let workgroup_extents = dimensions.map(|dimensions| dimensions.map(u64::from));
    let retained_extent = |axis: usize| {
        authenticated_launch
            .filter(|layout| {
                layout.full_physical_workgroups
                    && workgroup_extents == Some(layout.workgroup_extents)
                    && layout.global_extents[axis] == layout.workgroup_extents[axis]
            })
            .and_then(|layout| u32::try_from(layout.global_extents[axis]).ok())
            .filter(|extent| *extent != 0)
            .map_or(LaunchExtent::Dynamic, LaunchExtent::Static)
    };
    let launch = match (launch_rank, dimensions) {
        (1, Some([_, 1, 1]) | None) => LaunchDomain::D1 {
            x: retained_extent(0),
        },
        (2, Some([_, _, 1]) | None) => LaunchDomain::D2 {
            x: retained_extent(0),
            y: retained_extent(1),
        },
        (3, Some(_) | None) => LaunchDomain::D3 {
            x: retained_extent(0),
            y: retained_extent(1),
            z: retained_extent(2),
        },
        _ => {
            return Err(unsupported(
                0,
                None,
                None,
                "authenticated launch rank disagrees with source workgroup axes",
            ));
        }
    };
    let mut kernel = Kernel::new(symbol, entry_function.id.clone(), launch);
    if let Some([x, y, z]) = required_workgroup {
        kernel.workgroup_size = Some(WorkgroupSize::new(x, y, z));
    }
    kernel
        .required_capabilities
        .extend(entry_function.required_capabilities.iter().cloned());
    Ok(kernel)
}

/// Common final root shell for ordinary and source-owned complete-body imports.
/// This helper derives metadata from the actual retained root/function/launch;
/// it does not authenticate a raw Function or create a checked output owner.
fn finish_semantic_root_module_v1(
    module: &mut Module,
    symbol: &str,
    required_workgroup: Option<[u32; 3]>,
    launch_rank: u8,
    authenticated_launch: Option<RetainedRankedLaunchRootV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if !module.kernels.is_empty() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let entry = module
        .function(&FunctionId::new(symbol))
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let kernel = semantic_kernel_metadata_v1(
        symbol,
        entry,
        required_workgroup,
        launch_rank,
        authenticated_launch,
    )?;
    module
        .kernels
        .try_reserve_exact(1)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    module.kernels.push(kernel);
    Ok(())
}
