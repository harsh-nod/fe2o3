use super::*;
use crate::engineering_gfx950::token_program::run_prepared_groups;

#[test]
fn token_groups_reuse_absolute_deadline_and_retire_every_signal_before_reuse() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut fake = Fake::default();
    assert_eq!(
        run_prepared_groups(vec![(); 129], deadline, |group, actual| {
            run_ordered_batch_deadline(
                &mut fake,
                group.len(),
                600_000,
                OrderedMode::Batch64,
                Some(actual),
            )?;
            Ok(())
        })
        .unwrap(),
        129
    );
    assert_eq!(fake.deadlines, vec![deadline; 3]);
    assert_eq!(
        fake.events
            .iter()
            .filter(|event| event.starts_with("validate_signal:"))
            .count(),
        129
    );
    let mut retired = 0;
    let mut signals = 0;
    for event in &fake.events {
        if event == "reset:0" {
            assert_eq!(signals, 0);
        }
        if event.starts_with("validate_signal:") {
            signals += 1;
        }
        if event == "complete_frontier" {
            assert_eq!(signals, if retired < 2 { 64 } else { 1 });
            signals = 0;
            retired += 1;
        }
    }
    assert_eq!(retired, 3);
    assert!(!fake.poisoned);
}

#[test]
fn failure_after_one_retired_group_poison_stops_remaining_groups() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut fake = Fake::default();
    let mut groups = 0;
    let result = run_prepared_groups(vec![(); 129], deadline, |group, actual| {
        groups += 1;
        if groups == 2 {
            fake.fail_at = Some(fake.events.len() + 1);
        }
        run_ordered_batch_deadline(
            &mut fake,
            group.len(),
            600_000,
            OrderedMode::Batch64,
            Some(actual),
        )?;
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(groups, 2);
    assert!(fake.poisoned);
    assert_eq!(
        fake.events
            .iter()
            .filter(|event| *event == "complete_frontier")
            .count(),
        1
    );
    assert_eq!(fake.deadlines, vec![deadline]);
}

#[test]
fn already_expired_program_deadline_poison_precedes_any_staging() {
    let mut fake = Fake::default();
    assert!(
        run_ordered_batch_deadline(
            &mut fake,
            64,
            600_000,
            OrderedMode::Batch64,
            Some(Instant::now())
        )
        .is_err()
    );
    assert!(fake.poisoned);
    assert_eq!(fake.events, vec!["dispatch_fence"]);
    assert!(fake.deadlines.is_empty());
}
