use crate::{RocmToolchain, require_tool};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SUPPORTED_HOST_TRIPLE: &str = "x86_64-unknown-linux-gnu";
const ARTIFACT_ID_HEX_BYTES: usize = 64;
const MAX_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
const MAX_OBJECT_BYTES: usize = 8 * 1024 * 1024;
const SYMBOL_PREFIX: &str = "__fe2o3_host_object_";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedHostObject {
    module_name: String,
    path: PathBuf,
    start_symbol: String,
    end_symbol: String,
    exact_object: Box<[u8]>,
}

impl GeneratedHostObject {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn module_name(&self) -> &str {
        &self.module_name
    }

    pub(crate) fn start_symbol(&self) -> &str {
        &self.start_symbol
    }

    pub(crate) fn end_symbol(&self) -> &str {
        &self.end_symbol
    }

    pub(crate) fn validate_unchanged(&self) -> Result<(), HostObjectError> {
        let actual = read_bounded_object(&self.path)?;
        if actual != self.exact_object.as_ref() {
            return Err(HostObjectError::ObjectChanged(self.path.clone()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostObjectToolchain {
    llvm_mc: PathBuf,
}

impl HostObjectToolchain {
    pub(crate) fn from_rocm(toolchain: &RocmToolchain) -> Result<Self, HostObjectError> {
        let llvm_bin = toolchain.rocm_path.join("lib/llvm/bin");
        let llvm_mc = require_tool(&llvm_bin, "llvm-mc").map_err(HostObjectError::Toolchain)?;
        Ok(Self { llvm_mc })
    }

    #[cfg(test)]
    fn for_test(llvm_mc: PathBuf) -> Self {
        Self { llvm_mc }
    }
}

pub(crate) fn generate_host_object(
    toolchain: &HostObjectToolchain,
    host_triple: &str,
    output_path: &Path,
    artifact_id: &str,
    payload: &[u8],
) -> Result<GeneratedHostObject, HostObjectError> {
    validate_request(toolchain, host_triple, output_path, artifact_id, payload)?;

    let start_symbol = format!("{SYMBOL_PREFIX}{artifact_id}_start");
    let end_symbol = format!("{SYMBOL_PREFIX}{artifact_id}_end");
    let module_name = format!("fe2o3-host-object-{artifact_id}");
    let assembly = render_assembly(artifact_id, &start_symbol, &end_symbol, payload);

    let mut child = Command::new(&toolchain.llvm_mc)
        .args(["-triple=x86_64-unknown-linux-gnu", "-filetype=obj", "-o"])
        .arg(output_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| HostObjectError::Io {
            path: toolchain.llvm_mc.clone(),
            source,
        })?;

    child
        .stdin
        .take()
        .ok_or_else(|| HostObjectError::ToolProtocol("llvm-mc stdin was unavailable".into()))?
        .write_all(assembly.as_bytes())
        .map_err(|source| HostObjectError::Io {
            path: toolchain.llvm_mc.clone(),
            source,
        })?;

    let output = child
        .wait_with_output()
        .map_err(|source| HostObjectError::Io {
            path: toolchain.llvm_mc.clone(),
            source,
        })?;
    if !output.status.success() {
        let _ = fs::remove_file(output_path);
        return Err(HostObjectError::ToolFailed {
            tool: toolchain.llvm_mc.clone(),
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    let exact_object = read_bounded_object(output_path)?.into_boxed_slice();
    Ok(GeneratedHostObject {
        module_name,
        path: output_path.to_path_buf(),
        start_symbol,
        end_symbol,
        exact_object,
    })
}

fn validate_request(
    toolchain: &HostObjectToolchain,
    host_triple: &str,
    output_path: &Path,
    artifact_id: &str,
    payload: &[u8],
) -> Result<(), HostObjectError> {
    if host_triple != SUPPORTED_HOST_TRIPLE {
        return Err(HostObjectError::UnsupportedHost(host_triple.to_owned()));
    }
    if !toolchain.llvm_mc.is_file() {
        return Err(HostObjectError::MissingTool(toolchain.llvm_mc.clone()));
    }
    if artifact_id.len() != ARTIFACT_ID_HEX_BYTES
        || !artifact_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(HostObjectError::InvalidArtifactId(artifact_id.to_owned()));
    }
    if payload.is_empty() {
        return Err(HostObjectError::EmptyPayload);
    }
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(HostObjectError::PayloadTooLarge(payload.len()));
    }
    if output_path.extension().and_then(|value| value.to_str()) != Some("o") {
        return Err(HostObjectError::InvalidOutputPath(
            output_path.to_path_buf(),
        ));
    }
    let Some(parent) = output_path.parent() else {
        return Err(HostObjectError::InvalidOutputPath(
            output_path.to_path_buf(),
        ));
    };
    if !parent.is_dir() {
        return Err(HostObjectError::MissingOutputDirectory(
            parent.to_path_buf(),
        ));
    }
    if output_path.exists() {
        return Err(HostObjectError::OutputExists(output_path.to_path_buf()));
    }
    Ok(())
}

fn render_assembly(
    artifact_id: &str,
    start_symbol: &str,
    end_symbol: &str,
    payload: &[u8],
) -> String {
    let mut assembly = format!(
        ".section .rodata.fe2o3.{artifact_id},\"a\",@progbits\n\
         .p2align 4\n\
         .globl {start_symbol}\n\
         .hidden {start_symbol}\n\
         .type {start_symbol},@object\n\
         {start_symbol}:\n"
    );
    for chunk in payload.chunks(16) {
        assembly.push_str(".byte ");
        for (index, byte) in chunk.iter().enumerate() {
            if index != 0 {
                assembly.push(',');
            }
            use fmt::Write as _;
            write!(assembly, "0x{byte:02x}").expect("writing to String cannot fail");
        }
        assembly.push('\n');
    }
    use fmt::Write as _;
    write!(
        assembly,
        ".globl {end_symbol}\n\
         .hidden {end_symbol}\n\
         .type {end_symbol},@object\n\
         {end_symbol}:\n\
         .size {start_symbol}, {end_symbol}-{start_symbol}\n\
         .size {end_symbol}, 0\n\
         .section .note.GNU-stack,\"\",@progbits\n"
    )
    .expect("writing to String cannot fail");
    assembly
}

fn read_bounded_object(path: &Path) -> Result<Vec<u8>, HostObjectError> {
    let metadata = fs::metadata(path).map_err(|source| HostObjectError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let length = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    if length == 0 || length > MAX_OBJECT_BYTES {
        return Err(HostObjectError::InvalidObjectSize {
            path: path.to_path_buf(),
            length,
        });
    }
    let bytes = fs::read(path).map_err(|source| HostObjectError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if bytes.len() != length {
        return Err(HostObjectError::ObjectChanged(path.to_path_buf()));
    }
    Ok(bytes)
}

#[derive(Debug)]
pub(crate) enum HostObjectError {
    Toolchain(crate::ToolchainError),
    UnsupportedHost(String),
    MissingTool(PathBuf),
    InvalidArtifactId(String),
    EmptyPayload,
    PayloadTooLarge(usize),
    InvalidOutputPath(PathBuf),
    MissingOutputDirectory(PathBuf),
    OutputExists(PathBuf),
    InvalidObjectSize {
        path: PathBuf,
        length: usize,
    },
    ObjectChanged(PathBuf),
    ToolProtocol(String),
    ToolFailed {
        tool: PathBuf,
        status: String,
        stderr: String,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for HostObjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toolchain(error) => write!(formatter, "{error}"),
            Self::UnsupportedHost(found) => write!(
                formatter,
                "generated host objects require {SUPPORTED_HOST_TRIPLE}; found {found}"
            ),
            Self::MissingTool(path) => {
                write!(
                    formatter,
                    "required host-object tool is missing: {}",
                    path.display()
                )
            }
            Self::InvalidArtifactId(id) => write!(
                formatter,
                "host-object artifact ID must be exactly 64 lowercase hexadecimal bytes; found {id:?}"
            ),
            Self::EmptyPayload => formatter.write_str("host-object payload must not be empty"),
            Self::PayloadTooLarge(length) => write!(
                formatter,
                "host-object payload is {length} bytes; maximum is {MAX_PAYLOAD_BYTES}"
            ),
            Self::InvalidOutputPath(path) => write!(
                formatter,
                "host-object output must be an .o path with a parent directory: {}",
                path.display()
            ),
            Self::MissingOutputDirectory(path) => write!(
                formatter,
                "host-object output directory does not exist: {}",
                path.display()
            ),
            Self::OutputExists(path) => write!(
                formatter,
                "refusing to overwrite host-object output: {}",
                path.display()
            ),
            Self::InvalidObjectSize { path, length } => write!(
                formatter,
                "host object {} has invalid bounded size {length}",
                path.display()
            ),
            Self::ObjectChanged(path) => write!(
                formatter,
                "host object changed after generation: {}",
                path.display()
            ),
            Self::ToolProtocol(reason) => {
                write!(formatter, "host-object tool protocol failed: {reason}")
            }
            Self::ToolFailed {
                tool,
                status,
                stderr,
            } => write!(
                formatter,
                "host-object tool {} failed with status {status}: {stderr}",
                tool.display()
            ),
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "host-object I/O failed for {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for HostObjectError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const PAYLOAD: &[u8] = b"fe2o3 synthetic host object payload\0\xff";
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-host-object-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn test_toolchain() -> Option<HostObjectToolchain> {
        [
            PathBuf::from("/opt/rocm/lib/llvm/bin/llvm-mc"),
            PathBuf::from("/usr/bin/llvm-mc"),
        ]
        .into_iter()
        .find(|path| path.is_file())
        .map(HostObjectToolchain::for_test)
    }

    #[test]
    fn generates_deterministic_bounded_object() {
        let Some(toolchain) = test_toolchain() else {
            eprintln!("skipping: llvm-mc is unavailable");
            return;
        };
        let first_dir = TestDir::new();
        let second_dir = TestDir::new();
        let first = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &first_dir.0.join("fixture.o"),
            ID,
            PAYLOAD,
        )
        .expect("generate first object");
        let second = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &second_dir.0.join("fixture.o"),
            ID,
            PAYLOAD,
        )
        .expect("generate second object");

        assert_eq!(first.exact_object, second.exact_object);
        assert_eq!(first.start_symbol(), format!("{SYMBOL_PREFIX}{ID}_start"));
        assert_eq!(first.end_symbol(), format!("{SYMBOL_PREFIX}{ID}_end"));
        first.validate_unchanged().expect("object remains exact");
    }

    #[test]
    fn linked_symbols_bracket_exact_payload() {
        let Some(toolchain) = test_toolchain() else {
            eprintln!("skipping: llvm-mc is unavailable");
            return;
        };
        let directory = TestDir::new();
        let object = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &directory.0.join("fixture.o"),
            ID,
            PAYLOAD,
        )
        .expect("generate object");
        let expected = PAYLOAD
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let source = format!(
            r#"
unsafe extern "C" {{
    #[link_name = "{start}"]
    static START: u8;
    #[link_name = "{end}"]
    static END: u8;
}}

fn main() {{
    let start = core::ptr::addr_of!(START);
    let end = core::ptr::addr_of!(END);
    let length = end.addr().checked_sub(start.addr()).expect("ordered symbols");
    let bytes = unsafe {{ core::slice::from_raw_parts(start, length) }};
    assert_eq!(bytes, &[{expected}]);
}}
"#,
            start = object.start_symbol(),
            end = object.end_symbol(),
        );
        let source_path = directory.0.join("consumer.rs");
        let executable_path = directory.0.join("consumer");
        fs::write(&source_path, source).expect("write consumer source");
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let compile = Command::new(rustc)
            .arg(&source_path)
            .arg("-o")
            .arg(&executable_path)
            .arg("-C")
            .arg(format!("link-arg={}", object.path().display()))
            .output()
            .expect("run rustc");
        assert!(
            compile.status.success(),
            "rustc failed: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let run = Command::new(executable_path)
            .output()
            .expect("run linked consumer");
        assert!(
            run.status.success(),
            "linked consumer failed: {}",
            String::from_utf8_lossy(&run.stderr)
        );
    }

    #[test]
    fn rejects_unsupported_or_unbounded_requests() {
        let directory = TestDir::new();
        let toolchain = HostObjectToolchain::for_test(PathBuf::from("/missing/llvm-mc"));
        let output = directory.0.join("fixture.o");

        assert!(matches!(
            validate_request(
                &toolchain,
                "aarch64-unknown-linux-gnu",
                &output,
                ID,
                PAYLOAD
            ),
            Err(HostObjectError::UnsupportedHost(_))
        ));
        assert!(matches!(
            validate_request(&toolchain, SUPPORTED_HOST_TRIPLE, &output, ID, PAYLOAD),
            Err(HostObjectError::MissingTool(_))
        ));
    }

    #[test]
    fn detects_object_replacement() {
        let Some(toolchain) = test_toolchain() else {
            eprintln!("skipping: llvm-mc is unavailable");
            return;
        };
        let directory = TestDir::new();
        let object = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &directory.0.join("fixture.o"),
            ID,
            PAYLOAD,
        )
        .expect("generate object");
        fs::write(object.path(), b"replacement").expect("replace object");

        assert!(matches!(
            object.validate_unchanged(),
            Err(HostObjectError::ObjectChanged(_))
        ));
    }
}
