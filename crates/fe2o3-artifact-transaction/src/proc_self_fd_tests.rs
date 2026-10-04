#[cfg(target_os = "linux")]
#[test]
fn proc_self_fd_opath_import_is_a_readable_handle_for_the_same_directory() {
    use std::os::fd::AsRawFd;

    let temp = TestDirectory::new();
    let retained = rustix::fs::open(
        &temp.path,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    let retained_stat = fstat(&retained).unwrap();
    let path = PathBuf::from(format!("/proc/self/fd/{}", retained.as_raw_fd()));

    let imported = duplicate_proc_self_fd_directory(&path).unwrap().unwrap();
    let imported_stat = fstat(&imported).unwrap();
    assert_eq!(imported_stat.st_dev, retained_stat.st_dev);
    assert_eq!(imported_stat.st_ino, retained_stat.st_ino);
    assert_eq!(imported_stat.st_mode, retained_stat.st_mode);

    let mut directory = Dir::read_from(&imported).unwrap();
    for entry in &mut directory {
        entry.unwrap();
    }
}
