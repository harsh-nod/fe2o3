use std::fmt::Write as _;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use fe2o3_artifacts::{
    ArtifactContainerV1, BUNDLE_INDEX_MAGIC, BundleIndexV1, CONTAINER_MAGIC, Capability,
    CodeObjectFormat, DigestAlgorithm, DigestBytes, Endianness, MANIFEST_MAGIC,
    MAX_CONTAINER_BYTES, ManifestV1, PointerWidth, TargetIdentity,
};

const USAGE: &str =
    "usage: cargo fe2o3 inspect [--format auto|container|manifest|bundle|hsaco] <path>";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InspectFormat {
    Auto,
    Container,
    Manifest,
    Bundle,
    Hsaco,
}

impl InspectFormat {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "container" => Ok(Self::Container),
            "manifest" => Ok(Self::Manifest),
            "bundle" => Ok(Self::Bundle),
            "hsaco" => Ok(Self::Hsaco),
            _ => Err(format!(
                "unknown inspect format `{value}`; expected auto, container, manifest, bundle, or hsaco"
            )),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    format: InspectFormat,
    path: PathBuf,
}

pub(crate) fn command(args: &[String]) -> Result<String, String> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        return Ok(USAGE.to_string());
    }
    let options = parse_options(args)?;
    let bytes = read_bounded(&options.path)?;
    inspect_bytes(options.format, &bytes)
        .map_err(|error| format!("failed to inspect {}: {error}", options.path.display()))
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut format = InspectFormat::Auto;
    let mut format_seen = false;
    let mut path = None;
    let mut positional_only = false;
    let mut index = 0;

    while index < args.len() {
        let argument = &args[index];
        if !positional_only && argument == "--" {
            positional_only = true;
        } else if !positional_only && argument == "--format" {
            if format_seen {
                return Err("inspect format was specified more than once".to_string());
            }
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| "--format requires a value".to_string())?;
            format = InspectFormat::parse(value)?;
            format_seen = true;
        } else if !positional_only && argument.starts_with("--format=") {
            if format_seen {
                return Err("inspect format was specified more than once".to_string());
            }
            format = InspectFormat::parse(&argument["--format=".len()..])?;
            format_seen = true;
        } else if !positional_only && argument.starts_with('-') {
            return Err(format!("unknown inspect option `{argument}`\n{USAGE}"));
        } else if path.replace(PathBuf::from(argument)).is_some() {
            return Err(format!("inspect accepts exactly one path\n{USAGE}"));
        }
        index += 1;
    }

    let path = path.ok_or_else(|| format!("inspect requires a path\n{USAGE}"))?;
    Ok(Options { format, path })
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let file =
        File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("failed to stat {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    if metadata.len() > MAX_CONTAINER_BYTES as u64 {
        return Err(format!(
            "{} exceeds the inspect input limit of {MAX_CONTAINER_BYTES} bytes",
            path.display()
        ));
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_CONTAINER_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if bytes.len() > MAX_CONTAINER_BYTES {
        return Err(format!(
            "{} grew beyond the inspect input limit of {MAX_CONTAINER_BYTES} bytes",
            path.display()
        ));
    }
    Ok(bytes)
}

fn inspect_bytes(format: InspectFormat, bytes: &[u8]) -> Result<String, String> {
    let format = match format {
        InspectFormat::Auto => detect_format(bytes)?,
        explicit => explicit,
    };

    match format {
        InspectFormat::Auto => unreachable!("auto format was resolved above"),
        InspectFormat::Container => ArtifactContainerV1::from_bytes(bytes)
            .map(|container| render_container(&container))
            .map_err(|error| format!("invalid fe2o3 container: {error}")),
        InspectFormat::Manifest => ManifestV1::from_bytes(bytes)
            .map(|manifest| render_manifest(&manifest, "fe2o3-manifest-v1"))
            .map_err(|error| format!("invalid fe2o3 manifest: {error}")),
        InspectFormat::Bundle => BundleIndexV1::from_bytes(bytes)
            .map(|bundle| render_bundle(&bundle))
            .map_err(|error| format!("invalid fe2o3 bundle index: {error}")),
        InspectFormat::Hsaco => fe2o3_hsaco::inspect(bytes)
            .map(|hsaco| render_hsaco(&hsaco))
            .map_err(|error| format!("invalid HSACO: {error}")),
    }
}

fn detect_format(bytes: &[u8]) -> Result<InspectFormat, String> {
    if bytes.starts_with(&CONTAINER_MAGIC) {
        Ok(InspectFormat::Container)
    } else if bytes.starts_with(&MANIFEST_MAGIC) {
        Ok(InspectFormat::Manifest)
    } else if bytes.starts_with(&BUNDLE_INDEX_MAGIC) {
        Ok(InspectFormat::Bundle)
    } else if bytes.starts_with(b"\x7fELF") {
        Ok(InspectFormat::Hsaco)
    } else {
        Err("unrecognized input magic; use --format only when the input is one of the supported bounded formats".to_string())
    }
}

fn render_container(container: &ArtifactContainerV1) -> String {
    let mut output = render_manifest(container.manifest(), "fe2o3-container-v1");
    output.push('\n');
    writeln!(
        output,
        "digest-algorithm: {}",
        digest_algorithm(container.digest_algorithm())
    )
    .expect("write to String");
    writeln!(output, "payloads: {}", container.payloads().len()).expect("write to String");
    for (index, payload) in container.payloads().iter().enumerate() {
        writeln!(
            output,
            "payload[{index}]: digest={} bytes={}",
            hex(payload.digest().bytes()),
            payload.bytes().len()
        )
        .expect("write to String");
    }
    trim_final_newline(output)
}

fn render_manifest(manifest: &ManifestV1, format: &str) -> String {
    let mut output = String::new();
    writeln!(output, "format: {format}").expect("write to String");
    writeln!(output, "authority: descriptive-only").expect("write to String");
    writeln!(
        output,
        "compiler: {} {}",
        manifest.compiler().name().as_str(),
        manifest.compiler().version().as_str()
    )
    .expect("write to String");
    writeln!(
        output,
        "producer: {} {}",
        manifest.producer().name().as_str(),
        manifest.producer().version().as_str()
    )
    .expect("write to String");
    write_target(&mut output, manifest.target(), "target");
    writeln!(output, "code-objects: {}", manifest.code_objects().len()).expect("write to String");
    for (index, object) in manifest.code_objects().iter().enumerate() {
        writeln!(
            output,
            "code-object[{index}]: digest={} format={} bytes={}",
            hex(object.digest()),
            code_object_format(object.format()),
            object.byte_len()
        )
        .expect("write to String");
    }
    writeln!(output, "kernels: {}", manifest.kernels().len()).expect("write to String");
    for (index, kernel) in manifest.kernels().iter().enumerate() {
        writeln!(
            output,
            "kernel[{index}]: id={} name={} symbol={} object={} abi-bytes={} abi-align={} abi-fields={}",
            hex(kernel.kernel_id()),
            kernel.name().as_str(),
            kernel.symbol().as_str(),
            hex(kernel.code_object_digest()),
            kernel.abi().size(),
            kernel.abi().alignment(),
            kernel.abi().fields().len()
        )
        .expect("write to String");
    }
    trim_final_newline(output)
}

fn render_bundle(bundle: &BundleIndexV1) -> String {
    let mut output = String::new();
    writeln!(output, "format: fe2o3-bundle-index-v1").expect("write to String");
    writeln!(output, "authority: descriptive-only").expect("write to String");
    writeln!(
        output,
        "target-associations: {}",
        bundle.target_associations().len()
    )
    .expect("write to String");
    for (index, association) in bundle.target_associations().iter().enumerate() {
        writeln!(
            output,
            "target-association[{index}].manifest: {}",
            hex(association.manifest_digest())
        )
        .expect("write to String");
        write_target(
            &mut output,
            association.target(),
            &format!("target-association[{index}].target"),
        );
    }
    writeln!(output, "payload-references: {}", bundle.payloads().len()).expect("write to String");
    for (index, payload) in bundle.payloads().iter().enumerate() {
        writeln!(
            output,
            "payload-reference[{index}]: digest={} format={} bytes={}",
            hex(payload.digest()),
            code_object_format(payload.format()),
            payload.byte_len()
        )
        .expect("write to String");
    }
    writeln!(output, "kernels: {}", bundle.kernels().len()).expect("write to String");
    for (index, kernel) in bundle.kernels().iter().enumerate() {
        writeln!(
            output,
            "kernel[{index}]: id={} symbol={} manifest={} payload-references={}",
            hex(kernel.kernel_id()),
            kernel.symbol().as_str(),
            hex(kernel.manifest_digest()),
            kernel.payload_digests().len()
        )
        .expect("write to String");
    }
    trim_final_newline(output)
}

fn render_hsaco(hsaco: &fe2o3_hsaco::InspectedHsaco) -> String {
    let mut output = String::new();
    writeln!(
        output,
        "format: hsaco-v{}",
        hsaco.code_object_version().number()
    )
    .expect("write to String");
    writeln!(output, "authority: descriptive-only").expect("write to String");
    writeln!(
        output,
        "metadata-version: {}.{}",
        hsaco.metadata_version().major(),
        hsaco.metadata_version().minor()
    )
    .expect("write to String");
    writeln!(output, "target: {}", hsaco.target()).expect("write to String");
    writeln!(output, "printf-metadata: {}", hsaco.has_printf_metadata()).expect("write to String");
    writeln!(output, "kernels: {}", hsaco.kernels().len()).expect("write to String");
    for (index, kernel) in hsaco.kernels().iter().enumerate() {
        writeln!(
            output,
            "kernel[{index}]: name={} symbol={} kernarg-bytes={} kernarg-align={} wave={} lds-bytes={} private-bytes={} explicit-args={} hidden-args={} sgprs={} vgprs={}",
            kernel.name(),
            kernel.symbol(),
            kernel.kernarg_segment_size(),
            kernel.kernarg_segment_alignment(),
            kernel.wavefront_size(),
            kernel.group_segment_fixed_size(),
            kernel.private_segment_fixed_size(),
            kernel.explicit_arguments().len(),
            kernel.hidden_arguments().len(),
            kernel.sgpr_count(),
            kernel.vgpr_count()
        )
        .expect("write to String");
    }
    trim_final_newline(output)
}

fn write_target(output: &mut String, target: &TargetIdentity, prefix: &str) {
    writeln!(
        output,
        "{prefix}: triple={} architecture={} pointer-width={} endianness={} capabilities={}",
        target.triple().as_str(),
        target.architecture().as_str(),
        pointer_width(target.pointer_width()),
        endianness(target.endianness()),
        target
            .capabilities()
            .iter()
            .map(|capability| capability_name(*capability))
            .collect::<Vec<_>>()
            .join(",")
    )
    .expect("write to String");
}

const fn pointer_width(width: PointerWidth) -> u8 {
    match width {
        PointerWidth::Bits32 => 32,
        PointerWidth::Bits64 => 64,
    }
}

const fn endianness(value: Endianness) -> &'static str {
    match value {
        Endianness::Little => "little",
        Endianness::Big => "big",
    }
}

const fn digest_algorithm(value: DigestAlgorithm) -> &'static str {
    match value {
        DigestAlgorithm::Sha256 => "sha256",
        _ => "unknown",
    }
}

const fn code_object_format(value: CodeObjectFormat) -> &'static str {
    match value {
        CodeObjectFormat::NativeExecutable => "native-executable",
        CodeObjectFormat::RelocatableObject => "relocatable-object",
        CodeObjectFormat::LlvmBitcode => "llvm-bitcode",
        CodeObjectFormat::SpirV => "spir-v",
    }
}

const fn capability_name(value: Capability) -> &'static str {
    match value {
        Capability::Subgroup => "subgroup",
        Capability::Ballot => "ballot",
        Capability::Shuffle => "shuffle",
        Capability::WorkgroupMemory => "workgroup-memory",
        Capability::MatrixMultiply => "matrix-multiply",
        Capability::AsyncCopy => "async-copy",
        Capability::Atomics => "atomics",
        Capability::AmdWave => "amd-wave",
        Capability::AmdMfma => "amd-mfma",
        Capability::AmdWmma => "amd-wmma",
        Capability::AmdDsPermute => "amd-ds-permute",
    }
}

fn hex(value: DigestBytes) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in value.as_bytes() {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn trim_final_newline(mut output: String) -> String {
    if output.ends_with('\n') {
        output.pop();
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{InspectFormat, Options, detect_format, inspect_bytes, parse_options};
    use fe2o3_artifacts::{BUNDLE_INDEX_MAGIC, CONTAINER_MAGIC, MANIFEST_MAGIC};
    use std::path::PathBuf;

    #[test]
    fn parses_strict_inspect_options() {
        assert_eq!(
            parse_options(&["--format=manifest".into(), "artifact.bin".into()]),
            Ok(Options {
                format: InspectFormat::Manifest,
                path: PathBuf::from("artifact.bin"),
            })
        );
        assert_eq!(
            parse_options(&["--".into(), "-artifact.bin".into()]),
            Ok(Options {
                format: InspectFormat::Auto,
                path: PathBuf::from("-artifact.bin"),
            })
        );
    }

    #[test]
    fn rejects_missing_duplicate_and_unknown_options() {
        for args in [
            vec![],
            vec!["one".into(), "two".into()],
            vec!["--unknown".into()],
            vec!["--format".into()],
            vec!["--format=raw".into(), "one".into()],
            vec![
                "--format=auto".into(),
                "--format=hsaco".into(),
                "one".into(),
            ],
        ] {
            assert!(parse_options(&args).is_err(), "accepted {args:?}");
        }
    }

    #[test]
    fn detects_only_supported_magic_values() {
        assert_eq!(
            detect_format(&CONTAINER_MAGIC),
            Ok(InspectFormat::Container)
        );
        assert_eq!(detect_format(&MANIFEST_MAGIC), Ok(InspectFormat::Manifest));
        assert_eq!(
            detect_format(&BUNDLE_INDEX_MAGIC),
            Ok(InspectFormat::Bundle)
        );
        assert_eq!(detect_format(b"\x7fELF"), Ok(InspectFormat::Hsaco));
        assert!(detect_format(b"unknown").is_err());
    }

    #[test]
    fn malformed_inputs_return_format_specific_diagnostics() {
        let manifest = inspect_bytes(InspectFormat::Manifest, &MANIFEST_MAGIC).unwrap_err();
        let bundle = inspect_bytes(InspectFormat::Bundle, &BUNDLE_INDEX_MAGIC).unwrap_err();
        let hsaco = inspect_bytes(InspectFormat::Hsaco, b"\x7fELF").unwrap_err();
        assert!(manifest.starts_with("invalid fe2o3 manifest:"));
        assert!(bundle.starts_with("invalid fe2o3 bundle index:"));
        assert!(hsaco.starts_with("invalid HSACO:"));
    }
}
