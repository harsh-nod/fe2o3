#[path = "../guarded_u32_xor_helper_store_composition_v3.rs"]
mod composition;

use vstd::prelude::*;

verus! {

proof fn hostile_mutation_v3(
    first: composition::DynamicSiteCertificateV3,
    second: composition::DynamicSiteCertificateV3,
    output: composition::DynamicSiteCertificateV3,
)
    ensures composition::exact_shared_guard_domain_v3(first, second, output, 0, 0, 2),
{
}

}
