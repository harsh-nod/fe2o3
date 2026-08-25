use vstd::prelude::*;

verus! {

proof fn noninjective_route_is_rejected(mapping: Seq<nat>)
    requires mapping.len() >= 2, mapping[0] == mapping[1],
    ensures 0nat == 1nat,
{
}

}

fn main() {}
