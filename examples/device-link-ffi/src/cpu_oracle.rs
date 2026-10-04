/// Independent CPU specification for the bidirectional fixture.
///
/// The source model computes `rust_accumulate(external_affine(value), lane)`
/// only where both input and output extents contain the lane. This oracle
/// intentionally does not call either boundary emulation.
pub fn evaluate(input: &[u32], output_len: usize, untouched: u32) -> Vec<u32> {
    let mut output = vec![untouched; output_len];
    for (lane, value) in input.iter().take(output_len).enumerate() {
        output[lane] = value
            .wrapping_mul(3)
            .wrapping_add(5)
            .wrapping_add(lane as u32);
    }
    output
}
