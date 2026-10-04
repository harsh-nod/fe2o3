use vstd::prelude::*;

verus! {

proof fn forbidden_assumption_regression() {
    assume ( false );
}

} // verus!
