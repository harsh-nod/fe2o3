// The unchanged arena include also defines the unused resource_domain_facts_body_v1.
#![allow(unused_macros)]
// Actual post-lock field observation, not an Arc or Mutex refinement.
// Existing node/path and record bodies are verified again in this root.
// Scalar/slice parameters are the production caller's locked-field projections.
// Pinned vstd equality, array/slice specifications and derive lowering remain
// compiler trust boundaries. No freshness, custody, conservation or native
// authority is established, and no frozen credit_ok result is supplied.
include!("retained_credit_record_v1.rs");
use vstd::prelude::verus as resource_domain_arena_declarations_v1;

verus! { type ResourceVectorV1 = R67ResourceVectorV1; }
include!("../../fe2o3-resource-accounting/src/domain/arena_declarations.rs");
include!("../../fe2o3-resource-accounting/src/domain/arena_bodies.rs");
include!("../../fe2o3-resource-accounting/src/domain/retained_observation_body.rs");

verus! {

impl vstd::std_specs::cmp::PartialEqSpecImpl for Key {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

#[derive(Clone, Copy)]
struct DomainRecord {
    leaf: Key,
    credit: Record,
}

spec fn live(nodes: Seq<Option<Node>>, key: Key) -> bool {
    key.slot < nodes.len() && nodes[key.slot as int].is_some()
        && nodes[key.slot as int]->Some_0.key == key
}

spec fn at(nodes: Seq<Option<Node>>, key: Key) -> Node {
    nodes[key.slot as int]->Some_0
}

#[verifier::opaque]
spec fn walk(nodes: Seq<Option<Node>>, next: Option<Key>, budget: int) -> Option<Seq<Key>>
    decreases budget,
{
    match next {
        None => Some(Seq::empty()),
        Some(key) => if budget <= 0 || !live(nodes, key) { None }
            else { match walk(nodes, at(nodes, key).parent, budget - 1) {
                None => None,
                Some(tail) => Some(seq![key] + tail),
            } },
    }
}

spec fn linked(nodes: Seq<Option<Node>>, path: Seq<Key>) -> bool {
    &&& path.len() > 0
    &&& forall|i: int| 0 <= i < path.len() ==> live(nodes, path[i])
    &&& forall|i: int| 0 <= i < path.len() - 1 ==> at(nodes, path[i]).parent == Some(path[i + 1])
    &&& at(nodes, path.last()).parent.is_none()
}

proof fn walk_shape(nodes: Seq<Option<Node>>, next: Option<Key>, budget: int)
    requires budget >= 0,
    ensures match walk(nodes, next, budget) {
        None => true,
        Some(path) => {
            &&& path.len() <= budget
            &&& match next { None => path.len() == 0,
                Some(key) => path.len() > 0 && path[0] == key && linked(nodes, path) }
        },
    },
    decreases budget,
{
    reveal(walk);
    if let Some(key) = next {
        if budget > 0 && live(nodes, key) {
            walk_shape(nodes, at(nodes, key).parent, budget - 1);
            if let Some(tail) = walk(nodes, at(nodes, key).parent, budget - 1) {
                let path = seq![key] + tail;
                assert forall|i: int| 0 <= i < path.len() implies live(nodes, path[i]) by {
                    if i > 0 { assert(path[i] == tail[i - 1]); }
                }
                assert forall|i: int| 0 <= i < path.len() - 1 implies
                    at(nodes, path[i]).parent == Some(path[i + 1]) by {
                    if i > 0 { assert(path[i] == tail[i - 1]); assert(path[i + 1] == tail[i]); }
                }
                if tail.len() > 0 { assert(path.last() == tail.last()); }
                else { assert(path.last() == key); }
            }
        }
    }
}

proof fn linked_no_duplicate(nodes: Seq<Option<Node>>, path: Seq<Key>, i: int, j: int)
    requires linked(nodes, path), 0 <= i < j < path.len(), path[i] == path[j],
    ensures false,
    decreases path.len() - j,
{
    if j == path.len() - 1 {
        assert(at(nodes, path[i]).parent == Some(path[i + 1]));
        assert(at(nodes, path[j]).parent.is_none());
    } else {
        assert(at(nodes, path[i]).parent == Some(path[i + 1]));
        assert(at(nodes, path[j]).parent == Some(path[j + 1]));
        linked_no_duplicate(nodes, path, i + 1, j + 1);
    }
}

proof fn linked_slots_unique(nodes: Seq<Option<Node>>, path: Seq<Key>)
    requires linked(nodes, path),
    ensures forall|i: int, j: int| 0 <= i < j < path.len() ==> path[i].slot != path[j].slot,
{
    assert forall|i: int, j: int| 0 <= i < j < path.len() implies path[i].slot != path[j].slot by {
        if path[i].slot == path[j].slot {
            assert(at(nodes, path[i]).key == path[i]);
            assert(at(nodes, path[j]).key == path[j]);
            linked_no_duplicate(nodes, path, i, j);
        }
    }
}

spec fn accepted(nodes: Seq<Option<Node>>, profile: usize, leaf: Key) -> bool {
    (profile == 3 || profile == 4) && match walk(nodes, Some(leaf), profile as int) {
        Some(path) => path.len() > 0 && path.last() == ROOT,
        None => false,
    }
}

fn domain_node_v1(nodes: &[Option<Node>], key: Key) -> (result: Option<&Node>)
    ensures
        result.is_some() == live(nodes@, key),
        match result { Some(node) => *node == at(nodes@, key), None => true },
{
    resource_domain_node_body_v1!(nodes, key)
}

spec fn exact_path_result(nodes: Seq<Option<Node>>, profile: usize, leaf: Key,
    result: Option<([Key; 4], usize)>) -> bool {
    &&& result.is_some() == accepted(nodes, profile, leaf)
    &&& match result {
            None => true,
            Some((path, depth)) => {
                &&& 0 < depth <= profile <= 4
                &&& path@[depth as int - 1] == ROOT
                &&& walk(nodes, Some(leaf), profile as int) == Some(path@.take(depth as int))
                &&& forall|i: int| depth <= i < 4 ==> path@[i] == ROOT
            },
        }
}

proof fn exact_path_implies_properties(nodes: Seq<Option<Node>>, profile: usize, leaf: Key,
    result: Option<([Key; 4], usize)>)
    requires exact_path_result(nodes, profile, leaf, result),
    ensures match result { None => true, Some((path, depth)) => {
        &&& path@[0] == leaf
        &&& linked(nodes, path@.take(depth as int))
        &&& forall|i: int, j: int| 0 <= i < j < depth ==> path@[i].slot != path@[j].slot
    } },
{
    if let Some((path, depth)) = result {
        walk_shape(nodes, Some(leaf), profile as int);
        let selected = path@.take(depth as int);
        linked_slots_unique(nodes, selected);
        assert forall|i: int, j: int| 0 <= i < j < depth implies path@[i].slot != path@[j].slot by {
            assert(selected[i] == path@[i]);
            assert(selected[j] == path@[j]);
        }
    }
}

fn domain_path_v1(nodes: &[Option<Node>], profile: usize, leaf: Key)
    -> (result: Option<([Key; MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1], usize)>)
    ensures exact_path_result(nodes@, profile, leaf, result),
{
    resource_domain_path_extract_body_v1!(verus_exec_expr, nodes, profile, leaf, path, next, depth, [
        invariant
            profile == 3 || profile == 4,
            0 <= depth <= profile <= 4,
            depth == 0 ==> next == Some(leaf),
            next.is_none() ==> walk(nodes@, Some(leaf), profile as int) == Some(path@.take(depth as int)),
            depth == profile && next.is_some() ==> walk(nodes@, Some(leaf), profile as int).is_none(),
            forall|i: int| depth <= i < 4 ==> path@[i] == ROOT,
            walk(nodes@, Some(leaf), profile as int) == match walk(nodes@, next, profile as int - depth as int) {
                None => None,
                Some(tail) => Some(path@.take(depth as int) + tail),
            },
        decreases profile - depth,
    ], [
        let ghost previous_path = path;
        let ghost previous_next = next;
        let ghost previous_depth = depth;
        proof { reveal(walk); }
    ], [
        proof {
            assert(path@.take(depth as int) =~= previous_path@.take(previous_depth as int) + seq![path@[depth as int - 1]]);
            reveal(walk);
            if let Some(tail) = walk(nodes@, next, profile as int - depth as int) {
                assert((previous_path@.take(previous_depth as int) + seq![path@[depth as int - 1]]) + tail
                    =~= previous_path@.take(previous_depth as int) + (seq![path@[depth as int - 1]] + tail));
            }
        }
    ], [
        proof {
            reveal(walk);
            assert(walk(nodes@, next, profile as int - depth as int) == Some(Seq::empty()));
            assert(path@.take(depth as int) + Seq::empty() =~= path@.take(depth as int));
        }
    ])
}

spec fn observation_matches(nodes: Seq<Option<Node>>, profile: usize,
    records: Seq<Option<DomainRecord>>, poisoned: bool, key: Key, slot: usize,
    owner: u64, expected: ResourceVectorV1) -> bool
{
    &&& !poisoned
    &&& accepted(nodes, profile, key)
    &&& slot < records.len()
    &&& records[slot as int].is_some()
    &&& records[slot as int]->Some_0.leaf == key
    &&& owner != 0
    &&& records[slot as int]->Some_0.credit.owner == owner
    &&& records[slot as int]->Some_0.credit.phase == Phase::Retained
    &&& records[slot as int]->Some_0.credit.charge.counts@ == expected.counts@
}

fn domain_retained_observation_v1(
    nodes: &[Option<Node>],
    profile: usize,
    records: &[Option<DomainRecord>],
    poisoned: bool,
    key: Key,
    slot: usize,
    owner: u64,
    expected: ResourceVectorV1,
) -> (out: bool)
    ensures out == observation_matches(nodes@, profile, records@, poisoned, key, slot, owner, expected),
{
    domain_retained_observation_body_v1!(nodes, profile, records, poisoned, key, slot, owner, expected)
}

}
