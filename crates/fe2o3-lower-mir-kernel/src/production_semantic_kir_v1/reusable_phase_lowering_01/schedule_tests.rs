//! Cursor-only tests. These rows are not source proof and never enter Runtime.
use super::*;
use fe2o3_mir_model::SsaVariableIdV1;

fn site(block: u32, statement: Option<u32>) -> Site {
    Site::new(SemanticBlockIdV1::from_index(block), statement)
}

fn cursor() -> ActionSchedule {
    let variable = SsaVariableIdV1::new(9);
    let row = |site: Site, event, action| PhaseEmissionRowV1 {
        phase: 0,
        action,
        boundary: PhaseEmissionBoundaryV1 {
            site,
            event,
            variable,
            value: SsaValueV1::BlockArgument {
                block: SsaBlockIdV1::new(0),
                variable,
            },
            kind: PhaseBoundaryKindV1::Define,
        },
    };
    ActionSchedule {
        rows: vec![
            (
                4,
                row(site(2, Some(0)), 3, PhaseEmissionActionV1::RelayClosure),
            ),
            (5, row(site(7, None), 10, PhaseEmissionActionV1::RelayDrop)),
            (6, row(site(7, None), 11, PhaseEmissionActionV1::End)),
        ],
        sites: vec![
            SiteRows {
                site: site(2, Some(0)),
                start: 0,
                end: 1,
                visit: Visit::Pending,
            },
            SiteRows {
                site: site(7, None),
                start: 1,
                end: 3,
                visit: Visit::Pending,
            },
        ],
        failed: false,
    }
}

#[test]
fn original_site_not_recipe_iteration_selects_rows_and_keeps_event_order() {
    let mut cursor = cursor();
    let mut emitted = Vec::new();
    let mut work = 100;
    assert!(
        cursor
            .consume_site(site(7, None), &mut work, |index, row, _| {
                emitted.push((index, row.boundary.event));
                Ok(())
            })
            .unwrap()
    );
    assert_eq!(emitted, [(5, 10), (6, 11)]);
    assert!(cursor.complete(&mut work).is_err());
    cursor
        .consume_site(site(2, Some(0)), &mut work, |index, row, _| {
            emitted.push((index, row.boundary.event));
            Ok(())
        })
        .unwrap();
    cursor.complete(&mut work).unwrap();
    assert_eq!(emitted, [(5, 10), (6, 11), (4, 3)]);
}

#[test]
fn foreign_statement_or_terminator_cannot_consume_a_site() {
    let mut cursor = cursor();
    let mut work = 100;
    for site in [
        site(2, None),
        site(2, Some(1)),
        site(7, Some(0)),
        site(8, None),
    ] {
        assert!(
            !cursor
                .consume_site(site, &mut work, |_, _, _| panic!("foreign site emitted"))
                .unwrap()
        );
    }
    assert!(cursor.complete(&mut work).is_err());
}

#[test]
fn duplicate_site_poison_prevents_further_emission_or_completion() {
    let mut cursor = cursor();
    let mut work = 100;
    cursor
        .consume_site(site(2, Some(0)), &mut work, |_, _, _| Ok(()))
        .unwrap();
    assert!(
        cursor
            .consume_site(site(2, Some(0)), &mut work, |_, _, _| panic!(
                "duplicate emitted"
            ))
            .is_err()
    );
    assert!(
        cursor
            .consume_site(site(7, None), &mut work, |_, _, _| panic!(
                "abandoned cursor emitted"
            ))
            .is_err()
    );
    assert!(cursor.complete(&mut work).is_err());
}

#[test]
fn failed_actual_emission_cannot_publish_or_retry_a_partial_site() {
    let mut cursor = cursor();
    let mut work = 100;
    let mut emitted = Vec::new();
    assert!(
        cursor
            .consume_site(site(7, None), &mut work, |index, _, _| {
                emitted.push(index);
                Err(rejected("controlled actual-emitter failure"))
            })
            .is_err()
    );
    assert_eq!(emitted, [5]);
    assert!(
        cursor
            .consume_site(site(7, None), &mut work, |_, _, _| panic!("retry emitted"))
            .is_err()
    );
    assert!(cursor.complete(&mut work).is_err());
}

#[test]
fn lookup_exhaustion_also_abandons_the_cursor() {
    let mut cursor = cursor();
    assert!(
        cursor
            .consume_site(site(7, None), &mut 0, |_, _, _| panic!("unfunded emission"))
            .is_err()
    );
    assert!(
        cursor
            .consume_site(site(7, None), &mut 100, |_, _, _| panic!("reset emission"))
            .is_err()
    );
}

#[test]
fn caller_work_and_emitter_work_share_one_exact_allowance() {
    let run = |initial: usize| {
        let mut cursor = cursor();
        let mut work = initial;
        let result = (|| {
            cursor.consume_site(site(2, Some(0)), &mut work, |_, _, work| spend(work, 2))?;
            cursor.consume_site(site(7, None), &mut work, |_, _, work| spend(work, 2))?;
            cursor.complete(&mut work)
        })();
        (result, work)
    };
    let (result, remaining) = run(100);
    result.unwrap();
    let required = 100 - remaining;
    let (result, remaining) = run(required);
    result.unwrap();
    assert_eq!(remaining, 0);
    assert!(run(required - 1).0.is_err());
}
