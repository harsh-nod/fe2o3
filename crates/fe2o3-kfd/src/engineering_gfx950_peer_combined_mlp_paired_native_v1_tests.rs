//! Nonzero synthetic numerical fixture for the actual paired GPU coordinator.
use super::super::super::paired as coordinator;
use super::*;

const IMAGE_HASHES: [&str; 3] = [
    "25338beac39121cf81adfdc5c82c6f39bcfc82c0f7effaa2166bf8a2a9598a25",
    "b0d1766fdb0fda17ab09496d1120ba3cee29100692b48fcc5b05cf0db0a8d589",
    IMAGE_SHA,
];
const IMAGE_SIZES: [usize; 3] = [10864, 33320, IMAGE_BYTES];
const R1_SYMBOL: &str = "ferric_qwen3_tp_peer_tp2_projection_residual_bf16_v1";
const INNER: usize = 6144;
const CHUNK: usize = 4 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairedRequest {
    schema: String,
    device_unique_ids: [u64; 2],
    images: [PathBuf; 3],
    image_sha256: [String; 3],
    timeout_ms: u32,
}

fn paired_request(bytes: &[u8]) -> Result<PairedRequest> {
    if bytes.len() > 4096 {
        return Err("paired native request size".into());
    }
    let req: PairedRequest = serde_json::from_slice(bytes).map_err(explain)?;
    if req.schema != "ferric-native-paired-guarded-mlp-request-v1"
        || req.device_unique_ids.contains(&0)
        || req.device_unique_ids[0] == req.device_unique_ids[1]
        || req.images.iter().any(|p| !p.is_absolute())
        || req.images.iter().collect::<BTreeSet<_>>().len() != 3
        || req.image_sha256.each_ref().map(String::as_str) != IMAGE_HASHES
        || !(1..=5000).contains(&req.timeout_ms)
    {
        return Err("paired native closed request".into());
    }
    Ok(req)
}

fn sign(rank: usize, column: usize) -> f32 {
    if (column * 13 + column / 64 + rank * 7) % 5 < 2 {
        -1.0
    } else {
        1.0
    }
}
fn gate_value(rank: usize, row: usize) -> f32 {
    if (row + rank) % 2 == 0 { 8.0 } else { 16.0 }
}
fn up_value(rank: usize, row: usize) -> f32 {
    let magnitude = if (row / 3 + rank) % 2 == 0 {
        0.125
    } else {
        0.25
    };
    if (row * 5 + rank) % 3 == 0 {
        -magnitude
    } else {
        magnitude
    }
}
fn down_columns(rank: usize, row: usize) -> [usize; 3] {
    std::array::from_fn(|i| (row * 13 + rank * 37 + i * 2049) % INNER)
}
fn down_weights(rank: usize, row: usize) -> [f32; 3] {
    [if (row + rank) % 2 == 0 { 0.5 } else { -0.5 }, 0.125, -0.25]
}
fn down_value(rank: usize, row: usize) -> f32 {
    down_columns(rank, row)
        .into_iter()
        .zip(down_weights(rank, row))
        .map(|(column, weight)| gate_value(rank, column) * up_value(rank, column) * weight)
        .sum()
}
fn bf16_values(count: usize, value: impl Fn(usize) -> f32) -> Result<Vec<u8>> {
    (0..count)
        .map(|i| narrow(value(i)))
        .collect::<Result<Vec<_>>>()
        .map(|v| v.into_iter().flat_map(u16::to_le_bytes).collect())
}
fn f32_values(count: usize, value: impl Fn(usize) -> f32) -> Vec<u8> {
    (0..count).flat_map(|i| value(i).to_le_bytes()).collect()
}

// Sparse, nonzero rows in dense physical allocations give independently
// calculable results while exercising every output row and three distinct roles.
fn matrix(rank: usize, role: usize) -> Result<Vec<u8>> {
    if rank >= 2 || role >= 3 {
        return Err("paired fixture matrix role".into());
    }
    let mut bytes = vec![0; 50_331_648];
    let mut put = |row: usize, column: usize, width: usize, value: f32| -> Result<()> {
        let at = (row * width + column) * 2;
        bytes[at..at + 2].copy_from_slice(&narrow(value)?.to_le_bytes());
        Ok(())
    };
    if role < 2 {
        for row in 0..INNER {
            let column = if role == 0 {
                (row * 17 + rank * 29) % WIDTH
            } else {
                (row * 31 + rank * 43 + 11) % WIDTH
            };
            let target = if role == 0 {
                gate_value(rank, row)
            } else {
                up_value(rank, row)
            };
            put(row, column, WIDTH, target * sign(rank, column))?;
        }
    } else {
        for row in 0..WIDTH {
            for (column, value) in down_columns(rank, row)
                .into_iter()
                .zip(down_weights(rank, row))
            {
                put(row, column, INNER, value)?;
            }
        }
    }
    Ok(bytes)
}

fn original_partial(rank: usize) -> Vec<u8> {
    f32_values(WIDTH, |i| {
        let first = ((i % 7) as f32 - 3.0) * 0.125;
        if rank == 0 { first } else { 0.125 - first }
    })
}
fn original_residual(rank: usize) -> Result<Vec<u8>> {
    bf16_values(WIDTH, |i| sign(rank, i) - 0.125)
}
fn expected_root(rank: usize, index: usize) -> Result<Vec<u8>> {
    match index {
        0 | 5 => bf16_values(WIDTH, |i| sign(rank, i)),
        1 => bf16_values(WIDTH, |_| 1.0),
        2..=4 => matrix(rank, index - 2),
        6 => bf16_values(INNER, |i| gate_value(rank, i)),
        7 => bf16_values(INNER, |i| up_value(rank, i)),
        8 => bf16_values(INNER, |i| gate_value(rank, i) * up_value(rank, i)),
        9 => Ok(f32_values(WIDTH, |i| down_value(rank, i))),
        _ => Err("paired fixture root".into()),
    }
}
fn expected_final(rank: usize) -> Result<Vec<u8>> {
    let mut result = Vec::with_capacity(WIDTH * 2);
    for row in 0..WIDTH {
        result.extend(
            reference(
                down_value(0, row),
                down_value(1, row),
                narrow(sign(rank, row))?,
            )?
            .to_le_bytes(),
        );
    }
    Ok(result)
}
fn poison(index: usize) -> Vec<u8> {
    if index == 9 {
        (0..WIDTH)
            .flat_map(|_| 0x7fc00001u32.to_le_bytes())
            .collect()
    } else {
        (0..profile::EXTENTS[index] / 2)
            .flat_map(|_| 0x7fc1u16.to_le_bytes())
            .collect()
    }
}
fn write_all(
    group: &mut Gfx950EngineeringPeerGroupV1,
    buffer: Gfx950EngineeringPeerBufferV1,
    bytes: &[u8],
) -> Result<()> {
    if buffer.bytes != bytes.len() as u64 {
        return Err("paired fixture write extent".into());
    }
    for (index, chunk) in bytes.chunks(CHUNK).enumerate() {
        group.write(buffer, (index * CHUNK) as u64, chunk)?;
    }
    Ok(())
}
fn check_all(
    group: &mut Gfx950EngineeringPeerGroupV1,
    buffer: Gfx950EngineeringPeerBufferV1,
    bytes: &[u8],
) -> Result<String> {
    if buffer.bytes != bytes.len() as u64 {
        return Err("paired fixture read extent".into());
    }
    let mut actual = Sha256::new();
    for (index, expected) in bytes.chunks(CHUNK).enumerate() {
        let observed = group.read(buffer, (index * CHUNK) as u64, expected.len() as u32)?;
        if observed != expected {
            let at = observed.iter().zip(expected).position(|(a, b)| a != b);
            return Err(format!(
                "paired rank{} buffer{} chunk{index} mismatch {at:?}",
                buffer.owner, buffer.id
            ));
        }
        actual.update(&observed);
    }
    Ok(hex(&actual.finalize()))
}

fn paired_exercise(
    group: &mut Gfx950EngineeringPeerGroupV1,
    req: &PairedRequest,
    images: &[Vec<u8>; 3],
    hashes: [[u8; 32]; 3],
) -> Result<serde_json::Value> {
    let mut owners = [allocate(group, 0)?, allocate(group, 1)?];
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
            <[Gfx950EngineeringPeerBufferV1; 10]>::try_from(row)
                .map_err(|_| "paired root count")?,
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
    // SAFETY: the disposable diagnostic owns both idle queues and exact pinned
    // images. Every role has a distinct genuine allocation; all producers have
    // completed synchronous host writes. Inputs are finite with bounded values.
    // This call, not a serial stand-in, publishes both five-packet batches.
    let completed = unsafe { coordinator::dispatch(group, &mut owners, inputs, req.timeout_ms) }?;
    let mut records = Vec::new();
    for rank in 0..2 {
        require_activation(owners[rank].activation, Activation::Completed)?;
        require_terminal(&completed.states[rank], 1)?;
        if owners[rank].generation != 1 || owners[rank].observe(group)? != completed.states[rank] {
            return Err("paired actual owner generation/snapshot drift".into());
        }
        let mut stages = Vec::new();
        for index in 0..10 {
            let expected = expected_root(rank, index)?;
            let sha = check_all(group, roots[rank][index], &expected)?;
            let exported = !matches!(index, 2..=4);
            // Export bytes actually read from VRAM, never the expected fixture.
            let actual = if exported {
                Some(hex(&group.read(
                    roots[rank][index],
                    0,
                    expected.len() as u32,
                )?))
            } else {
                None
            };
            stages.push(serde_json::json!({"root":index,"bytes":expected.len(),"sha256":sha,"le_hex":actual}));
        }
        let final_sha = check_all(group, outputs[rank], &expected_final(rank)?)?;
        let partial_sha = check_all(group, partials[rank], &original_partial(rank))?;
        let residual_sha = check_all(group, residuals[rank], &original_residual(rank)?)?;
        records.push(serde_json::json!({"rank":rank,"stages":stages,
            "partial_le_hex":hex(&group.read(partials[rank],0,16384)?),"partial_sha256":partial_sha,
            "residual_le_hex":hex(&group.read(residuals[rank],0,8192)?),"residual_sha256":residual_sha,
            "output_le_hex":hex(&group.read(outputs[rank],0,8192)?),"output_sha256":final_sha,
            "prefix":completed.states[rank].prefix.to_vec(),"guard":completed.states[rank].guard}));
    }
    Ok(
        serde_json::json!({"records":records,"generation":1,"kernel_dispatches":8,"barrier_packets":2,
        "completion_signals":10,"paired_coordinator_tested":true,"owner_lifecycle_tested":true,
        "rearm_tested":false,"fixture":"nonzero-sparse-rows-v1","observed_queue_frontiers":completed.observed_queue_frontiers,
        "segment_host_ns":completed.segment_host_ns,"performance_claim":false,"canaries_tested":false}),
    )
}

fn paired_run_with(
    exercise: fn(
        &mut Gfx950EngineeringPeerGroupV1,
        &PairedRequest,
        &[Vec<u8>; 3],
        [[u8; 32]; 3],
    ) -> Result<serde_json::Value>,
    schema: &str,
) -> Result<serde_json::Value> {
    if std::env::var("FE2O3_ALLOW_UNAUTHENTICATED_MACHINE_CODE_V1")
        .ok()
        .as_deref()
        != Some("1")
    {
        return Err("paired native explicit engineering opt-in".into());
    }
    let path = PathBuf::from(
        std::env::var_os("FE2O3_NATIVE_PAIRED_MLP_REQUEST_V1").ok_or("paired request missing")?,
    );
    let bytes = read_regular(&path, 4096)?;
    if std::env::var("FE2O3_NATIVE_PAIRED_MLP_REQUEST_SHA256")
        .ok()
        .as_deref()
        != Some(digest(&bytes).as_str())
    {
        return Err("paired request identity".into());
    }
    let req = paired_request(&bytes)?;
    let mut images = Vec::new();
    for index in 0..3 {
        let image = read_regular(&req.images[index], IMAGE_SIZES[index])?;
        if image.len() != IMAGE_SIZES[index] || digest(&image) != IMAGE_HASHES[index] {
            return Err("paired image identity".into());
        }
        images.push(image);
    }
    let images: [Vec<u8>; 3] = images.try_into().map_err(|_| "paired image count")?;
    let hashes = images.each_ref().map(|v| Sha256::digest(v).into());
    // SAFETY: root-selected disposable process owns both selected devices.
    let mut group =
        unsafe { Gfx950EngineeringPeerGroupV1::open_unchecked(&req.device_unique_ids) }?;
    let result = exercise(&mut group, &req, &images, hashes);
    if group.poisoned {
        return Err(result
            .err()
            .unwrap_or_else(|| "paired poisoned group".into()));
    }
    let closed = group.close();
    let mut result = match (result, closed) {
        (Ok(value), Ok(())) => value,
        (Err(error), Ok(())) => return Err(error),
        (Ok(_), Err(error)) => return Err(format!("paired close failed: {error}")),
        (Err(error), Err(close)) => return Err(format!("{error}; paired close failed: {close}")),
    };
    if read_regular(&path, 4096)? != bytes {
        return Err("paired request changed".into());
    }
    for index in 0..3 {
        if read_regular(&req.images[index], IMAGE_SIZES[index])? != images[index] {
            return Err("paired image changed".into());
        }
    }
    result["schema"] = schema.into();
    result["request_sha256"] = digest(&bytes).into();
    result["image_sha256"] = serde_json::json!(IMAGE_HASHES);
    result["device_unique_ids"] = serde_json::json!(req.device_unique_ids);
    result["healthy_close"] = true.into();
    result["gpu_execution"] = true.into();
    result["full_model_acceptance"] = false.into();
    result["production_authority"] = false.into();
    Ok(result)
}

#[test]
#[ignore = "requires explicit two-device MI350 paired diagnostic and three checked images"]
fn native_paired_guarded_mlp_v1() {
    let result = paired_run_with(
        paired_exercise,
        "ferric-native-paired-guarded-mlp-observation-v1",
    )
    .expect("native paired guarded MLP failed");
    println!("FERRIC_NATIVE_PAIRED_MLP_V1={result}");
}

#[path = "engineering_gfx950_peer_combined_mlp_paired_reuse_native_v1_tests.rs"]
mod reuse;

#[path = "engineering_gfx950_peer_combined_mlp_paired_interleaved_native_v1_tests.rs"]
mod interleaved;

#[test]
fn paired_native_fixture_matches_independent_staged_arithmetic() {
    let normalized = (1.0f64 / (1.0f64 + 1e-6f64).sqrt()) as f32;
    assert_eq!(narrow(normalized).unwrap(), 0x3f80);
    for rank in 0..2 {
        for row in 0..INNER {
            let gate = gate_value(rank, row) as f64;
            let silu = narrow((gate / (1.0 + (-gate).exp())) as f32).unwrap();
            let materialized = f32::from_bits(u32::from(silu) << 16) * up_value(rank, row);
            assert_eq!(
                narrow(materialized).unwrap(),
                narrow(gate_value(rank, row) * up_value(rank, row)).unwrap()
            );
            assert!(down_value(rank, row % WIDTH).is_finite());
        }
        assert_eq!(expected_final(rank).unwrap().len(), 8192);
        for index in [0, 1, 5, 6, 7, 8, 9] {
            assert_eq!(
                expected_root(rank, index).unwrap().len(),
                profile::EXTENTS[index]
            );
        }
        let a = original_partial(0);
        let b = original_partial(1);
        let residual = original_residual(rank).unwrap();
        for i in 0..WIDTH {
            let p = |v: &[u8]| f32::from_le_bytes(v[i * 4..i * 4 + 4].try_into().unwrap());
            let r = u16::from_le_bytes(residual[i * 2..i * 2 + 2].try_into().unwrap());
            assert_eq!(
                reference(p(&a), p(&b), r).unwrap(),
                narrow(sign(rank, i)).unwrap()
            );
        }
    }
}

#[test]
fn paired_native_matrix_roles_are_distinct_with_every_row_nonzero() {
    let mut hashes = BTreeSet::new();
    for rank in 0..2 {
        for role in 0..3 {
            let bytes = matrix(rank, role).unwrap();
            assert_eq!(bytes.len(), 50_331_648);
            assert!(hashes.insert(digest(&bytes)));
            let width = if role == 2 { INNER } else { WIDTH };
            for row in bytes.chunks_exact(width * 2) {
                let nonzero = row
                    .chunks_exact(2)
                    .filter(|b| b[0] != 0 || b[1] != 0)
                    .count();
                assert_eq!(nonzero, if role == 2 { 3 } else { 1 });
            }
        }
    }
    assert!(matrix(2, 0).is_err());
    assert!(matrix(0, 3).is_err());
}

#[test]
fn paired_native_request_refuses_wrong_images_devices_and_unknown_fields() {
    let base = serde_json::json!({"schema":"ferric-native-paired-guarded-mlp-request-v1",
        "device_unique_ids":[11,22],"images":["/tmp/r1","/tmp/mlp","/tmp/guard"],
        "image_sha256":IMAGE_HASHES,"timeout_ms":5000});
    assert!(paired_request(&serde_json::to_vec(&base).unwrap()).is_ok());
    for (field, value) in [
        ("device_unique_ids", serde_json::json!([11, 11])),
        ("device_unique_ids", serde_json::json!([0, 22])),
        (
            "images",
            serde_json::json!(["relative", "/tmp/mlp", "/tmp/guard"]),
        ),
        (
            "images",
            serde_json::json!(["/tmp/r1", "/tmp/r1", "/tmp/guard"]),
        ),
        (
            "image_sha256",
            serde_json::json!([IMAGE_HASHES[1], IMAGE_HASHES[0], IMAGE_HASHES[2]]),
        ),
        ("timeout_ms", serde_json::json!(0)),
        ("timeout_ms", serde_json::json!(5001)),
        ("unreviewed", serde_json::json!(true)),
    ] {
        let mut changed = base.clone();
        changed[field] = value;
        assert!(paired_request(&serde_json::to_vec(&changed).unwrap()).is_err());
    }
}
