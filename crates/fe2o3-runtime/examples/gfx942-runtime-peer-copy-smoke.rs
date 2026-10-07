//! Production constructor peer-copy check with no authority to launch kernels.

use std::time::{Duration, Instant};

use fe2o3_kfd::{GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1, Gfx942ComputeXgmiPacketPlanV1};
use fe2o3_runtime::{
    KfdMultiDeviceRuntimeBackendV1, KfdRuntimeAtomicExecutionProfileV1,
    KfdRuntimeAuthorityRequestV1, KfdRuntimeCollectiveExecutionProfileV1,
    KfdRuntimeLaunchAuthorityV1, KfdRuntimeSemanticLaunchAuthorityV1, RuntimeAccessV1,
    RuntimeAllocationIdV1, RuntimeContextV1, RuntimeErrorV1, RuntimeMemoryKindV1,
    RuntimeMemoryRegionV1, RuntimePollV1, RuntimeStreamIdV1, RuntimeValidationErrorV1,
};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
const BYTES: u64 = 2 * GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as u64 + 37;
const ROUNDS: usize = 2;
const WAIT: Duration = Duration::from_secs(30);
const USAGE: &str = "usage: gfx942-runtime-peer-copy-smoke [--semantic-authority] [--staged-default] <0xsource-unique-id> <0xdestination-unique-id>";

#[derive(Debug)]
struct NoCompute;

// SAFETY: No invocation is authorized, regardless of artifact or arguments.
unsafe impl KfdRuntimeLaunchAuthorityV1 for NoCompute {
    fn authorize_launch_v1(&self, _: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        false
    }
}

// SAFETY: Immutable empty profiles advertise no atomic or collective authority.
unsafe impl KfdRuntimeSemanticLaunchAuthorityV1 for NoCompute {
    fn atomic_profiles_v1(&self) -> &[KfdRuntimeAtomicExecutionProfileV1] {
        &[]
    }

    fn collective_profiles_v1(&self) -> &[KfdRuntimeCollectiveExecutionProfileV1] {
        &[]
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    ids: [u64; 2],
    semantic: bool,
    staged: bool,
}

fn options(arguments: &[String]) -> ResultV1<Options> {
    let mut semantic = false;
    let mut staged = false;
    let mut arguments = arguments;
    while let Some(flag) = arguments.first().filter(|arg| arg.starts_with("--")) {
        match flag.as_str() {
            "--semantic-authority" if !semantic => semantic = true,
            "--staged-default" if !staged => staged = true,
            _ => return Err(USAGE.into()),
        }
        arguments = &arguments[1..];
    }
    if arguments.len() != 2 {
        return Err(USAGE.into());
    }
    let mut ids = [0; 2];
    for (id, text) in ids.iter_mut().zip(arguments) {
        let hex = text
            .strip_prefix("0x")
            .filter(|hex| !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(USAGE)?;
        *id = u64::from_str_radix(hex, 16).map_err(|_| USAGE)?;
    }
    if ids.contains(&0) || ids[0] == ids[1] {
        return Err(USAGE.into());
    }
    Ok(Options {
        ids,
        semantic,
        staged,
    })
}

fn failure(stage: &str, error: impl core::fmt::Debug) -> String {
    let detail = format!("stage={stage} {error:?}");
    // Report before live native custody can abort on the error return path.
    eprintln!("production peer-copy diagnostic: {detail}");
    detail
}

fn open(options: &Options) -> ResultV1<KfdMultiDeviceRuntimeBackendV1> {
    let backend = if options.semantic {
        let devices = options
            .ids
            .into_iter()
            .map(|id| {
                (
                    id,
                    Box::new(NoCompute) as Box<dyn KfdRuntimeSemanticLaunchAuthorityV1>,
                )
            })
            .collect();
        if options.staged {
            KfdMultiDeviceRuntimeBackendV1::open_default_with_semantic_authorities_v1(devices)
        } else {
            KfdMultiDeviceRuntimeBackendV1::open_default_with_semantic_authorities_and_native_peer_copy_v1(devices)
        }
    } else {
        let devices = options
            .ids
            .into_iter()
            .map(|id| {
                (
                    id,
                    Box::new(NoCompute) as Box<dyn KfdRuntimeLaunchAuthorityV1>,
                )
            })
            .collect();
        if options.staged {
            KfdMultiDeviceRuntimeBackendV1::open_default(devices)
        } else {
            KfdMultiDeviceRuntimeBackendV1::open_default_with_native_peer_copy_v1(devices)
        }
    };
    backend.map_err(|error| failure("device-admission", error))
}

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: BYTES,
    }
}

fn pattern(index: u64, round: usize) -> u8 {
    let mut value = index.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    (value ^ (value >> 31)) as u8 ^ (round as u8).wrapping_mul(73)
}

fn verify(context: &mut Context, data: RuntimeAllocationIdV1, expected: &[u8]) -> ResultV1<()> {
    let mut observed = vec![0; expected.len()];
    context
        .read_allocation(data, 0, &mut observed)
        .map_err(|error| failure("full-readback", error))?;
    if let Some(offset) = observed.iter().zip(expected).position(|(a, b)| a != b) {
        return Err(failure("full-byte-mismatch", offset));
    }
    Ok(())
}

fn upload(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    host: RuntimeAllocationIdV1,
    data: RuntimeAllocationIdV1,
    contents: &[u8],
) -> ResultV1<()> {
    context
        .write_allocation(host, 0, contents)
        .map_err(|error| failure("host-write", error))?;
    let mut copy = context
        .copy_async(
            stream,
            region(host, RuntimeAccessV1::Read),
            region(data, RuntimeAccessV1::Write),
            &[],
        )
        .map_err(|error| failure("upload-enqueue", error))?;
    context
        .flush_stream(stream)
        .map_err(|error| failure("upload-flush", error))?;
    if context
        .wait(&mut copy, WAIT)
        .map_err(|error| failure("upload-wait", error))?
        != RuntimePollV1::Succeeded
    {
        return Err(failure("upload-wait", "upload did not succeed"));
    }
    context
        .release_submission(copy)
        .map_err(|error| failure("upload-release", error))
}

fn run(options: Options) -> ResultV1<()> {
    let plan = Gfx942ComputeXgmiPacketPlanV1::new(BYTES).ok_or("invalid witness plan")?;
    if plan.count() != 3 || plan.packet(2).map(|packet| packet.bytes) != Some(37) {
        return Err("unexpected witness plan".into());
    }
    let mut context =
        Context::open(open(&options)?).map_err(|error| failure("context-open", error))?;
    if context.devices().len() != 2
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err("exact two-device gfx942:xnack- roster required".into());
    }
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let mut allocations = Vec::with_capacity(2);
    for device in devices {
        let stream = context
            .create_stream(device)
            .map_err(|error| failure("stream-create", error))?;
        let host = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, BYTES, 4096)
            .map_err(|error| failure("host-allocation", error))?;
        let data = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, BYTES, 4096)
            .map_err(|error| failure("device-allocation", error))?;
        allocations.push((stream, host, data));
    }
    for round in 0..ROUNDS {
        let source: Vec<_> = (0..BYTES).map(|index| pattern(index, round)).collect();
        let sentinel: Vec<_> = source.iter().map(|byte| !byte).collect();
        for ((stream, host, data), contents) in allocations.iter().zip([&source, &sentinel]) {
            upload(&mut context, *stream, *host, *data, contents)?;
            verify(&mut context, *data, contents)?;
        }
        let expected_before = if options.staged { 0 } else { round as u64 };
        if context.backend().completed_compute_xgmi_copies_v1() != expected_before {
            return Err("unexpected prior native logical copy count".into());
        }
        let mut copy = context
            .peer_copy(
                allocations[1].0,
                region(allocations[0].2, RuntimeAccessV1::Read),
                region(allocations[1].2, RuntimeAccessV1::Write),
                &[],
            )
            .map_err(|error| failure("peer-enqueue", error))?;
        if context
            .poll(&mut copy)
            .map_err(|error| failure("observer-poll", error))?
            != RuntimePollV1::Pending
            || context
                .wait(&mut copy, Duration::ZERO)
                .map_err(|error| failure("observer-wait", error))?
                != RuntimePollV1::Pending
            || !matches!(
                context.drain(&mut copy, Instant::now()),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidDeadline
                ))
            )
            || context.backend().completed_compute_xgmi_copies_v1() != expected_before
        {
            return Err("observer or expired drain progressed the peer copy".into());
        }
        let expected_after = if options.staged {
            0
        } else {
            expected_before + 1
        };
        let deadline = Instant::now() + WAIT;
        loop {
            if Instant::now() >= deadline {
                return Err("peer-copy deadline expired".into());
            }
            context
                .flush_stream(allocations[1].0)
                .map_err(|error| failure("peer-flush", error))?;
            let status = context
                .poll(&mut copy)
                .map_err(|error| failure("peer-poll", error))?;
            let count = context.backend().completed_compute_xgmi_copies_v1();
            match status {
                RuntimePollV1::Pending if count == expected_before => {
                    std::thread::sleep(Duration::from_micros(50))
                }
                RuntimePollV1::Succeeded if count == expected_after => break,
                _ => return Err(failure("peer-result-or-logical-count", (status, count))),
            }
        }
        context
            .release_submission(copy)
            .map_err(|error| failure("peer-release", error))?;
        verify(&mut context, allocations[0].2, &source)?;
        verify(&mut context, allocations[1].2, &source)?;
    }
    let native_copies = context.backend().completed_compute_xgmi_copies_v1();
    for (stream, host, data) in allocations.into_iter().rev() {
        context
            .release_allocation(data)
            .map_err(|error| failure("device-release", error))?;
        context
            .release_allocation(host)
            .map_err(|error| failure("host-release", error))?;
        context
            .destroy_stream(stream)
            .map_err(|error| failure("stream-destroy", error))?;
    }
    let mut backend = context
        .shutdown()
        .map_err(|error| failure("context-shutdown", error))?;
    backend
        .shutdown_native_v1()
        .map_err(|error| failure("native-shutdown", error))?;
    println!(
        "PASS schema=fe2o3.production-peer-copy-smoke.v1 authority={} transport={} devices=2 launches=0 peer_copies={} bytes_per_copy={} native_packets={} observed_native_copies={} source_unique_id=0x{:016x} destination_unique_id=0x{:016x} allocations=4 streams=2 readbacks={} rounds_changed=true pre_flush_observers=pending expired_drain=rejected logical_counter=final-only source_unchanged=full-byte-pass destination_sentinel=full-byte-pass output=full-byte-pass cleanup=logical-and-native-explicit performance_acceptance=false formal_refinement=false",
        if options.semantic {
            "semantic-deny-all"
        } else {
            "production-deny-all"
        },
        if options.staged {
            "HOST-STAGED"
        } else {
            "NATIVE-XGMI"
        },
        ROUNDS,
        BYTES,
        if options.staged {
            0
        } else {
            ROUNDS * plan.count()
        },
        native_copies,
        options.ids[0],
        options.ids[1],
        4 * ROUNDS
    );
    Ok(())
}

fn main() {
    let result = options(&std::env::args().skip(1).collect::<Vec<_>>()).and_then(run);
    if let Err(error) = result {
        eprintln!("production peer-copy smoke failed before complete cleanup: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> ResultV1<Options> {
        options(
            &args
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn cli_requires_exact_distinct_ids_and_unique_leading_flags() {
        assert_eq!(
            parse(&["0x1", "0x2"]).unwrap(),
            Options {
                ids: [1, 2],
                semantic: false,
                staged: false
            }
        );
        for args in [
            vec![],
            vec!["0x1"],
            vec!["0x1", "0x1"],
            vec!["0x0", "0x2"],
            vec!["1", "2"],
            vec!["0x+1", "0x2"],
            vec!["--unknown", "0x1", "0x2"],
            vec!["--semantic-authority", "--semantic-authority", "0x1", "0x2"],
            vec!["--staged-default", "--staged-default", "0x1", "0x2"],
            vec!["0x1", "--staged-default", "0x2"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
        for flags in [
            ["--semantic-authority", "--staged-default"],
            ["--staged-default", "--semantic-authority"],
        ] {
            assert_eq!(
                parse(&[flags[0], flags[1], "0x1", "0x2"]).unwrap(),
                Options {
                    ids: [1, 2],
                    semantic: true,
                    staged: true
                }
            );
        }
    }

    #[test]
    fn copy_only_authority_never_admits_a_kernel_or_semantic_profile() {
        for image in [&[][..], &[0xff_u8; 16][..]] {
            let request = KfdRuntimeAuthorityRequestV1 {
                module_image: image,
                module_sha256: [0; 32],
                kernel_name: "not-authorized",
                signature: [0; 32],
                explicit_kernarg: image,
                complete_kernarg_template: image,
                bindings: &[],
                dispatch_abi: &[],
                allocations: &[],
                geometry: fe2o3_runtime::RuntimeLaunchGeometryV1 {
                    grid: [1; 3],
                    workgroup: [1; 3],
                    dynamic_shared_bytes: 0,
                },
                semantic_launch: fe2o3_runtime::KfdRuntimeSemanticLaunchV1::Ordinary,
            };
            assert!(!NoCompute.authorize_launch_v1(request));
        }
        assert!(NoCompute.atomic_profiles_v1().is_empty());
        assert!(NoCompute.collective_profiles_v1().is_empty());
    }

    #[test]
    fn changed_patterns_detect_packet_replay_and_old_rounds() {
        let cap = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
        let plan = Gfx942ComputeXgmiPacketPlanV1::new(BYTES).unwrap();
        assert_eq!(plan.count(), 3);
        assert_eq!(plan.packet(2).unwrap().bytes, 37);
        for index in [0, 1, cap - 1, cap, cap + 1, 2 * cap, BYTES - 1] {
            assert_ne!(pattern(index, 0), pattern(index, 1));
            assert_ne!(pattern(index, 0), !pattern(index, 0));
        }
        assert!((0..37).any(|index| pattern(index, 0) != pattern(2 * cap + index, 0)));
    }
}
