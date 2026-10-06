use super::*;

fn token(rank: usize, slot: usize) -> Gfx950EngineeringPeerBufferV1 {
    Gfx950EngineeringPeerBufferV1 {
        group: 17,
        id: (rank * 32 + slot + 1) as u64,
        owner: rank,
        bytes: (slot as u64 + 1) * 64,
    }
}

#[test]
fn facade_inputs_preserve_every_role_and_kernel_reference() {
    let kernels: [[Gfx950EngineeringPeerKernelV1; 4]; 2] = std::array::from_fn(|rank| {
        std::array::from_fn(|slot| Gfx950EngineeringPeerKernelV1 {
            group: 17,
            rank,
            id: (rank * 4 + slot + 1) as u64,
            metadata: KernelMetadataV1 {
                symbol: format!("rank{rank}_role{slot}"),
                object_sha256: [rank as u8 * 4 + slot as u8; 32],
                kernarg_bytes: 0,
                kernarg_alignment: 8,
                group_segment_bytes: 0,
                private_segment_bytes: 0,
                wavefront_size: 64,
                implicit_argument_offset: None,
                implicit_argument_bytes: 0,
                explicit_arguments: vec![],
            },
        })
    });
    for exact_alias in [false, true] {
        let public = Gfx950EngineeringPeerGuardedMlpInputsV1 {
            ranks: std::array::from_fn(|rank| Gfx950EngineeringPeerGuardedMlpRankInputsV1 {
                kernels: kernels[rank].each_ref(),
                mlp_roots: std::array::from_fn(|slot| token(rank, slot)),
                residual_input: token(rank, 11),
                output: token(rank, if exact_alias { 11 } else { 12 }),
            }),
            partials: [token(0, 10), token(1, 10)],
            projection_sha256: std::array::from_fn(|i| i as u8),
            mlp_sha256: std::array::from_fn(|i| 255 - i as u8),
        };
        let private = public.private_inputs();
        assert_eq!(private.partials, public.partials);
        assert_eq!(private.projection_sha256, public.projection_sha256);
        assert_eq!(private.mlp_sha256, public.mlp_sha256);
        for rank in 0..2 {
            for slot in 0..4 {
                assert!(std::ptr::eq(
                    private.ranks[rank].kernels[slot],
                    &kernels[rank][slot]
                ));
            }
            assert_eq!(private.ranks[rank].mlp_roots, public.ranks[rank].mlp_roots);
            assert_eq!(
                private.ranks[rank].residual_input,
                public.ranks[rank].residual_input
            );
            assert_eq!(private.ranks[rank].output, public.ranks[rank].output);
            assert_eq!(
                private.ranks[rank].output == private.ranks[rank].residual_input,
                exact_alias
            );
        }
    }
}

#[test]
fn facade_observations_preserve_all_words_and_lagging_read_frontiers() {
    for frontiers in [[(5, 3), (40, 35)], [(u64::MAX, u64::MAX - 1), (0, 0)]] {
        let states: [CombinedMlpSnapshotV1; 2] =
            std::array::from_fn(|rank| CombinedMlpSnapshotV1 {
                prefix: std::array::from_fn(|i| ((rank as u32) << 24) | i as u32),
                guard: [rank as u32 + 1, 7, 1, 9],
            });
        let expected = states.clone();
        let observation = Gfx950EngineeringPeerGuardedMlpObservationV1::from(Completion {
            states,
            observed_queue_frontiers: frontiers,
            segment_host_ns: u64::MAX,
        });
        assert_eq!(
            observation.prefixes,
            expected.each_ref().map(|state| state.prefix)
        );
        assert_eq!(
            observation.guards,
            expected.each_ref().map(|state| state.guard)
        );
        assert_eq!(observation.observed_queue_frontiers, frontiers);
        assert_eq!(observation.segment_host_ns, u64::MAX);
    }
}
