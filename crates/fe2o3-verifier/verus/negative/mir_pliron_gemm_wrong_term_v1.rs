use vstd::prelude::*;

verus! {

spec fn trace(trace: Seq<int>, terms: Seq<int>) -> bool {
    trace.len() == terms.len() + 1
        && trace[0] == 0
        && forall|i: int| 0 <= i < terms.len() ==>
            #[trigger] trace[i + 1] == trace[i] + terms[i]
}

proof fn wrong_gemm_term(actual: Seq<int>, reference: Seq<int>, terms: Seq<int>)
    requires
        trace(actual, terms),
        trace(reference, terms),
        terms.len() > 0,
    ensures
        actual[terms.len() as int] == reference[terms.len() as int] + 1,
{
}

}

fn main() {}
