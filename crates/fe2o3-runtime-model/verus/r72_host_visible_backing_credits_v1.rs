// Production cost projection corresponding to src/r72_host_visible_backing_credits.rs.
// Profile/flag selection and actual native layout extraction are adapter obligations.
// This proves bounded ordinary spans and the existing R67 vector projection,
// not native disposal, exact account custody, executable refinement or global budgets.
use vstd::prelude::*;
use vstd::assert_seqs_equal;
verus! {
pub open spec fn ordinary_host_span_v1(requested: u64, cpu: u64, gpu_va: u64) -> bool {
    &&& 0 < requested <= cpu <= 2147483648
    &&& (cpu as int) - (requested as int) < 4096
    &&& cpu % 4096 == 0
    &&& gpu_va == cpu
}

pub open spec fn host_backing_charge_v1(cpu: u64) -> Seq<u64> {
    Seq::new(19, |i: int| if i == 2 { cpu } else if i == 18 { 1u64 } else { 0u64 })
}

pub fn host_visible_backing_charge_v1(requested: u64, cpu: u64, gpu_va: u64) -> (result: Option<[u64; 19]>)
    ensures
        result.is_some() == ordinary_host_span_v1(requested, cpu, gpu_va),
        match result {
            Some(charge) => charge@ == host_backing_charge_v1(cpu),
            None => true,
        },
{
    if requested == 0 || cpu > 2147483648 || cpu < requested
        || cpu - requested >= 4096 || cpu % 4096 != 0 || gpu_va != cpu
    { return None; }
    let mut charge = [0u64; 19];
    charge[2] = cpu;
    charge[18] = 1;
    proof { assert_seqs_equal!(charge@, host_backing_charge_v1(cpu)); }
    Some(charge)
}

pub proof fn accepted_span_is_positive_bounded_and_covers_request_v1(requested: u64, cpu: u64, gpu_va: u64)
    requires ordinary_host_span_v1(requested, cpu, gpu_va),
    ensures
        0 < requested <= cpu <= 2147483648,
        0 <= (cpu as int) - (requested as int) < 4096,
        cpu % 4096 == 0, gpu_va == cpu,
{}

pub proof fn projected_host_backing_and_record_are_exact_v1(requested: u64, cpu: u64, gpu_va: u64)
    requires ordinary_host_span_v1(requested, cpu, gpu_va),
    ensures host_backing_charge_v1(cpu).len() == 19,
        host_backing_charge_v1(cpu)[2] == cpu,
        host_backing_charge_v1(cpu)[18] == 1,
{}

pub proof fn cpu_gpu_views_do_not_charge_other_dimensions_v1(requested: u64, cpu: u64, gpu_va: u64, index: int)
    requires ordinary_host_span_v1(requested, cpu, gpu_va), 0 <= index < 19, index != 2, index != 18,
    ensures host_backing_charge_v1(cpu)[index] == 0,
{}

// R67 reserve/release postconditions instantiated with this projection. These
// premises are arithmetic, not evidence that any native disposal took place.
pub proof fn projected_reserve_release_conserves_v1(
    requested: u64, cpu: u64, gpu_va: u64,
    used: Seq<u64>, next: Seq<u64>, released: Seq<u64>,
)
    requires ordinary_host_span_v1(requested, cpu, gpu_va),
        used.len() == 19, next.len() == 19, released.len() == 19,
        forall|i: int| 0 <= i < 19 ==> (next[i] as int) == (used[i] as int) + (host_backing_charge_v1(cpu)[i] as int),
        forall|i: int| 0 <= i < 19 ==> (released[i] as int) == (next[i] as int) - (host_backing_charge_v1(cpu)[i] as int),
    ensures released == used,
{
    assert_seqs_equal!(released, used);
}
}
