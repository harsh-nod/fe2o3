// Child of erased owner tests: exercise fresh source reconstruction separately
// from immutable constructor custody and the frozen exact N/E graph relation.
use super::*;

fn replay_original(
    source: &ProductionPreRankedKirOwnerV1,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let floor = FLOOR + source.unit_local_source_storage_floor_v1().unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = source_output_unit_local_replay_v1(source, &mut budget);
    assert_eq!(budget.storage(), floor);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn fresh_unit_source_replay_reconstructs_all_root_and_helper_origins() {
    for expected in [false, true] {
        for roots in [1, 2] {
            let (source, _) = erased_effect_fixture(expected, roots);
            replay_original(&source, WORK, STORAGE).0.unwrap();
        }
    }
}

#[test]
fn fresh_unit_source_replay_refuses_all_retained_origin_row_substitutions() {
    for mutation in 0..7 {
        let (mut source, _) = erased_effect_fixture(true, 2);
        match mutation {
            0 => source.assert_origins.aliases[0].binding = usize::MAX,
            1 => {
                source.assert_origins.aliases[0].site.semantic_block =
                    SemanticBlockIdV1::from_index(99)
            }
            2 => source.assert_origins.functions[0].reachable_blocks += 1,
            3 => source.assert_origins.functions[0].canonical.0 += 1,
            4 => {
                source.assert_origins.bindings[0].expected =
                    !source.assert_origins.bindings[0].expected
            }
            5 => {
                source.assert_origins.bindings[0].semantic_success =
                    SemanticBlockIdV1::from_index(99)
            }
            6 => source.correspondence.statement_operation_spans[0].operation_count += 1,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                replay_original(&source, WORK, STORAGE).0,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn fresh_unit_source_replay_refuses_independent_source_and_launch_changes() {
    for mutation in 0..5 {
        let (mut source, _) = erased_effect_fixture(true, 1);
        match mutation {
            0 => {
                let (other, _) = erased_effect_fixture(false, 1);
                source.semantic_ssa = other.semantic_ssa;
            }
            1 => {
                let (other, _) = erased_effect_fixture(false, 1);
                source.source_launch = other.source_launch;
            }
            2 => source.launch_roots[0].global_extents[0] += 1,
            3 => source.launch_roots[0].workgroup_extents[0] += 1,
            4 => {
                let semantic = source.semantic_ssa.source_semantic();
                let entry = semantic.functions()[semantic.roots()[0].index() as usize]
                    .kernel_entry()
                    .unwrap();
                let name = std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap();
                let launch = crate::ProductionSourceLaunchRootInputV1::new(
                    name,
                    *entry.kernel_binding_identity().as_bytes(),
                    crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [2, 1, 1]),
                );
                source.source_launch =
                    crate::ProductionSourceLaunchRosterV1::try_new(semantic, &[launch]).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                replay_original(&source, WORK, STORAGE).0,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn fresh_unit_source_replay_has_exact_measured_work_storage_boundaries() {
    let (source, _) = erased_effect_fixture(false, 2);
    let (result, work, peak) = replay_original(&source, WORK, STORAGE);
    result.unwrap();
    let floor = FLOOR + source.unit_local_source_storage_floor_v1().unwrap();
    assert!(work > 100 && peak > floor);
    replay_original(&source, work, peak).0.unwrap();
    assert!(replay_original(&source, work - 1, STORAGE).0.is_err());
    assert!(replay_original(&source, WORK, peak - 1).0.is_err());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let below = source.unit_local_source_storage_floor_v1().unwrap() - 1;
    budget.reserve_storage(below).unwrap();
    assert!(matches!(
        source_output_unit_local_replay_v1(&source, &mut budget),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                AssertOriginResourceV1::Accounting
            )
        )
    ));
    assert_eq!(budget.storage(), below);
}

#[test]
fn erased_common_scope_now_rejects_forged_original_correspondence() {
    let mut owner = produced_erased_owner(UnitCase::ScalarSlot, &[1]);
    owner.original.correspondence.semantic_sha256[0] ^= 1;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let floor = FLOOR + owner.retained_storage_floor_v1();
    budget.reserve_storage(floor).unwrap();
    let mut entered = false;
    assert!(matches!(
        owner.with_checked_erasure_v1(&mut budget, |_, _| {
            entered = true;
            Ok(())
        }),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(!entered);
    assert_eq!(budget.storage(), floor);
}
