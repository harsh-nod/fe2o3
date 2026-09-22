verus! {

pub open spec fn enrollment_nonincreasing(values: Seq<Option<AllocationReferenceV1>>) -> bool {
    forall|i: int, j: int| 0 <= i < j < values.len()
        ==> (#[trigger] values[i]).unwrap().slot >= (#[trigger] values[j]).unwrap().slot
}

pub fn enrollment_ordered(values: &[Option<AllocationReferenceV1>], reverse: bool) -> (result: bool)
    requires enrollment_all_some_v1(values@),
    ensures result == if reverse { enrollment_nonincreasing(values@) } else { enrollment_sort_frontier_v1(values@, 0) },
{
    enrollment_ordered_body!(verus_exec_expr, values, reverse, index, previous, [
        invariant 1 <= index <= values.len(), enrollment_all_some_v1(values@),
            previous == values@[index - 1].unwrap().slot,
            forall|i: int, j: int| 0 <= i < j < index && j < values.len()
                ==> if reverse { (#[trigger] values@[i]).unwrap().slot >= (#[trigger] values@[j]).unwrap().slot }
                    else { (#[trigger] values@[i]).unwrap().slot <= (#[trigger] values@[j]).unwrap().slot },
        decreases values.len() - index,
    ])
}

pub fn enrollment_reverse_halves(left: &mut [Option<AllocationReferenceV1>], right: &mut [Option<AllocationReferenceV1>])
    requires old(left)@.len() == old(right)@.len(),
        enrollment_all_some_v1(old(left)@), enrollment_all_some_v1(old(right)@),
    ensures final(left)@ == old(right)@.reverse(), final(right)@ == old(left)@.reverse(),
{
    let ghost before_left = left@;
    let ghost before_right = right@;
    enrollment_reverse_halves_body!(verus_exec_expr, left, right, index, len, [
        invariant before_left == old(left)@, before_right == old(right)@,
            index <= len, left.len() == len, right.len() == len,
            before_left.len() == len, before_right.len() == len,
            forall|i: int| 0 <= i < len ==> (#[trigger] left@[i]) ==
                if i < index { before_right[len - 1 - i] } else { before_left[i] },
            forall|i: int| 0 <= i < len ==> (#[trigger] right@[i]) ==
                if len - index <= i { before_left[len - 1 - i] } else { before_right[i] },
        decreases len - index,
    ]);
    assert(left@ =~= before_right.reverse());
    assert(right@ =~= before_left.reverse());
}

pub fn enrollment_reverse(values: &mut [Option<AllocationReferenceV1>])
    requires enrollment_all_some_v1(old(values)@),
    ensures final(values)@.len() == old(values)@.len(),
        enrollment_all_some_v1(final(values)@),
        final(values)@.to_multiset() == old(values)@.to_multiset(),
        forall|i: int| 0 <= i < final(values)@.len()
            ==> (#[trigger] final(values)@[i]) == old(values)@[old(values)@.len() - 1 - i],
{
    let ghost before = values@;
    enrollment_reverse_body!(verus_exec_expr, values, half, left, tail, middle, right,
        [let ghost left_before = left@; let ghost right_before = right@;],
        [proof {
            let after = left@ + (middle@ + right@);
            assert forall|i: int| 0 <= i < before.len() implies
                (#[trigger] after[i]) == before[before.len() - 1 - i] by {
                if i < half { assert(left@[i] == right_before[half - 1 - i]); }
                else if i < half + middle.len() { assert(i == before.len() - 1 - i); }
                else { assert(right@[i - half - middle.len()] == left_before[before.len() - 1 - i]); }
            }
            assert(after =~= before.reverse());
            before.lemma_reverse_to_multiset();
        }]
    )
}

pub open spec fn enrollment_prefix_sorted(values: Seq<Option<AllocationReferenceV1>>, end: int) -> bool {
    forall|i: int, j: int| 0 <= i < j < end
        ==> (#[trigger] values[i]).unwrap().slot <= (#[trigger] values[j]).unwrap().slot
}

pub proof fn enrollment_swap_multiset(before: Seq<Option<AllocationReferenceV1>>, left: int, right: int)
    requires 0 <= left < before.len(), 0 <= right < before.len(),
    ensures before.update(left, before[right]).update(right, before[left]).to_multiset() == before.to_multiset(),
{
    broadcast use vstd::seq_lib::group_to_multiset_ensures;
    broadcast use vstd::multiset::group_multiset_axioms;
    let a = before[left];
    let b = before[right];
    vstd::seq_lib::to_multiset_update(before, left, b);
    vstd::seq_lib::to_multiset_update(before.update(left, b), right, a);
    if left == right {
        assert(before.update(left, b).update(right, a) =~= before);
    } else {
        assert(before.update(left, b).update(right, a).to_multiset() =~= before.to_multiset());
    }
}

#[verifier::spinoff_prover]
pub fn enrollment_insertion(values: &mut [Option<AllocationReferenceV1>])
    requires enrollment_all_some_v1(old(values)@),
    ensures enrollment_sort_relation_v1(old(values)@, final(values)@),
        enrollment_sort_frontier_v1(old(values)@, 0) ==> final(values)@ == old(values)@,
{
    let ghost before = values@;
    enrollment_insertion_body!(verus_exec_expr, values, index, hole, held, [
        invariant before == old(values)@, values@.len() == before.len(),
            1 <= index <= values.len() + 1, enrollment_all_some_v1(values@),
            values@.to_multiset() == before.to_multiset(),
            enrollment_prefix_sorted(values@, index as int),
            enrollment_sort_frontier_v1(before, 0) ==> values@ == before,
        decreases values.len() + 1 - index,
    ], [proof { assert(values@.update(hole as int, held) =~= values@); }], [
        invariant before == old(values)@, values@.len() == before.len(),
            hole <= index < values.len(), held.is_some(), enrollment_all_some_v1(values@),
            hole == index ==> values@[hole as int] == held,
            enrollment_sort_frontier_v1(before, 0) ==> values@ == before && hole == index,
            values@.update(hole as int, held).to_multiset() == before.to_multiset(),
            forall|i: int, j: int| 0 <= i < j <= index && i != hole && j != hole
                ==> (#[trigger] values@[i]).unwrap().slot <= (#[trigger] values@[j]).unwrap().slot,
            forall|i: int| hole < i <= index ==> held.unwrap().slot < (#[trigger] values@[i]).unwrap().slot,
        decreases hole,
    ], [
        let ghost prior = values@;
        let ghost prior_hole = hole;
    ], [proof {
        let repaired = prior.update(prior_hole as int, held);
        enrollment_swap_multiset(repaired, prior_hole as int, hole as int);
        assert(values@.update(hole as int, held) =~=
            repaired.update(prior_hole as int, repaired[hole as int]).update(hole as int, held));
        assert forall|i: int, j: int| 0 <= i < j <= index && i != hole && j != hole implies
            (#[trigger] values@[i]).unwrap().slot <= (#[trigger] values@[j]).unwrap().slot by {
            if j == prior_hole { assert(prior[i].unwrap().slot <= prior[hole as int].unwrap().slot); }
            else if i == prior_hole { assert(prior[hole as int].unwrap().slot <= prior[j].unwrap().slot); }
        }
    }], [let ghost prior = values@;], [proof {
        assert(values@ =~= prior.update(hole as int, held));
        assert forall|i: int, j: int| 0 <= i < j <= index implies
            (#[trigger] values@[i]).unwrap().slot <= (#[trigger] values@[j]).unwrap().slot by {
            if j == hole && i < hole - 1 {
                assert(prior[i].unwrap().slot <= prior[hole - 1].unwrap().slot);
            }
        }
    }])
}

pub open spec fn enrollment_median(a: usize, b: usize, c: usize, value: usize) -> bool {
    &&& value == a || value == b || value == c
    &&& (a <= value && b <= value) || (a <= value && c <= value) || (b <= value && c <= value)
    &&& (value <= a && value <= b) || (value <= a && value <= c) || (value <= b && value <= c)
}

pub open spec fn enrollment_median_value(a: usize, b: usize, c: usize) -> usize {
    if a < b {
        if b < c { b } else if a < c { c } else { a }
    } else if a < c { a } else if b < c { c } else { b }
}

pub fn enrollment_median_of_three(first: usize, middle: usize, last: usize) -> (result: usize)
    ensures enrollment_median(first, middle, last, result),
        result == enrollment_median_value(first, middle, last),
{
    enrollment_median_body!(first, middle, last)
}

pub open spec fn enrollment_pivot_decision(values: Seq<Option<AllocationReferenceV1>>) -> usize {
    let len = values.len();
    let middle = len / 2;
    let step = len / 8;
    if len == 0 { 0 }
    else if len < 128 {
        enrollment_median_value(values[0].unwrap().slot, values[middle as int].unwrap().slot,
            values[len - 1].unwrap().slot)
    } else {
        enrollment_median_value(
            enrollment_median_value(values[0].unwrap().slot, values[step as int].unwrap().slot,
                values[(step * 2) as int].unwrap().slot),
            enrollment_median_value(values[(middle - step) as int].unwrap().slot,
                values[middle as int].unwrap().slot, values[(middle + step) as int].unwrap().slot),
            enrollment_median_value(values[(len - 1 - step * 2) as int].unwrap().slot,
                values[(len - 1 - step) as int].unwrap().slot, values[len - 1].unwrap().slot))
    }
}

pub fn enrollment_pivot(values: &[Option<AllocationReferenceV1>]) -> (pivot: usize)
    requires values.len() > 0, enrollment_all_some_v1(values@),
    ensures pivot == enrollment_pivot_decision(values@),
        exists|i: int| 0 <= i < values.len() && (#[trigger] values@[i]).unwrap().slot == pivot,
{
    enrollment_pivot_body!(values)
}

pub open spec fn enrollment_bands(values: Seq<Option<AllocationReferenceV1>>, pivot: usize, less: int, greater: int) -> bool {
    &&& 0 <= less <= greater <= values.len()
    &&& forall|i: int| 0 <= i < less ==> (#[trigger] values[i]).unwrap().slot < pivot
    &&& forall|i: int| less <= i < greater ==> (#[trigger] values[i]).unwrap().slot == pivot
    &&& forall|i: int| greater <= i < values.len() ==> (#[trigger] values[i]).unwrap().slot > pivot
}

#[verifier::spinoff_prover]
pub fn enrollment_partition(values: &mut [Option<AllocationReferenceV1>], pivot: usize) -> (result: (usize, usize))
    requires enrollment_all_some_v1(old(values)@),
    ensures final(values)@.len() == old(values)@.len(),
        final(values)@.to_multiset() == old(values)@.to_multiset(),
        enrollment_all_some_v1(final(values)@),
        enrollment_bands(final(values)@, pivot, result.0 as int, result.1 as int),
{
    let ghost before = values@;
    enrollment_partition_body!(verus_exec_expr, values, pivot, less, scan, greater, [
        invariant before == old(values)@, values@.len() == before.len(),
            less <= scan <= greater <= values.len(), enrollment_all_some_v1(values@),
            values@.to_multiset() == before.to_multiset(),
            forall|i: int| 0 <= i < less ==> (#[trigger] values@[i]).unwrap().slot < pivot,
            forall|i: int| less <= i < scan ==> (#[trigger] values@[i]).unwrap().slot == pivot,
            forall|i: int| greater <= i < values.len() ==> (#[trigger] values@[i]).unwrap().slot > pivot,
        decreases greater - scan,
    ])
}

pub open spec fn enrollment_depth_decision(remaining: usize, depth: u32) -> u32
    decreases remaining,
{
    if remaining > 1 && depth < 64 {
        enrollment_depth_decision((remaining / 2) as usize, (depth + 1) as u32)
    } else { (depth * 2) as u32 }
}

pub fn enrollment_depth(len: usize) -> (result: u32)
    ensures result <= 128, result == enrollment_depth_decision(len, 0),
{
    enrollment_depth_body!(verus_exec_expr, len, remaining, depth, [
        invariant depth <= 64,
            enrollment_depth_decision(len, 0) == enrollment_depth_decision(remaining, depth),
        decreases remaining,
    ])
}

pub open spec fn enrollment_weak_bands(values: Seq<Option<AllocationReferenceV1>>, pivot: usize, less: int, greater: int) -> bool {
    &&& 0 <= less <= greater <= values.len()
    &&& forall|i: int| 0 <= i < less ==> (#[trigger] values[i]).unwrap().slot <= pivot
    &&& forall|i: int| less <= i < greater ==> (#[trigger] values[i]).unwrap().slot == pivot
    &&& forall|i: int| greater <= i < values.len() ==> (#[trigger] values[i]).unwrap().slot >= pivot
}

#[verifier::spinoff_prover]
pub fn enrollment_binary_partition(values: &mut [Option<AllocationReferenceV1>], pivot: usize) -> (result: (usize, usize))
    requires enrollment_all_some_v1(old(values)@),
    ensures final(values)@.len() == old(values)@.len(),
        final(values)@.to_multiset() == old(values)@.to_multiset(),
        enrollment_all_some_v1(final(values)@), result.0 == result.1,
        enrollment_weak_bands(final(values)@, pivot, result.0 as int, result.1 as int),
{
    let ghost before = values@;
    enrollment_binary_partition_body!(verus_exec_expr, values, pivot, left, right, [
        invariant before == old(values)@, values@.len() == before.len(),
            left <= values.len(), right <= values.len(), left <= right + 1,
            enrollment_all_some_v1(values@), values@.to_multiset() == before.to_multiset(),
            forall|i: int| 0 <= i < left ==> (#[trigger] values@[i]).unwrap().slot <= pivot,
            forall|i: int| right <= i < values.len() ==> (#[trigger] values@[i]).unwrap().slot >= pivot,
        decreases right + 1 - left,
    ], [let ghost outer_left = left; let ghost outer_right = right;], [
        invariant before == old(values)@, values@.len() == before.len(), left <= right <= values.len(),
            outer_left <= left, right == outer_right, outer_left < outer_right,
            enrollment_all_some_v1(values@), values@.to_multiset() == before.to_multiset(),
            forall|i: int| 0 <= i < left ==> (#[trigger] values@[i]).unwrap().slot <= pivot,
            forall|i: int| right <= i < values.len() ==> (#[trigger] values@[i]).unwrap().slot >= pivot,
        decreases right - left,
    ], [
        invariant before == old(values)@, values@.len() == before.len(), left <= right <= values.len(),
            outer_left <= left, right <= outer_right, outer_left < outer_right,
            enrollment_all_some_v1(values@), values@.to_multiset() == before.to_multiset(),
            left < right ==> values@[left as int].unwrap().slot >= pivot,
            forall|i: int| 0 <= i < left ==> (#[trigger] values@[i]).unwrap().slot <= pivot,
            forall|i: int| right <= i < values.len() ==> (#[trigger] values@[i]).unwrap().slot >= pivot,
        decreases right - left,
    ])
}

#[verifier::spinoff_prover]
pub fn enrollment_lomuto(values: &mut [Option<AllocationReferenceV1>], pivot: usize) -> (result: (usize, usize))
    requires enrollment_all_some_v1(old(values)@),
    ensures final(values)@.len() == old(values)@.len(),
        final(values)@.to_multiset() == old(values)@.to_multiset(),
        enrollment_all_some_v1(final(values)@), result.0 == result.1,
        enrollment_weak_bands(final(values)@, pivot, result.0 as int, result.1 as int),
{
    let ghost before = values@;
    enrollment_lomuto_body!(verus_exec_expr, values, pivot, less, scan, gap, held, below,
        [proof { assert(values@.update(gap as int, held) =~= values@); }], [
        invariant before == old(values)@, values@.len() == before.len(),
            less <= gap < scan <= values.len(), gap + 1 == scan,
            held.is_some(), enrollment_all_some_v1(values@),
            values@.update(gap as int, held).to_multiset() == before.to_multiset(),
            forall|i: int| 0 <= i < less ==> (#[trigger] values@[i]).unwrap().slot < pivot,
            forall|i: int| less <= i < gap ==> (#[trigger] values@[i]).unwrap().slot >= pivot,
        decreases values.len() - scan,
    ], [
        let ghost prior = values@;
        let ghost prior_gap = gap;
        let ghost prior_less = less;
        let ghost prior_scan = scan;
    ], [proof {
        let repaired = prior.update(prior_gap as int, held);
        let swapped = repaired.update(prior_gap as int, repaired[prior_less as int])
                              .update(prior_less as int, held);
        enrollment_swap_multiset(repaired, prior_gap as int, prior_less as int);
        enrollment_swap_multiset(swapped, prior_less as int, prior_scan as int);
        assert(values@.update(gap as int, held) =~=
            swapped.update(prior_less as int, swapped[prior_scan as int]).update(prior_scan as int, held));
    }], [let ghost prior = values@; let ghost prior_less = less;], [proof {
        let repaired = prior.update(gap as int, held);
        enrollment_swap_multiset(repaired, gap as int, prior_less as int);
        assert(values@ =~= repaired.update(gap as int, repaired[prior_less as int])
                                  .update(prior_less as int, held));
    }])
}

pub fn enrollment_partition_adaptive(values: &mut [Option<AllocationReferenceV1>], pivot: usize) -> (result: (usize, usize))
    requires old(values).len() > 0, enrollment_all_some_v1(old(values)@),
    ensures final(values)@.len() == old(values)@.len(),
        final(values)@.to_multiset() == old(values)@.to_multiset(),
        enrollment_all_some_v1(final(values)@),
        enrollment_weak_bands(final(values)@, pivot, result.0 as int, result.1 as int),
{
    enrollment_partition_adaptive_body!(values, pivot)
}

pub proof fn enrollment_permutation_origin(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>, i: int) -> (j: int)
    requires before.to_multiset() == after.to_multiset(), 0 <= i < after.len(),
    ensures 0 <= j < before.len(), before[j] == after[i],
{
    after.lemma_index_contains(i);
    vstd::seq_lib::to_multiset_contains(after, after[i]);
    vstd::seq_lib::to_multiset_contains(before, after[i]);
    assert(before.contains(after[i]));
    before.lemma_contains_to_index(after[i])
}

#[verifier::spinoff_prover]
pub proof fn enrollment_compose(left_before: Seq<Option<AllocationReferenceV1>>,
    middle: Seq<Option<AllocationReferenceV1>>, right_before: Seq<Option<AllocationReferenceV1>>,
    left: Seq<Option<AllocationReferenceV1>>, right: Seq<Option<AllocationReferenceV1>>, pivot: usize)
    requires enrollment_sort_relation_v1(left_before, left), enrollment_sort_relation_v1(right_before, right),
        enrollment_all_some_v1(middle),
        forall|i: int| 0 <= i < left_before.len() ==> (#[trigger] left_before[i]).unwrap().slot <= pivot,
        forall|i: int| 0 <= i < middle.len() ==> (#[trigger] middle[i]).unwrap().slot == pivot,
        forall|i: int| 0 <= i < right_before.len() ==> (#[trigger] right_before[i]).unwrap().slot >= pivot,
    ensures enrollment_sort_relation_v1(left_before + (middle + right_before), left + (middle + right)),
{
    assert forall|i: int| 0 <= i < left.len() implies (#[trigger] left[i]).unwrap().slot <= pivot by {
        let j = enrollment_permutation_origin(left_before, left, i);
        assert(left_before[j].unwrap().slot <= pivot);
    }
    assert forall|i: int| 0 <= i < right.len() implies (#[trigger] right[i]).unwrap().slot >= pivot by {
        let j = enrollment_permutation_origin(right_before, right, i);
        assert(right_before[j].unwrap().slot >= pivot);
    }
    let after = left + (middle + right);
    assert forall|i: int| 0 <= i < after.len() implies (#[trigger] after[i]).is_some() by {
        if i < left.len() { assert(after[i] == left[i]); }
        else if i < left.len() + middle.len() { assert(after[i] == middle[i - left.len()]); }
        else { assert(after[i] == right[i - left.len() - middle.len()]); }
    }
    assert forall|i: int, j: int| 0 <= i < j < after.len() implies
        (#[trigger] after[i]).unwrap().slot <= (#[trigger] after[j]).unwrap().slot by {
        if j < left.len() {
            assert(left[i].unwrap().slot <= left[j].unwrap().slot);
        } else if i < left.len() {
            assert(left[i].unwrap().slot <= pivot);
            if j < left.len() + middle.len() { assert(middle[j - left.len()].unwrap().slot == pivot); }
            else { assert(right[j - left.len() - middle.len()].unwrap().slot >= pivot); }
        } else if i < left.len() + middle.len() {
            assert(middle[i - left.len()].unwrap().slot == pivot);
            if j < left.len() + middle.len() { assert(middle[j - left.len()].unwrap().slot == pivot); }
            else { assert(right[j - left.len() - middle.len()].unwrap().slot >= pivot); }
        } else {
            assert(right[i - left.len() - middle.len()].unwrap().slot
                <= right[j - left.len() - middle.len()].unwrap().slot);
        }
    }
    vstd::seq_lib::lemma_multiset_commutative(middle, right_before);
    vstd::seq_lib::lemma_multiset_commutative(left_before, middle + right_before);
    vstd::seq_lib::lemma_multiset_commutative(middle, right);
    vstd::seq_lib::lemma_multiset_commutative(left, middle + right);
}

#[verifier::spinoff_prover]
pub fn enrollment_introsort(values: &mut [Option<AllocationReferenceV1>], depth: u32)
    requires enrollment_all_some_v1(old(values)@),
    ensures enrollment_sort_relation_v1(old(values)@, final(values)@),
    decreases depth,
{
    let ghost before = values@;
    enrollment_introsort_body!(verus_exec_expr, values, depth, pivot, less, greater,
        left, tail, middle, right,
        [let ghost partitioned = values@;],
        [let ghost left_before = left@;
         let ghost middle_before = middle@;
         let ghost right_before = right@;
         proof { assert(partitioned =~= left_before + (middle_before + right_before)); }],
        [proof {
            enrollment_compose(left_before, middle@, right_before, left@, right@, pivot);
        }]
    )
}

pub fn adaptive_sort_slots(values: &mut [Option<AllocationReferenceV1>])
    requires enrollment_all_some_v1(old(values)@),
    ensures enrollment_sort_relation_v1(old(values)@, final(values)@),
        enrollment_sort_frontier_v1(old(values)@, 0) ==> final(values)@ == old(values)@,
{
    let ghost before = values@;
    enrollment_adaptive_body!(verus_exec_expr, values, [proof {
        assert forall|i: int, j: int| 0 <= i < j < values@.len() implies
            (#[trigger] values@[i]).unwrap().slot <= (#[trigger] values@[j]).unwrap().slot by {
            assert(before[before.len() - 1 - j].unwrap().slot >= before[before.len() - 1 - i].unwrap().slot);
        }
    }])
}

}
