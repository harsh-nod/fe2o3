use super::*;

// Component occurrence keys, not provider, source, SSA or Workgroup authority.
type Key = (u32, u32, u32); // expanded instance, original function, original block

fn flow(instance: u32) -> [Key; 3] {
    [(instance, 4, 7), (instance + 1, 9, 2), (instance, 4, 10)]
}

fn entries(instances: &[u32]) -> Vec<(Role, Key)> {
    instances
        .iter()
        .flat_map(|instance| {
            let [issue, stage, publish] = flow(*instance);
            [
                (Role::Issue, issue),
                (Role::Stage, stage),
                (Role::Publish, publish),
            ]
        })
        .collect()
}

#[test]
fn all_expanded_instances_of_the_same_static_flow_are_required() {
    let mut work = 16_384;
    let mut roster = Roster::new(entries(&[2, 8, 14]), &mut work).unwrap();
    for instance in [14, 2, 8] {
        let [issue, stage, publish] = flow(instance);
        roster
            .consume_flow(issue, stage, publish, &mut work)
            .unwrap();
    }
    roster.finish(&mut work).unwrap();
}

#[test]
fn omitted_expanded_instance_is_not_an_empty_or_dead_path_waiver() {
    let mut work = 8192;
    let mut roster = Roster::new(entries(&[2, 8]), &mut work).unwrap();
    let [issue, stage, publish] = flow(2);
    roster
        .consume_flow(issue, stage, publish, &mut work)
        .unwrap();
    assert_eq!(
        roster.finish(&mut work),
        Err(Error::Source(
            "transpose source occurrence roster incomplete",
        ))
    );
}

#[test]
fn changed_instance_function_or_block_never_consumes_any_occurrence() {
    for changed in [(99, 9, 2), (3, 99, 2), (3, 9, 99)] {
        let mut work = 8192;
        let mut roster = Roster::new(entries(&[2]), &mut work).unwrap();
        let [issue, _, publish] = flow(2);
        assert_eq!(
            roster.consume_flow(issue, changed, publish, &mut work),
            Err(Error::Source(
                "transpose flow changed required source occurrence"
            ))
        );
        assert!(roster.used.iter().all(|used| !used));
        let [issue, stage, publish] = flow(2);
        roster
            .consume_flow(issue, stage, publish, &mut work)
            .unwrap();
        roster.finish(&mut work).unwrap();
    }
}

#[test]
fn duplicate_static_role_in_one_instance_is_rejected() {
    let mut work = 8192;
    let mut duplicated = entries(&[2]);
    duplicated.push((Role::Stage, flow(2)[1]));
    assert!(matches!(
        Roster::new(duplicated, &mut work),
        Err(Error::Source("duplicate source occurrence key"))
    ));
}

#[test]
fn every_expanded_flow_must_be_consumed_exactly_once() {
    let mut work = 8192;
    let mut roster = Roster::new(entries(&[2, 8]), &mut work).unwrap();
    let [issue, stage, publish] = flow(2);
    roster
        .consume_flow(issue, stage, publish, &mut work)
        .unwrap();
    let before = roster.used.clone();
    assert_eq!(
        roster.consume_flow(issue, stage, publish, &mut work),
        Err(Error::Source("transpose source occurrence consumed twice"))
    );
    assert_eq!(roster.used, before);
    let [issue, stage, publish] = flow(8);
    roster
        .consume_flow(issue, stage, publish, &mut work)
        .unwrap();
    roster.finish(&mut work).unwrap();
}

#[test]
fn zero_budget_and_failed_roster_build_never_yield_a_receipt() {
    let mut zero = 0;
    assert!(matches!(
        Roster::new(entries(&[2]), &mut zero),
        Err(Error::Work)
    ));
    assert_eq!(zero, 0);
    let mut work = 8192;
    let roster = Roster::new(entries(&[2]), &mut work).unwrap();
    assert!(matches!(roster.finish(&mut zero), Err(Error::Work)));
}
