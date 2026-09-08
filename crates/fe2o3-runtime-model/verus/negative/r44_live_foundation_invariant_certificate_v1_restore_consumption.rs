// Expected-negative R44 mutation: final restore leaves a live certificate.
use vstd::prelude::*;
verus! {
pub struct RestoreOutcomeV1 {
    pub session_authority_returned: bool,
    pub certificate_live: bool,
}

// Mutation: successful restore returns Session authority without consuming the
// certificate.
pub open spec fn mutated_restore_v1(_certificate_live_before: bool) -> RestoreOutcomeV1 {
    RestoreOutcomeV1 {
        session_authority_returned: true,
        certificate_live: true,
    }
}
pub proof fn mutated_restore_consumes_certificate_v1()
    ensures {
        let out = mutated_restore_v1(true);
        out.session_authority_returned && !out.certificate_live
    },
{}
}
