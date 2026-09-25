use super::*;
use std::cell::Cell;

/// Identity-comparison fixture only; no admission, device, or native execution.
struct BindingComparisonFixture {
    binding: Gfx942RuntimeInvocationBindingV1,
    callbacks: Cell<usize>,
    stale: bool,
}

// SAFETY: confined to private preflight tests; never passed to a launch entrypoint.
unsafe impl WorkerV3Gfx942ExecutionAuthorityV1 for BindingComparisonFixture {
    type CurrentnessError = &'static str;

    fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        [1; 32]
    }
    fn finalized_hsaco_length(&self) -> u64 {
        7_000
    }
    fn kernel_name(&self) -> &str {
        "kernel_v1"
    }
    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        [2; 32]
    }
    fn invocation_binding(&self) -> Gfx942RuntimeInvocationBindingV1 {
        self.binding
    }
    fn device_unique_id(&self) -> u64 {
        0x1234
    }
    fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError> {
        self.callbacks.set(self.callbacks.get() + 1);
        if self.stale {
            Err("stale publication")
        } else {
            Ok(())
        }
    }
}

fn conditional(contract: u8, premise: u8) -> Gfx942RuntimeInvocationBindingV1 {
    Gfx942RuntimeInvocationBindingV1::ConditionalNominalV4 {
        contract_identity: [contract; 32],
        premise_identity: [premise; 32],
    }
}

fn check(
    authority: &BindingComparisonFixture,
    prepared: Gfx942RuntimeInvocationBindingV1,
) -> Result<(), Gfx942AuthorizedRuntimeExecutionErrorV1<&'static str>> {
    validate_authority_bindings_v1(
        authority,
        [1; 32],
        7_000,
        "kernel_v1",
        [2; 32],
        prepared,
        0x1234,
    )
}

#[test]
fn family_omission_and_payload_substitution_precede_callbacks_even_with_matching_hashes() {
    use Gfx942RuntimeInvocationBindingV1::OrdinaryV1;
    for (retained, prepared) in [
        (OrdinaryV1, conditional(3, 4)),
        (conditional(3, 4), OrdinaryV1),
        (conditional(3, 4), conditional(5, 4)),
        (conditional(3, 4), conditional(3, 5)),
        (conditional(3, 4), conditional(4, 3)),
    ] {
        let authority = BindingComparisonFixture {
            binding: retained,
            callbacks: Cell::new(0),
            stale: true,
        };
        assert!(matches!(
            check(&authority, prepared),
            Err(Gfx942AuthorizedRuntimeExecutionErrorV1::InvocationFamilyMismatch)
        ));
        assert_eq!(authority.callbacks.get(), 0);
    }
}

#[test]
fn matching_family_still_requires_current_publication() {
    for binding in [
        Gfx942RuntimeInvocationBindingV1::OrdinaryV1,
        conditional(3, 4),
    ] {
        for stale in [false, true] {
            let authority = BindingComparisonFixture {
                binding,
                callbacks: Cell::new(0),
                stale,
            };
            let result = check(&authority, binding);
            if stale {
                assert!(matches!(
                    result,
                    Err(
                        Gfx942AuthorizedRuntimeExecutionErrorV1::CurrentnessBeforeDispatch(
                            "stale publication"
                        )
                    )
                ));
            } else {
                assert!(result.is_ok());
            }
            assert_eq!(authority.callbacks.get(), 1);
        }
    }
}

#[test]
fn matching_conditional_coordinates_do_not_bypass_dispatch_or_device_binding() {
    let binding = conditional(3, 4);
    let authority = BindingComparisonFixture {
        binding,
        callbacks: Cell::new(0),
        stale: false,
    };
    assert!(matches!(
        validate_authority_bindings_v1(
            &authority,
            [1; 32],
            7_000,
            "kernel_v1",
            [9; 32],
            binding,
            0x1234,
        ),
        Err(Gfx942AuthorizedRuntimeExecutionErrorV1::DispatchContractMismatch)
    ));
    assert!(matches!(
        validate_authority_bindings_v1(
            &authority,
            [1; 32],
            7_000,
            "kernel_v1",
            [2; 32],
            binding,
            0x5678,
        ),
        Err(Gfx942AuthorizedRuntimeExecutionErrorV1::DeviceIdentityMismatch)
    ));
    assert_eq!(authority.callbacks.get(), 2);
}
