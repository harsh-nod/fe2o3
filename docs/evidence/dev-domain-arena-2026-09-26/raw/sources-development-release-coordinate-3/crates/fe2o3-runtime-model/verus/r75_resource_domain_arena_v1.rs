// Exact immutable arena lookup, path traversal and planner-fact extraction.
verus! {
spec fn live(nodes: Seq<Option<Node>>, key: Key) -> bool {
    key.slot < nodes.len() && nodes[key.slot as int].is_some()
        && nodes[key.slot as int]->Some_0.key == key
}

spec fn at(nodes: Seq<Option<Node>>, key: Key) -> Node {
    nodes[key.slot as int]->Some_0
}

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

fn domain_path_v1(nodes: &[Option<Node>], profile: usize, leaf: Key)
    -> (result: Option<([Key; MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1], usize)>)
    ensures
        result.is_some() == accepted(nodes@, profile, leaf),
        match result {
            None => true,
            Some((path, depth)) => {
                &&& 0 < depth <= profile <= 4
                &&& path@[0] == leaf
                &&& path@[depth as int - 1] == ROOT
                &&& walk(nodes@, Some(leaf), profile as int) == Some(path@.take(depth as int))
                &&& linked(nodes@, path@.take(depth as int))
                &&& forall|i: int, j: int| 0 <= i < j < depth ==> path@[i].slot != path@[j].slot
                &&& forall|i: int| depth <= i < 4 ==> path@[i] == ROOT
            },
        },
{
    resource_domain_path_extract_body_v1!(verus_exec_expr, nodes, profile, leaf, path, next, depth, [
        invariant
            profile == 3 || profile == 4,
            0 <= depth <= profile <= 4,
            depth == 0 ==> next == Some(leaf),
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
            assert(walk(nodes@, next, profile as int - depth as int) == Some(Seq::empty()));
            assert(path@.take(depth as int) + Seq::empty() =~= path@.take(depth as int));
            walk_shape(nodes@, Some(leaf), profile as int);
            let selected = path@.take(depth as int);
            linked_slots_unique(nodes@, selected);
        }
    ])
}

spec fn fact_of(node: Node) -> R75ResourceDomainFactsV1 {
    R75ResourceDomainFactsV1 { used: node.used, capacity: node.capacity, counts: node.counts, record_limit: node.record_limit }
}

spec fn empty_fact(fact: R75ResourceDomainFactsV1) -> bool {
    fact.used == ResourceVectorV1::ZERO && fact.capacity == ResourceVectorV1::ZERO
        && fact.counts@ == seq![0usize, 0usize, 0usize] && fact.record_limit == 0
}

fn domain_facts_v1(nodes: &[Option<Node>], path: &[Key; 4], depth: usize)
    -> (result: Option<[R75ResourceDomainFactsV1; 4]>)
    ensures
        result.is_some() == (0 < depth <= 4 && forall|i: int| 0 <= i < depth ==> live(nodes@, path@[i])),
        match result { None => true, Some(facts) => {
            &&& forall|i: int| 0 <= i < depth ==> facts@[i] == fact_of(at(nodes@, path@[i]))
            &&& forall|i: int| depth <= i < 4 ==> empty_fact(facts@[i])
        } },
{
    resource_domain_facts_body_v1!(verus_exec_expr, nodes, path, depth, facts, index, [
        proof {
            assert forall|i: int| 0 <= i < 4 implies empty_fact(facts@[i]) by {
                assert(facts@[i].counts@ =~= seq![0usize, 0usize, 0usize]);
            }
        }
    ], [
        invariant
            0 < depth <= 4,
            0 <= index <= depth,
            forall|i: int| 0 <= i < index ==> live(nodes@, path@[i]),
            forall|i: int| 0 <= i < index ==> facts@[i] == fact_of(at(nodes@, path@[i])),
            forall|i: int| index <= i < 4 ==> empty_fact(facts@[i]),
        decreases depth - index,
    ], [])
}
}
