//! Explicit one-shot TP2 terminal-join witness, not a benchmark.
//!
//! Invoke with --allow-unauthenticated-machine-code --request /absolute/request.json.
//! The caller owns device exclusivity, process/resource bounds and native audits.

#![allow(unsafe_code)]

use fe2o3_kfd::engineering_wire::{BufferAccessV1 as Access, KernelMetadataV1};
use fe2o3_kfd::{
    Gfx950EngineeringPeerBufferV1 as Buffer, Gfx950EngineeringPeerDispatchV1 as Dispatch,
    Gfx950EngineeringPeerGroupV1 as Group, Gfx950EngineeringPeerKernelV1 as Kernel,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

type Result<T> = std::result::Result<T, String>;
const REQUEST_SCHEMA: &str = "ferric-peer-dependency-native-request-v228-v2";
const RESULT_SCHEMA: &str = "ferric-peer-dependency-native-observation-v228-v2";
const SYMBOL: &str = "ferric_qwen3_tp_peer_copy_bf16_v4";
const IMAGE_BYTES: usize = 14_624;
const IMAGE_SHA: &str = "8436c1861dcc14f0346aeecc136e4187c459ac1c0b31778265c88b3d8c63396d";
const WIDTH: usize = 4096;
const GUARD: usize = 64;
const PAYLOAD_BYTES: usize = WIDTH * 2;
const BUFFER_BYTES: usize = PAYLOAD_BYTES + GUARD * 2;
const KERNARG_BYTES: usize = 40 + 256;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceSelector {
    unique_id: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    path: PathBuf,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    devices: [DeviceSelector; 2],
    artifact: Artifact,
    timeout_ms: u32,
    witness: bool,
}

fn absolute_file_path(path: &Path) -> bool {
    path.is_absolute()
        && path.file_name().is_some()
        && path
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
}

fn parse_request(bytes: &[u8]) -> Result<Request> {
    if bytes.len() > 4096 {
        return Err("request exceeds 4096 bytes".into());
    }
    let request: Request = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if request.schema != REQUEST_SCHEMA
        || request.devices[0].unique_id == 0
        || request.devices[1].unique_id == 0
        || request.devices[0].unique_id == request.devices[1].unique_id
        || request.artifact.sha256 != IMAGE_SHA
        || !absolute_file_path(&request.artifact.path)
        || !(1..=600_000).contains(&request.timeout_ms)
    {
        return Err("closed peer-dependency request contract".into());
    }
    Ok(request)
}

fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn read_regular(path: &Path, limit: usize) -> Result<Vec<u8>> {
    if !absolute_file_path(path) {
        return Err("input path must be absolute without parent traversal".into());
    }
    let before = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !before.is_file() || before.len() > limit as u64 {
        return Err("input is not a bounded ordinary file".into());
    }
    let mut file: File = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|error| error.to_string())?;
    if !same_file(
        &before,
        &file.metadata().map_err(|error| error.to_string())?,
    ) {
        return Err("input changed during open".into());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let after = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if bytes.len() as u64 != before.len()
        || bytes.len() > limit
        || !same_file(&before, &after)
        || !same_file(
            &before,
            &file.metadata().map_err(|error| error.to_string())?,
        )
    {
        return Err("input changed during read".into());
    }
    Ok(bytes)
}

fn digest(bytes: &[u8]) -> String {
    let hash: [u8; 32] = Sha256::digest(bytes).into();
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn check_metadata(metadata: &KernelMetadataV1, hash: [u8; 32]) -> Result<()> {
    if metadata.symbol != SYMBOL
        || metadata.object_sha256 != hash
        || metadata.kernarg_bytes != KERNARG_BYTES as u32
        || metadata.kernarg_alignment != 8
        || metadata.group_segment_bytes != 0
        || metadata.private_segment_bytes != 0
        || metadata.wavefront_size != 64
        || metadata.implicit_argument_offset != Some(40)
        || metadata.implicit_argument_bytes != 256
        || metadata.explicit_arguments.len() != 5
    {
        return Err("exact retained copy kernel metadata mismatch".into());
    }
    for (index, access) in [Access::Read, Access::Write].into_iter().enumerate() {
        let pointer = &metadata.explicit_arguments[index * 2];
        let length = &metadata.explicit_arguments[index * 2 + 1];
        if pointer.offset != (index * 16) as u32
            || pointer.bytes != 8
            || !pointer.global_buffer
            || pointer.pointee_alignment.is_some_and(|value| value != 2)
            || pointer.access.is_some_and(|value| value != access)
            || length.offset != (index * 16 + 8) as u32
            || length.bytes != 8
            || length.global_buffer
        {
            return Err("exact retained copy slice ABI mismatch".into());
        }
    }
    let rows = &metadata.explicit_arguments[4];
    if rows.offset != 32 || rows.bytes != 4 || rows.global_buffer {
        return Err("exact retained copy rows ABI mismatch".into());
    }
    Ok(())
}

fn guarded_pattern(rank: usize, iteration: usize) -> Vec<u8> {
    let mut bytes = vec![0xa5; BUFFER_BYTES];
    for (index, word) in bytes[GUARD..GUARD + PAYLOAD_BYTES]
        .chunks_exact_mut(2)
        .enumerate()
    {
        let bits = 0x3c00 | ((index * 13 + rank * 257 + iteration * 521) % 1024) as u16;
        word.copy_from_slice(&bits.to_le_bytes());
    }
    bytes
}

fn guarded_poison() -> Vec<u8> {
    let mut bytes = vec![0xa5; BUFFER_BYTES];
    bytes[GUARD..GUARD + PAYLOAD_BYTES].fill(0);
    bytes
}

struct Buffers {
    seeds: [Buffer; 2],
    intermediate: Buffer,
    outputs: [Buffer; 2],
}

fn allocate_rank(group: &mut Group, rank: usize) -> Result<Buffers> {
    let seeds = [
        group.allocate(rank, &[], BUFFER_BYTES as u64)?,
        group.allocate(rank, &[], BUFFER_BYTES as u64)?,
    ];
    let intermediate = group.allocate(rank, &[1 - rank], BUFFER_BYTES as u64)?;
    let outputs = [
        group.allocate(rank, &[], BUFFER_BYTES as u64)?,
        group.allocate(rank, &[], BUFFER_BYTES as u64)?,
    ];
    for (iteration, seed) in seeds.iter().enumerate() {
        group.write(*seed, 0, &guarded_pattern(rank, iteration))?;
    }
    for buffer in [intermediate, outputs[0], outputs[1]] {
        group.write(buffer, 0, &guarded_poison())?;
    }
    Ok(Buffers {
        seeds,
        intermediate,
        outputs,
    })
}

fn copy_arguments() -> Vec<u8> {
    let mut bytes = vec![0; KERNARG_BYTES];
    bytes[8..16].copy_from_slice(&(WIDTH as u64).to_le_bytes());
    bytes[24..32].copy_from_slice(&(WIDTH as u64).to_le_bytes());
    bytes[32..36].copy_from_slice(&1_u32.to_le_bytes());
    bytes
}

fn copy_dispatch<'a>(
    kernel: &'a Kernel,
    input: Buffer,
    output: Buffer,
    timeout_ms: u32,
) -> Dispatch<'a> {
    Dispatch {
        kernel,
        bytes: copy_arguments(),
        workgroup: [64, 1, 1],
        grid: [WIDTH as u32, 1, 1],
        pointers: vec![
            input.pointer(0, GUARD as u64, PAYLOAD_BYTES as u64, Access::Read),
            output.pointer(16, GUARD as u64, PAYLOAD_BYTES as u64, Access::Write),
        ],
        timeout_ms,
    }
}

fn commands<'a>(
    kernel: &'a Kernel,
    own: &Buffers,
    peer: &Buffers,
    timeout_ms: u32,
) -> [Dispatch<'a>; 4] {
    [
        copy_dispatch(kernel, own.seeds[0], own.intermediate, timeout_ms),
        copy_dispatch(kernel, peer.intermediate, own.outputs[0], timeout_ms),
        copy_dispatch(kernel, own.seeds[1], own.intermediate, timeout_ms),
        copy_dispatch(kernel, peer.intermediate, own.outputs[1], timeout_ms),
    ]
}

#[derive(Serialize)]
struct BufferObservation {
    owner_rank: usize,
    role: &'static str,
    bytes: usize,
    sha256: String,
}

fn checked_bytes(actual: &[u8], expected: &[u8], rank: usize, role: &str) -> Result<()> {
    if actual.len() != BUFFER_BYTES || actual != expected {
        return Err(format!("rank {rank} {role} data or guard mismatch"));
    }
    Ok(())
}

fn verify_rank(
    group: &mut Group,
    rank: usize,
    buffers: &Buffers,
) -> Result<Vec<BufferObservation>> {
    let expected = [
        (buffers.seeds[0], "seed0", guarded_pattern(rank, 0)),
        (buffers.seeds[1], "seed1", guarded_pattern(rank, 1)),
        (
            buffers.intermediate,
            "intermediate",
            guarded_pattern(rank, 1),
        ),
        (buffers.outputs[0], "output0", guarded_pattern(1 - rank, 0)),
        (buffers.outputs[1], "output1", guarded_pattern(1 - rank, 1)),
    ];
    let mut observations = Vec::with_capacity(5);
    for (buffer, role, expected) in expected {
        let actual = group.read(buffer, 0, BUFFER_BYTES as u32)?;
        checked_bytes(&actual, &expected, rank, role)?;
        observations.push(BufferObservation {
            owner_rank: rank,
            role,
            bytes: actual.len(),
            sha256: digest(&actual),
        });
    }
    Ok(observations)
}

fn check_observation(requested: bool, observed: bool, completion_count: u32) -> Result<()> {
    if requested != observed || completion_count != 16 {
        return Err("native witness or sixteen-completion contract mismatch".into());
    }
    Ok(())
}

#[derive(Serialize)]
struct Observation {
    schema: &'static str,
    request_sha256: String,
    device_unique_ids: [u64; 2],
    artifact_path: PathBuf,
    artifact_bytes: usize,
    artifact_sha256: &'static str,
    kernel_symbol: &'static str,
    timeout_ms: u32,
    witness_requested: bool,
    witness_observed: bool,
    completion_count: u32,
    elapsed_ns: u64,
    elapsed_scope: &'static str,
    device_iterations: u32,
    kernel_packets: u32,
    barrier_packets: u32,
    intermediate_reused_between_iterations: bool,
    completion_slots_reset_between_iterations: bool,
    repeated_host_calls_tested: bool,
    buffers: Vec<BufferObservation>,
    all_data_and_guards_verified: bool,
    input_files_unchanged: bool,
    healthy_close: bool,
    performance_claim: bool,
    production_authority: bool,
}

fn run() -> Result<Observation> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3
        || args[0] != "--allow-unauthenticated-machine-code"
        || args[1] != "--request"
    {
        return Err("expected --allow-unauthenticated-machine-code --request ABSOLUTE_JSON".into());
    }
    let request_path = PathBuf::from(&args[2]);
    let request_bytes = read_regular(&request_path, 4096)?;
    let request = parse_request(&request_bytes)?;
    let object = read_regular(&request.artifact.path, IMAGE_BYTES)?;
    if object.len() != IMAGE_BYTES || digest(&object) != IMAGE_SHA {
        return Err("artifact is not the retained copy HSACO".into());
    }
    let hash: [u8; 32] = Sha256::digest(&object).into();
    let ids = [request.devices[0].unique_id, request.devices[1].unique_id];
    // SAFETY: explicit opt-in selects an exclusive disposable process; the
    // caller owns external audits. Every native error is terminal, without retry.
    let mut group = unsafe { Group::open_unchecked(&ids) }?;
    let kernels = [
        group.load_kernel(0, object.clone(), hash, SYMBOL.into())?,
        group.load_kernel(1, object.clone(), hash, SYMBOL.into())?,
    ];
    for (rank, kernel) in kernels.iter().enumerate() {
        if kernel.rank() != rank {
            return Err("loaded kernel rank mismatch".into());
        }
        check_metadata(kernel.metadata(), hash)?;
    }
    let buffers = [allocate_rank(&mut group, 0)?, allocate_rank(&mut group, 1)?];
    let dispatches = [
        commands(&kernels[0], &buffers[0], &buffers[1], request.timeout_ms),
        commands(&kernels[1], &buffers[1], &buffers[0], request.timeout_ms),
    ];
    // SAFETY: each rank's P0/C0/P1/C1 uses the exact checked copy object with
    // retained roots. The runtime inserts both-producer and both-consumer
    // barriers before reuse of either intermediate, plus a final both-C1 join.
    // Seeds/outputs are distinct;
    // no host buffer access occurs until all sixteen completions are retired.
    let native = unsafe {
        group.dispatch_peer_dependency_terminal_join_unchecked_v2(
            dispatches,
            request.timeout_ms,
            request.witness,
        )
    }?;
    check_observation(
        request.witness,
        native.witness_observed,
        native.completion_count,
    )?;
    let mut observed_buffers = verify_rank(&mut group, 0, &buffers[0])?;
    observed_buffers.extend(verify_rank(&mut group, 1, &buffers[1])?);
    group.close()?;
    if read_regular(&request_path, 4096)? != request_bytes
        || read_regular(&request.artifact.path, IMAGE_BYTES)? != object
    {
        return Err("request or retained artifact changed during observation".into());
    }
    Ok(Observation {
        schema: RESULT_SCHEMA,
        request_sha256: digest(&request_bytes),
        device_unique_ids: ids,
        artifact_path: request.artifact.path,
        artifact_bytes: IMAGE_BYTES,
        artifact_sha256: IMAGE_SHA,
        kernel_symbol: SYMBOL,
        timeout_ms: request.timeout_ms,
        witness_requested: request.witness,
        witness_observed: native.witness_observed,
        completion_count: native.completion_count,
        elapsed_ns: native.elapsed_ns,
        elapsed_scope: "native aggregate host interval; not GPU duration or throughput",
        device_iterations: 2,
        kernel_packets: 8,
        barrier_packets: 8,
        intermediate_reused_between_iterations: true,
        completion_slots_reset_between_iterations: false,
        repeated_host_calls_tested: false,
        buffers: observed_buffers,
        all_data_and_guards_verified: true,
        input_files_unchanged: true,
        healthy_close: true,
        performance_claim: false,
        production_authority: false,
    })
}

fn main() {
    let result = run().and_then(|observation| {
        let mut bytes = serde_json::to_vec(&observation).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        std::io::stdout()
            .lock()
            .write_all(&bytes)
            .map_err(|error| error.to_string())
    });
    if let Err(error) = result {
        eprintln!("peer dependency observation failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> serde_json::Value {
        serde_json::json!({
            "schema": REQUEST_SCHEMA,
            "devices": [{"unique_id": 16366993098680759275_u64}, {"unique_id": 10838076764495710945_u64}],
            "artifact": {"path": "/retained/residual.hsaco", "sha256": IMAGE_SHA},
            "timeout_ms": 60000,
            "witness": true
        })
    }

    #[test]
    fn closed_request_preserves_full_width_device_ids() {
        let value = parse_request(&serde_json::to_vec(&request()).unwrap()).unwrap();
        assert_eq!(value.devices[0].unique_id, 16366993098680759275);
        assert_eq!(value.devices[1].unique_id, 10838076764495710945);
    }

    #[test]
    fn v2_schemas_are_distinct_and_v1_request_is_rejected() {
        assert_eq!(
            REQUEST_SCHEMA,
            "ferric-peer-dependency-native-request-v228-v2"
        );
        assert_eq!(
            RESULT_SCHEMA,
            "ferric-peer-dependency-native-observation-v228-v2"
        );
        let mut value = request();
        value["schema"] = "ferric-peer-dependency-native-request-v228-v1".into();
        assert!(parse_request(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn v2_rejects_v1_fourteen_completion_observation() {
        assert!(check_observation(true, true, 14).is_err());
        assert!(check_observation(false, false, 14).is_err());
    }

    #[test]
    fn request_rejects_unknown_fields_duplicates_and_unbounded_timeout() {
        let mut value = request();
        value["repetitions"] = 2.into();
        assert!(parse_request(&serde_json::to_vec(&value).unwrap()).is_err());
        value = request();
        value["devices"][1] = value["devices"][0].clone();
        assert!(parse_request(&serde_json::to_vec(&value).unwrap()).is_err());
        for timeout in [0, 600001] {
            value = request();
            value["timeout_ms"] = timeout.into();
            assert!(parse_request(&serde_json::to_vec(&value).unwrap()).is_err());
        }
    }

    #[test]
    fn artifact_identity_and_path_are_closed() {
        for path in ["relative.hsaco", "/retained/../residual.hsaco"] {
            let mut value = request();
            value["artifact"]["path"] = path.into();
            assert!(parse_request(&serde_json::to_vec(&value).unwrap()).is_err());
        }
        let mut value = request();
        value["artifact"]["sha256"] = "0".repeat(64).into();
        assert!(parse_request(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn all_four_patterns_are_distinct_at_every_word_and_finite() {
        let values = [
            guarded_pattern(0, 0),
            guarded_pattern(0, 1),
            guarded_pattern(1, 0),
            guarded_pattern(1, 1),
        ];
        for index in 0..WIDTH {
            let offset = GUARD + index * 2;
            let words: Vec<_> = values
                .iter()
                .map(|bytes| u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
                .collect();
            for left in 0..4 {
                assert_ne!(words[left] & 0x7f80, 0x7f80);
                for right in left + 1..4 {
                    assert_ne!(words[left], words[right]);
                }
            }
        }
    }

    #[test]
    fn guard_seed_and_wrong_iteration_corruption_are_detected() {
        let expected = guarded_pattern(0, 0);
        assert!(checked_bytes(&expected, &expected, 0, "seed0").is_ok());
        for index in [0, GUARD - 1, GUARD, GUARD + PAYLOAD_BYTES, BUFFER_BYTES - 1] {
            let mut actual = expected.clone();
            actual[index] ^= 1;
            assert!(checked_bytes(&actual, &expected, 0, "seed0").is_err());
        }
        assert!(checked_bytes(&guarded_pattern(0, 1), &expected, 0, "output0").is_err());
        assert!(checked_bytes(&guarded_poison(), &expected, 0, "output0").is_err());
    }

    #[test]
    fn copy_kernargs_preserve_exact_slice_and_row_geometry() {
        let bytes = copy_arguments();
        assert_eq!(bytes.len(), 296);
        assert_eq!(&bytes[8..16], &4096_u64.to_le_bytes());
        assert_eq!(&bytes[24..32], &4096_u64.to_le_bytes());
        assert_eq!(&bytes[32..36], &1_u32.to_le_bytes());
        assert!(
            bytes[..8]
                .iter()
                .chain(&bytes[16..24])
                .chain(&bytes[36..])
                .all(|byte| *byte == 0)
        );
    }

    #[test]
    fn witness_and_all_sixteen_completions_are_required() {
        assert!(check_observation(true, true, 16).is_ok());
        assert!(check_observation(false, false, 16).is_ok());
        assert!(check_observation(true, false, 16).is_err());
        assert!(check_observation(false, true, 16).is_err());
        for count in [0, 8, 13, 15, 17] {
            assert!(check_observation(true, true, count).is_err());
        }
    }
}
