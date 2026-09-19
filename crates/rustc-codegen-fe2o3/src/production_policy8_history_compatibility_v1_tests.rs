//! Independently execute the exact pre-factoring P7 history sequence.
use super::*;
use crate::production_ranked_projection_v1::{
    with_backend_policy7_direct_prefix_v1, with_backend_policy7_erased_prefix_v1,
};

fn observe(
    prefix: Prefix6,
    factored: bool,
    budget: &mut Budget<'_>,
) -> (Vec<u8>, Vec<u8>, usize, usize, usize) {
    let floor = budget.storage();
    let start_work = budget.work();
    let observed = scoped(prefix.minimum().unwrap(), budget, |budget| {
        let (admitted, execution, added) = if factored {
            prepare_history_v1(prefix, budget)?
        } else {
            let (admitted, added) = prefix.continue_once(budget)?;
            budget.reserve_storage(added).map_err(resource)?;
            let execution = Policy7ExecutionWitnessV1::prepare(&admitted, budget)?;
            budget
                .reserve_storage(execution.retained_storage())
                .map_err(resource)?;
            (admitted, execution, added)
        };
        let expected_floor = floor + added + execution.retained_storage();
        assert_eq!(budget.storage(), expected_floor);
        Ok((
            admitted.output().canonical().canonical_bytes().to_vec(),
            execution.canonical_bytes().to_vec(),
            budget.work() - start_work,
            expected_floor,
            budget.peak_storage(),
        ))
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
    observed
}

#[test]
fn policy8_private_history_factoring_preserves_p7_bytes_work_and_storage() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for duplicate in [false, true] {
            let mut before = None;
            with_backend_policy7_direct_prefix_v1(profile, duplicate, |prefix, _, budget| {
                before = Some(observe(Prefix6::Direct(prefix), false, budget));
            });
            with_backend_policy7_direct_prefix_v1(profile, duplicate, |prefix, _, budget| {
                assert_eq!(
                    before.unwrap(),
                    observe(Prefix6::Direct(prefix), true, budget)
                );
            });
            let mut before = None;
            with_backend_policy7_erased_prefix_v1(profile, duplicate, |prefix, _, budget| {
                before = Some(observe(Prefix6::Erased(prefix), false, budget));
            });
            with_backend_policy7_erased_prefix_v1(profile, duplicate, |prefix, _, budget| {
                assert_eq!(
                    before.unwrap(),
                    observe(Prefix6::Erased(prefix), true, budget)
                );
            });
        }
    }
}
