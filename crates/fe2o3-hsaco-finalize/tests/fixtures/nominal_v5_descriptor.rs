//! Caller-authored inert content shared by V5 artifact and fixture-worker tests.
#![allow(dead_code)]
use fe2o3_kernel_descriptor::*;
use sha2::{Digest, Sha256};
#[path = "../../../fe2o3-kernel-descriptor/tests/support/conditional_v5.rs"]
pub mod descriptor;

pub fn free(_: usize) -> Result<(), &'static str> {
    Ok(())
}

pub fn wires(
    target: &str,
    entries: usize,
    inputs: usize,
    workgroup: Option<u32>,
    release: &str,
) -> (Vec<u8>, Vec<u8>) {
    descriptor::with_input(target, entries, inputs, |input| {
        assert!(workgroup.is_none() || entries == 1);
        let compiler = CompilerIdentityV1::new(
            Text::new("rustc").unwrap(),
            Text::new(release).unwrap(),
            [7; 20],
        );
        let launch = LaunchConstraintsV1::new(
            1,
            workgroup.map_or(BlockSizeV1::Any, |n| {
                BlockSizeV1::Exact(DimensionsV1::new(n, 1, 1).unwrap())
            }),
            DimensionsV1::new(1024, 1, 1).unwrap(),
            256,
            0,
            0,
        )
        .unwrap();
        let explicit = ((inputs + 1) * 16) as u32;
        let kernels = input
            .nominal
            .kernels
            .iter()
            .map(|k| KernelDescriptorInputV3 {
                kernel_id: k.kernel_id,
                logical_name: if workgroup.is_some() {
                    "vecadd"
                } else {
                    k.logical_name
                },
                entry_name: if workgroup.is_some() {
                    "vecadd"
                } else {
                    k.entry_name
                },
                descriptor_symbol: if workgroup.is_some() {
                    "vecadd.kd"
                } else {
                    k.descriptor_symbol
                },
                source_evidence: k.source_evidence,
                executable_ir_evidence: k.executable_ir_evidence,
                capabilities: k.capabilities,
                abi_layout: KernelAbiLayoutV1::new(explicit, explicit + 256, 8).unwrap(),
                launch: &launch,
                arguments: k.arguments,
            })
            .collect::<Vec<_>>();
        let input = DeviceDescriptorTableInputV5 {
            nominal: DeviceDescriptorTableInputV3 {
                compiler: &compiler,
                kernels: &kernels,
                ..input.nominal
            },
            contracts: input.contracts,
        };
        let mut v5 = vec![0; encoded_device_descriptor_table_v5_len(&input, &mut free).unwrap()];
        encode_device_descriptor_table_v5(&input, &mut v5, &mut free).unwrap();
        let mut v3 =
            vec![0; encoded_device_descriptor_table_v3_len(&input.nominal, &mut free).unwrap()];
        encode_device_descriptor_table_v3(&input.nominal, &mut v3, &mut free).unwrap();
        (v5, v3)
    })
}

// One-contract fixture mutation. Fixed V2 offsets independently spell the seven
// formula fields; repairing inert hashes never manufactures source/proof evidence.
pub fn substitute_cpu(wire: &[u8], coherent: bool) -> Vec<u8> {
    let mut changed = wire.to_vec();
    let start = descriptor::contract_start(&changed);
    changed[start + 576] ^= 0x40;
    if coherent {
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V2/LE/SHARED-IEEE/CPU-SOURCE\0");
        for offset in [92, 352, 448, 480, 512, 544, 576] {
            hash.update(&changed[start + offset..start + offset + 32]);
        }
        changed[start + 320..start + 352].copy_from_slice(&hash.finalize());
    }
    descriptor::repair_identity(&mut changed);
    changed
}
