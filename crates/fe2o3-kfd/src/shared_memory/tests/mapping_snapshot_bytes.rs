//! Lossless test observations of mostly-zero, full-size context-save mappings.

const CHUNK_BYTES: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MappingBytesV1 {
    len: usize,
    nonzero_chunks: Vec<usize>,
    payload: Vec<u8>,
}

impl MappingBytesV1 {
    pub(super) fn capture(bytes: &[u8]) -> Self {
        const ZERO_CHUNK: [u8; CHUNK_BYTES] = [0; CHUNK_BYTES];
        let mut result = Self {
            len: bytes.len(),
            nonzero_chunks: Vec::new(),
            payload: Vec::new(),
        };
        // Zero chunks are omitted only after a full comparison; every byte of a
        // nonzero chunk is copied. Sorted indices and exact tail lengths make
        // this representation canonical, so derived equality is full equality.
        for (index, chunk) in bytes.chunks(CHUNK_BYTES).enumerate() {
            if chunk != &ZERO_CHUNK[..chunk.len()] {
                result.nonzero_chunks.push(index);
                result.payload.extend_from_slice(chunk);
            }
        }
        result
    }

    pub(super) fn len(&self) -> usize {
        self.len
    }

    pub(super) fn to_dense(&self) -> Vec<u8> {
        let mut result = vec![0; self.len];
        let mut consumed = 0;
        for &index in &self.nonzero_chunks {
            let start = index * CHUNK_BYTES;
            let count = CHUNK_BYTES.min(self.len - start);
            result[start..start + count].copy_from_slice(&self.payload[consumed..consumed + count]);
            consumed += count;
        }
        assert_eq!(consumed, self.payload.len());
        result
    }
}

#[test]
fn compact_mapping_snapshot_matches_dense_oracle_for_all_chunk_shapes() {
    for len in [0_usize, 1, 4095, 4096, 4097, 8192, 8193, 16387] {
        for pattern in 0..4 {
            let bytes = (0..len)
                .map(|index| match pattern {
                    0 => 0,
                    1 => 0xff,
                    2 => (index.wrapping_mul(37).wrapping_add(11) % 251) as u8,
                    _ => u8::from(index == 0 || index == 4096 || index + 1 == len),
                })
                .collect::<Vec<_>>();
            let snapshot = MappingBytesV1::capture(&bytes);
            assert_eq!(snapshot.len(), bytes.len());
            assert_eq!(snapshot.to_dense(), bytes);
            assert_eq!(snapshot.clone(), MappingBytesV1::capture(&bytes));
            assert!(
                snapshot
                    .nonzero_chunks
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
            );
        }
    }
}

#[test]
fn compact_mapping_snapshot_equality_matches_dense_pairwise_oracle() {
    let inputs = [
        Vec::new(),
        vec![0],
        vec![1],
        vec![0; 4096],
        vec![0; 4097],
        vec![1; 4096],
        vec![1; 4097],
        vec![0; 8193],
        (0..8193).map(|index| u8::from(index == 4096)).collect(),
        (0..8193).map(|index| u8::from(index == 4097)).collect(),
        (0..8193).map(|index| u8::from(index == 8192)).collect(),
    ];
    let snapshots = inputs
        .iter()
        .map(|bytes| MappingBytesV1::capture(bytes))
        .collect::<Vec<_>>();
    for (left, left_snapshot) in inputs.iter().zip(&snapshots) {
        for (right, right_snapshot) in inputs.iter().zip(&snapshots) {
            assert_eq!(left_snapshot == right_snapshot, left == right);
        }
    }
}

#[test]
fn compact_mapping_snapshot_detects_every_interior_and_tail_byte_change() {
    let mut bytes = vec![0; CHUNK_BYTES * 2 + 1];
    bytes[0] = 11;
    bytes[CHUNK_BYTES] = 23;
    let before = MappingBytesV1::capture(&bytes);
    for index in 0..bytes.len() {
        bytes[index] ^= 0x5a;
        let changed = MappingBytesV1::capture(&bytes);
        assert_ne!(changed, before, "changed byte {index}");
        assert_eq!(changed.to_dense(), bytes);
        bytes[index] ^= 0x5a;
    }
    assert_eq!(MappingBytesV1::capture(&bytes), before);
    bytes.pop();
    assert_ne!(MappingBytesV1::capture(&bytes), before);
}

#[test]
fn compact_mapping_snapshot_is_immutable_after_source_changes() {
    let mut bytes = vec![0; CHUNK_BYTES + 1];
    bytes[CHUNK_BYTES] = 19;
    let original = bytes.clone();
    let snapshot = MappingBytesV1::capture(&bytes);
    bytes.fill(0xff);
    drop(bytes);
    assert_eq!(snapshot.to_dense(), original);
}

#[test]
fn compact_mapping_snapshot_preserves_actual_cwsr_headers_without_dense_storage() {
    use crate::queue::submit::{
        GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1, GFX942_CWSR_TOTAL_BYTES_V1, GFX942_CWSR_XCC_COUNT_V1,
        initialize_gfx942_cwsr_headers,
    };
    use fe2o3_kfd_uapi::{KfdQueueExceptionPayloadAddressV1, KfdSignalEventIdV1};

    let mut bytes = vec![0; GFX942_CWSR_TOTAL_BYTES_V1];
    initialize_gfx942_cwsr_headers(
        &mut bytes,
        KfdQueueExceptionPayloadAddressV1::new(0x1000).unwrap(),
        KfdSignalEventIdV1::new(7).unwrap(),
    )
    .unwrap();
    let snapshot = MappingBytesV1::capture(&bytes);
    assert_eq!(snapshot.len(), GFX942_CWSR_TOTAL_BYTES_V1);
    assert_eq!(
        snapshot.nonzero_chunks,
        (0..GFX942_CWSR_XCC_COUNT_V1)
            .map(|xcc| xcc * GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1 / CHUNK_BYTES)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        snapshot.payload.len(),
        GFX942_CWSR_XCC_COUNT_V1 * CHUNK_BYTES
    );
    assert_eq!(snapshot.to_dense(), bytes);
    for index in [
        0,
        CHUNK_BYTES - 1,
        CHUNK_BYTES,
        GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1 / 2,
        GFX942_CWSR_TOTAL_BYTES_V1 - 1,
    ] {
        bytes[index] ^= 0xa5;
        assert_ne!(
            MappingBytesV1::capture(&bytes),
            snapshot,
            "changed byte {index}"
        );
        bytes[index] ^= 0xa5;
    }
    assert_eq!(MappingBytesV1::capture(&bytes), snapshot);
}
