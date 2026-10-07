//! CPU-only startup parsing and original pipe ownership, not admitted native custody.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_runtime_protocol::WorkerV3ApplicationIdentityV1;
use std::os::fd::IntoRawFd;

fn field<const N: usize>(bytes: &[u8]) -> Field<N> {
    let mut value = Field::missing();
    value.len = Some(bytes.len().min(N));
    value.oversized = bytes.len() > N;
    value.bytes[..bytes.len().min(N)].copy_from_slice(&bytes[..bytes.len().min(N)]);
    value
}

fn hex(bytes: &[u8]) -> Vec<u8> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|b| [DIGITS[usize::from(b >> 4)], DIGITS[usize::from(b & 15)]])
        .collect()
}

fn occurrence(count: usize) -> Occurrence {
    let inputs: Vec<_> = (0..count)
        .map(|i| {
            InputOccurrence::from_linux_descriptor_v1(
                (i + 1) as u16,
                1,
                (i + 10) as u64,
                libc::S_IFREG,
            )
            .unwrap()
        })
        .collect();
    Occurrence::new(
        WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(
            &crate::application_descriptor_handoff::sealed_static_test_elf_v1(),
        )
        .unwrap(),
        [7; 32],
        &inputs,
    )
    .unwrap()
}

fn snapshot(raw: [RawFd; 4]) -> Snapshot {
    Snapshot {
        marker: field(b"1"),
        slots: raw.map(|fd| field(fd.to_string().as_bytes())),
        occurrence: field(&hex(&occurrence(4).encode_canonical().unwrap())),
        challenge: field(&hex(&Challenge::from_bytes([9; 32])
            .unwrap()
            .encode_canonical()
            .unwrap())),
        forbidden: [false; FORBIDDEN.len()],
    }
}

// The caller assumes the four sole writer owners. Reading EOF on the retained
// reader proves those exact pipe writers closed, without racing raw-FD reuse.
fn pipes() -> ([OwnedFd; 4], [RawFd; 4]) {
    let pairs: [_; 4] = std::array::from_fn(|_| {
        rustix::pipe::pipe_with(rustix::pipe::PipeFlags::NONBLOCK).unwrap()
    });
    let mut raw = [0; 4];
    let mut index = 0;
    let readers = pairs.map(|(reader, writer)| {
        let transferred = rustix::io::fcntl_dupfd_cloexec(&writer, 256).unwrap();
        rustix::io::fcntl_setfd(&transferred, rustix::io::FdFlags::empty()).unwrap();
        drop(writer);
        raw[index] = transferred.into_raw_fd();
        index += 1;
        reader
    });
    assert!(raw.iter().all(|fd| valid_application_descriptor(*fd)));
    (readers, raw)
}

fn assert_closed(readers: &[OwnedFd]) {
    for reader in readers {
        assert_eq!(rustix::io::read(reader, &mut [0; 1]).unwrap(), 0);
    }
}

#[test]
fn native_slot_claim_preserves_originals_and_meters_temporary_decode() {
    crate::eof_test_process::run(
        concat!(
            module_path!(),
            "::native_slot_claim_preserves_originals_and_meters_temporary_decode"
        )
        .strip_prefix("fe2o3_host::")
        .unwrap(),
        native_slot_claim_preserves_originals_and_meters_temporary_decode_inner,
    );
}

fn native_slot_claim_preserves_originals_and_meters_temporary_decode_inner() {
    let (readers, raw) = pipes();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let floor = 123;
    budget.reserve_storage(floor).unwrap();
    let value = claim_snapshot(snapshot(raw), &mut budget).unwrap();
    assert_eq!(value.descriptors.each_ref().map(AsRawFd::as_raw_fd), raw);
    for fd in &value.descriptors {
        assert!(
            rustix::io::fcntl_getfd(fd)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }
    assert_eq!(value.occurrence, occurrence(4));
    assert_eq!(value.challenge.as_bytes(), [9; 32]);
    assert_eq!(budget.storage(), floor + value.scratch);
    let scratch = value.scratch;
    drop(value);
    assert_closed(&readers);
    budget.release_storage(scratch).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn native_slot_refusals_close_originals_even_when_budget_denied() {
    crate::eof_test_process::run(
        concat!(
            module_path!(),
            "::native_slot_refusals_close_originals_even_when_budget_denied"
        )
        .strip_prefix("fe2o3_host::")
        .unwrap(),
        native_slot_refusals_close_originals_even_when_budget_denied_inner,
    );
}

fn native_slot_refusals_close_originals_even_when_budget_denied_inner() {
    for case in 0..14 {
        let (readers, raw) = pipes();
        let mut value = snapshot(raw);
        match case {
            0 => value.marker = field(b"0"),
            1..=6 => value.forbidden[case - 1] = true,
            7 => value.occurrence.bytes[0] = b'A',
            8 => value.occurrence = field(b"00"),
            9 => value.occurrence = field(&hex(&occurrence(3).encode_canonical().unwrap())),
            10 => value.challenge.bytes[0] = b'A',
            11 => value.challenge = field(b"00"),
            12 | 13 => (),
            _ => unreachable!(),
        }
        let mut work = Work::new(if case == 12 { 0 } else { 1_000_000 });
        let mut budget = Budget::new(&mut work, if case == 13 { 0 } else { 1_000_000 });
        assert!(claim_snapshot(value, &mut budget).is_err(), "case {case}");
        assert_closed(&readers);
        if matches!(case, 7..=11) {
            assert!(budget.storage() > 0, "failed decode must not reset charges");
        }
    }
}

#[test]
fn native_slot_alias_refusal_closes_once_and_reserved_slot_is_not_owned() {
    crate::eof_test_process::run(
        concat!(
            module_path!(),
            "::native_slot_alias_refusal_closes_once_and_reserved_slot_is_not_owned"
        )
        .strip_prefix("fe2o3_host::")
        .unwrap(),
        native_slot_alias_refusal_closes_once_and_reserved_slot_is_not_owned_inner,
    );
}

fn native_slot_alias_refusal_closes_once_and_reserved_slot_is_not_owned_inner() {
    let (readers, raw) = pipes();
    // The fourth original remains outside the declared roster for this control.
    let fourth = claim_inherited_descriptor(raw[3], "CPU fixture fourth").unwrap();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    assert!(claim_snapshot(snapshot([raw[0], raw[1], raw[2], raw[2]]), &mut budget).is_err());
    assert_closed(&readers[..3]);
    assert_eq!(
        rustix::io::read(&readers[3], &mut [0; 1]),
        Err(rustix::io::Errno::AGAIN)
    );
    drop(fourth);
    assert_closed(&readers);
    assert_eq!(descriptor(&field(b"195")), None);
}

#[test]
fn native_environment_grammar_rejects_noncanonical_and_unbounded_values() {
    for bytes in [
        b"".as_slice(),
        b"0",
        b"1",
        b"2",
        b"195",
        b"03",
        b"+3",
        b" 3",
        b"3 ",
        b"-4",
        b"2147483648",
        b"99999999999",
        &[0xff],
    ] {
        assert_eq!(descriptor(&field(bytes)), None, "{bytes:?}");
    }
    assert_eq!(descriptor(&field(b"2147483647")), Some(i32::MAX));
    assert!(field::<1>(b"11").bytes().is_err());
    assert!(Field::<1>::missing().bytes().is_err());
    for bytes in [b"".as_slice(), b"a", b"AA", b"0g", b"0 ", b"000000"] {
        assert!(decode_hex(bytes, &mut [0; 2]).is_err());
    }
    let mut output = [0; 2];
    assert_eq!(decode_hex(b"00ff", &mut output).unwrap(), 2);
    assert_eq!(output, [0, 255]);
}
