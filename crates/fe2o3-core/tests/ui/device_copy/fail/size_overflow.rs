use fe2o3_core::DeviceCopy;

type Chunk = [u8; usize::MAX / 16];

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct SizeOverflow {
    field01: Chunk,
    field02: Chunk,
    field03: Chunk,
    field04: Chunk,
    field05: Chunk,
    field06: Chunk,
    field07: Chunk,
    field08: Chunk,
    field09: Chunk,
    field10: Chunk,
    field11: Chunk,
    field12: Chunk,
    field13: Chunk,
    field14: Chunk,
    field15: Chunk,
    field16: Chunk,
    field17: Chunk,
}

fn main() {}
