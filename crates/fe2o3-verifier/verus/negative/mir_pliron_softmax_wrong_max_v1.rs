use vstd::prelude::*;

verus! {

proof fn wrong_softmax_max(left: int, right: int)
    requires left < right,
    ensures
        (if left < right { right } else { left })
            == (if left > right { right } else { left }),
{
}

}

fn main() {}
