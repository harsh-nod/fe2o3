//! Actual consecutive generations: same owners/payloads, freshly staged batches.
use super::*;

fn scaled_root(rank: usize, index: usize, generation: u64) -> Result<Vec<u8>> {
    if !matches!(generation, 1 | 2) {
        return Err("reuse fixture generation".into());
    }
    let mut bytes = expected_root(rank, index)?;
    if generation == 2 {
        match index {
            3 => {
                // Only Up weights change. Keep identical sparsity and the exact
                // dense allocation extent; every output row has one nonzero.
                for row in 0..INNER {
                    let col = (31 * row + 43 * rank + 11) % WIDTH;
                    let at = (row * WIDTH + col) * 2;
                    // The normalized input is sign(rank,col), so the matrix
                    // coefficient must include its sign as in matrix(role=1).
                    bytes[at..at + 2].copy_from_slice(
                        &narrow(2.0 * up_value(rank, row) * sign(rank, col))?.to_le_bytes(),
                    );
                }
            }
            7 | 8 => {
                for word in bytes.chunks_exact_mut(2) {
                    let value = f32::from_bits(
                        u32::from(u16::from_le_bytes((&*word).try_into().unwrap())) << 16,
                    );
                    word.copy_from_slice(&narrow(2.0 * value)?.to_le_bytes());
                }
            }
            9 => {
                for word in bytes.chunks_exact_mut(4) {
                    let value = f32::from_le_bytes((&*word).try_into().unwrap());
                    word.copy_from_slice(&(2.0 * value).to_le_bytes());
                }
            }
            _ => {}
        }
    }
    Ok(bytes)
}

fn scaled_final(rank: usize, generation: u64) -> Result<Vec<u8>> {
    if !matches!(generation, 1 | 2) {
        return Err("reuse final generation".into());
    }
    (0..WIDTH)
        .map(|row| {
            reference(
                generation as f32 * down_value(0, row),
                generation as f32 * down_value(1, row),
                narrow(sign(rank, row))?,
            )
        })
        .collect::<Result<Vec<_>>>()
        .map(|words| words.into_iter().flat_map(u16::to_le_bytes).collect())
}

fn session_write(
    session: &mut coordinator::Session<'_, '_>,
    buffer: Gfx950EngineeringPeerBufferV1,
    bytes: &[u8],
) -> Result<()> {
    if buffer.bytes != bytes.len() as u64 {
        return Err("reuse write extent".into());
    }
    for (index, chunk) in bytes.chunks(CHUNK).enumerate() {
        session.write(buffer, (index * CHUNK) as u64, chunk)?;
    }
    Ok(())
}

fn session_check(
    session: &mut coordinator::Session<'_, '_>,
    buffer: Gfx950EngineeringPeerBufferV1,
    bytes: &[u8],
) -> Result<String> {
    if buffer.bytes != bytes.len() as u64 {
        return Err("reuse read extent".into());
    }
    let mut actual = Sha256::new();
    for (index, expected) in bytes.chunks(CHUNK).enumerate() {
        let observed = session.read(buffer, (index * CHUNK) as u64, expected.len() as u32)?;
        if observed != expected {
            return Err(format!(
                "reuse rank{} buffer{} chunk{index} mismatch",
                buffer.owner, buffer.id
            ));
        }
        actual.update(&observed);
    }
    Ok(hex(&actual.finalize()))
}

fn reuse_exercise(
    group: &mut Gfx950EngineeringPeerGroupV1,
    req: &PairedRequest,
    images: &[Vec<u8>; 3],
    hashes: [[u8; 32]; 3],
) -> Result<serde_json::Value> {
    let mut owners = [allocate(group, 0)?, allocate(group, 1)?];
    let owner_tokens = [owners[0].buffer, owners[1].buffer];
    let mut kernels = Vec::new();
    for rank in 0..2 {
        let mut row = Vec::new();
        for (image, symbol) in [
            (0, R1_SYMBOL),
            (1, profile::SYMBOL),
            (2, GUARD_SYMBOL),
            (2, R2_SYMBOL),
        ] {
            row.push(group.load_kernel(
                rank,
                images[image].clone(),
                hashes[image],
                symbol.into(),
            )?);
        }
        kernels.push(row);
    }
    let mut roots = Vec::new();
    for rank in 0..2 {
        let mut row = Vec::new();
        for index in 0..10 {
            let peers = if index == 9 { vec![1 - rank] } else { vec![] };
            let buffer = group.allocate(rank, &peers, profile::EXTENTS[index] as u64)?;
            let bytes = if (1..=4).contains(&index) {
                expected_root(rank, index)?
            } else {
                poison(index)
            };
            write_all(group, buffer, &bytes)?;
            row.push(buffer);
        }
        roots.push(
            <[Gfx950EngineeringPeerBufferV1; 10]>::try_from(row).map_err(|_| "reuse root count")?,
        );
    }
    let partials = [
        group.allocate(0, &[1], 16384)?,
        group.allocate(1, &[0], 16384)?,
    ];
    let residuals = [group.allocate(0, &[], 8192)?, group.allocate(1, &[], 8192)?];
    let outputs = [group.allocate(0, &[], 8192)?, group.allocate(1, &[], 8192)?];
    for rank in 0..2 {
        write_all(group, partials[rank], &original_partial(rank))?;
        write_all(group, residuals[rank], &original_residual(rank)?)?;
        write_all(group, outputs[rank], &poison(0))?;
    }
    let inputs = coordinator::Inputs {
        ranks: std::array::from_fn(|rank| coordinator::RankInputs {
            kernels: [
                &kernels[rank][0],
                &kernels[rank][1],
                &kernels[rank][2],
                &kernels[rank][3],
            ],
            mlp_roots: roots[rank],
            residual_input: residuals[rank],
            output: outputs[rank],
        }),
        partials,
        projection_sha256: hashes[0],
        mlp_sha256: hashes[1],
    };
    // SAFETY: exact reviewed images, distinct genuine payload allocations,
    // bounded finite inputs and completed coherent host writes in a disposable
    // process. The same obligations hold for the doubled-Up generation below.
    let mut session = unsafe { coordinator::Session::new(group, &mut owners, inputs) };
    let mut generations = Vec::new();
    let mut last_states = None;
    for generation in 1..=2 {
        if generation == 2 {
            if session.rearm_next(req.timeout_ms)? != 2 {
                return Err("reuse consecutive generation".into());
            }
            for rank in 0..2 {
                session_write(&mut session, roots[rank][3], &scaled_root(rank, 3, 2)?)?;
                for index in [0, 5, 6, 7, 8, 9] {
                    session_write(&mut session, roots[rank][index], &poison(index))?;
                }
                session_write(&mut session, outputs[rank], &poison(0))?;
            }
        }
        let completed = session.run(req.timeout_ms)?;
        let mut records = Vec::new();
        for rank in 0..2 {
            require_terminal(&completed.states[rank], generation)?;
            let mut stages = Vec::new();
            for index in 0..10 {
                let expected = scaled_root(rank, index, generation)?;
                let sha = session_check(&mut session, roots[rank][index], &expected)?;
                let actual = if matches!(index, 2..=4) {
                    None
                } else {
                    Some(hex(&session.read(
                        roots[rank][index],
                        0,
                        expected.len() as u32,
                    )?))
                };
                stages.push(serde_json::json!({"root":index,"bytes":expected.len(),"sha256":sha,"le_hex":actual}));
            }
            let final_sha = session_check(
                &mut session,
                outputs[rank],
                &scaled_final(rank, generation)?,
            )?;
            let partial_sha = session_check(&mut session, partials[rank], &original_partial(rank))?;
            let residual_sha =
                session_check(&mut session, residuals[rank], &original_residual(rank)?)?;
            records.push(serde_json::json!({"rank":rank,"stages":stages,
                "partial_le_hex":hex(&session.read(partials[rank],0,16384)?),"partial_sha256":partial_sha,
                "residual_le_hex":hex(&session.read(residuals[rank],0,8192)?),"residual_sha256":residual_sha,
                "output_le_hex":hex(&session.read(outputs[rank],0,8192)?),"output_sha256":final_sha,
                "prefix":completed.states[rank].prefix.to_vec(),"guard":completed.states[rank].guard}));
        }
        generations.push(serde_json::json!({"records":records,"generation":generation,
            "observed_queue_frontiers":completed.observed_queue_frontiers,"segment_host_ns":completed.segment_host_ns}));
        last_states = Some(completed.states);
    }
    drop(session);
    let last = last_states.ok_or("reuse terminal missing")?;
    for rank in 0..2 {
        require_activation(owners[rank].activation, Activation::Completed)?;
        if owners[rank].generation != 2
            || owners[rank].buffer != owner_tokens[rank]
            || owners[rank].observe(group)? != last[rank]
        {
            return Err("reuse owner identity/generation/snapshot drift".into());
        }
    }
    Ok(
        serde_json::json!({"generations":generations,"kernel_dispatches":16,"barrier_packets":4,
        "completion_signals":20,"paired_coordinator_tested":true,"owner_lifecycle_tested":true,
        "rearm_tested":true,"owners_reused":true,"payloads_reused":true,"fresh_arenas_per_generation":true,
        "fixture":"nonzero-sparse-rows-double-up-v1","performance_claim":false,"canaries_tested":false}),
    )
}

#[test]
#[ignore = "requires explicit two-device MI350 consecutive-generation diagnostic and three checked images"]
fn native_paired_guarded_mlp_reuse_v1() {
    let result = paired_run_with(
        reuse_exercise,
        "ferric-native-paired-guarded-mlp-reuse-observation-v1",
    )
    .expect("native paired guarded MLP reuse failed");
    println!("FERRIC_NATIVE_PAIRED_REUSE_MLP_V1={result}");
}

#[test]
fn reuse_fixture_changes_every_final_word_and_preserves_exact_positive_zeros() {
    let hashes = [
        "403903da99ade382e932312fb9fcdaa5d5fe3e41bf05768f614667fd77b160bf",
        "e7164a702f121bbaa1b9e940960177f2b7670bf279f641e6e827be6bdfba3e93",
    ];
    for rank in 0..2 {
        let first = scaled_final(rank, 1).unwrap();
        let second = scaled_final(rank, 2).unwrap();
        assert_eq!(first, expected_final(rank).unwrap());
        assert_eq!(digest(&second), hashes[rank]);
        assert!(
            first
                .chunks_exact(2)
                .zip(second.chunks_exact(2))
                .all(|(a, b)| a != b)
        );
        assert_eq!(
            second.chunks_exact(2).filter(|b| *b == [0, 0]).count(),
            [820, 819][rank]
        );
        for index in [7, 8, 9] {
            let a = scaled_root(rank, index, 1).unwrap();
            let b = scaled_root(rank, index, 2).unwrap();
            let width = if index == 9 { 4 } else { 2 };
            assert!(
                a.chunks_exact(width)
                    .zip(b.chunks_exact(width))
                    .all(|(x, y)| x != y)
            );
        }
    }
    assert!(scaled_root(0, 7, 0).is_err());
    assert!(scaled_final(0, 3).is_err());
}

#[test]
fn reuse_fixture_doubles_only_up_matrix_coefficients() {
    let hashes = [
        "4d8d301047aab087afdf5c54302051fd4848fac603ac85605688d3b1e1a47da5",
        "1c1d077694c7f6a826ff26c6a770ee9887cb9c376d6f65193545df3180583dd9",
    ];
    for rank in 0..2 {
        assert_eq!(digest(&scaled_root(rank, 3, 2).unwrap()), hashes[rank]);
        for index in [0, 1, 2, 4, 5, 6] {
            assert_eq!(
                scaled_root(rank, index, 2).unwrap(),
                expected_root(rank, index).unwrap()
            );
        }
    }
}
