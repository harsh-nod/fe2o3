//! Source-component target binding and complete root-roster checks.
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_amdgcn_model::check_production_target_coordinate_preservation_v1 as check_coordinates;

fn profile(target: Target) -> Profile {
    match target {
        Target::Gfx942 => Profile::Gfx942,
        Target::Gfx950 => Profile::Gfx950,
    }
}

pub(super) fn binding(
    prebind: &Owner,
    bound: &Owner,
    target: Target,
    budget: &mut Budget<'_>,
) -> ResultV1<()> {
    let retained;
    let result = {
        let (coordinates, storage) = check_coordinates(prebind, bound, profile(target), budget)
            .map_err(|e| failure(Phase::Bound, e))?;
        retained = storage.retained_storage();
        budget
            .reserve_storage(retained)
            .map_err(|e| failure(Phase::Bound, e))?;
        require(
            std::ptr::eq(coordinates.input(), prebind) && std::ptr::eq(coordinates.output(), bound),
            Phase::Bound,
            "actual prebind/bound coordinate custody",
        )
    };
    budget
        .release_storage(retained)
        .map_err(|e| failure(Phase::Bound, e))?;
    result
}

fn root_roster(original: &Module, bound: &Module, output: &Module, count: usize) -> ResultV1<()> {
    require(
        original.kernels.len() == count
            && bound.kernels.len() == count
            && bound.kernels == output.kernels
            && original.kernels.iter().zip(&bound.kernels).all(|(a, b)| {
                a.id == b.id
                    && a.entry == b.entry
                    && a.domain == b.domain
                    && a.workgroup_size == b.workgroup_size
            }),
        Phase::Abi,
        "ordered source roots/launch and full bound/final kernel roster",
    )
}

pub(super) fn checked_modules<'a>(
    stage: &'a Stage,
    output: &'a Owner,
    count: usize,
) -> ResultV1<[&'a Module; 3]> {
    let modules = [
        stage.original_module(),
        stage.test_bound_owner_v1().module(),
        output.module(),
    ];
    root_roster(modules[0], modules[1], modules[2], count)?;
    Ok(modules)
}

fn neutral() -> Module {
    use fe2o3_kernel_ir::{BasicBlock, BlockId, Kernel, Signature};
    let mut module = Module::new("scalar-source-target-checks");
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
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
    }
    module
}

fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 1024 * 1024 * 1024);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, storage.retained_storage())
}

fn bound(module: &Module, target: Target) -> Module {
    fe2o3_amdgcn_model::bind_production_target_v1(module, profile(target))
        .unwrap()
        .module()
        .clone()
}

fn check_pair(prebind: &Owner, a: usize, output: &Owner, b: usize, target: Target) -> ResultV1<()> {
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 1024 * 1024 * 1024);
    let floor = a + b + 37;
    budget.reserve_storage(floor).unwrap();
    let result = binding(prebind, output, target, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > 0);
    result
}

#[test]
fn exact_prebind_target_delta_and_complete_bound_final_roster_are_required() {
    let original = neutral();
    let (prebind, prebind_storage) = admit(&original);
    for target in [Target::Gfx942, Target::Gfx950] {
        let bound = bound(&original, target);
        assert_ne!(original.kernels, bound.kernels);
        let (owner, storage) = admit(&bound);
        check_pair(&prebind, prebind_storage, &owner, storage, target).unwrap();
        root_roster(&original, &bound, &bound, 2).unwrap();
        for field in 0..6 {
            let mut changed = bound.clone();
            match field {
                0 => changed.kernels.reverse(),
                1 => changed.kernels[0].id = "foreign".into(),
                2 => changed.kernels[0].entry = "second".into(),
                3 => {
                    changed.kernels[0].domain = LaunchDomain::D1 {
                        x: LaunchExtent::Static(1),
                    }
                }
                4 => changed.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1)),
                5 => changed.kernels[0].required_capabilities.clear(),
                _ => unreachable!(),
            }
            assert!(root_roster(&original, &bound, &changed, 2).is_err());
            if field != 5 {
                assert!(root_roster(&original, &changed, &changed, 2).is_err());
            }
        }
        assert!(root_roster(&original, &bound, &bound, 1).is_err());
    }
}

#[test]
fn wrong_missing_and_extra_target_capabilities_refuse_at_every_metadata_level() {
    use fe2o3_kernel_ir::{
        TargetCapability, gfx942_xnack_minus_target_capability,
        gfx950_xnack_minus_target_capability,
    };
    let original = neutral();
    let (prebind, prebind_storage) = admit(&original);
    for target in [Target::Gfx942, Target::Gfx950] {
        let (expected, foreign) = match target {
            Target::Gfx942 => (
                gfx942_xnack_minus_target_capability(),
                gfx950_xnack_minus_target_capability(),
            ),
            Target::Gfx950 => (
                gfx950_xnack_minus_target_capability(),
                gfx942_xnack_minus_target_capability(),
            ),
        };
        for level in 0..3 {
            for mutation in 0..3 {
                let mut changed = bound(&original, target);
                let capabilities = match level {
                    0 => &mut changed.required_capabilities,
                    1 => &mut changed.kernels[0].required_capabilities,
                    2 => &mut changed.functions[0].required_capabilities,
                    _ => unreachable!(),
                };
                assert!(capabilities.contains(&expected));
                match mutation {
                    0 => {
                        assert!(capabilities.remove(&expected));
                        capabilities.insert(foreign.clone());
                    }
                    1 => {
                        assert!(capabilities.remove(&expected));
                    }
                    2 => {
                        capabilities.insert(TargetCapability::Extension {
                            namespace: "scalar.test".into(),
                            name: "unrequested".into(),
                        });
                    }
                    _ => unreachable!(),
                }
                let (owner, storage) = admit(&changed);
                let error =
                    check_pair(&prebind, prebind_storage, &owner, storage, target).unwrap_err();
                assert!(matches!(error.phase, Phase::Bound), "{error:?}");
                assert!(
                    error
                        .detail
                        .starts_with("production target metadata rejected:"),
                    "{error:?}"
                );
            }
        }
    }
}
