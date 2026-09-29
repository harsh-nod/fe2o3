//! Mathematical coordinate bounds from the checked production launch geometry.

use super::*;

impl ProductionGeometryV1 {
    // Never clamp these physical bounds to a KIR Static extent. Host dispatch
    // accepts every grid up to max_grid, and target lowering adds no such guard.
    // The physical-envelope extractor preserves source identity while covering
    // these bounds; the old Exact formal API still requires static equality.
    pub(crate) fn formal_coordinate_envelope_v2(
        self,
    ) -> Result<[u64; 3], ProductionGeometryErrorV1> {
        let mut extents = [1_u64; 3];
        if !(1..=3).contains(&self.rank) {
            return Err(ProductionGeometryErrorV1::KernelClosure);
        }
        for (axis, extent) in extents.iter_mut().enumerate() {
            *extent = u64::from(self.workgroup[axis])
                .checked_mul(u64::from(self.max_grid[axis]))
                .ok_or(ProductionGeometryErrorV1::ArithmeticOverflow(
                    "formal global coordinate envelope",
                ))?;
            if *extent == 0 || (axis >= usize::from(self.rank) && *extent != 1) {
                return Err(ProductionGeometryErrorV1::KernelClosure);
            }
        }
        // D1 lowering zero-extends group/local IDs and computes group*WG+local
        // in i64. The checked product bounds every legal coordinate without
        // overflow. Higher-rank lowering uses live dispatch strides: do not
        // infer X uniqueness for padded multidimensional launches. Singleton
        // remaining axes are enough, regardless of the declared source rank.
        if extents[1] != 1 || extents[2] != 1 {
            return Err(
                ProductionGeometryErrorV1::UnsupportedFormalCoordinateEnvelope {
                    rank: self.rank,
                    extents,
                },
            );
        }
        Ok(extents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifacts::Dimensions;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, ExplicitLaunchExtent, FormalIndexWidth, FormalMemoryIncompleteReason,
        FormalMemoryObligationAnalysis, FormalPhysicalLaunchEnvelopeV2, Function, Kernel,
        LaunchDomain, Signature, Terminator, WorkgroupSize,
        derive_kernel_memory_obligations_from_verified_for_launch,
        derive_kernel_memory_obligations_from_verified_for_physical_envelope_v2, verify_module_ref,
    };
    use fe2o3_mir_model::semantic_mir_v1::SemanticWorkgroupDimensionsV1;

    fn geometry(rank: u8, workgroup: [u32; 3], max_grid: [u32; 3]) -> ProductionGeometryV1 {
        ProductionGeometryV1 {
            rank,
            workgroup,
            max_grid,
            max_flat_workgroup_size: 1024,
            static_shared_memory_bytes: 0,
            allow_exact_tiled_matrix: false,
            allow_workgroup_memory: false,
        }
    }

    #[test]
    fn legal_coordinate_envelope_covers_every_group_local_coordinate() {
        for workgroup in [1_u32, 64, 256, 1024] {
            for groups in [1_u32, 2, 19, u32::MAX] {
                let envelope = geometry(1, [workgroup, 1, 1], [groups, 1, 1])
                    .formal_coordinate_envelope_v2()
                    .unwrap();
                let last = u64::from(groups - 1) * u64::from(workgroup) + u64::from(workgroup - 1);
                assert_eq!(envelope, [last + 1, 1, 1]);
                assert!(last < u64::MAX);
            }
        }
    }

    #[test]
    fn multidimensional_x_guard_is_not_a_single_invocation_proof() {
        for (rank, group, grid) in [
            (2, [64, 1, 1], [2, 2, 1]),
            (2, [1, 2, 1], [2, 1, 1]),
            (3, [64, 1, 1], [2, 1, 2]),
            (3, [1, 1, 2], [2, 1, 1]),
        ] {
            assert!(matches!(
                geometry(rank, group, grid).formal_coordinate_envelope_v2(),
                Err(ProductionGeometryErrorV1::UnsupportedFormalCoordinateEnvelope { .. })
            ));
        }
        for rank in [1, 2, 3] {
            assert_eq!(
                geometry(rank, [64, 1, 1], [17, 1, 1])
                    .formal_coordinate_envelope_v2()
                    .unwrap(),
                [1088, 1, 1]
            );
        }
    }

    #[test]
    fn invalid_coordinate_rank_zero_and_inactive_axes_are_not_envelopes() {
        for geometry in [
            geometry(0, [64, 1, 1], [1, 1, 1]),
            geometry(4, [64, 1, 1], [1, 1, 1]),
            geometry(1, [0, 1, 1], [1, 1, 1]),
            geometry(1, [64, 1, 1], [0, 1, 1]),
            geometry(1, [64, 1, 1], [1, 2, 1]),
            geometry(2, [64, 1, 2], [1, 1, 1]),
        ] {
            assert_eq!(
                geometry.formal_coordinate_envelope_v2(),
                Err(ProductionGeometryErrorV1::KernelClosure)
            );
        }
    }

    fn composed_noop(
        extent: LaunchExtent,
        max_groups: u32,
        physical_coverage: bool,
    ) -> (ProductionGeometryV1, FormalMemoryObligationAnalysis) {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        let mut module = Module::new("static_envelope_composition");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![block],
        ));
        let mut kernel = Kernel::new("entry", "entry", LaunchDomain::D1 { x: extent });
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        let workgroup = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
        let launch = LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(64, 1, 1).unwrap()),
            Dimensions::new(max_groups, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        // Exercise the real source/KIR/descriptor/target geometry checks. This
        // value fixture is not a replacement for live rustc target custody.
        let geometry = derive_production_geometry_from_launch_for_target_v1(
            &module,
            "entry",
            Some(workgroup),
            Some(workgroup),
            &launch,
            "gfx942",
        )
        .unwrap();
        let extents = geometry.formal_coordinate_envelope_v2().unwrap();
        let original = module.clone();
        let report = if physical_coverage {
            derive_kernel_memory_obligations_from_verified_for_physical_envelope_v2(
                verify_module_ref(&module).unwrap(),
                &module.kernels[0].id,
                FormalPhysicalLaunchEnvelopeV2::new(1, extents),
                FormalIndexWidth::Bits64,
            )
        } else {
            derive_kernel_memory_obligations_from_verified_for_launch(
                verify_module_ref(&module).unwrap(),
                &module.kernels[0].id,
                ExplicitLaunchExtent::Exact { rank: 1, extents },
                FormalIndexWidth::Bits64,
            )
        }
        .unwrap();
        assert_eq!(module, original);
        (geometry, report)
    }

    #[test]
    fn composed_exact_static_and_dynamic_geometry_preserve_fresh_formal_analysis() {
        for (extent, groups) in [
            (LaunchExtent::Static(64), 1),
            (LaunchExtent::Static(128), 2),
            (LaunchExtent::Dynamic, 17),
        ] {
            let (geometry, report) = composed_noop(extent, groups, false);
            assert_eq!(
                geometry.formal_coordinate_envelope_v2().unwrap(),
                [u64::from(groups) * 64, 1, 1]
            );
            assert!(matches!(
                report,
                FormalMemoryObligationAnalysis::Complete(_)
            ));
        }
    }

    #[test]
    fn composed_extra_grid_and_padded_static_domains_remain_unqualified_by_exact_api() {
        for (static_items, groups, physical_items) in [(64, 17, 1088), (65, 2, 128)] {
            let (geometry, report) =
                composed_noop(LaunchExtent::Static(static_items), groups, false);
            assert_eq!(
                geometry.formal_coordinate_envelope_v2().unwrap(),
                [physical_items, 1, 1]
            );
            let FormalMemoryObligationAnalysis::Incomplete { reasons, .. } = report else {
                panic!("static KIR is not authority to clamp physical invocation coverage");
            };
            assert_eq!(
                reasons,
                vec![FormalMemoryIncompleteReason::StaticLaunchExtentMismatch {
                    expected: static_items,
                    actual: physical_items,
                }]
            );
        }
    }

    #[test]
    fn composed_physical_geometry_analyzes_padded_and_extra_grid_noops_without_clamping() {
        for (static_items, groups, physical_items) in [(64, 17, 1088), (65, 2, 128), (128, 2, 128)]
        {
            let (geometry, report) =
                composed_noop(LaunchExtent::Static(static_items), groups, true);
            assert_eq!(
                geometry.formal_coordinate_envelope_v2().unwrap(),
                [physical_items, 1, 1]
            );
            assert!(report.is_complete());
            assert_eq!(
                report.obligations().invocations().unwrap().end_exclusive(),
                physical_items
            );
        }
    }
}
