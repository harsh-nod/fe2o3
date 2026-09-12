const TYPED_LAYOUT_ABI: [u8; 32] = *include_bytes!("signature.bin");

struct TypedLayoutRuntimeArguments {
    value: f32,
    allocation: fe2o3_runtime::RuntimeAllocationIdV1,
    elements: u64,
}

impl fe2o3_runtime::RuntimeArgumentsV1 for TypedLayoutRuntimeArguments {
    const SIGNATURE_V1: [u8; 32] = TYPED_LAYOUT_ABI;

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        let mut bytes = vec![0; 40];
        bytes[0..4].copy_from_slice(&self.value.to_bits().to_le_bytes());
        bytes[16..24].copy_from_slice(&self.elements.to_le_bytes());
        bytes[32..40].copy_from_slice(&self.elements.to_le_bytes());
        bytes
    }

    fn bindings_v1(&self) -> Vec<fe2o3_runtime::RuntimeBindingV1> {
        vec![
            fe2o3_runtime::RuntimeBindingV1 {
                region: fe2o3_runtime::RuntimeMemoryRegionV1 {
                    allocation: self.allocation,
                    access: fe2o3_runtime::RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 64 * 4,
                },
                kernarg_byte_offset: 8,
            },
            fe2o3_runtime::RuntimeBindingV1 {
                region: fe2o3_runtime::RuntimeMemoryRegionV1 {
                    allocation: self.allocation,
                    access: fe2o3_runtime::RuntimeAccessV1::ReadWrite,
                    byte_offset: 64 * 4,
                    byte_len: 64 * 4,
                },
                kernarg_byte_offset: 24,
            },
        ]
    }
}

struct WrongTypedLayoutRuntimeArguments;

impl fe2o3_runtime::RuntimeArgumentsV1 for WrongTypedLayoutRuntimeArguments {
    const SIGNATURE_V1: [u8; 32] = {
        let mut signature = TYPED_LAYOUT_ABI;
        signature[0] ^= 1;
        signature
    };

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        Vec::new()
    }

    fn bindings_v1(&self) -> Vec<fe2o3_runtime::RuntimeBindingV1> {
        Vec::new()
    }
}

fn main() {
    let bundle = std::fs::read(std::env::args_os().nth(1).expect("bundle path"))
        .expect("read exact compiler-produced bundle");
    let backend = fe2o3_sim_runtime::SimRuntimeBackendV1::gfx942([0x91; 32]).unwrap();
    assert_eq!(
        backend.evidence(),
        fe2o3_sim_runtime::SimRuntimeEvidenceV1 {
            mode: "cpu-kir-semantic-simulation",
            simulated: true,
            hardware: false,
            performance_prediction: false,
        }
    );
    let mut runtime = fe2o3_runtime::RuntimeContextV1::open(backend).unwrap();
    let device = runtime.devices()[0].id();
    let allocation = runtime
        .allocate(
            device,
            fe2o3_runtime::RuntimeMemoryKindV1::HostVisible,
            128 * 4,
            4,
        )
        .unwrap();
    runtime
        .write_allocation(allocation, 0, &vec![0; 128 * 4])
        .unwrap();
    let stream = runtime.create_stream(device).unwrap();
    assert!(
        runtime
            .load_module(device, b"not-a-fe2sim-v3-bundle")
            .is_err()
    );
    let module = runtime.load_module(device, &bundle).unwrap();
    assert!(
        runtime
            .resolve_kernel::<WrongTypedLayoutRuntimeArguments>(module, "typed_layout_corpus")
            .is_err()
    );
    let kernel = runtime
        .resolve_kernel::<TypedLayoutRuntimeArguments>(module, "typed_layout_corpus")
        .unwrap();
    let bad_arguments = TypedLayoutRuntimeArguments {
        value: 1.0,
        allocation,
        elements: 63,
    };
    assert!(
        runtime
            .launch(
                stream,
                &kernel,
                &bad_arguments,
                fe2o3_runtime::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                &[],
            )
            .is_err()
    );
    let first_arguments = TypedLayoutRuntimeArguments {
        value: 1.25,
        allocation,
        elements: 64,
    };
    let mut first = runtime
        .launch(
            stream,
            &kernel,
            &first_arguments,
            fe2o3_runtime::RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
            &[],
        )
        .unwrap();
    let event = runtime.record_event(&first).unwrap();
    assert_eq!(
        runtime
            .wait(&mut first, std::time::Duration::from_secs(10))
            .unwrap(),
        fe2o3_runtime::RuntimePollV1::Succeeded
    );
    let second_arguments = TypedLayoutRuntimeArguments {
        value: 2.5,
        allocation,
        elements: 64,
    };
    let mut second = runtime
        .launch(
            stream,
            &kernel,
            &second_arguments,
            fe2o3_runtime::RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
            &[event],
        )
        .unwrap();
    assert_eq!(
        runtime
            .wait(&mut second, std::time::Duration::from_secs(10))
            .unwrap(),
        fe2o3_runtime::RuntimePollV1::Succeeded
    );
    let mut copied_back = vec![0; 64 * 4];
    runtime
        .read_allocation(allocation, 64 * 4, &mut copied_back)
        .unwrap();
    assert!(
        copied_back
            .chunks_exact(4)
            .all(|bytes| { f32::from_bits(u32::from_le_bytes(bytes.try_into().unwrap())) == 2.5 })
    );
    let mut input = vec![0xff; 64 * 4];
    runtime.read_allocation(allocation, 0, &mut input).unwrap();
    assert_eq!(input, vec![0; 64 * 4], "input region must remain unchanged");
    runtime.release_event(event).unwrap();
    runtime.release_submission(first).unwrap();
    runtime.release_submission(second).unwrap();
    runtime.destroy_stream(stream).unwrap();
    runtime.unload_module(module).unwrap();
    runtime.release_allocation(allocation).unwrap();
    let backend = runtime.shutdown().unwrap();
    assert!(!backend.uses_gpu());
}
