#![allow(
    dead_code,
    reason = "host-object generation remains dormant until the typed artifact producer is connected"
)]

use crate::{RocmToolchain, require_tool};
use reserved_fe2o3_symbols::{
    KernelBindingIdV1, artifact_length_symbol_v1, artifact_pointer_symbol_v1,
};
use rustc_codegen_ssa::{CompiledModule, CompiledModules, ModuleKind};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SUPPORTED_HOST_TRIPLE: &str = "x86_64-unknown-linux-gnu";
const ARTIFACT_ID_HEX_BYTES: usize = 64;
const MAX_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
const MAX_OBJECT_BYTES: usize = 8 * 1024 * 1024;
const MAX_HOST_OBJECTS: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedHostObject {
    module_name: String,
    path: PathBuf,
    pointer_symbol: String,
    length_symbol: String,
    exact_object: Box<[u8]>,
}

impl GeneratedHostObject {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn module_name(&self) -> &str {
        &self.module_name
    }

    pub(crate) fn pointer_symbol(&self) -> &str {
        &self.pointer_symbol
    }

    pub(crate) fn length_symbol(&self) -> &str {
        &self.length_symbol
    }

    pub(crate) fn validate_unchanged(&self) -> Result<(), HostObjectError> {
        let actual = read_bounded_object(&self.path)?;
        if actual != self.exact_object.as_ref() {
            return Err(HostObjectError::ObjectChanged(self.path.clone()));
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub(crate) struct GeneratedHostObjects {
    objects: Vec<GeneratedHostObject>,
}

impl GeneratedHostObjects {
    pub(crate) fn register(&mut self, object: GeneratedHostObject) -> Result<(), HostObjectError> {
        if self.objects.len() >= MAX_HOST_OBJECTS {
            return Err(HostObjectError::TooManyObjects);
        }
        object.validate_unchanged()?;
        for existing in &self.objects {
            if existing.path() == object.path() {
                return Err(HostObjectError::DuplicateObjectPath(
                    object.path().to_path_buf(),
                ));
            }
            if symbols_collide(existing, &object) {
                return Err(HostObjectError::SymbolCollision {
                    first: existing.pointer_symbol().to_owned(),
                    second: object.pointer_symbol().to_owned(),
                });
            }
            if existing.module_name() == object.module_name() {
                return Err(HostObjectError::ModuleNameCollision(
                    object.module_name().to_owned(),
                ));
            }
        }
        self.objects.push(object);
        Ok(())
    }

    pub(crate) fn append_to(
        self,
        compiled_modules: &mut CompiledModules,
    ) -> Result<(), HostObjectError> {
        if self.objects.is_empty() {
            return Ok(());
        }

        for object in &self.objects {
            object.validate_unchanged()?;
            if compiled_modules
                .modules
                .iter()
                .any(|module| module.name == object.module_name())
            {
                return Err(HostObjectError::ModuleNameCollision(
                    object.module_name().to_owned(),
                ));
            }
            if compiled_modules.modules.iter().any(|module| {
                module.object.as_deref() == Some(object.path())
                    || module.dwarf_object.as_deref() == Some(object.path())
            }) {
                return Err(HostObjectError::DuplicateObjectPath(
                    object.path().to_path_buf(),
                ));
            }
        }

        let mut objects = self.objects;
        objects.sort_by(|first, second| first.module_name.cmp(&second.module_name));
        compiled_modules
            .modules
            .extend(objects.into_iter().map(|object| CompiledModule {
                name: object.module_name,
                kind: ModuleKind::Regular,
                object: Some(object.path),
                dwarf_object: None,
                bytecode: None,
                assembly: None,
                llvm_ir: None,
                links_from_incr_cache: Vec::new(),
            }));
        Ok(())
    }
}

fn symbols_collide(first: &GeneratedHostObject, second: &GeneratedHostObject) -> bool {
    [first.pointer_symbol(), first.length_symbol()]
        .into_iter()
        .any(|first_symbol| {
            [second.pointer_symbol(), second.length_symbol()]
                .into_iter()
                .any(|second_symbol| first_symbol == second_symbol)
        })
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
    kernel_binding: KernelBindingIdV1,
    payload: &[u8],
) -> Result<GeneratedHostObject, HostObjectError> {
    validate_request(toolchain, host_triple, output_path, artifact_id, payload)?;

    let pointer_symbol = artifact_pointer_symbol_v1(kernel_binding);
    let length_symbol = artifact_length_symbol_v1(kernel_binding);
    let module_name = format!("fe2o3-host-object-{artifact_id}");
    let assembly = render_assembly(artifact_id, &pointer_symbol, &length_symbol, payload);

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

    let mut tool_stdin = child
        .stdin
        .take()
        .ok_or_else(|| HostObjectError::ToolProtocol("llvm-mc stdin was unavailable".into()))?;
    if let Err(source) = tool_stdin.write_all(assembly.as_bytes()) {
        drop(tool_stdin);
        let _ = child.kill();
        let _ = child.wait();
        let _ = fs::remove_file(output_path);
        return Err(HostObjectError::Io {
            path: toolchain.llvm_mc.clone(),
            source,
        });
    }
    drop(tool_stdin);

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

    let exact_object = match read_bounded_object(output_path) {
        Ok(exact_object) => exact_object.into_boxed_slice(),
        Err(error) => {
            let _ = fs::remove_file(output_path);
            return Err(error);
        }
    };
    Ok(GeneratedHostObject {
        module_name,
        path: output_path.to_path_buf(),
        pointer_symbol,
        length_symbol,
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
    pointer_symbol: &str,
    length_symbol: &str,
    payload: &[u8],
) -> String {
    let mut assembly = format!(
        ".section .rodata.fe2o3.{artifact_id},\"a\",@progbits\n\
         .p2align 4\n\
         .type .Lfe2o3_artifact_data_{artifact_id},@object\n\
         .Lfe2o3_artifact_data_{artifact_id}:\n"
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
        ".size .Lfe2o3_artifact_data_{artifact_id}, {payload_length}\n\
         .section .text.fe2o3.{artifact_id},\"ax\",@progbits\n\
         .p2align 4\n\
         .globl {pointer_symbol}\n\
         .hidden {pointer_symbol}\n\
         .type {pointer_symbol},@function\n\
         {pointer_symbol}:\n\
         leaq .Lfe2o3_artifact_data_{artifact_id}(%rip), %rax\n\
         retq\n\
         .size {pointer_symbol}, .-{pointer_symbol}\n\
         .p2align 4\n\
         .globl {length_symbol}\n\
         .hidden {length_symbol}\n\
         .type {length_symbol},@function\n\
         {length_symbol}:\n\
         movl ${payload_length}, %eax\n\
         retq\n\
         .size {length_symbol}, .-{length_symbol}\n\
         .section .note.GNU-stack,\"\",@progbits\n",
        payload_length = payload.len(),
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
    validate_host_object_header(path, &bytes)?;
    Ok(bytes)
}

fn validate_host_object_header(path: &Path, bytes: &[u8]) -> Result<(), HostObjectError> {
    const ELF64_HEADER_BYTES: usize = 64;
    const ELF_MAGIC: &[u8; 4] = b"\x7fELF";
    const ELFCLASS64: u8 = 2;
    const ELFDATA2LSB: u8 = 1;
    const EV_CURRENT: u8 = 1;
    const ET_REL: [u8; 2] = 1_u16.to_le_bytes();
    const EM_X86_64: [u8; 2] = 62_u16.to_le_bytes();
    const EV_CURRENT_WORD: [u8; 4] = 1_u32.to_le_bytes();

    let valid = bytes.len() >= ELF64_HEADER_BYTES
        && bytes.get(0..4) == Some(ELF_MAGIC.as_slice())
        && bytes[4] == ELFCLASS64
        && bytes[5] == ELFDATA2LSB
        && bytes[6] == EV_CURRENT
        && bytes.get(16..18) == Some(ET_REL.as_slice())
        && bytes.get(18..20) == Some(EM_X86_64.as_slice())
        && bytes.get(20..24) == Some(EV_CURRENT_WORD.as_slice());
    if !valid {
        return Err(HostObjectError::UnsupportedObjectFormat(path.to_path_buf()));
    }
    Ok(())
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
    UnsupportedObjectFormat(PathBuf),
    TooManyObjects,
    DuplicateObjectPath(PathBuf),
    SymbolCollision {
        first: String,
        second: String,
    },
    ModuleNameCollision(String),
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
            Self::UnsupportedObjectFormat(path) => write!(
                formatter,
                "host object is not an x86-64 little-endian relocatable ELF file: {}",
                path.display()
            ),
            Self::TooManyObjects => write!(
                formatter,
                "one crate may inject at most {MAX_HOST_OBJECTS} generated host objects"
            ),
            Self::DuplicateObjectPath(path) => write!(
                formatter,
                "generated host-object path was registered more than once: {}",
                path.display()
            ),
            Self::SymbolCollision { first, second } => write!(
                formatter,
                "generated host-object symbols collide: {first} and {second}"
            ),
            Self::ModuleNameCollision(name) => write!(
                formatter,
                "generated host-object module name collides: {name}"
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
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const OTHER_ID: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
    const KERNEL_BINDING: KernelBindingIdV1 = KernelBindingIdV1::from_bytes([0x42; 32]);
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

    fn test_toolchain() -> HostObjectToolchain {
        let rocm = RocmToolchain::detect().expect("detect configured ROCm toolchain");
        HostObjectToolchain::from_rocm(&rocm).expect("detect pinned ROCm llvm-mc")
    }

    fn empty_compiled_modules() -> CompiledModules {
        CompiledModules {
            modules: Vec::new(),
            allocator_module: None,
        }
    }

    #[test]
    #[ignore = "requires the configured ROCm LLVM toolchain"]
    fn generates_deterministic_bounded_object() {
        let toolchain = test_toolchain();
        let first_dir = TestDir::new();
        let second_dir = TestDir::new();
        let first = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &first_dir.0.join("fixture.o"),
            ID,
            KERNEL_BINDING,
            PAYLOAD,
        )
        .expect("generate first object");
        let second = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &second_dir.0.join("fixture.o"),
            ID,
            KERNEL_BINDING,
            PAYLOAD,
        )
        .expect("generate second object");

        assert_eq!(first.exact_object, second.exact_object);
        assert_eq!(
            first.pointer_symbol(),
            artifact_pointer_symbol_v1(KERNEL_BINDING)
        );
        assert_eq!(
            first.length_symbol(),
            artifact_length_symbol_v1(KERNEL_BINDING)
        );
        first.validate_unchanged().expect("object remains exact");
    }

    #[test]
    #[ignore = "requires the configured ROCm LLVM toolchain"]
    fn linked_accessors_return_exact_payload() {
        let toolchain = test_toolchain();
        let directory = TestDir::new();
        let object = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &directory.0.join("fixture.o"),
            ID,
            KERNEL_BINDING,
            PAYLOAD,
        )
        .expect("generate object");
        let mut host_objects = GeneratedHostObjects::default();
        host_objects
            .register(object.clone())
            .expect("register object");
        let mut compiled_modules = empty_compiled_modules();
        host_objects
            .append_to(&mut compiled_modules)
            .expect("append object");
        let linked_path = compiled_modules.modules[0]
            .object
            .as_deref()
            .expect("regular object path");
        assert_eq!(compiled_modules.modules[0].kind, ModuleKind::Regular);
        let expected = PAYLOAD
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let source = format!(
            r#"
unsafe extern "C" {{
    #[link_name = "{pointer}"]
    fn artifact_pointer() -> *const u8;
    #[link_name = "{length}"]
    fn artifact_length() -> usize;
}}

fn main() {{
    let pointer = unsafe {{ artifact_pointer() }};
    let length = unsafe {{ artifact_length() }};
    assert!(!pointer.is_null());
    let bytes = unsafe {{ core::slice::from_raw_parts(pointer, length) }};
    assert_eq!(bytes, &[{expected}]);
}}
"#,
            pointer = object.pointer_symbol(),
            length = object.length_symbol(),
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
            .arg(format!("link-arg={}", linked_path.display()))
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
    fn rejects_zero_and_oversized_payloads() {
        let directory = TestDir::new();
        let toolchain = HostObjectToolchain::for_test(PathBuf::from("/bin/true"));
        let output = directory.0.join("fixture.o");

        assert!(matches!(
            validate_request(&toolchain, SUPPORTED_HOST_TRIPLE, &output, ID, &[]),
            Err(HostObjectError::EmptyPayload)
        ));
        let oversized = vec![0; MAX_PAYLOAD_BYTES + 1];
        assert!(matches!(
            validate_request(
                &toolchain,
                SUPPORTED_HOST_TRIPLE,
                &output,
                ID,
                &oversized
            ),
            Err(HostObjectError::PayloadTooLarge(length))
                if length == MAX_PAYLOAD_BYTES + 1
        ));
    }

    #[test]
    fn refuses_to_overwrite_output() {
        let directory = TestDir::new();
        let toolchain = HostObjectToolchain::for_test(PathBuf::from("/bin/true"));
        let output = directory.0.join("fixture.o");
        fs::write(&output, b"existing object").expect("write occupied output");

        assert!(matches!(
            validate_request(
                &toolchain,
                SUPPORTED_HOST_TRIPLE,
                &output,
                ID,
                PAYLOAD
            ),
            Err(HostObjectError::OutputExists(path)) if path == output
        ));
    }

    #[test]
    fn rejects_failed_tool_without_retaining_partial_output() {
        let directory = TestDir::new();
        let output = directory.0.join("fixture.o");
        let fake_tool = directory.0.join("failing-llvm-mc");
        fs::write(
            &fake_tool,
            b"#!/bin/sh\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = -o ]; then\n    shift\n    printf partial > \"$1\"\n  fi\n  shift\ndone\ncat >/dev/null\nprintf 'synthetic failure\\n' >&2\nexit 7\n",
        )
        .expect("write deterministic failing tool");
        fs::set_permissions(&fake_tool, fs::Permissions::from_mode(0o700))
            .expect("make failing tool executable");
        let toolchain = HostObjectToolchain::for_test(fake_tool);

        let error = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &output,
            ID,
            KERNEL_BINDING,
            PAYLOAD,
        )
        .expect_err("failing tool must reject host object generation");
        assert!(matches!(
            error,
            HostObjectError::ToolFailed { ref status, ref stderr, .. }
                if status.contains('7') && stderr == "synthetic failure"
        ));
        assert!(!output.exists());
        assert!(matches!(
            generate_host_object(
                &toolchain,
                SUPPORTED_HOST_TRIPLE,
                &output,
                ID,
                KERNEL_BINDING,
                PAYLOAD
            ),
            Err(HostObjectError::ToolFailed { .. })
        ));
        assert!(!output.exists());
    }

    #[test]
    #[ignore = "requires the configured ROCm LLVM toolchain"]
    fn detects_object_replacement() {
        let toolchain = test_toolchain();
        let directory = TestDir::new();
        let object = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &directory.0.join("fixture.o"),
            ID,
            KERNEL_BINDING,
            PAYLOAD,
        )
        .expect("generate object");
        let mut replacement = fs::read(object.path()).expect("read object");
        let last = replacement.last_mut().expect("nonempty object");
        *last ^= 1;
        fs::write(object.path(), replacement).expect("replace object");

        assert!(matches!(
            object.validate_unchanged(),
            Err(HostObjectError::ObjectChanged(_))
        ));
    }

    #[test]
    #[ignore = "requires the configured ROCm LLVM toolchain"]
    fn rejects_duplicate_paths_and_accessors() {
        let toolchain = test_toolchain();
        let first_dir = TestDir::new();
        let second_dir = TestDir::new();
        let first = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &first_dir.0.join("fixture.o"),
            ID,
            KERNEL_BINDING,
            PAYLOAD,
        )
        .expect("generate first object");
        let second = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &second_dir.0.join("fixture.o"),
            OTHER_ID,
            KERNEL_BINDING,
            b"different synthetic payload",
        )
        .expect("generate second object");

        let mut duplicate_path = GeneratedHostObjects::default();
        duplicate_path
            .register(first.clone())
            .expect("first registration");
        assert!(matches!(
            duplicate_path.register(first),
            Err(HostObjectError::DuplicateObjectPath(_))
        ));

        let mut duplicate_symbols = GeneratedHostObjects::default();
        duplicate_symbols
            .register(second)
            .expect("second registration");
        let first_again = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &first_dir.0.join("fixture-again.o"),
            ID,
            KERNEL_BINDING,
            PAYLOAD,
        )
        .expect("generate alternate path");
        assert!(matches!(
            duplicate_symbols.register(first_again),
            Err(HostObjectError::SymbolCollision { .. })
        ));
    }

    #[test]
    #[ignore = "requires the configured ROCm LLVM toolchain"]
    fn rejects_missing_registered_object() {
        let toolchain = test_toolchain();
        let directory = TestDir::new();
        let object = generate_host_object(
            &toolchain,
            SUPPORTED_HOST_TRIPLE,
            &directory.0.join("fixture.o"),
            ID,
            KERNEL_BINDING,
            PAYLOAD,
        )
        .expect("generate object");
        let mut host_objects = GeneratedHostObjects::default();
        host_objects
            .register(object.clone())
            .expect("register object");
        fs::remove_file(object.path()).expect("remove object");

        assert!(matches!(
            host_objects.append_to(&mut empty_compiled_modules()),
            Err(HostObjectError::Io { .. })
        ));
    }

    #[test]
    fn empty_registry_leaves_ordinary_modules_untouched() {
        let missing_path = PathBuf::from("/missing/ordinary-module.o");
        let mut compiled_modules = CompiledModules {
            modules: vec![CompiledModule {
                name: "ordinary".to_owned(),
                kind: ModuleKind::Regular,
                object: Some(missing_path.clone()),
                dwarf_object: None,
                bytecode: None,
                assembly: None,
                llvm_ir: None,
                links_from_incr_cache: Vec::new(),
            }],
            allocator_module: None,
        };

        GeneratedHostObjects::default()
            .append_to(&mut compiled_modules)
            .expect("empty registry is a no-op");
        assert_eq!(compiled_modules.modules.len(), 1);
        assert_eq!(
            compiled_modules.modules[0].object.as_ref(),
            Some(&missing_path)
        );
    }
}
