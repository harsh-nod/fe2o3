use std::env;
use std::fs::File;
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::ExitCode;

use fe2o3_profiler_protocol::{
    MAX_AGENT_KFD_PROFILER_REQUEST_BYTES_V1, MAX_KFD_RUNTIME_PROFILE_BYTES_V1,
    answer_agent_kfd_profiler_request_v1, decode_kfd_runtime_profile_v1,
    encode_agent_kfd_profiler_error_response_v1,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("fe2o3-kfd-profiler-query: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let path = arguments
        .next()
        .ok_or("usage: fe2o3-kfd-profiler-query <capture.json>")?;
    if arguments.next().is_some() {
        return Err("usage: fe2o3-kfd-profiler-query <capture.json>".to_owned());
    }
    let capture = read_stable_capture(Path::new(&path))?;
    decode_kfd_runtime_profile_v1(&capture).map_err(|error| error.to_string())?;

    let input = io::stdin();
    let output = io::stdout();
    serve_requests(&capture, input.lock(), output.lock())
}

fn serve_requests(
    capture: &[u8],
    mut input: impl BufRead,
    mut output: impl Write,
) -> Result<(), String> {
    let mut request = Vec::new();
    loop {
        request.clear();
        let read = input
            .by_ref()
            .take(MAX_AGENT_KFD_PROFILER_REQUEST_BYTES_V1 + 2)
            .read_until(b'\n', &mut request)
            .map_err(|error| format!("request read: {error}"))?;
        if read == 0 {
            return Ok(());
        }
        let terminated = request.last() == Some(&b'\n');
        if terminated {
            request.pop();
        }
        if request.last() == Some(&b'\r') {
            request.pop();
        }
        let response = match answer_agent_kfd_profiler_request_v1(&capture, &request) {
            Ok(response) => response,
            Err(error) => {
                encode_agent_kfd_profiler_error_response_v1(request_id_hint(&request), &error)
                    .map_err(|error| error.to_string())?
            }
        };
        output
            .write_all(&response)
            .map_err(|error| format!("response write: {error}"))?;
        output
            .write_all(b"\n")
            .map_err(|error| format!("response write: {error}"))?;
        output
            .flush()
            .map_err(|error| format!("response flush: {error}"))?;
        if read as u64 > MAX_AGENT_KFD_PROFILER_REQUEST_BYTES_V1 && !terminated {
            discard_through_newline(&mut input)?;
        }
    }
}

fn request_id_hint(bytes: &[u8]) -> u64 {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|value| value.get("request_id")?.as_u64())
        .unwrap_or(0)
}

fn discard_through_newline(input: &mut impl BufRead) -> Result<(), String> {
    loop {
        let buffered = input
            .fill_buf()
            .map_err(|error| format!("oversized request discard: {error}"))?;
        if buffered.is_empty() {
            return Ok(());
        }
        let newline = buffered.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(buffered.len(), |index| index + 1);
        input.consume(consumed);
        if newline.is_some() {
            return Ok(());
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileSnapshotV1 {
    device: u64,
    inode: u64,
    links: u64,
    mode: u32,
    len: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl FileSnapshotV1 {
    fn read(file: &File) -> Result<Self, String> {
        let metadata = file
            .metadata()
            .map_err(|error| format!("capture metadata: {error}"))?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.len() == 0
            || metadata.len() > MAX_KFD_RUNTIME_PROFILE_BYTES_V1
            || metadata.mode() & 0o022 != 0
        {
            return Err(
                "capture must be a bounded, single-link regular file without group/other write access"
                    .to_owned(),
            );
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            links: metadata.nlink(),
            mode: metadata.mode(),
            len: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        })
    }
}

fn read_stable_capture(path: &Path) -> Result<Vec<u8>, String> {
    read_stable_capture_with_hook(path, || {})
}

fn read_stable_capture_with_hook(
    path: &Path,
    after_first_read: impl FnOnce(),
) -> Result<Vec<u8>, String> {
    let mut file = open_capture_path(path)?;
    let initial = FileSnapshotV1::read(&file)?;
    let capacity =
        usize::try_from(initial.len).map_err(|_| "capture length overflow".to_owned())?;
    let mut first = Vec::new();
    first
        .try_reserve_exact(capacity)
        .map_err(|_| "capture allocation failed".to_owned())?;
    Read::by_ref(&mut file)
        .take(MAX_KFD_RUNTIME_PROFILE_BYTES_V1 + 1)
        .read_to_end(&mut first)
        .map_err(|error| format!("capture read: {error}"))?;
    if first.len() as u64 != initial.len || FileSnapshotV1::read(&file)? != initial {
        return Err("capture changed or exceeded its bound while being read".to_owned());
    }
    after_first_read();
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("capture rewind: {error}"))?;
    let mut second = Vec::new();
    second
        .try_reserve_exact(capacity)
        .map_err(|_| "capture verification allocation failed".to_owned())?;
    Read::by_ref(&mut file)
        .take(MAX_KFD_RUNTIME_PROFILE_BYTES_V1 + 1)
        .read_to_end(&mut second)
        .map_err(|error| format!("capture verification read: {error}"))?;
    if second != first || FileSnapshotV1::read(&file)? != initial {
        return Err("capture changed while its content snapshot was admitted".to_owned());
    }
    if FileSnapshotV1::read(&open_capture_path(path)?)? != initial {
        return Err("capture path changed while its content snapshot was admitted".to_owned());
    }
    Ok(first)
}

fn open_capture_path(path: &Path) -> Result<File, String> {
    rustix::fs::openat2(
        rustix::fs::CWD,
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
        rustix::fs::ResolveFlags::NO_SYMLINKS | rustix::fs::ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|error| format!("capture open: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    struct TestDirectoryV1(std::path::PathBuf);

    impl TestDirectoryV1 {
        fn new() -> Self {
            let path = env::temp_dir().join(format!(
                "fe2o3-kfd-profiler-query-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn file(&self, name: &str, bytes: &[u8]) -> std::path::PathBuf {
            let path = self.0.join(name);
            fs::write(&path, bytes).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            path
        }
    }

    impl Drop for TestDirectoryV1 {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn stable_reader_rejects_symlinks_and_hardlinks() {
        let directory = TestDirectoryV1::new();
        let source = directory.file("source", b"bounded capture bytes");
        let alias = directory.0.join("alias");
        symlink(&source, &alias).unwrap();
        assert!(read_stable_capture(&alias).is_err());

        fs::remove_file(&alias).unwrap();
        fs::hard_link(&source, &alias).unwrap();
        assert!(read_stable_capture(&source).is_err());
        assert!(read_stable_capture(&alias).is_err());
    }

    #[test]
    fn stable_reader_rejects_an_intermediate_symlink() {
        let directory = TestDirectoryV1::new();
        let real_parent = directory.0.join("real");
        fs::create_dir(&real_parent).unwrap();
        let source = real_parent.join("capture");
        fs::write(&source, b"bounded capture bytes").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let alias_parent = directory.0.join("alias-parent");
        symlink(&real_parent, &alias_parent).unwrap();
        assert!(read_stable_capture(&alias_parent.join("capture")).is_err());
    }

    #[test]
    fn stable_reader_rejects_in_place_mutation_between_observations() {
        let directory = TestDirectoryV1::new();
        let source = directory.file("source", b"first bounded content");
        let mutation_path = source.clone();
        let result = read_stable_capture_with_hook(&source, move || {
            fs::write(mutation_path, b"other bounded content").unwrap();
        });
        assert!(result.is_err());
    }

    #[test]
    fn stable_reader_rejects_parent_and_path_substitution_after_open() {
        let directory = TestDirectoryV1::new();
        let parent = directory.0.join("parent");
        let displaced = directory.0.join("displaced");
        fs::create_dir(&parent).unwrap();
        let source = parent.join("capture");
        fs::write(&source, b"first bounded content").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let replacement_parent = parent.clone();
        let result = read_stable_capture_with_hook(&source, move || {
            fs::rename(&replacement_parent, &displaced).unwrap();
            fs::create_dir(&replacement_parent).unwrap();
            let replacement = replacement_parent.join("capture");
            fs::write(&replacement, b"first bounded content").unwrap();
            fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
        });
        assert!(result.is_err());
    }

    #[test]
    fn request_id_hint_is_bounded_to_an_unsigned_json_integer() {
        assert_eq!(request_id_hint(br#"{"request_id":17}"#), 17);
        assert_eq!(request_id_hint(br#"{"request_id":"17"}"#), 0);
        assert_eq!(request_id_hint(b"not-json"), 0);
    }

    #[test]
    fn rejected_request_does_not_end_or_desynchronize_the_session() {
        let capture = fe2o3_profiler_protocol::KfdRuntimeProfileV1::new(
            fe2o3_profiler_protocol::ProfileIdentityV1::new([1; 32]).unwrap(),
            fe2o3_profiler_protocol::KfdProfileDeviceV1::observed(7, "gfx942:xnack-", 64).unwrap(),
            fe2o3_profiler_protocol::KfdProfileHostContentModeV1::RangeOnly,
            Vec::new(),
            0,
        )
        .unwrap();
        let capture = fe2o3_profiler_protocol::encode_kfd_runtime_profile_v1(&capture).unwrap();
        let valid = fe2o3_profiler_protocol::encode_agent_kfd_profiler_request_v1(
            &fe2o3_profiler_protocol::AgentKfdProfilerRequestV1 {
                schema: fe2o3_profiler_protocol::AGENT_KFD_PROFILER_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 19,
                operation:
                    fe2o3_profiler_protocol::AgentKfdProfilerOperationV1::DiscoverCapabilities,
                cursor: None,
                limit: None,
                dispatch: None,
            },
        )
        .unwrap();
        let mut requests = b"{\"request_id\":18,\"bad\":true}\n".to_vec();
        requests.extend_from_slice(&valid);
        requests.push(b'\n');
        let mut responses = Vec::new();
        serve_requests(&capture, io::Cursor::new(requests), &mut responses).unwrap();

        let rows: Vec<_> = responses.split(|byte| *byte == b'\n').collect();
        assert_eq!(rows.len(), 3);
        let first =
            fe2o3_profiler_protocol::decode_agent_kfd_profiler_response_v1(rows[0]).unwrap();
        assert_eq!(first.request_id, 18);
        assert!(matches!(
            first.body,
            fe2o3_profiler_protocol::AgentKfdProfilerResponseBodyV1::Error { ref code, .. }
                if code == "invalid_request"
        ));
        let second =
            fe2o3_profiler_protocol::decode_agent_kfd_profiler_response_v1(rows[1]).unwrap();
        assert_eq!(second.request_id, 19);
        assert!(matches!(
            second.body,
            fe2o3_profiler_protocol::AgentKfdProfilerResponseBodyV1::Capabilities { .. }
        ));
        assert!(rows[2].is_empty());
    }
}
