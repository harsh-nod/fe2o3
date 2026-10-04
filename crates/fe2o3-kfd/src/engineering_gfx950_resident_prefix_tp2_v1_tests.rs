use super::*;
use std::sync::{Arc, Mutex, OnceLock};

struct Fixture {
    inputs: [Vec<u8>; 7],
    caches: [Vec<u8>; 2],
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
        }
    })
}

fn metadata(producer: bool, digest: [u8; 32]) -> KernelMetadataV1 {
    let explicit_arguments = if producer {
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
    };
    KernelMetadataV1 {
        symbol: if producer { PRODUCER } else { CONSUMER }.into(),
        object_sha256: digest,
        kernarg_bytes: if producer { 376 } else { 424 },
        kernarg_alignment: 8,
        group_segment_bytes: if producer { 512 } else { 0 },
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(if producer { 120 } else { 168 }),
        implicit_argument_bytes: 256,
        explicit_arguments,
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
    Producer,
    Consumer,
    BadState,
    ReadOnly,
    CacheOutside,
    PartialNan,
    Bf16Nan,
    ConsumerPartialWrite,
    ConsumerStateWrite,
    ResidualMismatch,
    Close,
    Alias,
    ShortRead,
    Load,
}

struct Fake {
    fault: Fault,
    events: Arc<Mutex<Vec<String>>>,
    produced: bool,
    consumed: bool,
}
impl Fake {
    fn event(&self, event: impl Into<String>) {
        self.events.lock().unwrap().push(event.into());
    }
}
impl Transaction for Fake {
    type Buffer = Buffer;
    type State = usize;
    fn load(
        &mut self,
        rank: usize,
        producer: bool,
        _object: Vec<u8>,
        digest: [u8; 32],
    ) -> Result<KernelMetadataV1> {
        self.event(format!("load:{rank}:{producer}"));
        if self.fault == Fault::Load {
            return Err("load fault".into());
        }
        Ok(metadata(producer, digest))
    }
    fn allocate(&mut self, rank: usize, role: usize, bytes: usize, peer: bool) -> Result<Buffer> {
        assert_eq!(peer, role == 13);
        assert_eq!(
            bytes,
            if role == 15 {
                RESIDUAL_BYTES
            } else {
                v5::EXTENTS[role]
            }
        );
        self.event(format!("allocate:{rank}:{role}:{peer}"));
        Ok(if self.fault == Fault::Alias && role == 1 {
            Buffer { rank, role: 0 }
        } else {
            Buffer { rank, role }
        })
    }
    fn state(&mut self, rank: usize) -> Result<usize> {
        self.event(format!("state:{rank}"));
        Ok(rank)
    }
    fn observe(&mut self, rank: &usize) -> Result<[u32; 22]> {
        self.event(format!("observe:{rank}:{}", self.consumed));
        if !self.produced {
            return Ok(core::array::from_fn(|i| u32::from(i < 2)));
        }
        let mut state = [64; 22];
        state[..6].copy_from_slice(&[1, 0, 65535, 65535, 0x55555555, 0]);
        if self.fault == Fault::BadState && *rank == 1 {
            state[4] &= !(3 << 30);
        }
        if self.fault == Fault::ConsumerStateWrite && self.consumed {
            state[0] = 2;
        }
        Ok(state)
    }
    fn write(&mut self, buffer: Buffer, offset: usize, bytes: &[u8]) -> Result<()> {
        assert!(
            !self.produced,
            "resident buffers must never be re-uploaded after producers"
        );
        assert!(bytes.len() <= MAX_TRANSFER_BYTES_V1 as usize);
        let f = fixture();
        if buffer.role < 7 {
            assert_eq!(bytes, &f.inputs[buffer.role][offset..offset + bytes.len()]);
        }
        if buffer.role == 10 || buffer.role == 11 {
            assert_eq!(
                bytes,
                &f.caches[buffer.role - 10][offset..offset + bytes.len()]
            );
        }
        self.event(format!(
            "write:{}:{}:{offset}:{}",
            buffer.rank,
            buffer.role,
            bytes.len()
        ));
        Ok(())
    }
    fn read(&mut self, buffer: Buffer, offset: usize, count: usize) -> Result<Vec<u8>> {
        assert!(self.produced);
        assert!(count <= MAX_TRANSFER_BYTES_V1 as usize);
        self.event(format!(
            "read:{}:{}:{}:{offset}:{count}",
            buffer.rank, buffer.role, self.consumed
        ));
        let f = fixture();
        let mut out = if buffer.role < 7 {
            f.inputs[buffer.role][offset..offset + count].to_vec()
        } else if buffer.role == 10 || buffer.role == 11 {
            let mut out = f.caches[buffer.role - 10][offset..offset + count].to_vec();
            for (index, byte) in out.iter_mut().enumerate() {
                if offset + index < 1024 {
                    *byte = 0;
                }
            }
            out
        } else if buffer.role == 13 {
            let word = (buffer.rank as f32 + 1.0).to_bits().to_le_bytes();
            (offset..offset + count)
                .map(|index| word[index % 4])
                .collect()
        } else if buffer.role == 15 {
            assert!(self.consumed);
            (offset..offset + count)
                .map(|index| 0x4040u16.to_le_bytes()[index % 2])
                .collect()
        } else {
            vec![0; count]
        };
        if offset == 0 {
            match self.fault {
                Fault::ReadOnly if buffer.rank == 1 && buffer.role == 6 => out[0] ^= 1,
                Fault::CacheOutside if buffer.role == 10 => out[1024] ^= 1,
                Fault::PartialNan if buffer.role == 13 => {
                    out[..4].copy_from_slice(&f32::NAN.to_bits().to_le_bytes())
                }
                Fault::Bf16Nan if buffer.role == 12 => {
                    out[..2].copy_from_slice(&0x7f80u16.to_le_bytes())
                }
                Fault::ConsumerPartialWrite if self.consumed && buffer.role == 13 => out[0] ^= 1,
                Fault::ResidualMismatch if buffer.rank == 1 && buffer.role == 15 => out[0] ^= 1,
                Fault::ShortRead if buffer.role == 8 => {
                    out.pop();
                }
                _ => (),
            }
        }
        Ok(out)
    }
    unsafe fn producers(
        &mut self,
        roots: &[[Buffer; ROLES]; 2],
        states: &[usize; 2],
        _timeout: u32,
    ) -> Result<[u64; 2]> {
        assert_eq!(*states, [0, 1]);
        for rank in 0..2 {
            for role in 0..ROLES {
                assert_eq!(roots[rank][role], Buffer { rank, role });
            }
        }
        self.event("producers");
        if self.fault == Fault::Producer {
            return Err("producer fault".into());
        }
        self.produced = true;
        Ok([11, 12])
    }
    unsafe fn consumers(
        &mut self,
        roots: &[[Buffer; ROLES]; 2],
        outputs: &[Buffer; 2],
        _timeout: u32,
    ) -> Result<[u64; 2]> {
        assert!(self.produced);
        for rank in 0..2 {
            assert_eq!(roots[rank][13], Buffer { rank, role: 13 });
            assert_eq!(roots[rank][0], Buffer { rank, role: 0 });
            assert_eq!(outputs[rank], Buffer { rank, role: 15 });
        }
        self.event("consumers");
        if self.fault == Fault::Consumer {
            return Err("consumer fault".into());
        }
        self.consumed = true;
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

fn run(fault: Fault) -> (Result<Gfx950EngineeringResidentPrefixResultV1>, Vec<String>) {
    let f = fixture();
    let inputs = f.inputs.each_ref().map(Vec::as_slice);
    let caches = f.caches.each_ref().map(Vec::as_slice);
    let events = Arc::new(Mutex::new(Vec::new()));
    let fake = Fake {
        fault,
        events: events.clone(),
        produced: false,
        consumed: false,
    };
    // SAFETY: mock-only transaction never opens a device or calls native terminals.
    let result = unsafe {
        coordinate(
            fake,
            vec![1],
            [1; 32],
            vec![2],
            [2; 32],
            [inputs; 2],
            [caches; 2],
            10,
        )
    };
    let recorded = events.lock().unwrap().clone();
    (result, recorded)
}

#[test]
fn closed_request_rejects_identity_digest_and_timeout_before_open() {
    let bytes = [1];
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    assert!(validate_request([1, 2], &bytes, digest, &bytes, digest, 150_000).is_ok());
    for (ids, timeout) in [([0, 2], 1), ([1, 1], 1), ([1, 2], 0), ([1, 2], 150_001)] {
        assert!(validate_request(ids, &bytes, digest, &bytes, digest, timeout).is_err());
    }
    assert!(validate_request([1, 2], &bytes, [0; 32], &bytes, digest, 1).is_err());
}

#[test]
fn consumer_abi_checks_every_pointer_length_and_scalar_field() {
    let good = metadata(false, [7; 32]);
    assert!(validate_consumer(&good, [7; 32]).is_ok());
    for index in 0..22 {
        let mut bad = good.clone();
        bad.explicit_arguments[index].offset += 4;
        assert!(validate_consumer(&bad, [7; 32]).is_err());
        let mut bad = good.clone();
        bad.explicit_arguments[index].global_buffer ^= true;
        assert!(validate_consumer(&bad, [7; 32]).is_err());
        let mut bad = good.clone();
        bad.explicit_arguments[index].bytes = 1;
        assert!(validate_consumer(&bad, [7; 32]).is_err());
    }
    for index in (0..20).step_by(2) {
        let mut bad = good.clone();
        bad.explicit_arguments[index].access = Some(BufferAccessV1::ReadWrite);
        assert!(validate_consumer(&bad, [7; 32]).is_err());
        let mut bad = good.clone();
        bad.explicit_arguments[index].pointee_alignment = Some(8);
        assert!(validate_consumer(&bad, [7; 32]).is_err());
    }
    let mut bad = good.clone();
    bad.implicit_argument_offset = Some(120);
    assert!(validate_consumer(&bad, [7; 32]).is_err());
    let mut bad = good.clone();
    bad.private_segment_bytes = 4;
    assert!(validate_consumer(&bad, [7; 32]).is_err());
    let mut bad = good.clone();
    bad.group_segment_bytes = 512;
    assert!(validate_consumer(&bad, [7; 32]).is_err());
    let mut bad = good.clone();
    bad.wavefront_size = 32;
    assert!(validate_consumer(&bad, [7; 32]).is_err());
}

#[test]
fn payload_has_zero_pointer_placeholders_zero_fillers_and_original_residual_length() {
    let bytes = consumer_bytes();
    assert_eq!(bytes.len(), 424);
    for slot in 0..10 {
        assert_eq!(&bytes[slot * 16..slot * 16 + 8], &[0; 8]);
        let count = u64::from_le_bytes(bytes[slot * 16 + 8..slot * 16 + 16].try_into().unwrap());
        assert_eq!(
            count,
            if matches!(slot, 0 | 1 | 8 | 9) {
                4096
            } else {
                0
            }
        );
    }
    // The scalar is rows, not the 4096-element vector width or grid extent.
    assert_eq!(u32::from_le_bytes(bytes[160..164].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(bytes[164..168].try_into().unwrap()), 2);
    assert!(bytes[168..].iter().all(|byte| *byte == 0));
}

#[test]
fn consumers_bind_original_ordered_partials_and_owner_original_input_not_normalized() {
    for rank in 0..2 {
        let bindings = consumer_bindings(rank);
        for (slot, binding) in bindings.iter().enumerate() {
            assert_eq!(binding.offset, slot as u32 * 16);
            if slot < 2 {
                assert_eq!(
                    (binding.rank, binding.role, binding.extent),
                    (slot, 13, 16384)
                );
            } else if slot < 8 {
                assert_eq!((binding.rank, binding.role, binding.extent), (0, 13, 0));
            } else {
                assert_eq!(
                    (binding.rank, binding.role, binding.extent),
                    (rank, if slot == 8 { 0 } else { 15 }, 8192)
                );
            }
            assert_eq!(
                binding.access,
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
fn mock_resident_success_checks_both_stages_before_consumers_and_closes_before_return() {
    let (result, events) = run(Fault::None);
    let result = result.unwrap();
    assert_eq!(result.producer_dispatch_ns, [11, 12]);
    assert_eq!(result.consumer_dispatch_ns, [21, 22]);
    assert_eq!(result.ranks[0].output_partial.as_ref(), &[1.0; 4096]);
    assert_eq!(result.ranks[1].output_partial.as_ref(), &[2.0; 4096]);
    assert_eq!(result.ranks[0].residual_words.as_ref(), &[0x4040; 4096]);
    assert_eq!(events.last().unwrap(), "close");
    assert!(!events.iter().any(|event| event == "quarantine"));
    let consumed = events
        .iter()
        .position(|event| event == "consumers")
        .unwrap();
    for rank in 0..2 {
        for role in 0..14 {
            assert!(
                events[..consumed]
                    .iter()
                    .any(|event| event.starts_with(&format!("read:{rank}:{role}:false:")))
            );
            assert!(
                events[consumed..]
                    .iter()
                    .any(|event| event.starts_with(&format!("read:{rank}:{role}:true:")))
            );
        }
    }
    assert!(
        events
            .iter()
            .any(|event| event == "write:0:2:20971520:4194304")
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.starts_with("allocate:") && event.ends_with(":true"))
            .count(),
        2
    );
}

#[test]
fn failed_producer_or_either_rank_validation_prevents_all_consumers_and_quarantines() {
    for fault in [
        Fault::Load,
        Fault::Alias,
        Fault::Producer,
        Fault::BadState,
        Fault::ReadOnly,
        Fault::CacheOutside,
        Fault::PartialNan,
        Fault::Bf16Nan,
        Fault::ShortRead,
    ] {
        let (result, events) = run(fault);
        assert!(result.is_err(), "{fault:?}");
        assert!(
            !events
                .iter()
                .any(|event| event == "consumers" || event == "close"),
            "{fault:?}"
        );
        assert_eq!(events.last().unwrap(), "quarantine", "{fault:?}");
    }
}

#[test]
fn consumer_failure_or_read_only_mutation_returns_no_observations() {
    for fault in [
        Fault::Consumer,
        Fault::ConsumerPartialWrite,
        Fault::ConsumerStateWrite,
        Fault::ResidualMismatch,
    ] {
        let (result, events) = run(fault);
        assert!(result.is_err(), "{fault:?}");
        assert!(events.iter().any(|event| event == "consumers"));
        assert!(!events.iter().any(|event| event == "close"));
        assert_eq!(events.last().unwrap(), "quarantine");
    }
}

#[test]
fn teardown_failure_quarantines_and_never_returns_staged_outputs() {
    let (result, events) = run(Fault::Close);
    assert!(result.is_err());
    assert_eq!(&events[events.len() - 2..], &["close", "quarantine"]);
}

#[test]
fn exact_inputs_require_original_rank_shared_residual_and_identical_metadata() {
    let f = fixture();
    let good = f.inputs.each_ref().map(Vec::as_slice);
    let caches = f.caches.each_ref().map(Vec::as_slice);
    assert_eq!(validate_inputs(&[good; 2], &[caches; 2]).unwrap(), [0, 0]);
    for role in [0, 5] {
        let mut altered = f.inputs[role].clone();
        altered[0] ^= 1;
        let mut ranks = [good; 2];
        ranks[1][role] = &altered;
        assert!(validate_inputs(&ranks, &[caches; 2]).is_err());
    }
    let mut short = [caches; 2];
    short[1][1] = &short[1][1][..2];
    assert!(validate_inputs(&[good; 2], &short).is_err());
}
