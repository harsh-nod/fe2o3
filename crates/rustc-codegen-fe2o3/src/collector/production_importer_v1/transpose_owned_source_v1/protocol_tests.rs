use super::*;

fn expected() -> [(Role, u32); 3] {
    [(Role::Issue, 10), (Role::Stage, 20), (Role::Publish, 30)]
}

// Source protocol component keys, not source/provider/SSA authority.
#[test]
fn complete_flow_consumes_all_three_relationships() {
    let mut work = 1024;
    let mut roster = Roster::new(expected(), &mut work).unwrap();
    roster.consume_flow(10, 20, 30, &mut work).unwrap();
    roster.finish(&mut work).unwrap();
}

#[test]
fn missing_changed_or_additional_original_occurrence_rejects() {
    let mut work = 4096;
    assert!(matches!(
        Roster::new(expected(), &mut work)
            .unwrap()
            .finish(&mut work),
        Err(Error::Source(
            "transpose source occurrence roster incomplete"
        ))
    ));
    let mut extra =
        Roster::new(expected().into_iter().chain([(Role::Issue, 11)]), &mut work).unwrap();
    extra.consume_flow(10, 20, 30, &mut work).unwrap();
    assert!(matches!(
        extra.finish(&mut work),
        Err(Error::Source(
            "transpose source occurrence roster incomplete"
        ))
    ));
    for flow in [(11, 20, 30), (10, 21, 30), (10, 20, 31), (20, 10, 30)] {
        let mut roster = Roster::new(expected(), &mut work).unwrap();
        assert!(matches!(
            roster.consume_flow(flow.0, flow.1, flow.2, &mut work),
            Err(Error::Source(
                "transpose flow changed required source occurrence"
            ))
        ));
        assert!(roster.used.iter().all(|used| !used));
    }
}

#[test]
fn duplicate_original_recipe_and_duplicate_flow_are_rejected() {
    let mut work = 1024;
    assert!(matches!(
        Roster::new(expected().into_iter().chain([(Role::Stage, 20)]), &mut work),
        Err(Error::Source("duplicate source occurrence key"))
    ));
    let mut roster = Roster::new(expected(), &mut work).unwrap();
    roster.consume_flow(10, 20, 30, &mut work).unwrap();
    assert!(matches!(
        roster.consume_flow(10, 20, 30, &mut work),
        Err(Error::Source("transpose source occurrence consumed twice"))
    ));
    roster.finish(&mut work).unwrap();
}

#[test]
fn role_alias_and_partial_failed_consume_never_publish_a_row() {
    let mut work = 1024;
    let mut roster = Roster::new(expected(), &mut work).unwrap();
    assert!(matches!(
        roster.consume_flow(10, 10, 30, &mut work),
        Err(Error::Source("transpose occurrence roles collapsed"))
    ));
    assert!(matches!(
        roster.consume_flow(10, 20, 99, &mut work),
        Err(Error::Source(
            "transpose flow changed required source occurrence"
        ))
    ));
    assert!(roster.used.iter().all(|used| !used));
    roster.consume_flow(10, 20, 30, &mut work).unwrap();
    roster.finish(&mut work).unwrap();
}

#[test]
fn budget_failure_has_no_partial_consumption_and_no_refund() {
    let mut roster = Roster::new(expected(), &mut 1024).unwrap();
    let one = bounded::search_work(3) + 1;
    let mut work = 2 * one;
    assert!(matches!(
        roster.consume_flow(10, 20, 30, &mut work),
        Err(Error::Work)
    ));
    assert_eq!(work, 0);
    assert!(roster.used.iter().all(|used| !used));
    let mut work = 3 * one;
    roster.consume_flow(10, 20, 30, &mut work).unwrap();
    assert_eq!(work, 0);
    assert!(matches!(roster.finish(&mut work), Err(Error::Work)));
}

#[test]
fn distinct_complete_flows_and_permuted_source_roster_are_equivalent() {
    let entries = [
        (Role::Stage, 21),
        (Role::Publish, 31),
        (Role::Issue, 11),
        (Role::Publish, 30),
        (Role::Issue, 10),
        (Role::Stage, 20),
    ];
    let mut work = 4096;
    let mut roster = Roster::new(entries, &mut work).unwrap();
    roster.consume_flow(11, 21, 31, &mut work).unwrap();
    roster.consume_flow(10, 20, 30, &mut work).unwrap();
    roster.finish(&mut work).unwrap();
}

#[test]
fn failed_key_insertion_keeps_original_storage_but_not_work() {
    let mut keys = vec![10u32, 30];
    let mut work = bounded::search_work(keys.len());
    assert!(matches!(
        bounded::insert_unique(&mut keys, 20, &mut work),
        Err(Error::Work)
    ));
    assert_eq!(keys, [10, 30]);
    assert_eq!(work, 0);
}
