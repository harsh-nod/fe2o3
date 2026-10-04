use fe2o3_device::{DisjointIndex, DisjointSlice, memory};

fn witnessed_writes<IndexSpace>(
    source: &[u32],
    mut output: DisjointSlice<u32, IndexSpace>,
    index: DisjointIndex<IndexSpace>,
) {
    memory::volatile_store(&mut output, &index, 7);
    memory::copy_one_nonoverlapping(source, 0, &mut output, &index);
}

fn expert_copy<IndexSpace>(
    source: &[u32],
    mut output: DisjointSlice<u32, IndexSpace>,
    destination_index: usize,
    count: usize,
) {
    // SAFETY: This contract fixture stands in for an expert proof that the
    // selected destination range is exclusive across all GPU invocations.
    unsafe {
        memory::copy_nonoverlapping_unchecked(
            source,
            0,
            &mut output,
            destination_index,
            count,
        )
    };
}

fn main() {}
