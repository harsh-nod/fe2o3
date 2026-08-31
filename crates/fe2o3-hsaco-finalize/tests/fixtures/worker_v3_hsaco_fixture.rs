use std::io::{self, Read, Write};

use sha2::{Digest, Sha256};

mod worker_derivation_fixture_support;
use worker_derivation_fixture_support::append_derivation_response_fields;

#[allow(dead_code)]
mod worker_v3_hsaco_test_support;

const WORKER_ID: &str = "fixture-worker-v3-hsaco-v1";
const PAYLOAD_MARKER: &[u8] = b"; FE2O3/TEST-HSACO-PAYLOAD/V2-HEX:";
const SCALAR_LLVM_BUILD_IDENTITY: &str =
    "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";

fn main() {
    let mut request = Vec::new();
    io::stdin().read_to_end(&mut request).unwrap();
    let is_v2 = request.get(..8) == Some(b"F3LREQ02");
    let compiler_input = is_v2.then(|| input_payload(field(&request, 9)));
    let scalar = compiler_input.is_some_and(is_scalar_add_module);
    let selector = compiler_input.map_or(0, embedded_source_selector);
    let scalar_fixture = compiler_input
        .filter(|_| scalar)
        .map(scalar_fixture_for_module);
    let output = if let Some(fixture) = &scalar_fixture {
        fixture.bytes.clone()
    } else if let Some(compiler_input) = compiler_input {
        hsaco_payload(compiler_input)
    } else {
        find_hsaco_input(field(&request, 6))
    };
    let mut diagnostics = if scalar {
        scalar_diagnostics(compiler_input.unwrap(), &output)
    } else {
        Vec::new()
    };
    if selector == 0x22 {
        diagnostics.push("post_link.check=substituted status=ok".to_owned());
        diagnostics.sort();
    }
    io::stdout()
        .write_all(&response(&request, is_v2, &output, &diagnostics, selector))
        .unwrap();
}

fn scalar_fixture_for_module(module: &[u8]) -> worker_v3_hsaco_test_support::Fixture {
    use worker_v3_hsaco_test_support::ScalarAddFixtureMutation as Mutation;

    let mutation = match embedded_source_selector(module) {
        0x01 => Mutation::RelSection,
        0x02 => Mutation::RelaSection,
        0x03 => Mutation::DynamicNeeded,
        0x04 => Mutation::DynamicForbiddenTag,
        0x05 => Mutation::DynamicDuplicateTag,
        0x06 => Mutation::DynamicMissingNull,
        0x07 => Mutation::DynamicMissingRequiredTags,
        0x08 => Mutation::ExtraLocalSymbol,
        0x09 => Mutation::UndefinedStaticSymbol,
        0x0a => Mutation::ExtraDynamicSymbol,
        0x0b => Mutation::UndefinedDynamicSymbol,
        0x0c => Mutation::DescriptorComputePgmRsrc3,
        0x0d => Mutation::DescriptorComputePgmRsrc1,
        0x0e => Mutation::DescriptorComputePgmRsrc2,
        0x0f => Mutation::DescriptorKernelCodeProperties,
        0x10 => Mutation::DescriptorReservedByte,
        0x11 => Mutation::MachineBytes,
        _ => Mutation::None,
    };
    worker_v3_hsaco_test_support::scalar_add_fixture_with(mutation)
}

fn embedded_source_selector(module: &[u8]) -> u8 {
    const PREFIX: &[u8] = b"!\"sha256:";
    let Some(start) = module
        .windows(PREFIX.len())
        .position(|window| window == PREFIX)
        .map(|position| position + PREFIX.len())
    else {
        return 0;
    };
    let Some(identity) = module.get(start..start + 64) else {
        return 0;
    };
    let Some(last) = identity.get(62..64) else {
        return 0;
    };
    std::str::from_utf8(last)
        .ok()
        .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        .unwrap_or(0)
}

fn response(
    request: &[u8],
    is_v2: bool,
    output_bytes: &[u8],
    diagnostics: &[String],
    selector: u8,
) -> Vec<u8> {
    let request_id = field(request, 1);
    let mut request_identity = field(request, if is_v2 { 15 } else { 10 }).to_vec();
    if selector == 0x23 {
        request_identity[0] ^= 1;
    }
    let mut bytes = if is_v2 {
        b"F3LRSP04".to_vec()
    } else {
        b"F3LRSP01".to_vec()
    };
    push_field(&mut bytes, 1, request_id);
    push_field(&mut bytes, 2, &request_identity);
    let offset = if is_v2 {
        let mut envelope = field(request, 8).to_vec();
        if selector == 0x24 {
            envelope[0] ^= 1;
        }
        push_field(&mut bytes, 3, &envelope);
        1
    } else {
        0
    };
    push_field(
        &mut bytes,
        3 + offset,
        if selector == 0x20 {
            b"fixture-substituted-worker-v1"
        } else {
            WORKER_ID.as_bytes()
        },
    );
    push_field(
        &mut bytes,
        4 + offset,
        &[if selector == 0x21 { 8 } else { 9 }],
    );
    push_field(&mut bytes, 5 + offset, &encode_strings(diagnostics));
    let mut output = vec![1];
    let mut output_identity = <[u8; 32]>::from(Sha256::digest(output_bytes));
    if selector == 0x25 {
        output_identity[0] ^= 1;
    }
    output.extend_from_slice(&output_identity);
    output.extend_from_slice(&(output_bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(output_bytes);
    push_field(&mut bytes, 6 + offset, &output);
    if is_v2 {
        append_derivation_response_fields(&mut bytes, request, output_bytes);
    }
    bytes
}

fn is_scalar_add_module(bytes: &[u8]) -> bool {
    bytes.starts_with(b"target triple = \"amdgcn-amd-amdhsa\"\n")
        && bytes
            .windows(b"define amdgpu_kernel void @scalar_add".len())
            .any(|window| window == b"define amdgpu_kernel void @scalar_add")
}

fn scalar_diagnostics(module: &[u8], output: &[u8]) -> Vec<String> {
    let module_sha = hex(Sha256::digest(module).into());
    let output_sha = hex(Sha256::digest(output).into());
    let mut diagnostics = vec![
        "post_link.check=exports status=ok symbols=[scalar_add,scalar_add.kd]".to_owned(),
        "post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-".to_owned(),
        format!(
            "post_link.check=pliron_scalar_add_v1_profile status=ok kernel=scalar_add required_workgroup=absent max_flat_workgroup_size=64 wavefront_size=64 kernarg_size=280 explicit_kernarg_size=24 hidden_kernarg_size=256 kernarg_align=8 group_size=0 private_size=0 sgpr_spills=0 vgpr_spills=0 dynamic_stack=false machine_calls=0 machine_branches=0 machine_atomics=0 machine_scratch=0 relocations=0 dynamic_dependencies=0 llvm_build_identity={SCALAR_LLVM_BUILD_IDENTITY} input_ir_sha256={module_sha} raw_hsaco_sha256={output_sha}"
        ),
        "post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c"
            .to_owned(),
        "post_link.check=unresolved status=ok symbols=[]".to_owned(),
        "post_link.kernel name=scalar_add symbol=scalar_add.kd kernarg_size=280 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=absent".to_owned(),
    ];
    diagnostics.sort();
    diagnostics
}

fn encode_strings(values: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for value in values {
        bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }
    bytes
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn find_hsaco_input(inputs: &[u8]) -> Vec<u8> {
    let count = u32::from_le_bytes(inputs[..4].try_into().unwrap()) as usize;
    let mut offset = 4;
    for _ in 0..count {
        offset += 1 + 32;
        let length = u64::from_le_bytes(inputs[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;
        let payload = &inputs[offset..offset + length];
        if let Some(payload) = payload
            .windows(PAYLOAD_MARKER.len())
            .position(|window| window == PAYLOAD_MARKER)
            .map(|position| &payload[position + PAYLOAD_MARKER.len()..])
        {
            let encoded = payload
                .split(|byte| *byte == b'\n')
                .next()
                .expect("embedded payload has one line");
            return hex_decode(encoded).expect("embedded HSACO payload is canonical lowercase hex");
        }
        offset += length;
    }
    panic!("generic request contains no embedded HSACO compiler module")
}

fn input_payload(input: &[u8]) -> &[u8] {
    let length = u64::from_le_bytes(input[33..41].try_into().unwrap()) as usize;
    &input[41..41 + length]
}

fn hsaco_payload(input: &[u8]) -> Vec<u8> {
    let position = input
        .windows(PAYLOAD_MARKER.len())
        .position(|window| window == PAYLOAD_MARKER)
        .expect("V2 compiler module contains no embedded HSACO");
    let encoded = input[position + PAYLOAD_MARKER.len()..]
        .split(|byte| *byte == b'\n')
        .next()
        .expect("embedded payload has one line");
    hex_decode(encoded).expect("embedded HSACO payload is canonical lowercase hex")
}

fn hex_decode(encoded: &[u8]) -> Option<Vec<u8>> {
    if !encoded.len().is_multiple_of(2) {
        return None;
    }
    encoded
        .chunks_exact(2)
        .map(|pair| Some((hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?))
        .collect()
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn field(bytes: &[u8], wanted: u16) -> &[u8] {
    let mut offset = 8;
    while offset < bytes.len() {
        let tag = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
        let length = u32::from_le_bytes(bytes[offset + 2..offset + 6].try_into().unwrap()) as usize;
        offset += 6;
        if tag == wanted {
            return &bytes[offset..offset + length];
        }
        offset += length;
    }
    panic!("missing field {wanted}")
}

fn push_field(bytes: &mut Vec<u8>, tag: u16, value: &[u8]) {
    bytes.extend_from_slice(&tag.to_le_bytes());
    bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
    bytes.extend_from_slice(value);
}
