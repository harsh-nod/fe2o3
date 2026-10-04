use dialect_dispatch::{
    RegistrationOutcome as DispatchOutcome, register_dialect as register_dispatch,
};
use dialect_gpu::{RegistrationOutcome as GpuOutcome, register_dialect as register_gpu};
use dialect_proof::{RegistrationOutcome as ProofOutcome, register_dialect as register_proof};
use pliron::context::Context;

#[test]
fn all_three_dialects_share_one_context_without_duplicate_registration() {
    let mut context = Context::new();

    assert_eq!(register_gpu(&mut context), Ok(GpuOutcome::Registered));
    assert_eq!(register_proof(&mut context), Ok(ProofOutcome::Registered));
    assert_eq!(
        register_dispatch(&mut context),
        Ok(DispatchOutcome::Registered)
    );

    assert_eq!(
        register_gpu(&mut context),
        Ok(GpuOutcome::AlreadyRegistered)
    );
    assert_eq!(
        register_proof(&mut context),
        Ok(ProofOutcome::AlreadyRegistered)
    );
    assert_eq!(
        register_dispatch(&mut context),
        Ok(DispatchOutcome::AlreadyRegistered)
    );
}
