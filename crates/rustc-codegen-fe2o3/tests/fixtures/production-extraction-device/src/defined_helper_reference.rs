use fe2o3_device::{DisjointSlice, kernel, thread};

#[inline(never)]
fn leaf(left: u32, right: u32) -> u32 {
    left & !right
}

#[cfg(feature = "defined-helper-reference-nested")]
#[inline(never)]
fn outer(left: u32, right: u32) -> u32 {
    leaf(right, left) ^ left
}

#[cfg(feature = "defined-helper-reference-alternate")]
#[inline(never)]
fn alternate(left: u32, right: u32) -> u32 {
    left | !right
}

fn reference(_point: usize, left: u32, right: u32, output: &mut u32) {
    #[cfg(feature = "defined-helper-reference-nested")]
    {
        *output = (right & !left) ^ left;
    }
    #[cfg(not(feature = "defined-helper-reference-nested"))]
    {
        *output = left & !right;
    }
}

#[kernel(
    typed,
    reference = reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn helper_reference(left: u32, right: u32, mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        #[cfg(feature = "defined-helper-reference-nested")]
        {
            *element = outer(left, right);
        }
        #[cfg(feature = "defined-helper-reference-swapped")]
        {
            *element = leaf(right, left);
        }
        #[cfg(feature = "defined-helper-reference-alternate")]
        {
            *element = alternate(left, right);
        }
        #[cfg(not(any(
            feature = "defined-helper-reference-nested",
            feature = "defined-helper-reference-swapped",
            feature = "defined-helper-reference-alternate",
        )))]
        {
            *element = leaf(left, right);
        }
    }
}
