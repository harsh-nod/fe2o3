use vstd::prelude::*;

// assume(false), assume_(false), verus_builtin::assume_(false), and admit() are inert here.
/* Nested comments are trivia: assume/* admit() */(false).
   So is pub uninterp spec fn hidden(value: int) -> int;. */
verus! {

pub uninterp spec fn exp_real_v1(value: real) -> real;
pub open spec fn assume_init_count_v1() -> nat { 0 }

pub proof fn ordinary_literals_are_not_code() {
    let message = "assume(false) assume_(false) admit() #[verifier::external_body]";
    let raw = r#"verus_builtin/*split*/::r#assume_(false)"#;
    assert(message.len() > 0 && raw.len() > 0);
}

} // verus!
