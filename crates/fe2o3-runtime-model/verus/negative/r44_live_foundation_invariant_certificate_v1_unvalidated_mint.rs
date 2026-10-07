// Expected-negative R44 mutation: unvalidated transfer mints a certificate.
use vstd::prelude::*;
verus! {
pub enum TransferValidationV1 { Unchecked, Validated }
pub open spec fn mutated_transfer_validation_v1() -> TransferValidationV1 {
    TransferValidationV1::Unchecked
}
pub proof fn mutated_unvalidated_transfer_mints_certificate_v1()
    ensures mutated_transfer_validation_v1() == TransferValidationV1::Validated,
{}
}
