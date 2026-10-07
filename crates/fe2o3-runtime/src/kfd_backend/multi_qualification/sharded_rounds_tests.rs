//! Constructor dispatch and independent authority state, not GPU execution.

use super::super::*;
use crate::qualification_gfx942_sharded_vecadd_rounds_v1::*;
use crate::qualification_gfx942_vecadd_v1::GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1;
use crate::{RuntimeArgumentsV1, RuntimeContextV1};

fn with_request(
    recipe: Gfx942ShardedVecaddRoundsQualificationRecipeV1,
    run: impl FnOnce(KfdRuntimeAuthorityRequestV1<'_>),
) {
    let buffers = recipe.host_buffers().unwrap();
    let kernarg = recipe.explicit_kernarg();
    let policies = GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1;
    let bindings: [_; 3] = core::array::from_fn(|index| BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: 11 + index as u64,
            access: policies[index].access,
            byte_offset: 0,
            byte_len: recipe.padded_bytes() as u64,
        },
        kernarg_byte_offset: policies[index].pointer_offset,
    });
    let abi = policies.map(|policy| KfdRuntimeAuthorityGlobalBufferV1 {
        explicit_argument_index: policy.explicit_argument_index,
        name: policy.name,
        kernarg_byte_offset: u64::from(policy.pointer_offset),
        pointee_alignment: policy.reconciled_pointee_alignment,
        access: if policy.access == RuntimeAccessV1::Read {
            ArgumentAccess::ReadOnly
        } else {
            ArgumentAccess::WriteOnly
        },
    });
    let bytes = [buffers.a(), buffers.b(), buffers.c_initial()];
    let allocations: [_; 3] = core::array::from_fn(|index| KfdRuntimeAuthorityAllocationV1 {
        allocation: 11 + index as u64,
        kind: RuntimeMemoryKindV1::DeviceLocal,
        alignment: 4096,
        byte_offset: 0,
        bytes: bytes[index],
        content_sha256: Some(Sha256::digest(bytes[index]).into()),
    });
    let image =
        crate::qualification_gfx942_sharded_vecadd_v1::gfx942_sharded_vecadd_qualification_hsaco_v1(
        );
    run(KfdRuntimeAuthorityRequestV1 {
        module_image: image,
        module_sha256: Sha256::digest(image).into(),
        kernel_name: "vecadd",
        signature: GFX942_SHARDED_VECADD_ROUNDS_QUALIFICATION_SIGNATURE_V1,
        explicit_kernarg: &kernarg,
        complete_kernarg_template: &kernarg,
        bindings: &bindings,
        dispatch_abi: &abi,
        allocations: &allocations,
        geometry: recipe.geometry(),
        semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
    });
}

#[test]
fn two_round_constructor_rejects_entire_invalid_roster_before_native_open() {
    for ids in [
        vec![],
        vec![1],
        (1..=9).collect(),
        vec![0, 2],
        vec![1, 0],
        vec![1, 1],
        vec![1, 2, 3, 4, 5, 6, 7, 0],
        vec![1, 2, 3, 4, 5, 6, 7, 1],
    ] {
        let error = KfdMultiDeviceRuntimeBackendV1::open_gfx942_sharded_vecadd_rounds_peer_qualification_v1(&ids).unwrap_err();
        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
    }
}

#[test]
fn two_round_indexed_dispatch_preserves_order_and_independent_phase_consumption() {
    for count in 2..=8 {
        let ids: Vec<_> = (0..count)
            .map(|index| 100 + (count - index) as u64)
            .collect();
        let gates = super::admit_sharded_vecadd_rounds_devices_v1(&ids).unwrap();
        assert_eq!(gates.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), ids);
        let observations: Vec<_> = gates
            .iter()
            .map(|(_, gate)| {
                let KfdRuntimeLaunchGateV1::ExactGfx942ShardedVecaddRounds(admitted) = gate else {
                    panic!("indexed constructor must install the two-round gate");
                };
                admitted.observation_v1()
            })
            .collect();
        for round in 0..2 {
            for (index, (_, gate)) in gates.iter().enumerate() {
                let KfdRuntimeLaunchGateV1::ExactGfx942ShardedVecaddRounds(admitted) = gate else {
                    unreachable!()
                };
                let recipe = admitted.recipe(round).unwrap();
                assert_eq!(
                    (recipe.count(), recipe.index(), recipe.round()),
                    (count, index, round)
                );
                let sibling = Gfx942ShardedVecaddRoundsQualificationRecipeV1::new(
                    count,
                    (index + 1) % count,
                    round,
                )
                .unwrap();
                with_request(sibling, |request| {
                    assert!(!gate.authorize_launch_v1(request))
                });
                assert_eq!(observations[index].accepted_rounds_v1(), Some(round));
                with_request(recipe, |request| {
                    assert!(gate.authorize_launch_v1(request));
                    assert!(!gate.authorize_launch_v1(request));
                });
                for (child, observation) in observations.iter().enumerate() {
                    assert_eq!(
                        observation.accepted_rounds_v1(),
                        Some(round + usize::from(child <= index))
                    );
                    assert_eq!(
                        observation.authorization_calls_v1(),
                        3 * (round + usize::from(child <= index)) as u64
                    );
                }
            }
        }
    }
}

#[test]
fn two_round_typed_arguments_preserve_exact_encoding_with_distinct_policy_signature() {
    use crate::qualification_gfx942_sharded_vecadd_v1::Gfx942ShardedVecaddQualificationArgumentsV1 as OneShotArguments;
    let mut context = RuntimeContextV1::open(KfdRuntimeBackendV1::mock()).unwrap();
    let device = context.devices()[0].id();
    for round in 0..2 {
        let recipe = Gfx942ShardedVecaddRoundsQualificationRecipeV1::new(3, 1, round).unwrap();
        let allocations: [_; 3] = core::array::from_fn(|_| {
            context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    recipe.padded_bytes() as u64,
                    4096,
                )
                .unwrap()
        });
        let [a, b, c] = allocations;
        let arguments =
            Gfx942ShardedVecaddRoundsQualificationArgumentsV1::new(recipe, a, b, c).unwrap();
        let original = OneShotArguments::new(recipe, a, b, c).unwrap();
        assert_eq!(arguments.recipe(), recipe);
        assert_eq!(arguments.allocations(), allocations);
        assert_eq!(
            arguments.encode_explicit_kernarg_v1(),
            original.encode_explicit_kernarg_v1()
        );
        assert_eq!(arguments.bindings_v1(), original.bindings_v1());
        assert_ne!(
            Gfx942ShardedVecaddRoundsQualificationArgumentsV1::SIGNATURE_V1,
            OneShotArguments::SIGNATURE_V1
        );
        for ids in [[a, a, c], [a, b, a], [a, b, b]] {
            assert!(
                Gfx942ShardedVecaddRoundsQualificationArgumentsV1::new(
                    recipe, ids[0], ids[1], ids[2]
                )
                .is_err()
            );
        }
        for allocation in allocations {
            context.release_allocation(allocation).unwrap();
        }
    }
    let mut backend = context.shutdown().unwrap();
    backend.shutdown_native_v1().unwrap();
}
