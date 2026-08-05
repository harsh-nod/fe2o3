mod common;

use common::{kernel, manifest, object, target, text};
use fe2o3_artifacts::{Capability, CompilerIdentity, ManifestV1, PointerWidth, ToolIdentity};

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0xf)]));
    }
    output
}

#[test]
fn encoding_is_canonical_for_set_like_input_order() {
    let compiler = CompilerIdentity::new(text("rustc"), text("1.94.0"));
    let producer = ToolIdentity::new(text("fe2o3"), text("0.1.0"));
    let first = ManifestV1::new(
        compiler.clone(),
        producer.clone(),
        target(
            PointerWidth::Bits64,
            vec![Capability::AmdWave, Capability::Atomics],
        ),
        vec![object(0x44), object(0x43)],
        vec![
            kernel(
                0x12,
                "z_kernel",
                "z_kernel.kd",
                0x44,
                vec![Capability::AmdWave],
            ),
            kernel(
                0x11,
                "a_kernel",
                "a_kernel.kd",
                0x43,
                vec![Capability::Atomics],
            ),
        ],
    )
    .unwrap();
    let second = ManifestV1::new(
        compiler,
        producer,
        target(
            PointerWidth::Bits64,
            vec![Capability::Atomics, Capability::AmdWave],
        ),
        vec![object(0x43), object(0x44)],
        vec![
            kernel(
                0x11,
                "a_kernel",
                "a_kernel.kd",
                0x43,
                vec![Capability::Atomics],
            ),
            kernel(
                0x12,
                "z_kernel",
                "z_kernel.kd",
                0x44,
                vec![Capability::AmdWave],
            ),
        ],
    )
    .unwrap();

    assert_eq!(first.to_bytes(), second.to_bytes());
}

#[test]
fn v1_golden_bytes_are_stable() {
    const GOLDEN_HEX: &str = "4645324f33414d0001000000050072757374630600312e39342e3005006665326f330500302e312e301100616d6467636e2d616d642d616d6468736107006766783131303001000200060007000100000044444444444444444444444444444444444444444444444444444444444444440039300000000000000100000011111111111111111111111111111111111111111111111111111111111111110a00766563746f725f6164640d00766563746f725f6164642e6b64222222222222222222222222222222222222222222222222222222222222222233333333333333333333333333333333333333333333333333333333333333334444444444444444444444444444444444444444444444444444444444444444010007000101000100000100000001000000ffff000001000000010000000000000000100000200000000000000008000000030001006e000000000000000004000000000000000400000000050000000500696e70757408000000000000000800000000000000080000000104000000000000000400000000010106006f7574707574100000000000000010000000000000000800000002040000000000000004000000010301";

    assert_eq!(hex(&manifest().to_bytes()), GOLDEN_HEX);
}
