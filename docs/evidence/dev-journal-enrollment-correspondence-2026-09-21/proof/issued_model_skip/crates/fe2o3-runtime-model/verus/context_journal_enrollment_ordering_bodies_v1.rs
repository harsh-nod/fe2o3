use self::ContextAllocationReferenceV1 as AllocationReferenceV1;
use self::ContextAllocationKeyV1 as AllocationKeyV1;
use self::ContextAllocationEnrollmentV1 as EnrollmentV1;

verus! {

pub open spec fn enrollment_key_less_v1(left: AllocationKeyV1, right: AllocationKeyV1) -> bool {
    left.context_generation < right.context_generation
        || (left.context_generation == right.context_generation && left.local < right.local)
}

fn enrollment_less(left: AllocationKeyV1, right: AllocationKeyV1) -> (result: bool)
    ensures result == enrollment_key_less_v1(left, right),
{
    enrollment_less_body!(left, right)
}

pub open spec fn enrollment_all_some_v1(values: Seq<Option<AllocationReferenceV1>>) -> bool {
    forall|i: int| 0 <= i < values.len() ==> (#[trigger] values[i]).is_some()
}

pub open spec fn enrollment_swap_relation_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>, left: usize, right: usize) -> bool {
    &&& after == before.update(left as int, before[right as int]).update(right as int, before[left as int])
    &&& after.to_multiset() == before.to_multiset()
    &&& enrollment_all_some_v1(before) ==> enrollment_all_some_v1(after)
}

pub fn enrollment_swap(values: &mut [Option<AllocationReferenceV1>], left: usize, right: usize)
    requires left < old(values).len(), right < old(values).len(),
    ensures enrollment_swap_relation_v1(old(values)@, final(values)@, left, right),
{
    let ghost before = values@;
    enrollment_swap_body!(values, left, right);
    proof {
        let a = before[left as int];
        let b = before[right as int];
        broadcast use vstd::seq_lib::group_to_multiset_ensures;
        broadcast use vstd::multiset::group_multiset_axioms;
        vstd::seq_lib::to_multiset_update(before, left as int, b);
        vstd::seq_lib::to_multiset_update(before.update(left as int, b), right as int, a);
        if left == right { assert(values@ =~= before); }
        else { assert(values@.to_multiset() =~= before.to_multiset()); }
    }
}

pub open spec fn enrollment_heap_edge_v1(values: Seq<Option<AllocationReferenceV1>>, child: int) -> bool {
    values[(child - 1) / 2].unwrap().slot >= values[child].unwrap().slot
}

pub open spec fn enrollment_heap_from_v1(values: Seq<Option<AllocationReferenceV1>>, start: int, end: int) -> bool {
    forall|child: int| 0 < child < end && start <= (child - 1) / 2
        ==> #[trigger] enrollment_heap_edge_v1(values, child)
}

pub open spec fn enrollment_heap_hole_v1(values: Seq<Option<AllocationReferenceV1>>, start: int, end: int, hole: int) -> bool {
    &&& forall|child: int| 0 < child < end && start <= (child - 1) / 2 && (child - 1) / 2 != hole
        ==> #[trigger] enrollment_heap_edge_v1(values, child)
    &&& hole > start ==> forall|child: int| 0 < child < end && (child - 1) / 2 == hole
        ==> values[(hole - 1) / 2].unwrap().slot >= (#[trigger] values[child]).unwrap().slot
}

pub open spec fn enrollment_sift_relation_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>, start: usize, end: usize) -> bool {
    &&& after.len() == before.len()
    &&& after.to_multiset() == before.to_multiset()
    &&& enrollment_all_some_v1(after)
    &&& enrollment_heap_from_v1(after, start as int, end as int)
    &&& forall|i: int| 0 <= i < before.len() && (i < start || i >= end) ==> after[i] == before[i]
    &&& forall|i: int| start <= i < end
        ==> #[trigger] enrollment_interval_origin_v1(before, after[i], start as int, end as int)
}

pub open spec fn enrollment_interval_origin_v1(before: Seq<Option<AllocationReferenceV1>>,
    value: Option<AllocationReferenceV1>, start: int, end: int) -> bool {
    exists|j: int| start <= j < end && (#[trigger] before[j]) == value
}

#[verifier::spinoff_prover]
pub fn enrollment_sift(values: &mut [Option<AllocationReferenceV1>], start: usize, end: usize)
    requires start < end <= old(values).len(), enrollment_all_some_v1(old(values)@),
        enrollment_heap_from_v1(old(values)@, start + 1, end as int),
    ensures enrollment_sift_relation_v1(old(values)@, final(values)@, start, end),
{
    let ghost before = values@;
    enrollment_sift_body!(verus_exec_expr, values, start, end, root, left, child,
        [proof {
        assert forall|i: int| start <= i < end implies
            #[trigger] enrollment_interval_origin_v1(before, values@[i], start as int, end as int) by {
            assert(values@[i] == before[i]);
        }
    }],
        [invariant start <= root < end <= values.len(), values@.len() == before.len(),
            before == old(values)@,
            root == start || start <= (root - 1) / 2,
            enrollment_all_some_v1(values@), values@.to_multiset() == before.to_multiset(),
            enrollment_heap_hole_v1(values@, start as int, end as int, root as int),
            forall|i: int| 0 <= i < before.len() && (i < start || i >= end) ==> values@[i] == before[i],
            forall|i: int| start <= i < end
                ==> #[trigger] enrollment_interval_origin_v1(before, values@[i], start as int, end as int),
        decreases end - root,],
        [proof {
                assert forall|c: int| 0 < c < end && (c - 1) / 2 == root
                    implies #[trigger] enrollment_heap_edge_v1(values@, c) by {
                    assert(c == left || c == left + 1);
                }
                assert(enrollment_heap_from_v1(values@, start as int, end as int));
                assert(enrollment_sift_relation_v1(before, values@, start, end));
            }],
        [let ghost old_values = values@;],
        [proof {
            assert forall|c: int| 0 < c < end && start <= (c - 1) / 2 && (c - 1) / 2 != child
                implies #[trigger] enrollment_heap_edge_v1(values@, c) by {
                let p = (c - 1) / 2;
                if p == root {
                    assert(c == left || c == left + 1);
                } else if c == root {
                    assert(old_values[p].unwrap().slot >= old_values[child as int].unwrap().slot);
                } else {
                    assert(enrollment_heap_edge_v1(old_values, c));
                }
            }
            assert forall|c: int| 0 < c < end && (c - 1) / 2 == child implies
                values@[(child - 1) / 2].unwrap().slot >= (#[trigger] values@[c]).unwrap().slot by {
                assert(enrollment_heap_edge_v1(old_values, c));
            }
        }],
        [proof {
        assert forall|c: int| 0 < c < end && start <= (c - 1) / 2
            implies #[trigger] enrollment_heap_edge_v1(values@, c) by {
            assert((c - 1) / 2 != root);
        }
        assert(enrollment_sift_relation_v1(before, values@, start, end));
    }]
    )
}

pub proof fn enrollment_heap_maximum_v1(values: Seq<Option<AllocationReferenceV1>>, end: int, node: int)
    requires 0 <= node < end <= values.len(), enrollment_heap_from_v1(values, 0, end),
    ensures values[node].unwrap().slot <= values[0].unwrap().slot,
    decreases node,
{
    if node > 0 {
        let parent = (node - 1) / 2;
        enrollment_heap_maximum_v1(values, end, parent);
        assert(enrollment_heap_edge_v1(values, node));
    }
}

pub open spec fn enrollment_sort_frontier_v1(values: Seq<Option<AllocationReferenceV1>>, end: int) -> bool {
    forall|i: int, j: int| 0 <= i < j < values.len() && end <= j
        ==> (#[trigger] values[i]).unwrap().slot <= (#[trigger] values[j]).unwrap().slot
}

pub open spec fn enrollment_sort_relation_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>) -> bool {
    &&& after.len() == before.len()
    &&& after.to_multiset() == before.to_multiset()
    &&& enrollment_all_some_v1(after)
    &&& enrollment_sort_frontier_v1(after, 0)
}

#[verifier::spinoff_prover]
pub fn enrollment_heapsort(values: &mut [Option<AllocationReferenceV1>])
    requires enrollment_all_some_v1(old(values)@),
    ensures enrollment_sort_relation_v1(old(values)@, final(values)@),
{
    let ghost before = values@;
    enrollment_heapsort_body!(verus_exec_expr, values, len, start, end,
        [proof {
        assert forall|c: int| 0 < c < len && start <= (c - 1) / 2
            implies #[trigger] enrollment_heap_edge_v1(values@, c) by {
            assert((c - 1) / 2 < start);
        }
    }], [invariant start <= len / 2, values.len() == len, before == old(values)@,
            values@.len() == before.len(), enrollment_all_some_v1(values@),
            values@.to_multiset() == before.to_multiset(),
            enrollment_heap_from_v1(values@, start as int, len as int),
        decreases start,], [invariant end <= len, values.len() == len, before == old(values)@,
            values@.len() == before.len(), enrollment_all_some_v1(values@),
            values@.to_multiset() == before.to_multiset(),
            enrollment_heap_from_v1(values@, 0, end as int),
            enrollment_sort_frontier_v1(values@, end as int),
        decreases end,],
        [let ghost previous = values@;
        proof {
            assert forall|i: int| 0 <= i <= end implies
                (#[trigger] previous[i]).unwrap().slot <= previous[0].unwrap().slot by {
                enrollment_heap_maximum_v1(previous, end + 1, i);
            }
        }], [let ghost swapped = values@;
        proof {
            assert forall|c: int| 0 < c < end && 1 <= (c - 1) / 2 implies
                #[trigger] enrollment_heap_edge_v1(swapped, c) by {
                assert(enrollment_heap_edge_v1(previous, c));
            }
            assert forall|i: int| 0 <= i < end implies
                (#[trigger] swapped[i]).unwrap().slot <= previous[0].unwrap().slot by {}
        }], [proof {
            assert forall|i: int| 0 <= i <= end implies
                (#[trigger] values@[i]).unwrap().slot <= previous[0].unwrap().slot by {
                if i < end {
                    assert(enrollment_interval_origin_v1(swapped, values@[i], 0, end as int));
                    let p = choose|p: int| 0 <= p < end && swapped[p] == values@[i];
                    assert(swapped[p].unwrap().slot <= previous[0].unwrap().slot);
                }
            }
            assert forall|i: int, j: int| 0 <= i < j < values@.len() && end <= j implies
                (#[trigger] values@[i]).unwrap().slot <= (#[trigger] values@[j]).unwrap().slot by {
                if j == end {
                    assert(values@[j] == previous[0]);
                } else if i <= end {
                    assert(previous[0].unwrap().slot <= previous[j].unwrap().slot);
                } else {
                    assert(previous[i].unwrap().slot <= previous[j].unwrap().slot);
                }
            }
        }]
    )
}

pub open spec fn enrollment_keys_sorted_v1(entries: Seq<EnrollmentV1>) -> bool {
    forall|i: int, j: int| 0 <= i < j < entries.len()
        ==> !enrollment_key_less_v1((#[trigger] entries[j]).key, (#[trigger] entries[i]).key)
}

#[verifier::spinoff_prover]
pub fn contains_key(entries: &[EnrollmentV1], key: AllocationKeyV1) -> (found: bool)
    requires enrollment_keys_sorted_v1(entries@),
    ensures found == (exists|i: int| 0 <= i < entries@.len() && (#[trigger] entries@[i]).key == key),
{
    enrollment_contains_key_body!(verus_exec_expr, entries, key, lo, hi, mid, found,
        [invariant lo <= hi <= entries.len(), enrollment_keys_sorted_v1(entries@),
            forall|i: int| 0 <= i < lo ==> enrollment_key_less_v1((#[trigger] entries@[i]).key, key),
            forall|i: int| hi <= i < entries@.len() ==> !enrollment_key_less_v1((#[trigger] entries@[i]).key, key),
        decreases hi - lo,], [proof {
                assert forall|i: int| 0 <= i <= mid implies enrollment_key_less_v1((#[trigger] entries@[i]).key, key) by {
                    if lo <= i < mid {
                        assert(!enrollment_key_less_v1(entries@[mid as int].key, entries@[i].key));
                    }
                }
            }], [proof {
                assert forall|i: int| mid <= i < entries@.len() implies !enrollment_key_less_v1((#[trigger] entries@[i]).key, key) by {
                    if mid < i < hi {
                        assert(!enrollment_key_less_v1(entries@[i].key, entries@[mid as int].key));
                    }
                }
            }], [proof {
        if !found {
            assert forall|i: int| 0 <= i < entries@.len() implies (#[trigger] entries@[i]).key != key by {
                if lo < i { assert(!enrollment_key_less_v1(entries@[i].key, entries@[lo as int].key)); }
            }
        } else {
            assert(entries@[lo as int].key == key);
        }
    }]
    )
}

#[verifier::spinoff_prover]
pub fn contains_slot(values: &[Option<AllocationReferenceV1>], slot: usize) -> (found: bool)
    requires enrollment_all_some_v1(values@), enrollment_sort_frontier_v1(values@, 0),
    ensures found == (exists|i: int| 0 <= i < values@.len() && (#[trigger] values@[i]).unwrap().slot == slot),
{
    enrollment_contains_slot_body!(verus_exec_expr, values, slot, lo, hi, mid, found,
        [invariant lo <= hi <= values.len(), enrollment_all_some_v1(values@), enrollment_sort_frontier_v1(values@, 0),
            forall|i: int| 0 <= i < lo ==> (#[trigger] values@[i]).unwrap().slot < slot,
            forall|i: int| hi <= i < values@.len() ==> (#[trigger] values@[i]).unwrap().slot >= slot,
        decreases hi - lo,], [proof {
                assert forall|i: int| 0 <= i <= mid implies (#[trigger] values@[i]).unwrap().slot < slot by {
                    if lo <= i < mid { assert(values@[i].unwrap().slot <= values@[mid as int].unwrap().slot); }
                }
            }], [proof {
                assert forall|i: int| mid <= i < values@.len() implies (#[trigger] values@[i]).unwrap().slot >= slot by {
                    if mid < i < hi { assert(values@[mid as int].unwrap().slot <= values@[i].unwrap().slot); }
                }
            }], [proof {
        if !found {
            assert forall|i: int| 0 <= i < values@.len() implies (#[trigger] values@[i]).unwrap().slot != slot by {
                if lo < i { assert(values@[lo as int].unwrap().slot <= values@[i].unwrap().slot); }
            }
        } else { assert(values@[lo as int].unwrap().slot == slot); }
    }]
    )
}


pub fn enrollment_sorted(values: &[Option<AllocationReferenceV1>]) -> (result: bool)
    requires enrollment_all_some_v1(values@),
    ensures result == enrollment_sort_frontier_v1(values@, 0),
{
    enrollment_sorted_body!(verus_exec_expr, values, index, [
        invariant 1 <= index <= values.len() + 1,
            enrollment_all_some_v1(values@),
            forall|i: int, j: int| 0 <= i < j < index && j < values.len()
                ==> (#[trigger] values@[i]).unwrap().slot <= (#[trigger] values@[j]).unwrap().slot,
        decreases values.len() + 1 - index,
    ])
}

pub fn sort_slots(values: &mut [Option<AllocationReferenceV1>])
    requires enrollment_all_some_v1(old(values)@),
    ensures enrollment_sort_relation_v1(old(values)@, final(values)@),
        enrollment_sort_frontier_v1(old(values)@, 0) ==> final(values)@ == old(values)@,
{
    enrollment_sort_body!(values)
}

}
