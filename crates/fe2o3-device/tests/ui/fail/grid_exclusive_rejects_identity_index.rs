use fe2o3_device::{DisjointSlice, GridExclusive, thread};

fn identity_is_not_exclusive(mut output: DisjointSlice<u32, GridExclusive>) {
    let index = thread::index_1d();
    let _ = output.get_mut(index);
}

fn main() {}
