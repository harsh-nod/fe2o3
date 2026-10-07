// Actual production declarations and execution bodies; no arena/lock refinement.
use vstd::prelude::*;
use vstd::prelude::verus as resource_vector_declarations_v1;
use vstd::prelude::verus as resource_domain_declarations_v1;
include!("../src/resource_vector_declarations.rs");
include!("../src/resource_vector_bodies.rs");
include!("../src/r75_resource_domain_declarations.rs");
include!("../src/r75_resource_domain_bodies.rs");

verus! {
impl R67ResourceVectorV1 {
    const ZERO: Self = resource_vector_zero_body_v1!();
}

spec fn sum(charges: Seq<R67ResourceVectorV1>, end: int, d: int) -> int
    decreases end,
{
    if end <= 0 { 0 } else { sum(charges, end - 1, d) + charges[end - 1].counts@[d] }
}

proof fn sum_monotone(charges: Seq<R67ResourceVectorV1>, a: int, b: int, d: int)
    requires 0 <= a <= b <= charges.len(), 0 <= d < 19,
    ensures 0 <= sum(charges, a, d) <= sum(charges, b, d),
    decreases b,
{
    if a < b { sum_monotone(charges, a, b - 1, d); }
    else if a > 0 { sum_monotone(charges, 0, a - 1, d); }
}

spec fn fits(used: R67ResourceVectorV1, charges: Seq<R67ResourceVectorV1>, cap: R67ResourceVectorV1) -> bool {
    forall|d: int| 0 <= d < 19 ==> used.counts@[d] + sum(charges, charges.len() as int, d) <= cap.counts@[d]
}

fn r67_resource_reserve_v1(used: R67ResourceVectorV1, charge: R67ResourceVectorV1, capacity: R67ResourceVectorV1)
    -> (result: Option<R67ResourceVectorV1>)
    ensures
        result.is_some() == (forall|d: int| 0 <= d < 19 ==> used.counts@[d] + charge.counts@[d] <= capacity.counts@[d]),
        match result { Some(next) => forall|d: int| 0 <= d < 19 ==>
            next.counts@[d] == used.counts@[d] + charge.counts@[d] && next.counts@[d] <= capacity.counts@[d], None => true },
{
    resource_vector_reserve_body_v1!(verus_exec_expr, used, charge, capacity, next, d, [
        invariant
            0 <= d <= 19,
            forall|i: int| 0 <= i < d ==> next.counts@[i] == used.counts@[i] + charge.counts@[i],
            forall|i: int| 0 <= i < d ==> next.counts@[i] <= capacity.counts@[i],
        decreases 19 - d,
    ])
}

fn r67_resource_release_v1(used: R67ResourceVectorV1, charge: R67ResourceVectorV1)
    -> (result: Option<R67ResourceVectorV1>)
    ensures
        result.is_some() == (forall|d: int| 0 <= d < 19 ==> charge.counts@[d] <= used.counts@[d]),
        match result { Some(next) => forall|d: int| 0 <= d < 19 ==>
            next.counts@[d] == used.counts@[d] - charge.counts@[d], None => true },
{
    resource_vector_release_body_v1!(verus_exec_expr, used, charge, next, d, [
        invariant
            0 <= d <= 19,
            forall|i: int| 0 <= i < d ==> next.counts@[i] == used.counts@[i] - charge.counts@[i],
            forall|i: int| 0 <= i < d ==> charge.counts@[i] <= used.counts@[i],
        decreases 19 - d,
    ])
}

spec fn record_error(fact: R75ResourceDomainFactsV1, count: int, owner: u64) -> Option<R75ResourceDomainErrorV1> {
    let occupied = fact.counts@[0] + fact.counts@[1] + fact.counts@[2];
    if occupied > usize::MAX || occupied > fact.record_limit { Some(R75ResourceDomainErrorV1::Invariant) }
    else if count == 0 || count > 65536 { Some(R75ResourceDomainErrorV1::InvalidMemberCount) }
    else if count > fact.record_limit - occupied { Some(R75ResourceDomainErrorV1::RecordCapacity) }
    else if owner == 0 || owner + count > u64::MAX { Some(R75ResourceDomainErrorV1::GenerationExhausted) }
    else { None }
}

fn resource_domain_records_v1(facts: R75ResourceDomainFactsV1, count: usize, owner: u64)
    -> (result: Result<(usize, u64), R75ResourceDomainErrorV1>)
    ensures match result {
        Ok((reserved, next_owner)) => {
            &&& record_error(facts, count as int, owner).is_none()
            &&& reserved == facts.counts@[0] + count
            &&& next_owner == owner + count
            &&& 0 < count <= 65536
            &&& owner > 0
        },
        Err(error) => record_error(facts, count as int, owner) == Some(error),
    },
{
    resource_domain_records_body_v1!(facts, count, owner)
}

fn resource_domain_leaf_v1(used: R67ResourceVectorV1, charges: &[R67ResourceVectorV1], capacity: R67ResourceVectorV1)
    -> (result: Result<R67ResourceVectorV1, R75ResourceDomainErrorV1>)
    requires charges.len() > 0,
    ensures match result {
        Ok(next) => {
            &&& fits(used, charges@, capacity)
            &&& forall|d: int| 0 <= d < 19 ==> next.counts@[d] == used.counts@[d] + sum(charges@, charges.len() as int, d)
            &&& forall|d: int| 0 <= d < 19 ==> next.counts@[d] <= capacity.counts@[d]
        },
        Err(error) => error == R75ResourceDomainErrorV1::Capacity && !fits(used, charges@, capacity),
    },
{
    resource_domain_leaf_body_v1!(verus_exec_expr, used, charges, capacity, next, member, [
        invariant
            charges.len() > 0,
            0 <= member <= charges.len(),
            forall|d: int| 0 <= d < 19 ==> next.counts@[d] == used.counts@[d] + sum(charges@, member as int, d),
            member > 0 ==> forall|d: int| 0 <= d < 19 ==> next.counts@[d] <= capacity.counts@[d],
        decreases charges.len() - member,
    ], [
        proof {
            let d = choose|d: int| 0 <= d < 19 && next.counts@[d] + charges@[member as int].counts@[d] > capacity.counts@[d];
            sum_monotone(charges@, member as int + 1, charges.len() as int, d);
            assert(!fits(used, charges@, capacity));
        }
    ], [
        proof {
            assert forall|d: int| 0 <= d < 19 implies next.counts@[d] == used.counts@[d] + sum(charges@, member as int, d) by {
                assert(sum(charges@, member as int, d) == sum(charges@, member as int - 1, d) + charges@[member as int - 1].counts@[d]);
            }
        }
    ])
}

spec fn level_error(fact: R75ResourceDomainFactsV1, charges: Seq<R67ResourceVectorV1>, owner: u64) -> Option<R75ResourceDomainErrorV1> {
    match record_error(fact, charges.len() as int, owner) {
        Some(error) => Some(error),
        None => if fits(fact.used, charges, fact.capacity) { None } else { Some(R75ResourceDomainErrorV1::Capacity) },
    }
}

spec fn first_error(facts: Seq<R75ResourceDomainFactsV1>, depth: usize, profile: usize, charges: Seq<R67ResourceVectorV1>, owner: u64) -> Option<R75ResourceDomainErrorV1> {
    if (profile != 3 && profile != 4) || depth == 0 || depth > profile || owner == 0 { Some(R75ResourceDomainErrorV1::Invariant) }
    else if level_error(facts[0], charges, owner).is_some() { level_error(facts[0], charges, owner) }
    else if depth > 1 && level_error(facts[1], charges, owner).is_some() { level_error(facts[1], charges, owner) }
    else if depth > 2 && level_error(facts[2], charges, owner).is_some() { level_error(facts[2], charges, owner) }
    else if depth > 3 && level_error(facts[3], charges, owner).is_some() { level_error(facts[3], charges, owner) }
    else { None }
}

proof fn first_failure(facts: Seq<R75ResourceDomainFactsV1>, depth: usize, profile: usize, charges: Seq<R67ResourceVectorV1>, owner: u64, level: int)
    requires
        facts.len() == 4, profile == 3 || profile == 4, 0 < depth <= profile, owner > 0,
        0 <= level < depth,
        forall|i: int| 0 <= i < level ==> level_error(facts[i], charges, owner).is_none(),
        level_error(facts[level], charges, owner).is_some(),
    ensures first_error(facts, depth, profile, charges, owner) == level_error(facts[level], charges, owner),
{
    if level == 0 {} else if level == 1 {} else if level == 2 {} else { assert(level == 3); }
}

fn r75_resource_domain_reserve_v1(
    facts: &[R75ResourceDomainFactsV1; 4], depth: usize, profile: usize,
    charges: &[R67ResourceVectorV1], owner: u64,
) -> (result: Result<R75ResourceDomainPlanV1, R75ResourceDomainErrorV1>)
    ensures match result {
        Err(error) => first_error(facts@, depth, profile, charges@, owner) == Some(error),
        Ok(plan) => {
            &&& first_error(facts@, depth, profile, charges@, owner).is_none()
            &&& (profile == 3 || profile == 4)
            &&& 0 < depth <= profile <= 4
            &&& 0 < charges.len() <= 65536
            &&& forall|i: int| 0 <= i < depth ==> level_error(facts@[i], charges@, owner).is_none()
            &&& plan.next_owner == owner + charges.len()
            &&& forall|i: int| 0 <= i < depth ==> plan.next_reserved@[i] == facts@[i].counts@[0] + charges.len()
            &&& forall|i: int, d: int| 0 <= i < depth && 0 <= d < 19 ==>
                plan.next_used@[i].counts@[d] == facts@[i].used.counts@[d] + sum(charges@, charges.len() as int, d)
            &&& forall|i: int, d: int| 0 <= i < depth && 0 <= d < 19 ==> plan.next_used@[i].counts@[d] <= facts@[i].capacity.counts@[d]
            &&& forall|i: int| depth <= i < 4 ==> plan.next_reserved@[i] == 0 && plan.next_used@[i] == R67ResourceVectorV1::ZERO
        },
    },
{
    resource_domain_path_body_v1!(verus_exec_expr, facts, depth, profile, charges, owner, plan, total, level, next, [
        invariant
            profile == 3 || profile == 4, 0 < depth <= profile, owner > 0,
            0 <= level <= depth,
            level > 0 ==> charges.len() > 0 && plan.next_owner == owner + charges.len(),
            forall|i: int| 0 <= i < level ==> level_error(facts@[i], charges@, owner).is_none(),
            forall|i: int| 0 <= i < level ==> plan.next_reserved@[i] == facts@[i].counts@[0] + charges.len(),
            forall|i: int, d: int| 0 <= i < level && 0 <= d < 19 ==>
                plan.next_used@[i].counts@[d] == facts@[i].used.counts@[d] + sum(charges@, charges.len() as int, d),
            forall|i: int, d: int| 0 <= i < level && 0 <= d < 19 ==> plan.next_used@[i].counts@[d] <= facts@[i].capacity.counts@[d],
            forall|i: int| level <= i < 4 ==> plan.next_reserved@[i] == 0 && plan.next_used@[i] == R67ResourceVectorV1::ZERO,
            level > 0 && depth > 1 ==> forall|d: int| 0 <= d < 19 ==> total.counts@[d] == sum(charges@, charges.len() as int, d),
        decreases depth - level,
    ], [
        proof { first_failure(facts@, depth, profile, charges@, owner, level as int); }
    ], [
        proof { first_failure(facts@, depth, profile, charges@, owner, level as int); }
    ], [
        proof {
            assert forall|d: int| 0 <= d < 19 implies sum(charges@, charges.len() as int, d) >= 0 by {
                sum_monotone(charges@, 0, charges.len() as int, d);
            }
        }
    ], [
        proof {
            if depth > 1 {
                assert forall|d: int| 0 <= d < 19 implies total.counts@[d] == sum(charges@, charges.len() as int, d) by {
                    if charges.len() == 1 {
                        reveal_with_fuel(sum, 2);
                        assert(sum(charges@, 1, d) == charges@[0].counts@[d]);
                    }
                }
            }
        }
    ], [
        proof {
            assert(!fits(facts@[level as int].used, charges@, facts@[level as int].capacity));
            first_failure(facts@, depth, profile, charges@, owner, level as int);
        }
    ], [
        let ghost before_plan = plan;
        proof {
            assert forall|d: int| 0 <= d < 19 implies
                next.counts@[d] == facts@[level as int].used.counts@[d] + sum(charges@, charges.len() as int, d)
                && next.counts@[d] <= facts@[level as int].capacity.counts@[d] by {}
        }
    ], [
        proof {
            assert(plan.next_used@ == before_plan.next_used@.update(level as int - 1, next));
            assert forall|i: int, d: int| 0 <= i < level && 0 <= d < 19 implies
                plan.next_used@[i].counts@[d] == facts@[i].used.counts@[d] + sum(charges@, charges.len() as int, d)
                && plan.next_used@[i].counts@[d] <= facts@[i].capacity.counts@[d] by {
                if i == level - 1 {
                    assert(plan.next_used@[i] == next);
                } else {
                    assert(i < level - 1);
                    assert(plan.next_used@[i] == before_plan.next_used@[i]);
                }
                assert(level_error(facts@[i], charges@, owner).is_none());
                assert(fits(facts@[i].used, charges@, facts@[i].capacity));
                assert(plan.next_used@[i].counts@[d] == facts@[i].used.counts@[d] + sum(charges@, charges.len() as int, d));
            }
        }
    ])
}
}
