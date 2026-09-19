// Genuine constructed source/N/E with global effects and a surviving Load.
use super::*;
#[path = "production_checked_output_owned_redundant_store_erased_v1_tests.rs"]
mod owned_tests;

fn last_store_before(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
    let body = owner.module().functions[coordinate.block.function.0 as usize]
        .body
        .as_ref()
        .expect("retained Load belongs to a defined function");
    let block = &body.blocks[coordinate.block.block as usize];
    let OperationKind::Load { pointer, .. } = block.operations[coordinate.operation as usize].kind
    else {
        panic!("actual retained Load");
    };
    let ordinal = block.operations[..coordinate.operation as usize].iter().rposition(|operation| {
        matches!(operation.kind, OperationKind::Store { pointer: stored, .. } if stored == pointer)
    }).unwrap();
    fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
        operation: ordinal as u32,
        ..coordinate
    }
}

fn redundant_fixture(
    expected: bool,
    roots: usize,
    kill: Option<SemanticStatementKindV1>,
) -> Fixture6 {
    let FinalFixture {
        source,
        bound,
        checked,
        bound_storage,
        ..
    } = final_fixture_from(
        erased_effect_fixture_mode(expected, roots, kill, true, true, true),
        None,
    );
    drop(checked);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(FLOOR + source.retained_storage_floor_v1() + bound_storage)
        .unwrap();
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, &mut budget)
            .unwrap();
    budget.reserve_storage(checked.retained_storage()).unwrap();
    continue6(Fixture5 {
        source,
        bound,
        checked,
        floor: budget.storage(),
        bound_storage,
    })
}

#[test]
fn redundant_store_erased_actual_deletion_rechecks_final_load_and_original_source() {
    for expected in [false, true] {
        for roots in [1, 2] {
            let input = redundant_fixture(expected, roots, None);
            let floor = input.floor;
            let owner = admit6(input, WORK, STORAGE).0.unwrap();
            assert_ne!(
                owner
                    .original_source()
                    .executable()
                    .canonical()
                    .canonical_bytes(),
                owner.erased().canonical().canonical_bytes()
            );
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let deletion =
                fe2o3_kernel_opt::optimize_checked_redundant_store_v1(owner.output(), &mut budget)
                    .unwrap();
            assert_eq!(deletion.rows().len(), roots);
            assert_ne!(
                owner.output().canonical().canonical_bytes(),
                deletion.output().canonical().canonical_bytes()
            );
            let private_loads = |module: &fe2o3_kernel_ir::Module| {
                module.functions.iter().filter_map(|f| f.body.as_ref()).flat_map(|body| &body.blocks).flat_map(|b| &b.operations)
                .filter(|op| matches!(op.kind, OperationKind::Load { access, .. } if access.address_space == fe2o3_kernel_ir::AddressSpace::Private)).count()
            };
            assert_eq!(private_loads(owner.output().module()), roots);
            assert_eq!(private_loads(deletion.output().module()), roots);
            budget.reserve_storage(deletion.retained_storage()).unwrap();
            let (final_inventory, inventory_storage) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(
                    deletion.output(),
                    &mut budget,
                )
                .unwrap();
            budget
                .reserve_storage(inventory_storage.retained_storage())
                .unwrap();
            let mut shifted_initializers = 0;
            for (actual, retained) in final_inventory
                .operations()
                .iter()
                .zip(deletion.retained_operations())
            {
                if matches!(actual.operation.kind, OperationKind::Load { access, .. } if access.address_space == fe2o3_kernel_ir::AddressSpace::Private)
                {
                    let old_store = last_store_before(owner.output(), retained.input);
                    let new_store = last_store_before(deletion.output(), retained.output);
                    let removed = deletion
                        .rows()
                        .iter()
                        .find(|row| row.removed == old_store)
                        .unwrap();
                    assert!(
                        deletion
                            .retained_operations()
                            .iter()
                            .any(|row| row.input == removed.anchor && row.output == new_store)
                    );
                    shifted_initializers += 1;
                }
            }
            assert_eq!(shifted_initializers, roots);
            drop(final_inventory);
            budget
                .release_storage(inventory_storage.retained_storage())
                .unwrap();
            let before = budget.storage();
            let storage = {
                let (checked, storage) = owner
                    .check_redundant_store_output_v1(&deletion, &mut budget)
                    .unwrap();
                assert_eq!(budget.storage(), before);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                checked.verify_equivalence(&mut budget).unwrap();
                assert!(!checked.grants_artifact_or_launch_authority());
                storage
            };
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), before);
        }
    }
}

#[test]
fn redundant_store_erased_source_restart_before_reinitialization_is_preserved() {
    let input = redundant_fixture(
        true,
        1,
        Some(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(3),
        )),
    );
    let floor = input.floor;
    let owner = admit6(input, WORK, STORAGE).0.unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let deletion =
        fe2o3_kernel_opt::optimize_checked_redundant_store_v1(owner.output(), &mut budget).unwrap();
    assert_eq!(deletion.rows().len(), 1);
    budget.reserve_storage(deletion.retained_storage()).unwrap();
    let before = budget.storage();
    assert!(matches!(
        owner.check_redundant_store_output_v1(&deletion, &mut budget),
        Err(crate::ProductionRedundantStoreAdmissionErrorV1::Admission(
            crate::ProductionCheckedOutputAdmissionErrorPolicy3V1::Unsupported {
                phase: "redundant Store source",
                detail: "no lifetime or Move invalidation between identical Stores",
            }
        ))
    ));
    assert_eq!(budget.storage(), before);
}
