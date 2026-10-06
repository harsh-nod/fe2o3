//! Serial component diagnostic. This does not exercise the paired coordinator.
use super::*;
use crate::engineering_wire::KernelMetadataV1;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

#[path = "engineering_gfx950_peer_combined_mlp_paired_native_v1_tests.rs"]
mod paired;

const IMAGE_SHA: &str = "de880dcebf79425ccb555c1a2bdca3ff78b926f7b2062d06b918763da4de0f66";
const IMAGE_BYTES: usize = 28_440;
const GUARD_SYMBOL: &str = "ferric_qwen3_mlp_state_guard_v2";
const R2_SYMBOL: &str = "ferric_qwen3_tp2_guarded_projection_residual_bf16_v2";
const WIDTH: usize = 4096;
const CANARY: usize = 64;
const GENERATIONS: [u64; 2] = [1, 0x1_0000_0003];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    device_unique_ids: [u64; 2],
    image: PathBuf,
    image_sha256: String,
    timeout_ms: u32,
}

fn request(bytes: &[u8]) -> Result<Request> {
    if bytes.len() > 4096 {
        return Err("native guarded R2 request too large".into());
    }
    let value: Request = serde_json::from_slice(bytes).map_err(explain)?;
    if value.schema != "ferric-native-stable-guarded-r2-request-v1"
        || value.device_unique_ids.contains(&0)
        || value.device_unique_ids[0] == value.device_unique_ids[1]
        || !value.image.is_absolute()
        || value.image_sha256 != IMAGE_SHA
        || !(1..=5_000).contains(&value.timeout_ms)
    {
        return Err("native guarded R2 request contract".into());
    }
    Ok(value)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn read_regular(path: &Path, cap: usize) -> Result<Vec<u8>> {
    if !path.is_absolute() || path.canonicalize().map_err(explain)? != path {
        return Err("native input must have canonical absolute ancestry".into());
    }
    let path_before = std::fs::symlink_metadata(path).map_err(explain)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(explain)?;
    let before = file.metadata().map_err(explain)?;
    let stamp = |m: &std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.mode(),
            m.nlink(),
            m.uid(),
            m.gid(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if !before.is_file() || before.len() > cap as u64 || stamp(&path_before) != stamp(&before) {
        return Err("native input must be a bounded regular file".into());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(explain)?;
    if bytes.len() > cap
        || bytes.len() as u64 != before.len()
        || stamp(&file.metadata().map_err(explain)?) != stamp(&before)
        || stamp(&std::fs::symlink_metadata(path).map_err(explain)?) != stamp(&before)
        || path.canonicalize().map_err(explain)? != path
    {
        return Err("native input size changed".into());
    }
    Ok(bytes)
}

fn metadata(value: &KernelMetadataV1, hash: [u8; 32], validator: bool) -> Result<()> {
    let slices = if validator { 1 } else { 6 };
    let explicit = if validator { 24 } else { 104 };
    if value.symbol != if validator { GUARD_SYMBOL } else { R2_SYMBOL }
        || value.object_sha256 != hash
        || value.kernarg_bytes != explicit + 256
        || value.kernarg_alignment != 8
        || value.group_segment_bytes != 0
        || value.private_segment_bytes != 0
        || value.wavefront_size != 64
        || value.implicit_argument_offset != Some(explicit)
        || value.implicit_argument_bytes != 256
        || value.explicit_arguments.len() != slices * 2 + 2
    {
        return Err("native guarded image metadata mismatch".into());
    }
    for (i, argument) in value.explicit_arguments.iter().enumerate() {
        let (offset, bytes, pointer) = if i < slices * 2 {
            ((i * 8) as u32, 8, i % 2 == 0)
        } else {
            ((slices * 16 + (i - slices * 2) * 4) as u32, 4, false)
        };
        if (argument.offset, argument.bytes, argument.global_buffer) != (offset, bytes, pointer)
            || argument.access.is_some()
            || argument.pointee_alignment.is_some()
        {
            return Err("native guarded image exact explicit argument roster".into());
        }
    }
    Ok(())
}

fn arguments(generation: u64, validator: bool) -> Vec<u8> {
    let mut bytes = vec![0; if validator { 280 } else { 360 }];
    let lengths: &[u64] = if validator {
        &[552]
    } else {
        &[4096, 4096, 4096, 4096, 4, 4]
    };
    for (i, length) in lengths.iter().enumerate() {
        bytes[i * 16 + 8..i * 16 + 16].copy_from_slice(&length.to_le_bytes());
    }
    let at = lengths.len() * 16;
    bytes[at..at + 4].copy_from_slice(&(generation as u32).to_le_bytes());
    bytes[at + 4..at + 8].copy_from_slice(&((generation >> 32) as u32).to_le_bytes());
    bytes
}

// Independent host arithmetic, not the device macro or its Bf16 conversion.
// A separate integer-only reference also consumes the emitted input/output bits.
fn narrow(value: f32) -> Result<u16> {
    if !value.is_finite() {
        return Err("nonfinite reference input".into());
    }
    let bits = value.to_bits();
    let high = bits >> 16;
    let low = bits & 0xffff;
    let result = (high + u32::from(low > 0x8000 || (low == 0x8000 && high & 1 != 0))) as u16;
    if result & 0x7f80 == 0x7f80 {
        return Err("reference BF16 overflow".into());
    }
    Ok(result)
}

fn reference(p0: f32, p1: f32, residual: u16) -> Result<u16> {
    let first = (0.0f64 + f64::from(p0)) as f32;
    let sum = (f64::from(first) + f64::from(p1)) as f32;
    let projection = f32::from_bits(u32::from(narrow(sum)?) << 16);
    let residual = f32::from_bits(u32::from(residual) << 16);
    if !p0.is_finite() || !p1.is_finite() || !first.is_finite() || !residual.is_finite() {
        return Err("nonfinite reference operand".into());
    }
    narrow((f64::from(projection) + f64::from(residual)) as f32)
}

const ANCHORS: [(u32, u32, u16, u16); 32] = [
    (0, 0, 0, 0),
    (0x80000000, 0x80000000, 0x8000, 0),
    (0x3f800000, 0xbf800000, 0x8000, 0),
    (0x4b800000, 0xcb800000, 0x3f80, 0x3f80),
    (0x3f800000, 0x3b800000, 0xbf80, 0),
    (0x3f800000, 0x3c400000, 0xbf80, 0x3c80),
    (0xbf800000, 0xbb800000, 0x3f80, 0),
    (0xbf800000, 0xbc400000, 0x3f80, 0xbc80),
    (0x3f804000, 0xbf800000, 0, 0x3b00),
    (0x3b800000, 0x3b800000, 0x3f80, 0x3f81),
    (0x4b800000, 0x3f800000, 0xcb80, 0),
    (0x3f800000, 0, 0x3b80, 0x3f80),
    (0x3f810000, 0, 0x3b80, 0x3f82),
    (0xbf800000, 0, 0xbb80, 0xbf80),
    (0xbf810000, 0, 0xbb80, 0xbf82),
    (0x00800000, 0x80800000, 1, 1),
    (0x00008000, 0, 0, 0),
    (0x00008001, 0, 0, 1),
    (0x00018000, 0, 0, 2),
    (0x80008001, 0, 0, 0x8001),
    (0x80008000, 0, 0x8000, 0x8000),
    (0x7f7f0000, 0, 0, 0x7f7f),
    (0xff7f0000, 0, 0, 0xff7f),
    (0x7f7f0000, 0xff7f0000, 0x3f81, 0x3f81),
    (0x007f8000, 0, 0, 0x0080),
    (0x807f8000, 0, 0, 0x8080),
    (0x3f807fff, 0, 0, 0x3f80),
    (0x3f808001, 0, 0, 0x3f81),
    (0xbf807fff, 0, 0, 0xbf80),
    (0xbf808001, 0, 0, 0xbf81),
    (0x3f817fff, 0, 0, 0x3f81),
    (0x3f818001, 0, 0, 0x3f82),
];

fn corpus(generation: usize) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> {
    let mut p0 = Vec::new();
    let mut p1 = Vec::new();
    let mut residual = Vec::new();
    let mut expected = Vec::new();
    for i in 0..WIDTH {
        let (a, b, r) = if i % 64 < ANCHORS.len() {
            let (a, b, r, _) = ANCHORS[(i % 64 + 7 * (i / 64) + 11 * generation) % ANCHORS.len()];
            (f32::from_bits(a), f32::from_bits(b), r)
        } else {
            (
                ((17 * i + 13 * generation) % 257) as f32 / 32.0 - 4.0,
                ((29 * i + 7 * generation) % 251) as f32 / 64.0 - 125.0 / 64.0,
                [0, 0x8000, 0x3f80, 0xbf80, 0x3f81, 0x0080][i % 6],
            )
        };
        p0.extend_from_slice(&a.to_le_bytes());
        p1.extend_from_slice(&b.to_le_bytes());
        residual.extend_from_slice(&r.to_le_bytes());
        expected.extend_from_slice(&reference(a, b, r)?.to_le_bytes());
    }
    Ok((p0, p1, residual, expected))
}

fn guarded(payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; payload.len() + CANARY * 2];
    for i in 0..CANARY {
        bytes[i] = 0xa5u8.wrapping_add((29 * i) as u8);
        bytes[CANARY + payload.len() + i] = 0x5au8.wrapping_add((17 * i) as u8);
    }
    bytes[CANARY..CANARY + payload.len()].copy_from_slice(payload);
    bytes
}

fn seed(
    group: &mut Gfx950EngineeringPeerGroupV1,
    owner: &CombinedMlpStateV1,
    snapshot: &CombinedMlpSnapshotV1,
) -> Result<()> {
    let result = (|| {
        check_contexts(&mut group.contexts, group.shared_full_currentness)?;
        let id = owner.local_id(group)?;
        require_activation(owner.activation, Activation::Ready)?;
        let allocation = group.contexts[owner.owner_rank()]
            .buffers
            .get_mut(&id)
            .ok_or("diagnostic combined allocation missing")?;
        // SAFETY: this test is serial, with no published work at seeding time.
        // These seeded bytes are deliberately not a valid submit()/rearm proof.
        unsafe {
            Backend::seed_combined_snapshot_quiescent_v1(
                &mut allocation.mapping,
                allocation.requested,
                &snapshot.prefix,
                snapshot.guard,
            )
        }
        .map_err(explain)?;
        std::sync::atomic::fence(Ordering::SeqCst);
        if owner.observe(group)? != *snapshot {
            return Err("diagnostic seed atomic readback mismatch".into());
        }
        check_contexts(&mut group.contexts, group.shared_full_currentness)
    })();
    group.finish(result)
}

#[derive(Clone)]
struct Case {
    name: String,
    state: [CombinedMlpSnapshotV1; 2],
    validators: bool,
    valid: bool,
}

fn cases(generation: u64) -> Vec<Case> {
    let valid = terminal(generation);
    let mut cases = vec![Case {
        name: "valid".into(),
        state: [valid.clone(), valid.clone()],
        validators: true,
        valid: true,
    }];
    for rank in 0..2 {
        for (name, word, value) in [
            ("stale-low", 0, (generation as u32) ^ 1),
            ("stale-high", 1, ((generation >> 32) as u32) ^ 1),
            ("pending", 2, 0),
            ("failed", 2, 2),
            ("reserved", 3, 1),
        ] {
            let mut state = [valid.clone(), valid.clone()];
            state[rank].guard[word] = value;
            cases.push(Case {
                name: format!("rank{rank}-{name}"),
                state,
                validators: false,
                valid: false,
            });
        }
        for index in [0, 3, 4, 8, 14, 22, 31, 32, 547] {
            let mut state = [valid.clone(), valid.clone()];
            state[rank].prefix[index] ^= 1;
            cases.push(Case {
                name: format!("rank{rank}-prefix-{index}"),
                state,
                validators: true,
                valid: false,
            });
        }
    }
    cases
}

fn exercise(
    group: &mut Gfx950EngineeringPeerGroupV1,
    req: &Request,
    object: &[u8],
    hash: [u8; 32],
) -> Result<serde_json::Value> {
    let owners = [allocate(group, 0)?, allocate(group, 1)?];
    let mut kernels = Vec::new();
    for rank in 0..2 {
        let guard = group.load_kernel(rank, object.to_vec(), hash, GUARD_SYMBOL.into())?;
        let r2 = group.load_kernel(rank, object.to_vec(), hash, R2_SYMBOL.into())?;
        if guard.rank() != rank || r2.rank() != rank {
            return Err("native kernel rank mismatch".into());
        }
        metadata(guard.metadata(), hash, true)?;
        metadata(r2.metadata(), hash, false)?;
        kernels.push((guard, r2));
    }
    let p0 = group.allocate(0, &[1], (WIDTH * 4 + CANARY * 2) as u64)?;
    let p1 = group.allocate(1, &[0], (WIDTH * 4 + CANARY * 2) as u64)?;
    let residual = [
        group.allocate(0, &[], (WIDTH * 2 + CANARY * 2) as u64)?,
        group.allocate(1, &[], (WIDTH * 2 + CANARY * 2) as u64)?,
    ];
    let output = [
        group.allocate(0, &[], (WIDTH * 2 + CANARY * 2) as u64)?,
        group.allocate(1, &[], (WIDTH * 2 + CANARY * 2) as u64)?,
    ];
    let poison_payload: Vec<u8> = (0..WIDTH)
        .flat_map(|i| (0x7fc0u16 | (1 + i % 63) as u16).to_le_bytes())
        .collect();
    let poison = guarded(&poison_payload);
    let mut records = Vec::new();
    let mut dispatches = 0;
    for (corpus_id, generation) in GENERATIONS.into_iter().enumerate() {
        let (a, b, r, expected) = corpus(corpus_id)?;
        group.write(p0, 0, &guarded(&a))?;
        group.write(p1, 0, &guarded(&b))?;
        for buffer in residual {
            group.write(buffer, 0, &guarded(&r))?;
        }
        for mut case in cases(generation) {
            for rank in 0..2 {
                if case.validators {
                    case.state[rank].guard = guard_words(generation, 0);
                }
                seed(group, &owners[rank], &case.state[rank])?;
                group.write(output[rank], 0, &poison)?;
            }
            if case.validators {
                for rank in 0..2 {
                    let pointer = owners[rank].regions(group)?.validator(rank)?;
                    // SAFETY: exact pinned bounded validator, sole owner writable
                    // region, no concurrent consumer; this call retires before R2.
                    unsafe {
                        group.dispatch_unchecked(
                            &kernels[rank].0,
                            arguments(generation, true),
                            [64, 1, 1],
                            [64, 1, 1],
                            &[pointer],
                            req.timeout_ms,
                        )
                    }?;
                    dispatches += 1;
                    let verdict = if case.state[rank].prefix == terminal(generation).prefix {
                        1
                    } else {
                        2
                    };
                    case.state[rank].guard = guard_words(generation, verdict);
                }
            }
            for rank in 0..2 {
                if owners[rank].observe(group)? != case.state[rank] {
                    return Err(format!(
                        "{}: rank{rank} validator/guard readback",
                        case.name
                    ));
                }
            }
            for rank in 0..2 {
                // Each serial call gets a fresh sentinel on both outputs so a
                // write to the wrong rank cannot be hidden by the next call.
                for buffer in output {
                    group.write(buffer, 0, &poison)?;
                }
                let pointers = [
                    p0.pointer(0, CANARY as u64, (WIDTH * 4) as u64, BufferAccessV1::Read),
                    p1.pointer(16, CANARY as u64, (WIDTH * 4) as u64, BufferAccessV1::Read),
                    residual[rank].pointer(
                        32,
                        CANARY as u64,
                        (WIDTH * 2) as u64,
                        BufferAccessV1::Read,
                    ),
                    output[rank].pointer(
                        48,
                        CANARY as u64,
                        (WIDTH * 2) as u64,
                        BufferAccessV1::Write,
                    ),
                    owners[0].regions(group)?.r2_guard(rank, 64)?,
                    owners[1].regions(group)?.r2_guard(rank, 80)?,
                ];
                // SAFETY: exact reviewed image and argument profile; both guards
                // are stable and visible before this serial, synchronous call.
                // Inputs and output are disjoint; peer bindings are read-only.
                unsafe {
                    group.dispatch_unchecked(
                        &kernels[rank].1,
                        arguments(generation, false),
                        [64, 1, 1],
                        [4096, 1, 1],
                        &pointers,
                        req.timeout_ms,
                    )
                }?;
                dispatches += 1;
                let bytes = group.read(output[rank], 0, (WIDTH * 2 + CANARY * 2) as u32)?;
                let wanted = if case.valid {
                    guarded(&expected)
                } else {
                    poison.clone()
                };
                if bytes != wanted {
                    let index = bytes.iter().zip(&wanted).position(|(x, y)| x != y);
                    return Err(format!(
                        "{} generation{generation} rank{rank}: output/canary mismatch at {index:?}",
                        case.name
                    ));
                }
                if group.read(output[1 - rank], 0, (WIDTH * 2 + CANARY * 2) as u32)? != poison {
                    return Err(format!(
                        "{}: rank{rank} changed the other output",
                        case.name
                    ));
                }
                records.push(serde_json::json!({"case":case.name,"generation":generation,
                    "rank":rank,"valid":case.valid,"validators_executed":case.validators,
                    "guards":[case.state[0].guard,case.state[1].guard],
                    "output_sha256":digest(&bytes),"all_output_bytes_and_canaries_checked":true,
                    "non_target_output_unchanged":true,
                    "p0_le_hex":if case.valid {Some(hex(&a))} else {None},
                    "p1_le_hex":if case.valid {Some(hex(&b))} else {None},
                    "residual_le_hex":if case.valid {Some(hex(&r))} else {None},
                    "output_le_hex":if case.valid {Some(hex(&bytes[CANARY..CANARY+WIDTH*2]))} else {None}}));
            }
            for rank in 0..2 {
                if owners[rank].observe(group)? != case.state[rank] {
                    return Err("R2 changed immutable combined atomics".into());
                }
            }
            for (buffer, payload) in [(p0, &a), (p1, &b), (residual[0], &r), (residual[1], &r)] {
                if group.read(buffer, 0, (payload.len() + CANARY * 2) as u32)? != guarded(payload) {
                    return Err("R2 changed input or input canary".into());
                }
            }
        }
    }
    Ok(
        serde_json::json!({"records":records,"dispatches":dispatches,
        "serial_host_ordered":true,"paired_coordinator_tested":false,
        "owner_lifecycle_tested":false,"performance_claim":false}),
    )
}

fn run() -> Result<serde_json::Value> {
    if std::env::var("FE2O3_ALLOW_UNAUTHENTICATED_MACHINE_CODE_V1")
        .ok()
        .as_deref()
        != Some("1")
    {
        return Err("native diagnostic requires explicit engineering opt-in".into());
    }
    let path = PathBuf::from(
        std::env::var_os("FE2O3_NATIVE_GUARDED_R2_REQUEST_V1")
            .ok_or("native guarded R2 request missing")?,
    );
    let request_bytes = read_regular(&path, 4096)?;
    if std::env::var("FE2O3_NATIVE_GUARDED_R2_REQUEST_SHA256")
        .ok()
        .as_deref()
        != Some(digest(&request_bytes).as_str())
    {
        return Err("native diagnostic request identity".into());
    }
    let req = request(&request_bytes)?;
    let object = read_regular(&req.image, IMAGE_BYTES)?;
    if object.len() != IMAGE_BYTES || digest(&object) != IMAGE_SHA {
        return Err("native guarded R2 image identity".into());
    }
    let hash = Sha256::digest(&object).into();
    // SAFETY: root-selected disposable process owns both devices and bounds.
    let mut group =
        unsafe { Gfx950EngineeringPeerGroupV1::open_unchecked(&req.device_unique_ids) }?;
    let result = exercise(&mut group, &req, &object, hash);
    // Native errors quarantine the group. Never retry close or access its data.
    if group.poisoned {
        return Err(result
            .err()
            .unwrap_or_else(|| "native group poisoned".into()));
    }
    let closed = group.close();
    let mut result = match (result, closed) {
        (Ok(value), Ok(())) => value,
        (Err(error), Ok(())) => return Err(error),
        (Ok(_), Err(error)) => return Err(format!("native close failed: {error}")),
        (Err(error), Err(close)) => return Err(format!("{error}; native close failed: {close}")),
    };
    if read_regular(&path, 4096)? != request_bytes
        || read_regular(&req.image, IMAGE_BYTES)? != object
    {
        return Err("native diagnostic input changed".into());
    }
    result["schema"] = "ferric-native-stable-guarded-r2-observation-v1".into();
    result["request_sha256"] = digest(&request_bytes).into();
    result["image_sha256"] = IMAGE_SHA.into();
    result["device_unique_ids"] = serde_json::json!(req.device_unique_ids);
    result["healthy_close"] = true.into();
    result["gpu_execution"] = true.into();
    result["full_model_acceptance"] = false.into();
    result["production_authority"] = false.into();
    Ok(result)
}

#[test]
#[ignore = "requires explicit two-device MI350 diagnostic and retained image"]
fn native_stable_guarded_r2_v1() {
    let result = run().expect("native stable guarded R2 diagnostic failed");
    println!("FERRIC_NATIVE_GUARDED_R2_V1={result}");
}

#[test]
fn native_reference_anchors_preserve_both_roundings_and_special_finite_values() {
    for (a, b, r, expected) in ANCHORS {
        assert_eq!(
            reference(f32::from_bits(a), f32::from_bits(b), r).unwrap(),
            expected,
            "{a:08x} {b:08x} {r:04x}"
        );
    }
    for bits in [0x7f800000, 0xff800000, 0x7fc00001] {
        assert!(reference(f32::from_bits(bits), 0.0, 0).is_err());
        assert!(reference(0.0, f32::from_bits(bits), 0).is_err());
    }
    for residual in [0x7f80, 0xff80, 0x7fc1] {
        assert!(reference(0.0, 0.0, residual).is_err());
    }
    for (a, b, residual) in [
        (0x7f7fffff, 0, 0xff7f),
        (0x7f7fffff, 0x7f7fffff, 0),
        (0x7f7f0000, 0, 0x7f7f),
        (0x7f7f0000, 0, 0x7b00),
    ] {
        assert!(reference(f32::from_bits(a), f32::from_bits(b), residual).is_err());
    }
    for generation in 0..2 {
        let (a, b, r, y) = corpus(generation).unwrap();
        assert_eq!(
            (a.len(), b.len(), r.len(), y.len()),
            (16384, 16384, 8192, 8192)
        );
    }
}

#[test]
fn native_case_matrix_covers_each_guard_rank_and_validator_boundary() {
    for generation in GENERATIONS {
        let cases = cases(generation);
        assert_eq!(cases.len(), 29);
        assert_eq!(cases.iter().filter(|c| c.valid).count(), 1);
        assert_eq!(cases.iter().filter(|c| c.validators).count(), 19);
        let names: std::collections::BTreeSet<_> = cases.iter().map(|c| &c.name).collect();
        assert_eq!(names.len(), cases.len());
        assert_eq!(arguments(generation, true).len(), 280);
        assert_eq!(arguments(generation, false).len(), 360);
    }
}

#[test]
fn native_request_rejects_duplicate_devices_and_unknown_fields() {
    let base = serde_json::json!({"schema":"ferric-native-stable-guarded-r2-request-v1",
        "device_unique_ids":[11,22],"image":"/tmp/retained.hsaco",
        "image_sha256":IMAGE_SHA,"timeout_ms":5000});
    assert!(request(&serde_json::to_vec(&base).unwrap()).is_ok());
    for (field, value) in [
        ("device_unique_ids", serde_json::json!([11, 11])),
        ("timeout_ms", serde_json::json!(5001)),
        ("image", serde_json::json!("relative")),
        ("extra", serde_json::json!(true)),
    ] {
        let mut bad = base.clone();
        bad[field] = value;
        assert!(request(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}
