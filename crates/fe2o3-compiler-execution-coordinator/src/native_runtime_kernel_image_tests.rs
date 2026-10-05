use super::*;

fn auxv(entries: &[(u64, u64)]) -> Vec<u8> {
    entries
        .iter()
        .flat_map(|(kind, value)| kind.to_ne_bytes().into_iter().chain(value.to_ne_bytes()))
        .collect()
}

#[test]
fn auxv_requires_exact_terminated_native_sysinfo_record() {
    assert_eq!(
        sysinfo_ehdr(&auxv(&[(6, 4096), (33, 0x8000), (0, 0)])).unwrap(),
        0x8000
    );
    for entries in [
        vec![(0, 0)],
        vec![(33, 0x8000)],
        vec![(33, 0), (0, 0)],
        vec![(33, 0x8001), (0, 0)],
        vec![(33, 0x8000), (33, 0x8000), (0, 0)],
        vec![(33, 0x8000), (0, 1)],
        vec![(33, 0x8000), (0, 0), (1, 2)],
    ] {
        assert!(sysinfo_ehdr(&auxv(&entries)).is_err(), "{entries:?}");
    }
    assert!(sysinfo_ehdr(&[0; 17]).is_err());
    assert!(sysinfo_ehdr(&[0; View::AUXV_BYTES + 16]).is_err());
}

#[test]
fn kernel_interval_uses_auxv_coordinate_and_not_a_map_name() {
    assert_eq!(
        kernel_interval("8000-a000 r-xp 00000000 00:00 0 ignored-label\n", 0x8000).unwrap(),
        (0x8000, 0xa000)
    );
    assert!(kernel_interval("9000-b000 r-xp 00000000 00:00 0 [vdso]\n", 0x8000).is_err());
    for row in [
        "8000-a000 rwxp 0 00:00 0 [vdso]",
        "8000-a000 r-xs 0 00:00 0 [vdso]",
        "8000-a000 r-xp 1 00:00 0 [vdso]",
        "8000-a000 r-xp 0 08:01 0 [vdso]",
        "8000-a000 r-xp 0 00:00 12 [vdso]",
        "8000-a001 r-xp 0 00:00 0 [vdso]",
        "8000-19000 r-xp 0 00:00 0 [vdso]",
    ] {
        assert!(kernel_interval(row, 0x8000).is_err(), "{row}");
    }
    assert!(
        kernel_interval(
            "8000-a000 r-xp 0 00:00 0 x\n8000-a000 r-xp 0 00:00 0 y",
            0x8000
        )
        .is_err()
    );
}

fn elf() -> Vec<u8> {
    let mut image = vec![0; 8192];
    image[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1]);
    image[16..18].copy_from_slice(&3u16.to_le_bytes());
    image[18..20].copy_from_slice(&62u16.to_le_bytes());
    image[32..40].copy_from_slice(&64u64.to_le_bytes());
    image[54..56].copy_from_slice(&56u16.to_le_bytes());
    image[56..58].copy_from_slice(&1u16.to_le_bytes());
    image[64..68].copy_from_slice(&1u32.to_le_bytes());
    image[68..72].copy_from_slice(&5u32.to_le_bytes());
    image[96..104].copy_from_slice(&5000u64.to_le_bytes());
    image
}

#[test]
fn kernel_elf_requires_whole_mapped_executable_range() {
    let mut image = elf();
    require_complete_elf(&image).unwrap();
    image[96..104].copy_from_slice(&4000u64.to_le_bytes());
    assert!(require_complete_elf(&image).is_err());
    image = elf();
    image[96..104].copy_from_slice(&9000u64.to_le_bytes());
    assert!(require_complete_elf(&image).is_err());
    image = elf();
    image[32..40].copy_from_slice(&8190u64.to_le_bytes());
    assert!(require_complete_elf(&image).is_err());
    image = elf();
    image[18..20].copy_from_slice(&183u16.to_le_bytes());
    assert!(require_complete_elf(&image).is_err());
    assert!(require_complete_elf(&[0; 16]).is_err());
}
