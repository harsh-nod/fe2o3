use vstd::prelude::*;

// assume(false), admit(), and #[verifier::external_body] are inert here.
/* Nested comments are trivia: assume/* admit() */(false).
   So is pub uninterp spec fn hidden(value: int) -> int;. */
verus! {

pub uninterp spec fn exp_real_v1(value: real) -> real;

pub proof fn ordinary_literals_are_not_code() {
    let message = "assume(false) admit() #[verifier::external_body] pub uninterp spec fn hidden();";
    let raw = r#"assume/*split*/(false)"#;
    assert(message.len() > 0 && raw.len() > 0);
}

} // verus!
