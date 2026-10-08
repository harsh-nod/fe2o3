#![cfg_attr(target_arch = "amdgpu", no_std)]

use fe2o3_device::{WriteOnlyDisjointSlice, kernel, thread};

#[kernel(
    typed,
    reference = copy_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn copy_u32(input: &[u32], mut output: WriteOnlyDisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(value) = input.get(index.get() as usize) {
        let _ = output.write(index, *value);
    }
}

fn copy_reference(point: usize, input: &[u32], output: &mut u32) {
    if let Some(value) = input.get(point) {
        *output = *value;
    }
}

#[cfg(test)]
mod tests {
    use super::copy_reference;

    #[test]
    fn copies_input_values_instead_of_thread_indices() {
        let input = [u32::MAX, 17, 0, 0x8000_0000];
        for (point, expected) in input.into_iter().enumerate() {
            let mut output = 0xfeed_beef;
            copy_reference(point, &input, &mut output);
            assert_eq!(output, expected);
        }
    }

    #[test]
    fn absent_input_preserves_the_output_coordinate() {
        for point in [0, 63, 64, usize::MAX] {
            let mut output = 0xfeed_beef;
            copy_reference(point, &[], &mut output);
            assert_eq!(output, 0xfeed_beef);
        }
        let mut output = 0xfeed_beef;
        copy_reference(3, &[4, 5, 6], &mut output);
        assert_eq!(output, 0xfeed_beef);
    }
}
