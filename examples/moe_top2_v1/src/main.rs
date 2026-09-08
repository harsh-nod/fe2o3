use fe2o3_host::{
    AqlDispatchGeometryV1, GeneratedHostReadSliceV1, GeneratedHostReadWriteSliceV1,
    TutorialRuntimeLaunchIdentityV1, TutorialRuntimeSemanticRegionsV1,
};
use fe2o3_moe_top2_v1::kernel::moe_top2_route_f32_t8_e4_k2_c4_v1_gpu;

fe2o3_host::compiler_generated_kernel_expectation_roster_v1! {
    struct MoeTop2Roster = [moe_top2_route_f32_t8_e4_k2_c4_v1_gpu::Marker];
}

#[allow(clippy::too_many_arguments, dead_code)]
fn generated_arguments<'allocation>(
    logits: &'allocation [f32],
    top2_experts: &'allocation mut [u32],
    requested_counts: &'allocation mut [u32],
    admitted_counts: &'allocation mut [u32],
    expert_offsets: &'allocation mut [u32],
    route_slots: &'allocation mut [u32],
    permutation: &'allocation mut [u32],
    inverse: &'allocation mut [u32],
) -> moe_top2_route_f32_t8_e4_k2_c4_v1_gpu::Arguments<'allocation> {
    moe_top2_route_f32_t8_e4_k2_c4_v1_gpu::Arguments::new(
        GeneratedHostReadSliceV1::new(logits),
        GeneratedHostReadWriteSliceV1::new(top2_experts),
        GeneratedHostReadWriteSliceV1::new(requested_counts),
        GeneratedHostReadWriteSliceV1::new(admitted_counts),
        GeneratedHostReadWriteSliceV1::new(expert_offsets),
        GeneratedHostReadWriteSliceV1::new(route_slots),
        GeneratedHostReadWriteSliceV1::new(permutation),
        GeneratedHostReadWriteSliceV1::new(inverse),
    )
}

fn canaries(storages: &[&[u32]]) -> Vec<u32> {
    storages
        .iter()
        .flat_map(|storage| [storage[0], storage[storage.len() - 1]])
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--gfx942-qualification"))
        || arguments.next().is_some()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: fe2o3-moe-top2-v1 --gfx942-qualification",
        )
        .into());
    }
    fe2o3_host::run_generated_application_v1::<MoeTop2Roster, _>(|application| {
        use fe2o3_moe_top2_v1::{MOE_EXPERTS_V1, MOE_ROUTES_V1, RoutingOutputsV1};
        let vector = fe2o3_moe_top2_v1::deterministic_vectors_v1()[0];
        let logits = vector.logits;
        let logits_before = logits;
        let canary = 0xa5a5_5a5a_u32;
        let mut top2 = vec![canary; MOE_ROUTES_V1 + 2];
        let mut requested = vec![canary; MOE_EXPERTS_V1 + 2];
        let mut admitted = vec![canary; MOE_EXPERTS_V1 + 2];
        let mut offsets = vec![canary; MOE_EXPERTS_V1 + 3];
        let mut slots = vec![canary; MOE_ROUTES_V1 + 2];
        let mut permutation = vec![canary; MOE_ROUTES_V1 + 2];
        let mut inverse = vec![canary; MOE_ROUTES_V1 + 2];
        for storage in [
            &mut top2[1..=MOE_ROUTES_V1],
            &mut requested[1..=MOE_EXPERTS_V1],
            &mut admitted[1..=MOE_EXPERTS_V1],
            &mut offsets[1..=MOE_EXPERTS_V1 + 1],
            &mut slots[1..=MOE_ROUTES_V1],
            &mut permutation[1..=MOE_ROUTES_V1],
            &mut inverse[1..=MOE_ROUTES_V1],
        ] {
            storage.fill(u32::MAX);
        }
        let canaries_before = canaries(&[
            &top2,
            &requested,
            &admitted,
            &offsets,
            &slots,
            &permutation,
            &inverse,
        ]);

        let arguments = generated_arguments(
            &logits,
            &mut top2[1..=MOE_ROUTES_V1],
            &mut requested[1..=MOE_EXPERTS_V1],
            &mut admitted[1..=MOE_EXPERTS_V1],
            &mut offsets[1..=MOE_EXPERTS_V1 + 1],
            &mut slots[1..=MOE_ROUTES_V1],
            &mut permutation[1..=MOE_ROUTES_V1],
            &mut inverse[1..=MOE_ROUTES_V1],
        );
        let geometry = AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid generated MoE geometry: {error:?}"),
            )
        })?;
        let _dispatch = application
            .prepare_generated_application_invocation_v1(arguments, geometry, 0, 30_000)?
            .execute()?;

        let mut expected = RoutingOutputsV1::filled(u32::MAX);
        fe2o3_moe_top2_v1::moe_top2_oracle_v1(&logits_before, &mut expected)?;
        let canaries_after = canaries(&[
            &top2,
            &requested,
            &admitted,
            &offsets,
            &slots,
            &permutation,
            &inverse,
        ]);
        let observed = RoutingOutputsV1 {
            top2_experts: top2[1..=MOE_ROUTES_V1].try_into()?,
            requested_counts: requested[1..=MOE_EXPERTS_V1].try_into()?,
            admitted_counts: admitted[1..=MOE_EXPERTS_V1].try_into()?,
            expert_offsets: offsets[1..=MOE_EXPERTS_V1 + 1].try_into()?,
            route_slots: slots[1..=MOE_ROUTES_V1].try_into()?,
            permutation: permutation[1..=MOE_ROUTES_V1].try_into()?,
            inverse: inverse[1..=MOE_ROUTES_V1].try_into()?,
        };
        if logits != logits_before || observed != expected || canaries_after != canaries_before {
            return Err("MoE top-2 gfx942 semantic observation mismatch".into());
        }
        fe2o3_host::publish_tutorial_runtime_semantic_observation_v1(
            TutorialRuntimeLaunchIdentityV1 {
                target: "gfx942",
                kernel_symbols: &["moe_top2_route_f32_t8_e4_k2_c4_v1"],
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_lds_bytes: 0,
            },
            TutorialRuntimeSemanticRegionsV1 {
                inputs_before: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &logits_before,
                )],
                inputs_after: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(&logits)],
                canaries_before: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &canaries_before,
                )],
                canaries_after: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &canaries_after,
                )],
                padding_before: &[],
                padding_after: &[],
                expected_output: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected.top2_experts),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected.requested_counts),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected.admitted_counts),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected.expert_offsets),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected.route_slots),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected.permutation),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected.inverse),
                ],
                observed_output: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&observed.top2_experts),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&observed.requested_counts),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&observed.admitted_counts),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&observed.expert_offsets),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&observed.route_slots),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&observed.permutation),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&observed.inverse),
                ],
            },
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use fe2o3_moe_top2_v1::{
        MOE_EXPERTS_V1, MOE_LOGIT_ELEMENTS_V1, MOE_ROUTES_V1, RoutingOutputsV1,
    };

    #[test]
    fn generated_argument_adapter_retains_all_seven_output_borrows() {
        let logits = [0.0_f32; MOE_LOGIT_ELEMENTS_V1];
        let mut output = RoutingOutputsV1::filled(u32::MAX);
        let arguments = super::generated_arguments(
            &logits,
            &mut output.top2_experts,
            &mut output.requested_counts,
            &mut output.admitted_counts,
            &mut output.expert_offsets,
            &mut output.route_slots,
            &mut output.permutation,
            &mut output.inverse,
        );
        drop(arguments);
        output.requested_counts[0] = 1;
        assert_eq!(output.requested_counts, [1, u32::MAX, u32::MAX, u32::MAX]);
        assert_eq!(output.top2_experts.len(), MOE_ROUTES_V1);
        assert_eq!(output.expert_offsets.len(), MOE_EXPERTS_V1 + 1);
    }
}
