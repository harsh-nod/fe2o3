//! Closed source-owned static binding. No deserializer or environment selector.
use fe2o3_private_one_stop_protocol::Refusal;
#[derive(Clone, Copy)]
pub(super) struct Pin {
    pub path: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}
pub(super) struct Profile {
    pub debugger: Pin,
    pub startup: Pin,
    /// Fixed actual parent path only; outer launcher pins both final executables.
    /// No controller-compiled expected parent digest (avoids a build-hash cycle).
    pub scope_owner_path: &'static str,
    pub data_root: &'static str,
    /// Complete generated source/build files plus current observed/static closure.
    /// Empty data stamps are explicit size-zero pins, never omitted.
    pub files: &'static [Pin],
    /// Aliased observed paths joined to exact canonical pins in files.
    pub aliases: &'static [(&'static str, &'static str)],
    pub absent: &'static [&'static str],
    /// Relative generated data directories including the empty root.
    pub data_directories: &'static [&'static str],
    /// Relative generated data files; each must have its canonical file pin.
    pub data_files: &'static [&'static str],
}
// Deliberately absent. The previous MI3 and false-gate GDB builds/closures are NOT
// a selected MI2 one-stop profile. Only a separately reviewed SOURCE successor
// may fill this constant after root measures the actual successor and closure.
const PROFILE: Option<&Profile> = None;
pub(super) fn selected() -> Result<&'static Profile, Refusal> {
    PROFILE.ok_or(Refusal::State)
}
pub(super) const USAGE: &str = "--acknowledge-fixed-owned-one-stop-controller-unqualified";
pub(super) const TARGET: Pin = Pin {
    path: fe2o3_private_one_stop_protocol::TARGET_PATH,
    bytes: 4_162_064,
    sha256: "1ec72e9d53df21a510089951a6bba9a0167d8b7e2323e3dcd1d1ec5add3f3bdb",
};
pub(super) const ARTIFACT: Pin = Pin {
    path: "/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/phase28-gfx950-one-stop-static-r1-O0/output.hsaco",
    bytes: 5_312,
    sha256: "cd3daab76db347104166c898a58dcc29c197ee1eac8b258b10a2c793e8779c87",
};
pub(super) fn closed_cli(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), Refusal> {
    if args.next().as_deref() != Some(std::ffi::OsStr::new(USAGE)) || args.next().is_some() {
        return Err(Refusal::Shape);
    }
    Ok(())
}
pub(super) fn arguments(data: &str) -> Result<Vec<String>, Refusal> {
    if !data.starts_with('/')
        || data.len() > 512
        || !data
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'_' | b'-'))
    {
        return Err(Refusal::Shape);
    }
    Ok(["--nx", "--nh", "--quiet", "--interpreter=mi2"]
        .into_iter()
        .map(String::from)
        .chain([format!("--data-directory={data}")])
        .chain(
            [
                "-iex",
                "set auto-load off",
                "-iex",
                "set debuginfod enabled off",
                "-iex",
                "set startup-with-shell off",
            ]
            .into_iter()
            .map(String::from),
        )
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_has_no_selected_profile() {
        assert!(matches!(selected(), Err(Refusal::State)));
    }
    #[test]
    fn no_path_pid_hash_or_eval_cli_constructor() {
        assert!(closed_cli([USAGE.into()].into_iter()).is_ok());
        for x in [
            vec![],
            vec!["--help"],
            vec![USAGE, "--pid=1"],
            vec![USAGE, "/tmp/gdb"],
            vec![USAGE, "--enable"],
            vec![USAGE, "--receipt=/tmp/forged"],
        ] {
            assert!(closed_cli(x.into_iter().map(Into::into)).is_err());
        }
    }
    #[test]
    fn exact_mi2_never_falls_back_to_mi3_or_shell() {
        let a = arguments("/fixture/data").unwrap();
        assert_eq!(
            a,
            [
                "--nx",
                "--nh",
                "--quiet",
                "--interpreter=mi2",
                "--data-directory=/fixture/data",
                "-iex",
                "set auto-load off",
                "-iex",
                "set debuginfod enabled off",
                "-iex",
                "set startup-with-shell off"
            ]
        );
        for p in ["/tmp/a b", "relative", "/tmp/a\n", "/tmp/$(cmd)", ""] {
            assert!(arguments(p).is_err());
        }
    }
}
