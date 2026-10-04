use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct InputIdentity {
    digest: [u8; 32],
    byte_len: u64,
    kind: u8,
}

pub fn append_derivation_response_fields(response: &mut Vec<u8>, request: &[u8], output: &[u8]) {
    append_derivation_response_fields_with_salt(response, request, output, &[]);
}

pub fn append_derivation_response_fields_with_salt(
    response: &mut Vec<u8>,
    request: &[u8],
    output: &[u8],
    stage_salt: &[u8],
) {
    push_field(response, 8, &[]);
    push_field(
        response,
        9,
        &derivation_evidence(request, output, stage_salt),
    );
    let response_identity = domain_hash(b"FE2O3/DIRECT-LLVM-WORKER-RESPONSE/V4\0", response);
    push_field(response, 10, &response_identity);
}

fn derivation_evidence(request: &[u8], output: &[u8], stage_salt: &[u8]) -> Vec<u8> {
    let module = input_identity(field(request, 9));
    let module_bytes = input_payload(field(request, 9));
    let linked = synthetic_stage_identity(b"fixture-linked-module", stage_salt, module_bytes);
    let optimized = synthetic_stage_identity(b"fixture-optimized-module", stage_salt, module_bytes);
    let generated = synthetic_stage_identity(b"fixture-generated-object", stage_salt, output);
    let hsaco = InputIdentity {
        digest: Sha256::digest(output).into(),
        byte_len: output.len() as u64,
        kind: 0,
    };

    let mut request_inputs = input_identities(field(request, 10));
    request_inputs.push(module);
    request_inputs.sort_by_key(|input| (input.digest, input.byte_len, input.kind));
    let mut native_inputs: Vec<(u8, InputIdentity)> = request_inputs
        .into_iter()
        .filter(|input| input.kind == 2)
        .map(|input| (1, input))
        .collect();
    native_inputs.push((2, generated));

    let mut body = vec![1];
    encode_content_identity(&mut body, linked);
    encode_content_identity(&mut body, optimized);
    encode_content_identity(&mut body, generated);
    body.extend_from_slice(&(native_inputs.len() as u32).to_le_bytes());
    for (source, input) in &native_inputs {
        body.push(*source);
        encode_content_identity(&mut body, *input);
    }
    body.extend_from_slice(&lld_invocation_identity(request, &native_inputs));
    encode_content_identity(&mut body, hsaco);
    let identity = domain_hash(b"FE2O3/UPSTREAM-LLVM-LLD-DERIVATION-EVIDENCE/V1\0", &body);
    body.extend_from_slice(&identity);
    body
}

fn synthetic_stage_identity(domain: &[u8], stage_salt: &[u8], bytes: &[u8]) -> InputIdentity {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(stage_salt);
    hasher.update(bytes);
    InputIdentity {
        digest: hasher.finalize().into(),
        byte_len: bytes.len().max(1) as u64,
        kind: 0,
    }
}

fn lld_invocation_identity(request: &[u8], inputs: &[(u8, InputIdentity)]) -> [u8; 32] {
    let symbols = decode_strings(field(request, 13));
    let mut arguments: Vec<String> = [
        "ld.lld",
        "--shared",
        "-Bsymbolic",
        "--no-undefined",
        "--export-dynamic",
        "--build-id=none",
        "--nostdlib",
        "--no-dependent-libraries",
        "--fatal-warnings",
        "--threads=1",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if field(request, 7)[1] != 0 {
        arguments.push("--strip-debug".to_owned());
    }
    arguments.extend(
        symbols
            .into_iter()
            .map(|symbol| format!("--undefined={}", std::str::from_utf8(symbol).unwrap())),
    );
    arguments.extend(inputs.iter().map(|(source, input)| {
        format!("@input={source}:{}:{}", hex(input.digest), input.byte_len)
    }));
    arguments.push("-o".to_owned());
    arguments.push("@output=linked.hsaco".to_owned());

    let mut hasher = Sha256::new();
    hasher.update(b"FE2O3/UPSTREAM-LLD-ELF-INVOCATION/V1\0");
    hasher.update((arguments.len() as u32).to_le_bytes());
    for argument in arguments {
        hasher.update((argument.len() as u32).to_le_bytes());
        hasher.update(argument.as_bytes());
    }
    hasher.finalize().into()
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn encode_content_identity(bytes: &mut Vec<u8>, identity: InputIdentity) {
    bytes.extend_from_slice(&identity.digest);
    bytes.extend_from_slice(&identity.byte_len.to_le_bytes());
}

fn input_identity(input: &[u8]) -> InputIdentity {
    InputIdentity {
        kind: input[0],
        digest: input[1..33].try_into().unwrap(),
        byte_len: u64::from_le_bytes(input[33..41].try_into().unwrap()),
    }
}

fn input_identities(inputs: &[u8]) -> Vec<InputIdentity> {
    let count = u32::from_le_bytes(inputs[..4].try_into().unwrap()) as usize;
    let mut offset = 4;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let value = input_identity(&inputs[offset..]);
        offset += 41 + value.byte_len as usize;
        values.push(value);
    }
    values
}

fn input_payload(input: &[u8]) -> &[u8] {
    let length = u64::from_le_bytes(input[33..41].try_into().unwrap()) as usize;
    &input[41..41 + length]
}

fn decode_strings(bytes: &[u8]) -> Vec<&[u8]> {
    let count = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    let mut offset = 4;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        values.push(&bytes[offset..offset + len]);
        offset += len;
    }
    values
}

fn field(bytes: &[u8], wanted: u16) -> &[u8] {
    let mut offset = 8;
    while offset < bytes.len() {
        let tag = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
        let len = u32::from_le_bytes(bytes[offset + 2..offset + 6].try_into().unwrap()) as usize;
        offset += 6;
        if tag == wanted {
            return &bytes[offset..offset + len];
        }
        offset += len;
    }
    panic!("missing field {wanted}")
}

fn push_field(bytes: &mut Vec<u8>, tag: u16, value: &[u8]) {
    bytes.extend_from_slice(&tag.to_le_bytes());
    bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
    bytes.extend_from_slice(value);
}
