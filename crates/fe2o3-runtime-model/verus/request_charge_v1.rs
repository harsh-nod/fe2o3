// Pure request-profile construction, not admission or a live credit observation.
// The source checker binds the actual nineteen-kind native order and dimension.
// Array view specifications and compiler lowering remain trusted boundaries;
// no allocation, Arc/Mutex, freshness, custody or native-cost fact is supplied.
use vstd::prelude::verus as resource_vector_declarations_v1;
use vstd::prelude::*;
include!("../src/resource_vector_declarations.rs");
include!("../src/request_charge_body.rs");

verus! {

spec fn request_charge_coordinates(counts: Seq<u64>, bytes: u64) -> bool {
    &&& counts.len() == 19
    &&& forall|index: int| 0 <= index < 19 ==> #[trigger] counts[index]
        == if index == 1 { bytes } else if index == 18 { 1u64 } else { 0u64 }
}

fn r67_requested_allocation_charge_v1(bytes: u64) -> (charge: R67ResourceVectorV1)
    ensures request_charge_coordinates(charge.counts@, bytes),
{
    resource_request_charge_body_v1!(bytes)
}

}
