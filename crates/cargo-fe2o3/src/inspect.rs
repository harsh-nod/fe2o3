use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fe2o3_artifacts::{
    ArtifactContainerV1, BUNDLE_INDEX_MAGIC, BundleIndexV1, CONTAINER_MAGIC, Capability,
    CodeObjectFormat, DigestAlgorithm, DigestBytes, Endianness, MANIFEST_MAGIC,
    MAX_CONTAINER_BYTES, ManifestV1, PointerWidth, TargetIdentity,
};

use fe2o3_source_isa_observation::agent_v1::{
    SourceIsaAdmittedViewV1, SourceIsaFrameOutcomeViewV1, SourceIsaInspectionV1,
    inspect_source_isa_agent_json_v1, run_agent_source_isa_jsonl_v1,
};
use fe2o3_source_isa_observation::wire_v1::MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1;

const USAGE: &str = "usage: cargo fe2o3 inspect [--format auto|container|manifest|bundle|hsaco|source-isa-observation] [--output human|agent-json-v1] [<path>]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InspectFormat {
    Auto,
    Container,
    Manifest,
    Bundle,
    Hsaco,
    SourceIsaObservation,
}

impl InspectFormat {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "container" => Ok(Self::Container),
            "manifest" => Ok(Self::Manifest),
            "bundle" => Ok(Self::Bundle),
            "hsaco" => Ok(Self::Hsaco),
            "source-isa-observation" => Ok(Self::SourceIsaObservation),
            _ => Err(format!(
                "unknown inspect format `{value}`; expected auto, container, manifest, bundle, hsaco, or source-isa-observation"
            )),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    format: InspectFormat,
    output: InspectOutput,
    path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InspectOutput {
    Human,
    AgentJsonV1,
}

impl InspectOutput {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "human" => Ok(Self::Human),
            "agent-json-v1" => Ok(Self::AgentJsonV1),
            _ => Err(format!(
                "unknown inspect output `{value}`; expected human or agent-json-v1"
            )),
        }
    }
}

enum CommandAction {
    Print(String),
    AgentJsonl,
}

pub(crate) fn command(args: &[String]) -> ExitCode {
    match prepare_command(args) {
        Ok(CommandAction::Print(output)) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Ok(CommandAction::AgentJsonl) => {
            let stdin = io::stdin();
            let stdout = io::stdout();
            match run_agent_source_isa_jsonl_v1(&mut stdin.lock(), &mut stdout.lock()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("failed to serve inspect agent JSONL: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn prepare_command(args: &[String]) -> Result<CommandAction, String> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        return Ok(CommandAction::Print(USAGE.to_string()));
    }
    let options = parse_options(args)?;
    match (options.output, options.path) {
        (InspectOutput::Human, Some(path)) => {
            let bytes = read_bounded_for_format(&path, options.format)?;
            inspect_bytes(options.format, &bytes)
                .map(CommandAction::Print)
                .map_err(|error| format!("failed to inspect {}: {error}", path.display()))
        }
        (InspectOutput::AgentJsonV1, Some(path)) => {
            if options.format != InspectFormat::SourceIsaObservation {
                return Err(
                    "agent-json-v1 path inspection requires --format source-isa-observation"
                        .to_owned(),
                );
            }
            let bytes = read_bounded(&path, MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1)?;
            Ok(CommandAction::Print(inspect_source_isa_agent_json_v1(
                &bytes,
            )))
        }
        (InspectOutput::AgentJsonV1, None) => {
            if options.format != InspectFormat::Auto {
                return Err(
                    "agent JSONL stdin mode does not accept --format; the request selects its operation"
                        .to_owned(),
                );
            }
            Ok(CommandAction::AgentJsonl)
        }
        (InspectOutput::Human, None) => Err(format!("inspect requires a path\n{USAGE}")),
    }
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut format = InspectFormat::Auto;
    let mut format_seen = false;
    let mut output = InspectOutput::Human;
    let mut output_seen = false;
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
        } else if !positional_only && argument == "--output" {
            if output_seen {
                return Err("inspect output was specified more than once".to_string());
            }
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| "--output requires a value".to_string())?;
            output = InspectOutput::parse(value)?;
            output_seen = true;
        } else if !positional_only && argument.starts_with("--output=") {
            if output_seen {
                return Err("inspect output was specified more than once".to_string());
            }
            output = InspectOutput::parse(&argument["--output=".len()..])?;
            output_seen = true;
        } else if !positional_only && argument.starts_with('-') {
            return Err(format!("unknown inspect option `{argument}`\n{USAGE}"));
        } else if path.replace(PathBuf::from(argument)).is_some() {
            return Err(format!("inspect accepts exactly one path\n{USAGE}"));
        }
        index += 1;
    }

    Ok(Options {
        format,
        output,
        path,
    })
}

fn read_bounded_for_format(path: &Path, format: InspectFormat) -> Result<Vec<u8>, String> {
    let mut file = open_regular_file(path)?;
    let limit = match format {
        InspectFormat::SourceIsaObservation => MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1,
        InspectFormat::Auto => {
            let mut magic = [0_u8; 8];
            let read = file
                .read(&mut magic)
                .map_err(|error| format!("failed to read {} magic: {error}", path.display()))?;
            if read == magic.len() && &magic == b"F2SICOL1" {
                MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1
            } else {
                MAX_CONTAINER_BYTES
            }
        }
        _ => MAX_CONTAINER_BYTES,
    };
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("failed to rewind {}: {error}", path.display()))?;
    read_open_file_bounded(path, file, limit)
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    read_open_file_bounded(path, open_regular_file(path)?, limit)
}

fn open_regular_file(path: &Path) -> Result<File, String> {
    let file =
        File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("failed to stat {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    Ok(file)
}

fn read_open_file_bounded(path: &Path, file: File, limit: usize) -> Result<Vec<u8>, String> {
    let metadata = file
        .metadata()
        .map_err(|error| format!("failed to stat {}: {error}", path.display()))?;
    if metadata.len() > limit as u64 {
        return Err(format!(
            "{} exceeds the inspect input limit of {limit} bytes",
            path.display()
        ));
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if bytes.len() > limit {
        return Err(format!(
            "{} grew beyond the inspect input limit of {limit} bytes",
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
        InspectFormat::SourceIsaObservation => SourceIsaInspectionV1::decode_canonical(bytes)
            .map(|inspection| render_source_isa_observation(&inspection))
            .map_err(|error| format!("invalid source/ISA observation collection: {error}")),
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
    } else if bytes.starts_with(b"F2SICOL1") {
        Ok(InspectFormat::SourceIsaObservation)
    } else {
        Err("unrecognized input magic; use --format only when the input is one of the supported bounded formats".to_string())
    }
}

fn render_source_isa_observation(inspection: &SourceIsaInspectionV1) -> String {
    let collection = &inspection.collection;
    let mut output = String::new();
    writeln!(output, "format: {}", collection.format).expect("write to String");
    writeln!(output, "authority: observation-only").expect("write to String");
    writeln!(output, "compiler-authority: false").expect("write to String");
    writeln!(output, "proof-authority: false").expect("write to String");
    writeln!(output, "artifact-authority: false").expect("write to String");
    writeln!(output, "runtime-authority: false").expect("write to String");
    writeln!(output, "hardware-execution-observed: false").expect("write to String");
    writeln!(output, "complete-machine-coverage-proved: false").expect("write to String");
    writeln!(output, "semantic-refinement-proved: false").expect("write to String");
    writeln!(
        output,
        "configuration: {}",
        collection.configuration_identity
    )
    .expect("write to String");
    writeln!(output, "session: {}", collection.session).expect("write to String");
    writeln!(output, "frames: {}", collection.frame_count).expect("write to String");
    writeln!(output, "missing-units: {}", collection.missing_unit_count).expect("write to String");
    writeln!(
        output,
        "transport-failure: {}",
        collection
            .transport_failure
            .as_ref()
            .map_or(0, |failure| failure.code)
    )
    .expect("write to String");

    for (index, frame) in inspection.frames().enumerate() {
        writeln!(
            output,
            "frame[{index}].identity: {}",
            frame.frame_evidence.digest
        )
        .expect("write to String");
        writeln!(output, "frame[{index}].unit: {}", frame.unit_identity).expect("write to String");
        writeln!(
            output,
            "frame[{index}].attempt: {}",
            format!(
                "{}:{}:{}",
                frame.attempt.generation, frame.attempt.session, frame.attempt.invocation_identity
            )
        )
        .expect("write to String");
        writeln!(
            output,
            "frame[{index}].finalization: {}",
            frame.finalization_identity
        )
        .expect("write to String");
        match &frame.outcome {
            SourceIsaFrameOutcomeViewV1::Admitted { evidence } => {
                render_admitted_source_isa_observation(&mut output, index, evidence);
            }
            SourceIsaFrameOutcomeViewV1::Unavailable { reason } => {
                writeln!(output, "frame[{index}].outcome: unavailable").expect("write to String");
                writeln!(output, "frame[{index}].reason-code: {}", reason.code)
                    .expect("write to String");
                writeln!(output, "frame[{index}].reason: {}", reason.label)
                    .expect("write to String");
            }
            SourceIsaFrameOutcomeViewV1::Error { error } => {
                writeln!(output, "frame[{index}].outcome: error").expect("write to String");
                writeln!(output, "frame[{index}].error-code: {}", error.code)
                    .expect("write to String");
                writeln!(output, "frame[{index}].error: {}", error.label).expect("write to String");
            }
        }
    }
    for (index, unit) in inspection.missing_units().enumerate() {
        writeln!(output, "missing-unit[{index}]: {unit}").expect("write to String");
    }
    trim_final_newline(output)
}

fn render_admitted_source_isa_observation(
    output: &mut String,
    index: usize,
    admitted: &SourceIsaAdmittedViewV1,
) {
    let artifact = &admitted.artifact;
    let structural = &admitted.structural;
    let records = admitted.records;
    let queries = admitted.queries;
    writeln!(output, "frame[{index}].outcome: admitted").expect("write to String");
    writeln!(
        output,
        "frame[{index}].correlation: {}",
        admitted.correlation_identity
    )
    .expect("write to String");
    writeln!(
        output,
        "frame[{index}].artifact: sha256={} bytes={}",
        artifact.sha256, artifact.byte_len
    )
    .expect("write to String");
    writeln!(
        output,
        "frame[{index}].target: {}",
        format!(
            "{}:{}",
            structural.target.architecture, structural.target.features[1]
        )
    )
    .expect("write to String");
    writeln!(
        output,
        "frame[{index}].kir-version: {}",
        structural.kir_version
    )
    .expect("write to String");
    writeln!(
        output,
        "frame[{index}].structural: identity={} functions={} defined-bodies={} blocks={} operations={} neutral-kir-sha256={} neutral-kir-bytes={} target-kir-sha256={} target-kir-bytes={}",
        structural.identity,
        structural.functions,
        structural.defined_bodies,
        structural.blocks,
        structural.operations,
        structural.neutral_kir.sha256,
        structural.neutral_kir.byte_len,
        structural.target_kir.sha256,
        structural.target_kir.byte_len,
    )
    .expect("write to String");
    writeln!(
        output,
        "frame[{index}].records: total={} source-anchored={} eliminated={} no-source={} source-anchored-without-isa={} isa-references={}",
        records.total,
        records.source_anchored,
        records.eliminated,
        records.no_source,
        records.source_anchored_without_isa,
        records.isa_references,
    )
    .expect("write to String");
    writeln!(
        output,
        "frame[{index}].queries: source-nodes={} source-spans={} isa-points={} max-source-node-cardinality={} max-source-span-cardinality={} max-exact-pc-cardinality={}",
        queries.distinct_source_nodes,
        queries.distinct_source_spans,
        queries.distinct_isa_points,
        queries.max_source_node_cardinality,
        queries.max_source_span_cardinality,
        queries.max_exact_pc_cardinality,
    )
    .expect("write to String");
    match &admitted.round_trip {
        Some(witness) => {
            let span = &witness.source_span;
            let isa = witness.isa_point;
            writeln!(
                output,
                "frame[{index}].round-trip: source-node={} file={} byte-start={} byte-end={} line={} column={} kernel-ordinal={} symbol-relative-pc={} source-node-matches={} source-span-matches={} isa-point-matches={}",
                witness.source_node_identity,
                span.file_identity,
                span.byte_start,
                span.byte_end,
                span.line,
                span.column,
                isa.kernel_ordinal,
                isa.symbol_relative_pc,
                witness.source_node_query_matches,
                witness.source_span_query_matches,
                witness.isa_point_query_matches,
            )
            .expect("write to String");
        }
        None => {
            writeln!(output, "frame[{index}].round-trip: unavailable").expect("write to String");
        }
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
    hex_bytes(value.as_bytes())
}

fn hex_bytes(value: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len().saturating_mul(2));
    for byte in value {
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
    use super::{
        InspectFormat, InspectOutput, Options, detect_format, inspect_bytes, parse_options,
    };
    use fe2o3_artifact_transaction::{BuildAttempt, BuildInvocation, BuildSession};
    use fe2o3_artifacts::{BUNDLE_INDEX_MAGIC, CONTAINER_MAGIC, MANIFEST_MAGIC};
    use fe2o3_source_isa_observation::wire_v1::{
        AdmittedSourceIsaObservationV1, SourceIsaObservationContentIdentityV1,
        SourceIsaObservationContextV1, SourceIsaObservationCountsV1,
        SourceIsaObservationErrorCodeV1, SourceIsaObservationFrameV1,
        SourceIsaObservationIsaPointV1, SourceIsaObservationKirVersionV1,
        SourceIsaObservationOutcomeV1, SourceIsaObservationQueryCountsV1,
        SourceIsaObservationRecordCountsV1, SourceIsaObservationRoundTripWitnessV1,
        SourceIsaObservationSourceSpanV1, SourceIsaObservationStructuralBindingV1,
        SourceIsaObservationStructuralCountsV1, SourceIsaObservationTargetProfileV1,
        SourceIsaObservationUnavailableReasonV1,
    };
    use sha2::{Digest, Sha256};
    use std::path::PathBuf;

    const CONFIG: [u8; 32] = [0x11; 32];
    const SESSION: [u8; 16] = [0x12; 16];

    fn attempt(generation: u64, invocation: u8) -> BuildAttempt {
        BuildAttempt::from_env_value(&format!(
            "{generation}:{}:{}",
            BuildSession::from_bytes(SESSION),
            BuildInvocation::from_bytes([invocation; 32])
        ))
        .expect("valid observer test attempt")
    }

    fn admitted_frame(
        unit: u8,
        target: SourceIsaObservationTargetProfileV1,
        operations: u64,
    ) -> SourceIsaObservationFrameV1 {
        let content = |byte, length| {
            SourceIsaObservationContentIdentityV1::new([byte; 32], length)
                .expect("valid observer content identity")
        };
        let structural = SourceIsaObservationStructuralBindingV1::new(
            [unit.wrapping_add(0x20); 32],
            target,
            SourceIsaObservationKirVersionV1::V8,
            content(unit.wrapping_add(0x30), 100 + operations),
            content(unit.wrapping_add(0x40), 200 + operations),
            SourceIsaObservationStructuralCountsV1 {
                functions: 1,
                defined_bodies: 1,
                blocks: operations,
                operations,
            },
        )
        .expect("valid observer structural binding");
        let counts = SourceIsaObservationCountsV1::new(
            SourceIsaObservationRecordCountsV1 {
                records: operations,
                source_anchored: operations,
                eliminated: 0,
                no_source: 0,
                source_anchored_without_isa: 0,
                isa_references: operations,
            },
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: operations,
                distinct_source_spans: operations,
                distinct_isa_points: operations,
                max_source_node_cardinality: 1,
                max_source_span_cardinality: 1,
                max_exact_pc_cardinality: 1,
            },
        )
        .expect("valid observer counts");
        let witness = SourceIsaObservationRoundTripWitnessV1::new(
            [unit.wrapping_add(0x50); 32],
            SourceIsaObservationSourceSpanV1::new([unit.wrapping_add(0x60); 32], 10, 14, 2, 3)
                .expect("valid observer source span"),
            SourceIsaObservationIsaPointV1::new(0, 16).expect("valid observer ISA point"),
            1,
            1,
            1,
        )
        .expect("valid observer round trip");
        let admitted = AdmittedSourceIsaObservationV1::new(
            [unit.wrapping_add(0x10); 32],
            content(unit.wrapping_add(0x70), 4096 + operations),
            structural,
            counts,
            Some(witness),
        )
        .expect("valid admitted observer result");
        observer_frame(unit, SourceIsaObservationOutcomeV1::Admitted(admitted))
    }

    fn observer_frame(
        unit: u8,
        outcome: SourceIsaObservationOutcomeV1,
    ) -> SourceIsaObservationFrameV1 {
        SourceIsaObservationFrameV1::new(
            SourceIsaObservationContextV1::new(
                CONFIG,
                [unit; 32],
                crate::source_isa_observation::inert_source_isa_attempt_v1(attempt(
                    u64::from(unit),
                    unit.wrapping_add(1),
                ))
                .expect("valid inert observer attempt"),
                [unit.wrapping_add(2); 32],
            )
            .expect("valid observer context"),
            outcome,
        )
    }

    fn collection(frames: &[SourceIsaObservationFrameV1]) -> Vec<u8> {
        const HEADER_BYTES: usize = 80;
        const IDENTITY_BYTES: usize = 32;
        const DOMAIN: &[u8] = b"FE2O3/SOURCE-ISA-OBSERVATION-COLLECTION/V1\0";
        let total = HEADER_BYTES + frames.len() * 680 + IDENTITY_BYTES;
        let mut bytes = Vec::with_capacity(total);
        bytes.extend_from_slice(b"F2SICOL1");
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&(HEADER_BYTES as u16).to_le_bytes());
        bytes.extend_from_slice(&(total as u32).to_le_bytes());
        bytes.extend_from_slice(&(frames.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&CONFIG);
        bytes.extend_from_slice(&SESSION);
        for frame in frames {
            bytes.extend_from_slice(&frame.encode());
        }
        let mut digest = Sha256::new();
        digest.update(DOMAIN);
        digest.update(&bytes);
        bytes.extend_from_slice(&digest.finalize());
        assert_eq!(bytes.len(), total);
        bytes
    }

    #[test]
    fn parses_strict_inspect_options() {
        assert_eq!(
            parse_options(&["--format=manifest".into(), "artifact.bin".into()]),
            Ok(Options {
                format: InspectFormat::Manifest,
                output: InspectOutput::Human,
                path: Some(PathBuf::from("artifact.bin")),
            })
        );
        assert_eq!(
            parse_options(&[
                "--format=source-isa-observation".into(),
                "observations.bin".into(),
            ]),
            Ok(Options {
                format: InspectFormat::SourceIsaObservation,
                output: InspectOutput::Human,
                path: Some(PathBuf::from("observations.bin")),
            })
        );
        assert_eq!(
            parse_options(&["--".into(), "-artifact.bin".into()]),
            Ok(Options {
                format: InspectFormat::Auto,
                output: InspectOutput::Human,
                path: Some(PathBuf::from("-artifact.bin")),
            })
        );
        assert_eq!(
            parse_options(&["--output=agent-json-v1".into()]),
            Ok(Options {
                format: InspectFormat::Auto,
                output: InspectOutput::AgentJsonV1,
                path: None,
            })
        );
    }

    #[test]
    fn rejects_missing_duplicate_and_unknown_options() {
        for args in [
            vec!["one".into(), "two".into()],
            vec!["--unknown".into()],
            vec!["--format".into()],
            vec!["--format=raw".into(), "one".into()],
            vec!["--output=raw".into()],
            vec!["--output=human".into(), "--output=agent-json-v1".into()],
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
        assert_eq!(
            detect_format(b"F2SICOL1"),
            Ok(InspectFormat::SourceIsaObservation)
        );
        assert!(detect_format(b"unknown").is_err());
    }

    #[test]
    fn synthetic_source_isa_six_frame_fixture_preserves_targets_structures_and_non_authority() {
        let frames = [
            admitted_frame(0x10, SourceIsaObservationTargetProfileV1::Gfx942, 4),
            admitted_frame(0x11, SourceIsaObservationTargetProfileV1::Gfx950, 4),
            admitted_frame(0x20, SourceIsaObservationTargetProfileV1::Gfx942, 16),
            admitted_frame(0x21, SourceIsaObservationTargetProfileV1::Gfx950, 16),
            admitted_frame(0x30, SourceIsaObservationTargetProfileV1::Gfx942, 64),
            admitted_frame(0x31, SourceIsaObservationTargetProfileV1::Gfx950, 64),
        ];
        let bytes = collection(&frames);
        let output = inspect_bytes(InspectFormat::Auto, &bytes).expect("inspect observer matrix");
        for required in [
            "format: fe2o3-source-isa-observation-collection-v1",
            "authority: observation-only",
            "compiler-authority: false",
            "proof-authority: false",
            "artifact-authority: false",
            "runtime-authority: false",
            "hardware-execution-observed: false",
            "complete-machine-coverage-proved: false",
            "semantic-refinement-proved: false",
            "frames: 6",
            "frame[0].target: gfx942:xnack-",
            "frame[1].target: gfx950:xnack-",
            "frame[2].structural:",
            "operations=16",
            "frame[4].structural:",
            "operations=64",
            "frame[5].round-trip:",
        ] {
            assert!(
                output.contains(required),
                "observer matrix omitted {required:?}:\n{output}"
            );
        }
        assert_eq!(output.matches(".outcome: admitted").count(), 6);
        assert_eq!(output.matches("target: gfx942:xnack-").count(), 3);
        assert_eq!(output.matches("target: gfx950:xnack-").count(), 3);
    }

    #[test]
    fn source_isa_inspection_preserves_missing_extension_as_unavailable() {
        let frame = observer_frame(
            0x40,
            SourceIsaObservationOutcomeV1::Unavailable(
                SourceIsaObservationUnavailableReasonV1::CarrierReceiptExtensionConstructionUnavailable,
            ),
        );
        let output = inspect_bytes(InspectFormat::SourceIsaObservation, &collection(&[frame]))
            .expect("inspect unavailable observer result");
        assert!(output.contains("frame[0].outcome: unavailable"));
        assert!(output.contains("frame[0].reason-code: 11"));
        assert!(
            output.contains("frame[0].reason: carrier-receipt-extension-construction-unavailable")
        );
        assert!(!output.contains("frame[0].target:"));
        assert!(!output.contains("frame[0].round-trip:"));
    }

    #[test]
    fn source_isa_inspection_preserves_typed_error_without_admitted_payload() {
        let frame = observer_frame(
            0x41,
            SourceIsaObservationOutcomeV1::Error(
                SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidArtifact,
            ),
        );
        let output = inspect_bytes(InspectFormat::SourceIsaObservation, &collection(&[frame]))
            .expect("inspect observer error result");
        assert!(output.contains("frame[0].outcome: error"));
        assert!(output.contains("frame[0].error-code: 8202"));
        assert!(output.contains("frame[0].error: semantic-anchor-invalid-artifact"));
        assert!(!output.contains("frame[0].target:"));
        assert!(!output.contains("frame[0].round-trip:"));
    }

    #[test]
    fn source_isa_inspection_rejects_corruption_and_trailing_bytes() {
        let exact = collection(&[admitted_frame(
            0x10,
            SourceIsaObservationTargetProfileV1::Gfx942,
            4,
        )]);
        let mut corrupted = exact.clone();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        assert!(
            inspect_bytes(InspectFormat::SourceIsaObservation, &corrupted)
                .unwrap_err()
                .contains("identity differs from its bytes")
        );

        let mut trailing = exact;
        trailing.push(0);
        assert!(
            inspect_bytes(InspectFormat::SourceIsaObservation, &trailing)
                .unwrap_err()
                .contains("malformed framing")
        );

        let mut unknown_version = collection(&[admitted_frame(
            0x10,
            SourceIsaObservationTargetProfileV1::Gfx942,
            4,
        )]);
        unknown_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
        assert!(
            inspect_bytes(InspectFormat::SourceIsaObservation, &unknown_version)
                .unwrap_err()
                .contains("malformed framing")
        );
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
