use super::*;
use std::sync::{Arc, Mutex, OnceLock};

struct Fixture {
    inputs: [Vec<u8>; 7],
    caches: [Vec<u8>; 2],
    norm: Vec<u8>,
    weight: Vec<u8>,
}
fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let mut inputs = core::array::from_fn(|role| vec![0; v5::EXTENTS[role]]);
        for page in 0..144u32 {
            inputs[5][(page as usize + 1) * 4..(page as usize + 2) * 4]
                .copy_from_slice(&page.to_le_bytes());
        }
        Fixture {
            inputs,
            caches: core::array::from_fn(|_| 0x7fc0u16.to_le_bytes().repeat(v5::CACHE_WORDS)),
            norm: vec![0; 8192],
            weight: vec![0; 50_331_648],
        }
    })
}

fn stage_metadata(kind: Kind, digest: [u8; 32]) -> KernelMetadataV1 {
    let pointers = bindings(kind, 0);
    KernelMetadataV1 {
        symbol: kind.entry().into(),
        object_sha256: digest,
        kernarg_bytes: kind.total() as u32,
        kernarg_alignment: 8,
        group_segment_bytes: 0,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(kind.hidden()),
        implicit_argument_bytes: 256,
        explicit_arguments: (0..kind.slices() * 2 + kind.scalars())
            .map(|index| {
                let pair = index < kind.slices() * 2;
                let pointer = pair && index % 2 == 0;
                ExplicitArgumentV1 {
                    offset: if pair {
                        index as u32 * 8
                    } else {
                        kind.slices() as u32 * 16 + (index - kind.slices() * 2) as u32 * 4
                    },
                    bytes: if pair { 8 } else { 4 },
                    global_buffer: pointer,
                    pointee_alignment: pointer.then_some(if kind == Kind::Down && index == 4 {
                        4
                    } else {
                        2
                    }),
                    access: pointer.then(|| pointers[index / 2].access),
                }
            })
            .collect(),
    }
}

fn prefix_metadata(producer: bool, digest: [u8; 32]) -> KernelMetadataV1 {
    KernelMetadataV1 {
        symbol: if producer {
            "ferric_qwen3_claimed_rmsnorm_qkv_attention_output_bf16_f32_v5"
        } else {
            "ferric_qwen3_tp_peer_tp2_ordered_residual_bf16_v18"
        }
        .into(),
        object_sha256: digest,
        kernarg_bytes: if producer { 376 } else { 424 },
        kernarg_alignment: 8,
        group_segment_bytes: if producer { 512 } else { 0 },
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(if producer { 120 } else { 168 }),
        implicit_argument_bytes: 256,
        explicit_arguments: if producer {
            (0..15)
                .map(|role| ExplicitArgumentV1 {
                    offset: role * 8,
                    bytes: 8,
                    global_buffer: true,
                    pointee_alignment: Some(if matches!(role, 4 | 5 | 13 | 14) {
                        4
                    } else {
                        2
                    }),
                    access: Some(v5::role_access(role as usize)),
                })
                .collect()
        } else {
            (0..22)
                .map(|index| {
                    let pointer = index < 20 && index % 2 == 0;
                    ExplicitArgumentV1 {
                        offset: if index < 20 {
                            index * 8
                        } else {
                            160 + (index - 20) * 4
                        },
                        bytes: if index < 20 { 8 } else { 4 },
                        global_buffer: pointer,
                        pointee_alignment: pointer.then_some(if index < 16 { 4 } else { 2 }),
                        access: pointer.then_some(if index == 18 {
                            BufferAccessV1::Write
                        } else {
                            BufferAccessV1::Read
                        }),
                    }
                })
                .collect()
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Buffer {
    rank: usize,
    role: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    None,
    Load(Kind),
    Stage(Kind),
    Nonfinite(Kind),
    FirstConsumer,
    FinalConsumer,
    State,
    Cache,
    Weight,
    PriorOutput,
    FirstResidual,
    DownWrite,
    Replica,
    NormReplica,
    Close,
    Alias,
    ShortRead,
    MlpLoad,
    MlpMetadata,
    MlpStateAllocation,
    MlpFresh,
    MlpFreshBeforeDispatch,
    MlpStateRead,
    MlpIncomplete,
    MlpOwner,
    MlpDispatch,
    MlpRead,
    MlpChangedAfterFinal,
}
struct Fake {
    fault: Fault,
    events: Arc<Mutex<Vec<String>>>,
    produced: bool,
    first: bool,
    stages: usize,
    final_done: bool,
}
impl Fake {
    fn event(&self, value: impl Into<String>) {
        self.events.lock().unwrap().push(value.into());
    }
}

impl prefix::Transaction for Fake {
    type Buffer = Buffer;
    type State = usize;
    fn load(
        &mut self,
        _rank: usize,
        producer: bool,
        _object: Vec<u8>,
        digest: [u8; 32],
    ) -> Result<KernelMetadataV1> {
        Ok(prefix_metadata(producer, digest))
    }
    fn allocate(&mut self, rank: usize, role: usize, bytes: usize, peer: bool) -> Result<Buffer> {
        assert_eq!(peer, role == 13 || role == 24);
        assert_eq!(
            bytes,
            if role < 14 {
                v5::EXTENTS[role]
            } else if role == 15 {
                8192
            } else {
                EXTENTS[role - 16]
            }
        );
        self.event(format!("allocate:{rank}:{role}:{peer}"));
        Ok(Buffer {
            rank,
            role: if self.fault == Fault::Alias && role == 24 {
                13
            } else {
                role
            },
        })
    }
    fn state(&mut self, rank: usize) -> Result<usize> {
        Ok(rank)
    }
    fn observe(&mut self, rank: &usize) -> Result<[u32; 22]> {
        if !self.produced {
            return Ok(core::array::from_fn(|index| u32::from(index < 2)));
        }
        let mut state = [64; 22];
        state[..6].copy_from_slice(&[1, 0, 65535, 65535, 0x55555555, 0]);
        if self.fault == Fault::State && *rank == 1 && self.stages > 0 {
            state[0] = 2;
        }
        Ok(state)
    }
    fn write(&mut self, buffer: Buffer, offset: usize, bytes: &[u8]) -> Result<()> {
        assert!(
            !self.produced,
            "no resident intermediate may be read back then uploaded"
        );
        assert!(bytes.len() <= MAX_TRANSFER_BYTES_V1 as usize);
        self.event(format!(
            "write:{}:{}:{offset}:{}",
            buffer.rank,
            buffer.role,
            bytes.len()
        ));
        Ok(())
    }
    fn read(&mut self, buffer: Buffer, offset: usize, count: usize) -> Result<Vec<u8>> {
        assert!(self.produced && count <= MAX_TRANSFER_BYTES_V1 as usize);
        if self.fault == Fault::MlpRead && buffer.role == 23 {
            return Err("MLP output read fault".into());
        }
        let f = fixture();
        let mut bytes = if buffer.role < 7 {
            f.inputs[buffer.role][offset..offset + count].to_vec()
        } else if matches!(buffer.role, 10 | 11) {
            let mut value = f.caches[buffer.role - 10][offset..offset + count].to_vec();
            for (index, byte) in value.iter_mut().enumerate() {
                if offset + index < 1024 {
                    *byte = 0;
                }
            }
            value
        } else if buffer.role == 16 {
            f.norm[offset..offset + count].to_vec()
        } else if (17..=19).contains(&buffer.role) {
            f.weight[offset..offset + count].to_vec()
        } else if matches!(buffer.role, 13 | 24) {
            if buffer.role == 24 {
                assert_eq!(self.stages, 5);
            }
            let word = (buffer.rank as f32 + if buffer.role == 13 { 1.0 } else { 5.0 })
                .to_bits()
                .to_le_bytes();
            (offset..offset + count)
                .map(|index| word[index % 4])
                .collect()
        } else {
            let word = match buffer.role {
                15 => {
                    assert!(self.first);
                    0x4040u16
                }
                20..=23 => {
                    assert!(self.stages >= buffer.role - 19);
                    0x3f80 + (buffer.role as u16 - 20) * 128
                }
                25 => {
                    assert!(self.final_done);
                    0x4160
                }
                _ => 0,
            }
            .to_le_bytes();
            (offset..offset + count)
                .map(|index| word[index % 2])
                .collect()
        };
        if offset == 0 {
            match self.fault {
                Fault::Nonfinite(kind) if buffer.role == kind.output() + 16 && self.stages > 0 => {
                    if kind == Kind::Down {
                        bytes[..4].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
                    } else {
                        bytes[..2].copy_from_slice(&0x7f80u16.to_le_bytes());
                    }
                }
                Fault::Cache if buffer.role == 10 && self.stages > 0 => bytes[1024] ^= 1,
                Fault::Weight if buffer.role == 18 && self.stages > 0 => bytes[0] ^= 1,
                Fault::PriorOutput if buffer.role == 21 && self.stages >= 3 => bytes[0] ^= 1,
                Fault::FirstResidual if buffer.role == 15 && self.stages > 0 => bytes[0] ^= 1,
                Fault::DownWrite if buffer.role == 24 && self.final_done => bytes[0] ^= 1,
                Fault::Replica if buffer.role == 25 && buffer.rank == 1 => bytes[0] ^= 1,
                Fault::NormReplica if buffer.role == 20 && buffer.rank == 1 => bytes[0] ^= 1,
                Fault::ShortRead if buffer.role == 23 => {
                    bytes.pop();
                }
                _ => (),
            }
        }
        Ok(bytes)
    }
    unsafe fn producers(
        &mut self,
        roots: &[[Buffer; 14]; 2],
        states: &[usize; 2],
        _timeout: u32,
    ) -> Result<[u64; 2]> {
        assert_eq!(*states, [0, 1]);
        for rank in 0..2 {
            for role in 0..14 {
                assert_eq!(roots[rank][role], Buffer { rank, role });
            }
        }
        self.event("producers");
        self.produced = true;
        Ok([11, 12])
    }
    unsafe fn consumers(
        &mut self,
        roots: &[[Buffer; 14]; 2],
        outputs: &[Buffer; 2],
        _timeout: u32,
    ) -> Result<[u64; 2]> {
        assert!(self.produced);
        for rank in 0..2 {
            assert_eq!(roots[rank][13], Buffer { rank, role: 13 });
            assert_eq!(outputs[rank], Buffer { rank, role: 15 });
        }
        self.event("first");
        if self.fault == Fault::FirstConsumer {
            return Err("first consumer fault".into());
        }
        self.first = true;
        Ok([21, 22])
    }
    fn close(&mut self) -> Result<()> {
        self.event("close");
        if self.fault == Fault::Close {
            Err("close fault".into())
        } else {
            Ok(())
        }
    }
    fn quarantine(self) {
        self.event("quarantine");
    }
}

impl Transaction for Fake {
    type MlpState = usize;
    fn load_mlp_worker(
        &mut self,
        rank: usize,
        _object: Vec<u8>,
        digest: [u8; 32],
    ) -> Result<KernelMetadataV1> {
        self.event(format!("mlp:load:{rank}"));
        if self.fault == Fault::MlpLoad {
            return Err("MLP load fault".into());
        }
        let mut metadata = mlp_worker_tests::metadata(digest);
        if self.fault == Fault::MlpMetadata {
            metadata.kernarg_bytes = 376;
        }
        Ok(metadata)
    }
    fn mlp_state(&mut self, rank: usize) -> Result<usize> {
        self.event(format!("mlp:state:{rank}"));
        if self.fault == Fault::MlpStateAllocation {
            return Err("MLP state allocation fault".into());
        }
        Ok(rank)
    }
    fn observe_mlp(&mut self, rank: &usize) -> Result<[u32; 11]> {
        if self.fault == Fault::MlpStateRead && self.stages > 0 {
            return Err("MLP acquire observation fault".into());
        }
        if self.stages == 0 {
            let mut state = [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            if self.fault == Fault::MlpFresh
                || (self.fault == Fault::MlpFreshBeforeDispatch && self.produced)
            {
                state[10] = 1;
            }
            return Ok(state);
        }
        let mut state = [1, 0, 31, 31, 0x155, 0, 64, 64, 64, 64, 64];
        if *rank == 1 {
            match self.fault {
                Fault::MlpIncomplete => state[10] = 63,
                Fault::MlpOwner => state[4] |= 3,
                Fault::MlpChangedAfterFinal if self.final_done => state[0] = 2,
                _ => (),
            }
        }
        Ok(state)
    }
    unsafe fn mlp_round(
        &mut self,
        residual: &[Buffer; 2],
        suffix: &[[Buffer; 10]; 2],
        states: &[usize; 2],
        _timeout: u32,
    ) -> Result<[u64; 2]> {
        assert!(self.first && !self.final_done && self.stages == 0);
        assert_eq!(*states, [0, 1]);
        for rank in 0..2 {
            assert_eq!(residual[rank], Buffer { rank, role: 15 });
            for role in 0..10 {
                assert_eq!(
                    suffix[rank][role],
                    Buffer {
                        rank,
                        role: 16 + role
                    }
                );
            }
        }
        self.event("mlp:dispatch");
        if self.fault == Fault::MlpDispatch {
            return Err("MLP dispatch fault".into());
        }
        self.stages = 5;
        Ok([91, 92])
    }
    fn load_stage(
        &mut self,
        _rank: usize,
        kind: Kind,
        _object: Vec<u8>,
        digest: [u8; 32],
    ) -> Result<KernelMetadataV1> {
        if self.fault == Fault::Load(kind) {
            return Err("stage load fault".into());
        }
        Ok(stage_metadata(kind, digest))
    }
    unsafe fn stage_round(
        &mut self,
        kind: Kind,
        residual: &[Buffer; 2],
        suffix: &[[Buffer; 10]; 2],
        _timeout: u32,
    ) -> Result<[u64; 2]> {
        assert!(self.first && !self.final_done);
        assert_eq!(kind, STAGES[self.stages]);
        for rank in 0..2 {
            assert_eq!(residual[rank], Buffer { rank, role: 15 });
            for role in 0..10 {
                assert_eq!(
                    suffix[rank][role],
                    Buffer {
                        rank,
                        role: role + 16
                    }
                );
            }
        }
        self.event(format!("stage:{kind:?}"));
        if self.fault == Fault::Stage(kind) {
            return Err("stage dispatch fault".into());
        }
        self.stages += 1;
        Ok([self.stages as u64 * 10 + 21, self.stages as u64 * 10 + 22])
    }
    unsafe fn final_round(
        &mut self,
        residual: &[Buffer; 2],
        suffix: &[[Buffer; 10]; 2],
        _timeout: u32,
    ) -> Result<[u64; 2]> {
        assert_eq!(self.stages, 5);
        assert!(self.first);
        for rank in 0..2 {
            assert_eq!(residual[rank], Buffer { rank, role: 15 });
            assert_eq!(suffix[rank][8], Buffer { rank, role: 24 });
        }
        self.event("final");
        if self.fault == Fault::FinalConsumer {
            return Err("final consumer fault".into());
        }
        self.final_done = true;
        Ok([81, 82])
    }
}

#[path = "engineering_gfx950_resident_layer_mlp_worker_tp2_v1_tests.rs"]
mod mlp_worker_tests;

fn run(fault: Fault) -> (Result<Gfx950EngineeringResidentLayerResultV1>, Vec<String>) {
    let f = fixture();
    let inputs = f.inputs.each_ref().map(Vec::as_slice);
    let caches = f.caches.each_ref().map(Vec::as_slice);
    let weights = [
        f.norm.as_slice(),
        f.weight.as_slice(),
        f.weight.as_slice(),
        f.weight.as_slice(),
    ];
    let events = Arc::new(Mutex::new(Vec::new()));
    let fake = Fake {
        fault,
        events: events.clone(),
        produced: false,
        first: false,
        stages: 0,
        final_done: false,
    };
    // SAFETY: mock-only orchestration, no native device or dispatch terminal.
    let result = unsafe {
        coordinate(
            fake,
            vec![1],
            [1; 32],
            vec![2],
            [2; 32],
            vec![3],
            [3; 32],
            vec![4],
            [4; 32],
            [inputs; 2],
            [caches; 2],
            [weights; 2],
            10,
        )
    };
    let events = events.lock().unwrap().clone();
    (result, events)
}

#[test]
fn stage_abi_rejects_every_field_and_resource_mutation() {
    for kind in STAGES {
        let good = stage_metadata(kind, [7; 32]);
        assert!(validate_metadata(&good, [7; 32], kind).is_ok());
        for index in 0..good.explicit_arguments.len() {
            let mut bad = good.clone();
            bad.explicit_arguments[index].offset += 4;
            assert!(validate_metadata(&bad, [7; 32], kind).is_err());
            let mut bad = good.clone();
            bad.explicit_arguments[index].bytes = 1;
            assert!(validate_metadata(&bad, [7; 32], kind).is_err());
            let mut bad = good.clone();
            bad.explicit_arguments[index].global_buffer ^= true;
            assert!(validate_metadata(&bad, [7; 32], kind).is_err());
        }
        for slot in 0..kind.slices() {
            let mut bad = good.clone();
            bad.explicit_arguments[slot * 2].access = Some(BufferAccessV1::ReadWrite);
            assert!(validate_metadata(&bad, [7; 32], kind).is_err());
            let mut bad = good.clone();
            bad.explicit_arguments[slot * 2].pointee_alignment = Some(8);
            assert!(validate_metadata(&bad, [7; 32], kind).is_err());
        }
        for field in 0..7 {
            let mut bad = good.clone();
            match field {
                0 => bad.kernarg_bytes += 8,
                1 => bad.kernarg_alignment = 16,
                2 => bad.implicit_argument_offset = Some(0),
                3 => bad.implicit_argument_bytes = 0,
                4 => bad.wavefront_size = 32,
                5 => bad.private_segment_bytes = 4,
                _ => bad.group_segment_bytes = 512,
            }
            assert!(validate_metadata(&bad, [7; 32], kind).is_err());
        }
        let mut bad = good.clone();
        bad.symbol.push('x');
        assert!(validate_metadata(&bad, [7; 32], kind).is_err());
        assert!(validate_metadata(&good, [8; 32], kind).is_err());
    }
}

#[test]
fn closed_payloads_preserve_dual_round_norm_and_exact_projection_shapes() {
    for kind in STAGES {
        let bytes = stage_bytes(kind);
        assert_eq!(bytes.len(), kind.total());
        for pointer in bindings(kind, 0) {
            assert_eq!(
                &bytes[pointer.offset as usize..pointer.offset as usize + 8],
                &[0; 8]
            );
        }
        assert!(
            bytes[kind.hidden() as usize..]
                .iter()
                .all(|byte| *byte == 0)
        );
    }
    let norm = stage_bytes(Kind::Norm);
    for (slot, count) in [4096, 0, 4096, 0, 4096].into_iter().enumerate() {
        assert_eq!(
            u64::from_le_bytes(norm[slot * 16 + 8..slot * 16 + 16].try_into().unwrap()),
            count
        );
    }
    assert_eq!(u32::from_le_bytes(norm[80..84].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(norm[84..88].try_into().unwrap()), 4096);
    assert_eq!(
        u32::from_le_bytes(norm[88..92].try_into().unwrap()),
        1e-6f32.to_bits()
    );
    assert_eq!(u32::from_le_bytes(norm[92..96].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(norm[24..32].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(norm[56..64].try_into().unwrap()), 0);
    for (kind, expected) in [
        (Kind::Gate, [1, 6144, 4096, 2, 4]),
        (Kind::Up, [1, 6144, 4096, 2, 5]),
        (Kind::Down, [1, 4096, 6144, 2, 2]),
    ] {
        let bytes = stage_bytes(kind);
        assert_eq!(&bytes[68..72], &[0; 4]);
        for (index, value) in expected.into_iter().enumerate() {
            assert_eq!(
                u32::from_le_bytes(bytes[48 + index * 4..52 + index * 4].try_into().unwrap()),
                value
            );
        }
        assert_eq!(
            u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
            WEIGHT_WORDS as u64
        );
        assert_eq!(
            u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            if kind == Kind::Down { 4096 } else { 6144 }
        );
    }
    assert_eq!(Kind::Gate.grid(), [6144 * 64, 1, 1]);
    assert_eq!(Kind::Down.grid(), [4096 * 64, 1, 1]);
    assert_eq!(Kind::Activation.grid(), [6144, 1, 1]);
    assert_eq!(Kind::Norm.grid(), [64, 1, 1]);
    let activation = stage_bytes(Kind::Activation);
    for slot in 0..3 {
        assert_eq!(
            u64::from_le_bytes(
                activation[slot * 16 + 8..slot * 16 + 16]
                    .try_into()
                    .unwrap()
            ),
            6144
        );
    }
    assert_eq!(
        u32::from_le_bytes(activation[48..52].try_into().unwrap()),
        1
    );
    assert_eq!(
        u32::from_le_bytes(activation[52..56].try_into().unwrap()),
        2
    );
}

#[test]
fn bindings_use_same_first_residual_and_original_down_allocations() {
    for rank in 0..2 {
        let norm = bindings(Kind::Norm, rank);
        let final_ = final_bindings(rank);
        assert_eq!((norm[0].role, norm[0].rank), (15, rank));
        assert_eq!((final_[8].role, final_[8].rank), (15, rank));
        assert_eq!(bindings(Kind::Down, rank)[2].role, 24);
        for (slot, value) in final_.iter().enumerate() {
            assert_eq!(value.offset, slot as u32 * 16);
            if slot < 2 {
                assert_eq!((value.rank, value.role, value.extent), (slot, 24, 16384));
            } else if slot < 8 {
                assert_eq!((value.rank, value.role, value.extent), (0, 24, 0));
            } else {
                assert_eq!(
                    (value.rank, value.role, value.extent),
                    (rank, if slot == 8 { 15 } else { 25 }, 8192)
                );
            }
            assert_eq!(
                value.access,
                if slot == 9 {
                    BufferAccessV1::Write
                } else {
                    BufferAccessV1::Read
                }
            );
        }
    }
}

#[test]
fn mock_full_layer_is_resident_ordered_and_publishes_only_after_close() {
    let (result, events) = run(Fault::None);
    let result = result.unwrap();
    assert_eq!(result.producer_dispatch_ns, [11, 12]);
    assert_eq!(result.first_residual_dispatch_ns, [21, 22]);
    assert_eq!(result.post_norm_dispatch_ns, [31, 32]);
    assert_eq!(result.down_dispatch_ns, [71, 72]);
    assert_eq!(result.final_residual_dispatch_ns, [81, 82]);
    assert_eq!(
        result.ranks[0].prefix.residual_words.as_ref(),
        &[0x4040; 4096]
    );
    assert_eq!(result.ranks[0].down_partial.as_ref(), &[5.0; 4096]);
    assert_eq!(result.ranks[1].down_partial.as_ref(), &[6.0; 4096]);
    assert_eq!(
        result.ranks[0].layer_output_words,
        result.ranks[1].layer_output_words
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| !event.starts_with("write:") && !event.starts_with("allocate:"))
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "producers",
            "first",
            "stage:Norm",
            "stage:Gate",
            "stage:Up",
            "stage:Activation",
            "stage:Down",
            "final",
            "close"
        ]
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.starts_with("allocate:") && event.ends_with(":true"))
            .count(),
        4
    );
    assert!(
        events
            .iter()
            .any(|event| event == "write:1:19:46137344:4194304")
    );
}

#[test]
fn every_stage_failure_stops_before_later_dispatch_and_quarantines() {
    for kind in STAGES {
        for fault in [Fault::Stage(kind), Fault::Nonfinite(kind)] {
            let (result, events) = run(fault);
            assert!(result.is_err(), "{fault:?}");
            assert!(
                !events
                    .iter()
                    .any(|event| event == "final" || event == "close")
            );
            assert_eq!(events.last().unwrap(), "quarantine");
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event.starts_with("stage:"))
                    .count(),
                STAGES.iter().position(|value| *value == kind).unwrap() + 1
            );
        }
    }
}

#[test]
fn prior_buffers_and_state_cannot_change_during_suffix() {
    for fault in [
        Fault::State,
        Fault::Cache,
        Fault::Weight,
        Fault::PriorOutput,
        Fault::FirstResidual,
        Fault::NormReplica,
        Fault::ShortRead,
        Fault::Alias,
        Fault::FirstConsumer,
    ] {
        let (result, events) = run(fault);
        assert!(result.is_err(), "{fault:?}");
        assert!(
            !events
                .iter()
                .any(|event| event == "final" || event == "close")
        );
        assert_eq!(events.last().unwrap(), "quarantine");
    }
}

#[test]
fn final_consumer_corruption_or_failed_close_never_returns_arrays() {
    for fault in [
        Fault::FinalConsumer,
        Fault::DownWrite,
        Fault::Replica,
        Fault::Close,
    ] {
        let (result, events) = run(fault);
        assert!(result.is_err(), "{fault:?}");
        assert!(events.iter().any(|event| event == "final"));
        assert_eq!(events.last().unwrap(), "quarantine");
        assert_eq!(
            events.iter().any(|event| event == "close"),
            fault == Fault::Close
        );
    }
}

#[test]
fn rejected_stage_image_prevents_all_allocations() {
    for kind in [Kind::Norm, Kind::Gate, Kind::Activation, Kind::Down] {
        let (result, events) = run(Fault::Load(kind));
        assert!(result.is_err());
        assert_eq!(events, ["quarantine"]);
    }
}

#[test]
fn exact_mlp_extents_and_shared_postnorm_are_checked() {
    let f = fixture();
    let good = [
        f.norm.as_slice(),
        f.weight.as_slice(),
        f.weight.as_slice(),
        f.weight.as_slice(),
    ];
    assert!(validate_weights(&[good; 2]).is_ok());
    for role in 0..4 {
        let mut bad = [good; 2];
        bad[1][role] = &bad[1][role][..bad[1][role].len() - 2];
        assert!(validate_weights(&bad).is_err());
    }
    let mut changed = f.norm.clone();
    changed[0] = 1;
    let mut bad = [good; 2];
    bad[1][0] = &changed;
    assert!(validate_weights(&bad).is_err());
}
