//! Pure bounded parsers/quotes, not actual proc acquisition or native admission.
use super::*;

#[test]
fn procfs_type_accepts_only_exact_magic_in_signed_and_unsigned_abis() {
    assert!(is_procfs_type(0x9fa0_i32));
    assert!(is_procfs_type(0x9fa0_u32));
    assert!(is_procfs_type(0x9fa0_i64));
    assert!(is_procfs_type(0x9fa0_u64));
    for value in [0_u64, 0x9f9f, 0x9fa1, 0x0102_1994] {
        assert!(!is_procfs_type(value));
        assert!(!is_procfs_type(i64::try_from(value).unwrap()));
    }
}

#[test]
fn procfs_type_rejects_negative_overflow_and_truncated_magic_aliases() {
    for value in [-1_i64, i64::MIN, -0x1_0000_0000 + 0x9fa0] {
        assert!(!is_procfs_type(value));
    }
    for value in [u64::MAX, 0x1_0000_9fa0, 0x8000_0000_0000_9fa0] {
        assert!(!is_procfs_type(value));
    }
    assert!(!is_procfs_type((1_u128 << 64) | 0x9fa0));
}

#[test]
fn kernel_personality_record_is_exact_and_preserves_the_observed_bits() {
    for (text, value) in [
        (&b"00000000\n"[..], 0),
        (b"00040000\n", 0x0004_0000),
        (b"ffbfffff\n", 0xffbf_ffff),
    ] {
        assert_eq!(parse(text), Some(value));
        for end in 0..text.len() {
            assert_eq!(parse(&text[..end]), None);
        }
    }
    for text in [
        &b"00400000\n"[..],
        b"00400001\n",
        b"ffffffff\n",
        b"FFBFFFFF\n",
        b"0000000\n",
        b"000000000\n",
        b"00000000",
        b"00000000\n\n",
        b"00000000\0",
        b"00000000\r\n",
        b"0000000g\n",
        b"+0000000\n",
        b" 0000000\n",
        b"0x000000\n",
        b"00000000\nextra",
    ] {
        assert_eq!(parse(text), None, "accepted {text:?}");
    }
    for bit in 0..32 {
        let value = 1_u32 << bit;
        assert_eq!(
            parse(format!("{value:08x}\n").as_bytes()),
            if bit == 22 { None } else { Some(value) }
        );
    }
}

#[test]
fn kernel_thread_self_coordinates_must_name_this_exact_task_canonically() {
    assert!(exact_task_path(b"123/task/123", 123, 123));
    assert!(exact_task_path(
        b"2147483647/task/2147483647",
        i32::MAX,
        i32::MAX
    ));
    for path in [
        &b"123/task/124"[..],
        b"124/task/123",
        b"0123/task/123",
        b"123/task/0123",
        b"/123/task/123",
        b"123/task/123/",
        b"123//task/123",
        b"123/../123",
        b"123/fd/123",
        b"123/task/123\0",
        b"123/task/123\n",
        b"0/task/0",
        b"-1/task/-1",
        b"2147483648/task/123",
        b"123/task/2147483648",
        b"123/task/123/personality",
    ] {
        assert!(!exact_task_path(path, 123, 123), "accepted {path:?}");
    }
}

#[test]
fn observation_has_no_destructor_and_quotes_complete_success_and_rollback() {
    assert!(!std::mem::needs_drop::<Observation>());
    assert_eq!(size_of::<libc::stat>(), 144);
    assert_eq!(size_of::<libc::statfs>(), 120);
    assert_eq!(size_of::<Observation>(), 12);
    assert_eq!(size_of::<OpenHow>(), 24);
    let operations = [
        "acquire getpid",
        "acquire gettid",
        "open proc root",
        "root fstatfs",
        "root fstat",
        "readlink thread-self",
        "open task",
        "task fstatfs",
        "task fstat",
        "open personality",
        "personality fstatfs",
        "personality fstat",
        "close task",
        "close root",
        "consume getpid",
        "consume gettid",
        "pread record",
        "pread EOF",
        "close personality",
    ];
    assert_eq!(WORK, operations.len() * 1088 + (32 + 10 + 1) * 64 + 256);
    assert_eq!(WORK, 23_680);
    assert_eq!(SCRATCH, 2284);
    assert_eq!(
        SCRATCH,
        6 * size_of::<libc::stat>()
            + 2 * size_of::<libc::statfs>()
            + 2 * 24
            + 2 * 12
            + 2 * 32
            + 2 * 10
            + 1024
    );
}
