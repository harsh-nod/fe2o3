use super::*;
use fe2o3_mir_model::{plan_ssa_v1, SsaBlockInputV1, SsaEdgeInputV1, SsaEdgeRoleV1, SsaEventV1, SsaConstructionInputV1};

fn variable() -> SsaVariableIdV1 { SsaVariableIdV1::new(0) }
fn plan(blocks: Vec<SsaBlockInputV1>) -> SsaConstructionPlanV1 {
    plan_ssa_v1(&SsaConstructionInputV1::new(SsaBlockIdV1::new(0), 1, vec![true], vec![], blocks)).unwrap()
}
fn block(events: Vec<SsaEventV1>, successors: &[u32]) -> SsaBlockInputV1 {
    SsaBlockInputV1::new(events, successors.iter().map(|target| SsaEdgeInputV1::new(
        SsaEdgeRoleV1::new(1), SsaBlockIdV1::new(*target), vec![],
    )).collect())
}
fn first(plan: &SsaConstructionPlanV1) -> (SsaValueV1, Point) {
    for block in plan.reverse_postorder() {
        for (index, event) in plan.resolved_events(*block).unwrap() {
            if let SsaResolvedEventV1::Define { value, .. } = event {
                return (*value, Point { block: *block, event: *index });
            }
        }
    }
    panic!("fixture must contain a real SSA definition")
}

#[test]
fn phase_dormant_lease_closes_at_the_existing_kill() {
    let plan = plan(vec![block(vec![SsaEventV1::Define(variable()), SsaEventV1::Kill(variable())], &[])]);
    let (value, definition) = first(&plan);
    assert_eq!(dormant_lease(&plan, variable(), value, definition, &mut || true),
        Ok(Point { block: SsaBlockIdV1::new(0), event: 1 }));
}

#[test]
fn phase_dormant_lease_closes_across_the_real_normal_successor() {
    let plan = plan(vec![
        block(vec![SsaEventV1::Define(variable())], &[1]),
        block(vec![SsaEventV1::Kill(variable())], &[]),
    ]);
    let (value, definition) = first(&plan);
    assert_eq!(dormant_lease(&plan, variable(), value, definition, &mut || true),
        Ok(Point { block: SsaBlockIdV1::new(1), event: 0 }));
}

#[test]
fn phase_dormant_lease_never_invents_a_missing_kill() {
    let plan = plan(vec![block(vec![SsaEventV1::Define(variable())], &[])]);
    let (value, definition) = first(&plan);
    assert_eq!(dormant_lease(&plan, variable(), value, definition, &mut || true), Err(Error::Kill));
}

#[test]
fn phase_dormant_lease_rejects_reads_copies_and_redefinitions() {
    for (middle, expected) in [(SsaEventV1::Use(variable()), Error::Use), (SsaEventV1::Define(variable()), Error::Definition)] {
        let plan = plan(vec![block(vec![SsaEventV1::Define(variable()), middle, SsaEventV1::Kill(variable())], &[])]);
        let (value, definition) = first(&plan);
        assert_eq!(dormant_lease(&plan, variable(), value, definition, &mut || true), Err(expected));
    }
}

#[test]
fn phase_dormant_lease_rejects_two_control_dependent_closes() {
    let plan = plan(vec![
        block(vec![SsaEventV1::Define(variable())], &[1, 2]),
        block(vec![SsaEventV1::Kill(variable())], &[]),
        block(vec![SsaEventV1::Kill(variable())], &[]),
    ]);
    let (value, definition) = first(&plan);
    assert_eq!(dormant_lease(&plan, variable(), value, definition, &mut || true), Err(Error::Kill));
}

#[test]
fn phase_dormant_lease_rejects_a_substituted_definition_coordinate() {
    let plan = plan(vec![block(vec![SsaEventV1::Define(variable()), SsaEventV1::Kill(variable())], &[])]);
    let (value, mut definition) = first(&plan);
    definition.event += 1;
    assert_eq!(dormant_lease(&plan, variable(), value, definition, &mut || true), Err(Error::Definition));
}

#[test]
fn phase_dormant_lease_uses_the_callers_exact_remaining_work() {
    let plan = plan(vec![block(vec![SsaEventV1::Define(variable()), SsaEventV1::Kill(variable())], &[])]);
    let (value, definition) = first(&plan);
    let mut used = 0usize;
    dormant_lease(&plan, variable(), value, definition, &mut || { used += 1; true }).unwrap();
    assert_eq!(used, 3);
    for budget in 0..=used {
        let mut remaining = budget;
        let result = dormant_lease(&plan, variable(), value, definition, &mut || {
            if remaining == 0 { false } else { remaining -= 1; true }
        });
        assert_eq!(result.is_ok(), budget == used);
        if budget < used { assert_eq!(result, Err(Error::Work)); }
        assert_eq!(remaining, 0);
    }
}
