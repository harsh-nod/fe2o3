//! Real preparation and consuming KFD transitions, without authority or a GPU.
use super::*;
use fe2o3_kfd::{
    ConditionalDispatchDomainV1, ConditionalDispatchPremisesV1, ConditionalDispatchSliceV1,
};

fn geometry() -> AqlDispatchGeometryV1 {
    AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap()
}

fn output_inputs() -> Gfx942RuntimeDispatchInputsV1 {
    let mut result = inputs(geometry());
    result.buffers[0] =
        Gfx942RuntimeDispatchBufferV1::new(vec![0xa5; 8], Gfx942RuntimeBufferAccessV1::WriteOnly)
            .unwrap();
    result
}

fn premises(input: &Gfx942RuntimeDispatchInputsV1, contract: u8) -> ConditionalDispatchPremisesV1 {
    ConditionalDispatchPremisesV1::new(
        [contract; 32],
        [5; 32],
        [6; 32],
        &input.explicit_kernarg,
        input.geometry,
        &[ConditionalDispatchSliceV1 {
            generated_field: 0,
            pointer_offset: 0,
            length_offset: 8,
            buffer_index: Some(0),
            buffer_byte_offset: 0,
            length: 2,
            element_bytes: 4,
            alignment: 4,
        }],
        0,
        ConditionalDispatchDomainV1::GuardedOutput,
        &[],
    )
    .unwrap()
}

#[test]
fn actual_preparation_forwards_exact_payload_and_completion_policy() {
    let hsaco = module_with_resources(0, Some(false));
    let ordinary = prepare_gfx942_runtime_dispatch_v1(&hsaco, "vecadd", output_inputs()).unwrap();
    assert_eq!(
        ordinary.invocation_binding(),
        Gfx942RuntimeInvocationBindingV1::OrdinaryV1
    );
    let mut prepared_hash = None;
    for unchecked in [false, true] {
        let input = output_inputs();
        let payload = premises(&input, 4);
        let binding = Gfx942RuntimeInvocationBindingV1::ConditionalNominalV4 {
            contract_identity: *payload.contract_identity(),
            premise_identity: *payload.identity(),
        };
        let input = input.with_conditional_premises_v1(payload).unwrap();
        assert_eq!(input.invocation_binding(), binding);
        let prepared = prepare_gfx942_runtime_dispatch_v1(&hsaco, "vecadd", input).unwrap();
        assert_eq!(prepared.invocation_binding(), binding);
        assert_eq!(prepared.identity(), ordinary.identity());
        assert_eq!(prepared.kernel_name(), ordinary.kernel_name());
        assert_ne!(
            prepared.dispatch_contract_sha256(),
            ordinary.dispatch_contract_sha256()
        );
        let expected = crate::conditional_transport_v1::dispatch_identity(
            ordinary.dispatch_contract_sha256(),
            binding,
        );
        assert_eq!(prepared.dispatch_contract_sha256(), expected);
        assert_eq!(*prepared_hash.get_or_insert(expected), expected);
        let request = if unchecked {
            prepared.into_unchecked_kfd_request()
        } else {
            let (request, policies) = prepared.into_authorized_execution_parts();
            assert_eq!(policies.len(), 1);
            assert_eq!(policies[0].access, Gfx942RuntimeBufferAccessV1::WriteOnly);
            assert_eq!(policies[0].byte_length, 8);
            assert!(policies[0].read_only_initial_bytes.is_none());
            request
        };
        assert_eq!(
            crate::conditional_transport_v1::request_binding(&request),
            binding
        );
        let forwarded = request.conditional_premises_v1().unwrap();
        assert_eq!(forwarded.contract_identity(), &[4; 32]);
        assert!(matches!(binding,
            Gfx942RuntimeInvocationBindingV1::ConditionalNominalV4 { premise_identity, .. }
                if &premise_identity == forwarded.identity()
        ));
    }
    assert!(
        ordinary
            .into_unchecked_kfd_request()
            .conditional_premises_v1()
            .is_none()
    );
}

#[test]
fn attachment_cannot_replace_an_existing_conditional_payload() {
    let input = output_inputs();
    let first = premises(&input, 4);
    let replacement = premises(&input, 9);
    let bound = input.with_conditional_premises_v1(first).unwrap();
    assert!(matches!(
        bound.with_conditional_premises_v1(replacement),
        Err(Gfx942RuntimePreparationErrorV1::ConditionalPremisesAlreadyBound)
    ));
}

#[test]
fn actual_preparation_rejects_transport_request_substitution() {
    let hsaco = module_with_resources(0, Some(false));
    for mutation in 0..4 {
        let input = output_inputs();
        let payload = premises(&input, 4);
        let mut input = input.with_conditional_premises_v1(payload).unwrap();
        match mutation {
            0 => input.explicit_kernarg[8] = 1,
            1 => input.geometry = AqlDispatchGeometryV1::new([128, 1, 1], [64, 1, 1]).unwrap(),
            2 => input.pointer_fixups[0] = Gfx942KfdDispatchPointerFixupV1::new(0, 0, 4, 4),
            _ => {
                input.buffers[0] = Gfx942RuntimeDispatchBufferV1::new(
                    vec![0; 4],
                    Gfx942RuntimeBufferAccessV1::WriteOnly,
                )
                .unwrap()
            }
        }
        assert!(
            matches!(
                prepare_gfx942_runtime_dispatch_v1(&hsaco, "vecadd", input),
                Err(Gfx942RuntimePreparationErrorV1::KfdRequest(
                    Gfx942KfdDispatchRequestErrorV1::ConditionalPremise(_)
                ))
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn payload_substitution_changes_prepared_identity_without_minting_authority() {
    let hsaco = module_with_resources(0, Some(false));
    let mut hashes = Vec::new();
    for contract in [4, 9] {
        let input = output_inputs();
        let payload = premises(&input, contract);
        let input = input.with_conditional_premises_v1(payload).unwrap();
        let prepared = prepare_gfx942_runtime_dispatch_v1(&hsaco, "vecadd", input).unwrap();
        hashes.push(prepared.dispatch_contract_sha256());
    }
    assert_ne!(hashes[0], hashes[1]);
}
