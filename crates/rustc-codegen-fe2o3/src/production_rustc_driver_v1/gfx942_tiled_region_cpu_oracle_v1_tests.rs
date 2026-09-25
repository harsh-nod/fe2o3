//! Independent dense integer oracle. This does not decode or execute KIR.
use super::*;
use fe2o3_kernel_ir::{AccessMode, ScalarType};
use fe2o3_kir_sim::*;
pub(super) const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();
pub(super) const PATTERNS: usize = 6;
pub(super) const LENGTHS: [usize; 3] = [64, 13, 0];
pub(super) const CANARY: u32 = 0x7f12_3456;
pub(super) const INPUT_IDS: [BufferBackingIdV1; 3] = [
    BufferBackingIdV1(11),
    BufferBackingIdV1(12),
    BufferBackingIdV1(13),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Words(pub [u32; 256]);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Output(pub [u8; 272]);
impl Serialize for Output {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut text = [0u8; 544];
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for (byte, pair) in self.0.iter().zip(text.chunks_exact_mut(2)) {
            pair[0] = HEX[usize::from(byte >> 4)];
            pair[1] = HEX[usize::from(byte & 15)];
        }
        serializer.serialize_str(std::str::from_utf8(&text).expect("fixed lowercase hex"))
    }
}
impl Serialize for Words {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut text = [0u8; 2048];
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for (word, chunk) in self.0.iter().zip(text.chunks_exact_mut(8)) {
            for (byte, pair) in word.to_le_bytes().iter().zip(chunk.chunks_exact_mut(2)) {
                pair[0] = HEX[usize::from(byte >> 4)];
                pair[1] = HEX[usize::from(byte & 15)];
            }
        }
        serializer.serialize_str(std::str::from_utf8(&text).expect("fixed lowercase hex"))
    }
}
pub(super) fn integer_bits(value: i64) -> u32 {
    if value == 0 {
        return 0;
    }
    let magnitude = value.unsigned_abs();
    assert!(magnitude < (1 << 24));
    let exponent = 63 - magnitude.leading_zeros();
    let fraction = ((magnitude - (1 << exponent)) << (23 - exponent)) as u32;
    (u32::from(value < 0) << 31) | ((exponent + 127) << 23) | fraction
}
pub(super) fn dense(pattern: usize) -> ([i64; 256], [i64; 256]) {
    assert!(pattern < PATTERNS);
    let mut a = [0; 256];
    let mut b = [0; 256];
    for row in 0..16 {
        for col in 0..16 {
            let index = row * 16 + col;
            (a[index], b[index]) = match pattern {
                0 => (0, 0),
                1 => (i64::from(row == col), (3 * row + col) as i64 % 17 - 8),
                2 => (
                    (5 * row + 3 * col) as i64 % 33 - 16,
                    (2 * row + 7 * col) as i64 % 31 - 15,
                ),
                3 => (16, 16),
                4 => (-16, 16),
                5 => (if col % 2 == 0 { 16 } else { -16 }, 16),
                _ => unreachable!(),
            };
        }
    }
    (a, b)
}
pub(super) fn expected(pattern: usize) -> Words {
    let (a, b) = dense(pattern);
    let mut output = [0; 256];
    for row in 0..16 {
        for col in 0..16 {
            let mut sum = 0_i64;
            for k in 0..16 {
                sum += a[row * 16 + k] * b[k * 16 + col];
            }
            assert!(sum.unsigned_abs() <= 4096);
            output[row * 16 + col] = integer_bits(sum);
        }
    }
    Words(output)
}
pub(super) fn lane_word(expected: &Words, lane: usize, component: usize) -> u32 {
    assert!(lane < 64 && component < 4);
    expected.0[(4 * (lane / 16) + component) * 16 + lane % 16]
}
pub(super) fn expected_output(pattern: usize, length: usize) -> Output {
    assert!(LENGTHS.contains(&length));
    let mut bytes = [0; 272];
    let expected = expected(pattern);
    for (slot, raw) in bytes.chunks_exact_mut(4).enumerate() {
        let value = if (2..2 + length).contains(&slot) {
            lane_word(&expected, slot - 2, 0)
        } else {
            CANARY
        };
        raw.copy_from_slice(&value.to_le_bytes());
    }
    Output(bytes)
}
pub(super) fn bytes(pattern: usize, input: usize) -> Vec<u8> {
    let (a, b) = dense(pattern);
    let values = if input == 0 { a } else { b };
    let mut bytes = Vec::with_capacity(512);
    for value in values {
        bytes.extend_from_slice(&((integer_bits(value) >> 16) as u16).to_le_bytes());
    }
    bytes
}
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) enum Control {
    Positive,
    UninitializedA,
    UninitializedB,
    NegativeZeroA,
    NegativeZeroB,
    FractionalA,
    FractionalB,
    SubnormalA,
    InfiniteB,
    OutsideDomainA,
    StepLimit,
    RecordLimit,
    DebugStop,
    EventFailure,
    Grid63,
    Grid65,
    Wave32,
}
pub(super) const NEGATIVES: [Control; 16] = [
    Control::UninitializedA,
    Control::UninitializedB,
    Control::NegativeZeroA,
    Control::NegativeZeroB,
    Control::FractionalA,
    Control::FractionalB,
    Control::SubnormalA,
    Control::InfiniteB,
    Control::OutsideDomainA,
    Control::StepLimit,
    Control::RecordLimit,
    Control::DebugStop,
    Control::EventFailure,
    Control::Grid63,
    Control::Grid65,
    Control::Wave32,
];
pub(super) fn request(
    kernel: &fe2o3_kernel_ir::KernelId,
    pattern: usize,
    length: usize,
    control: Control,
) -> SimulationRequestV1 {
    assert!(LENGTHS.contains(&length));
    let mut shared = Vec::with_capacity(3);
    for input in 0..2 {
        let mut raw = bytes(pattern, input);
        let mut init = vec![true; 512];
        let change = match (control, input) {
            (Control::UninitializedA, 0) | (Control::UninitializedB, 1) => {
                init[510] = false;
                None
            }
            (Control::NegativeZeroA, 0) | (Control::NegativeZeroB, 1) => Some(0x8000_u16),
            (Control::FractionalA, 0) | (Control::FractionalB, 1) => Some(0x3f00),
            (Control::SubnormalA, 0) => Some(1),
            (Control::InfiniteB, 1) => Some(0x7f80),
            (Control::OutsideDomainA, 0) => Some(0x4188),
            _ => None,
        };
        if let Some(bits) = change {
            raw[510..512].copy_from_slice(&bits.to_le_bytes());
        }
        shared.push(SharedBufferV1 {
            id: INPUT_IDS[input],
            buffer: BufferArgumentV1::new(
                ScalarType::U16,
                AccessMode::ReadOnly,
                2,
                raw,
                init,
                TARGET,
            )
            .unwrap(),
        });
    }
    let mut output = Vec::with_capacity(272);
    for _ in 0..68 {
        output.extend_from_slice(&CANARY.to_le_bytes());
    }
    shared.push(SharedBufferV1 {
        id: INPUT_IDS[2],
        buffer: BufferArgumentV1::new(
            ScalarType::F32,
            AccessMode::ReadWrite,
            4,
            output,
            vec![true; 272],
            TARGET,
        )
        .unwrap(),
    });
    let mut arguments = Vec::with_capacity(4);
    for (index, ty, access, align, offset, elements) in [
        (0, ScalarType::U16, AccessMode::ReadOnly, 2, 0, 256),
        (1, ScalarType::U16, AccessMode::ReadOnly, 2, 0, 256),
        (2, ScalarType::F32, AccessMode::ReadWrite, 4, 8, length),
    ] {
        arguments.push(SimulationArgumentV1::BufferView(
            BufferViewArgumentV1::new(
                INPUT_IDS[index],
                ty,
                access,
                align,
                offset,
                elements,
                TARGET,
            )
            .unwrap(),
        ));
    }
    arguments.push(SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0)));
    let grid = match control {
        Control::Grid63 => 63,
        Control::Grid65 => 65,
        _ => 64,
    };
    let wave = if matches!(control, Control::Wave32) {
        32
    } else {
        64
    };
    let mut request =
        SimulationRequestV1::new(kernel.clone(), [grid, 1, 1], [wave, 1, 1], arguments)
            .with_shared_buffers(shared);
    if matches!(control, Control::EventFailure) {
        request.events = EventPolicyV1::Enabled;
    }
    request
}
pub(super) fn check_output(run: &SimulationExecutionV1, pattern: usize, length: usize) {
    assert_eq!(run.invocations_executed(), 64);
    assert_eq!(run.workgroups_visited(), 1);
    assert_eq!(run.shared_buffers().len(), 3);
    for input in 0..2 {
        let buffer = run.shared_buffer(INPUT_IDS[input]).unwrap();
        assert_eq!(buffer.bytes(), bytes(pattern, input));
        assert!(buffer.initialized().iter().all(|value| *value));
    }
    let buffer = run.shared_buffer(INPUT_IDS[2]).unwrap();
    assert_eq!(buffer.bytes().len(), 272);
    assert!(buffer.initialized().iter().all(|value| *value));
    let expected = expected(pattern);
    for (slot, raw) in buffer.bytes().chunks_exact(4).enumerate() {
        let bits = u32::from_le_bytes(raw.try_into().unwrap());
        let wanted = if (2..2 + length).contains(&slot) {
            lane_word(&expected, slot - 2, 0)
        } else {
            CANARY
        };
        assert_eq!(
            bits, wanted,
            "source-authored component-zero sink or canary differs"
        );
    }
    assert!(!run.grants_execution_authority());
}
#[test]
fn exact_dense_oracle_and_lane_projection_are_independent() {
    assert_eq!(integer_bits(0), 0);
    assert_eq!(integer_bits(4096), 0x4580_0000);
    assert_eq!(integer_bits(-4096), 0xc580_0000);
    let (_, b) = dense(1);
    for (actual, value) in expected(1).0.iter().zip(b) {
        assert_eq!(*actual, integer_bits(value));
    }
    assert!(expected(5).0.iter().all(|word| *word == 0));
    let mut seen = [false; 256];
    for lane in 0..64 {
        for component in 0..4 {
            let index = (4 * (lane / 16) + component) * 16 + lane % 16;
            assert!(!seen[index]);
            seen[index] = true;
        }
    }
    assert!(seen.into_iter().all(|value| value));
}
#[test]
fn closed_source_cases_fit_original_attempt_and_integer_bounds() {
    assert_eq!(PATTERNS * LENGTHS.len(), 18);
    assert!(PATTERNS * LENGTHS.len() + NEGATIVES.len() <= 64);
    for pattern in 0..PATTERNS {
        let (a, b) = dense(pattern);
        assert!(
            a.into_iter()
                .chain(b)
                .all(|value| (-16..=16).contains(&value))
        );
    }
}
#[test]
fn integer_encoder_and_every_output_tail_boundary_have_independent_controls() {
    for value in -4096..=4096 {
        assert_eq!(integer_bits(value), (value as f32).to_bits());
    }
    for pattern in 0..PATTERNS {
        for length in LENGTHS {
            let output = expected_output(pattern, length);
            assert_eq!(
                &output.0[..8],
                &[CANARY.to_le_bytes(), CANARY.to_le_bytes()].concat()
            );
            assert_eq!(
                &output.0[264..],
                &[CANARY.to_le_bytes(), CANARY.to_le_bytes()].concat()
            );
            for lane in length..64 {
                assert_eq!(
                    &output.0[8 + lane * 4..12 + lane * 4],
                    &CANARY.to_le_bytes()
                );
            }
        }
    }
}
