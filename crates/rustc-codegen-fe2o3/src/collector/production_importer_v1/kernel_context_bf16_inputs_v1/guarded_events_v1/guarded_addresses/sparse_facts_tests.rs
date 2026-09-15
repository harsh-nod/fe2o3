use super::super as formula;
use super::*;
#[path = "cold_oracle.rs"]
mod cold;
#[path = "differential_existing_tests.rs"]
mod differential_existing;

fn check_differential(p: &Formula, f: Charge<'_>) -> Result<()> {
    let expected = cold::check(p, &mut |_| true);
    let actual = check(p, f);
    if actual != Err(Error::Budget) {
        assert_eq!(actual, expected);
    }
    actual
}

#[test]
fn sparse_fact_queries_preserve_every_included_predicate_and_order() {
    for mask in 0_u64..256 {
        let mut facts = Facts::new();
        for i in (0..8).rev() {
            if mask & (1 << i) != 0 {
                facts.push_descending(i).unwrap();
            }
        }
        assert_eq!(
            facts.ascending().collect::<Vec<_>>(),
            (0..8).filter(|i| mask & (1 << i) != 0).collect::<Vec<_>>()
        );
    }
    let mut facts = Facts::new();
    for i in (0..NODES).rev() {
        facts.push_descending(i as Id).unwrap();
    }
    assert_eq!(facts.ascending().count(), NODES);
    assert_eq!(facts.push_descending(0), Err(Error::Graph));
    let mut facts = Facts::new();
    facts.push_descending(3).unwrap();
    assert_eq!(facts.push_descending(3), Err(Error::Graph));
    assert_eq!(facts.push_descending(4), Err(Error::Graph));
}

#[test]
fn guarded_theorem_sparse_scans_reduce_actual_charged_work_for_both_builders() {
    for role in [false, true] {
        for reference in [false, true] {
            let p = if reference {
                formula::reference(role, &mut |_| true)
            } else {
                formula::projected(role, &mut |_| true)
            }
            .unwrap();
            let mut old = 0;
            let mut new = 0;
            assert_eq!(
                cold::check(&p, &mut |n| {
                    old += n;
                    true
                }),
                Ok(())
            );
            assert_eq!(
                check(&p, &mut |n| {
                    new += n;
                    true
                }),
                Ok(())
            );
            eprintln!(
                "guarded-addresses role_b={role} reference={reference} old_work={old} new_work={new}"
            );
            assert!(new < old);
        }
    }
}

#[test]
fn sparse_theorem_preserves_exact_budget_failure_and_spent_work() {
    let p = formula::reference(false, &mut |_| true).unwrap();
    let mut needed = 0;
    check(&p, &mut |n| {
        needed += n;
        true
    })
    .unwrap();
    for allowed in [0, needed - 1, needed] {
        let mut remaining = allowed;
        let result = check(&p, &mut |n| {
            let Some(left) = remaining.checked_sub(n) else {
                return false;
            };
            remaining = left;
            true
        });
        assert_eq!(
            result,
            if allowed == needed {
                Ok(())
            } else {
                Err(Error::Budget)
            }
        );
        if allowed == needed {
            assert_eq!(remaining, 0);
        }
        assert!(remaining <= allowed);
    }
}
