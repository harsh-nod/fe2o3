#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::process::ExitCode;

use fe2o3_semantic_import::*;
use fe2o3_semantic_trace::*;

const MAX_ARGUMENTS: usize = 1_600;
const MAX_ARGUMENT_BYTES: usize = 512;

fn main() -> ExitCode {
    match run() {
        Ok(output) if std::io::stdout().lock().write_all(&output).is_ok() => ExitCode::SUCCESS,
        Ok(_) => fail("stdout_write"),
        Err(code) => fail(code),
    }
}

fn run() -> Result<Vec<u8>, &'static str> {
    let arguments = arguments()?;
    let request = RequestV4::parse(&arguments)?;
    let source = read_bounded()?;
    let environment = ProfilerEnvironmentBindingV4 {
        environment: request.environment.ok_or("environment")?,
        collector_tool: request.tool.ok_or("tool")?,
        collector_configuration: request.configuration.ok_or("configuration")?,
        stable_device_bindings: request.device_bindings,
    };
    let bundle = match request.command {
        CommandV4::Att => import_rocprofv3_att_profiler_bundle_v4(
            &source,
            ProfilerAttBindingV4 {
                environment,
                source_agent_id: request.att_agent_id.ok_or("att_agent_id")?,
                referenced_artifacts: request.att_artifacts,
            },
        ),
        CommandV4::DispatchJson | CommandV4::DispatchCsv => {
            if !request.att_artifacts.is_empty() {
                return Err("arguments");
            }
            let kernel_ir_claim = KernelIrIdentityClaimV1::canonical_v7_claim(
                request.kir_digest.ok_or("kir")?,
                request.kir_len.ok_or("kir")?,
            )
            .map_err(|_| "kir")?;
            let artifact = request.artifact.map(artifact_claim).transpose()?;
            let source_map = request.source_map.map(trace_content_identity).transpose()?;
            let binding = ProfilerDispatchBindingV4 {
                environment,
                kernel_ir_claim,
                artifact,
                source_map,
                wave_width: request.wave_width.ok_or("wave_width")?,
            };
            if request.command == CommandV4::DispatchJson {
                import_rocprofv3_json_profiler_bundle_v4(&source, binding)
            } else {
                import_rocprofv3_csv_profiler_bundle_v4(&source, binding)
            }
        }
    }
    .map_err(|_| "import")?;
    encode_profiler_bundle_v4(&bundle).map_err(|_| "encode")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandV4 {
    DispatchJson,
    DispatchCsv,
    Att,
}

struct RequestV4 {
    command: CommandV4,
    environment: Option<ContentIdentityRecordV1>,
    tool: Option<ContentIdentityRecordV1>,
    configuration: Option<ContentIdentityRecordV1>,
    device_bindings: Vec<ProfilerDeviceBindingV4>,
    att_agent_id: Option<u64>,
    kir_digest: Option<OpaqueIdentityV1>,
    kir_len: Option<u64>,
    wave_width: Option<WaveWidthV1>,
    artifact: Option<ContentIdentityRecordV1>,
    source_map: Option<ContentIdentityRecordV1>,
    att_artifacts: Vec<ProfilerAttArtifactBindingV4>,
}

impl RequestV4 {
    fn parse(arguments: &[String]) -> Result<Self, &'static str> {
        let command = match arguments.first().map(String::as_str) {
            Some("dispatch-json-v4") => CommandV4::DispatchJson,
            Some("dispatch-csv-v4") => CommandV4::DispatchCsv,
            Some("att-v4") => CommandV4::Att,
            _ => return Err("arguments"),
        };
        let mut request = Self {
            command,
            environment: None,
            tool: None,
            configuration: None,
            device_bindings: Vec::new(),
            att_agent_id: None,
            kir_digest: None,
            kir_len: None,
            wave_width: None,
            artifact: None,
            source_map: None,
            att_artifacts: Vec::new(),
        };
        let mut index = 1;
        while index < arguments.len() {
            let flag = arguments[index].as_str();
            let value = arguments.get(index + 1).ok_or("arguments")?;
            index += 2;
            match flag {
                "--environment" => set_once(&mut request.environment, parse_content(value)?)?,
                "--tool" => set_once(&mut request.tool, parse_content(value)?)?,
                "--config" => set_once(&mut request.configuration, parse_content(value)?)?,
                "--device-binding" => {
                    if request.device_bindings.len() == MAX_PROFILER_DEVICE_BINDINGS_V4 {
                        return Err("arguments");
                    }
                    let (source_agent_id, identity) =
                        value.split_once('=').ok_or("device_binding")?;
                    request.device_bindings.push(ProfilerDeviceBindingV4 {
                        source_agent_id: parse_canonical_u64(source_agent_id)?,
                        stable_identity: parse_content(identity)?,
                    });
                }
                "--att-agent-id" => {
                    set_once(&mut request.att_agent_id, parse_canonical_u64(value)?)?
                }
                "--kir-sha256" => set_once(&mut request.kir_digest, parse_opaque(value)?)?,
                "--kir-len" => set_once(&mut request.kir_len, parse_nonzero(value)?)?,
                "--wave-width" => set_once(
                    &mut request.wave_width,
                    match value.as_str() {
                        "32" => WaveWidthV1::Wave32,
                        "64" => WaveWidthV1::Wave64,
                        _ => return Err("wave_width"),
                    },
                )?,
                "--artifact" => set_once(&mut request.artifact, parse_content(value)?)?,
                "--source-map" => set_once(&mut request.source_map, parse_content(value)?)?,
                "--att-artifact" => {
                    if request.att_artifacts.len() == MAX_PROFILER_ATT_REFERENCES_V4 {
                        return Err("arguments");
                    }
                    let (reference, content) = value.split_once('=').ok_or("att_artifact")?;
                    if reference.is_empty() {
                        return Err("att_artifact");
                    }
                    request.att_artifacts.push(ProfilerAttArtifactBindingV4 {
                        reference: reference.to_owned(),
                        content: parse_content(content)?,
                    });
                }
                _ => return Err("arguments"),
            }
        }
        if request.device_bindings.is_empty() {
            return Err("device");
        }
        match command {
            CommandV4::Att
                if request.kir_digest.is_some()
                    || request.kir_len.is_some()
                    || request.wave_width.is_some()
                    || request.artifact.is_some()
                    || request.source_map.is_some()
                    || request.att_agent_id.is_none() =>
            {
                Err("arguments")
            }
            CommandV4::DispatchJson | CommandV4::DispatchCsv
                if request.kir_digest.is_none()
                    || request.kir_len.is_none()
                    || request.wave_width.is_none()
                    || request.att_agent_id.is_some() =>
            {
                Err("arguments")
            }
            _ => Ok(request),
        }
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), &'static str> {
    if slot.replace(value).is_some() {
        return Err("duplicate_argument");
    }
    Ok(())
}

fn parse_content(value: &str) -> Result<ContentIdentityRecordV1, &'static str> {
    let mut parts = value.split(':');
    let scheme = match parts.next() {
        Some("raw") => ContentSchemeV1::RawCanonicalSha256,
        Some("domain") => ContentSchemeV1::DomainSeparatedSha256,
        _ => return Err("content_identity"),
    };
    let format_version = parts
        .next()
        .ok_or("content_identity")?
        .parse::<u16>()
        .map_err(|_| "content_identity")?;
    let digest = parse_capture_identity(parts.next().ok_or("content_identity")?)?;
    let canonical_len = parse_nonzero(parts.next().ok_or("content_identity")?)?;
    if parts.next().is_some() || format_version == 0 {
        return Err("content_identity");
    }
    Ok(ContentIdentityRecordV1 {
        scheme,
        format_version,
        digest,
        canonical_len,
    })
}

fn artifact_claim(identity: ContentIdentityRecordV1) -> Result<ArtifactClaimV1, &'static str> {
    if identity.scheme != ContentSchemeV1::RawCanonicalSha256 {
        return Err("artifact");
    }
    Ok(ArtifactClaimV1 {
        identity: OpaqueIdentityV1::new(identity.digest.as_bytes()).map_err(|_| "artifact")?,
        canonical_len: identity.canonical_len,
        format_version: identity.format_version,
    })
}

fn trace_content_identity(
    identity: ContentIdentityRecordV1,
) -> Result<ContentIdentityV1, &'static str> {
    ContentIdentityV1::new(
        match identity.scheme {
            ContentSchemeV1::RawCanonicalSha256 => ContentIdentitySchemeV1::RawCanonicalSha256,
            ContentSchemeV1::DomainSeparatedSha256 => {
                ContentIdentitySchemeV1::DomainSeparatedSha256
            }
        },
        identity.format_version,
        OpaqueIdentityV1::new(identity.digest.as_bytes()).map_err(|_| "source_map")?,
        identity.canonical_len,
    )
    .map_err(|_| "source_map")
}

fn parse_opaque(value: &str) -> Result<OpaqueIdentityV1, &'static str> {
    OpaqueIdentityV1::new(parse_hex(value)?).map_err(|_| "identity")
}

fn parse_capture_identity(value: &str) -> Result<CaptureIdentityV1, &'static str> {
    CaptureIdentityV1::new(parse_hex(value)?).map_err(|_| "identity")
}

fn parse_hex(value: &str) -> Result<[u8; 32], &'static str> {
    if value.len() != 64 {
        return Err("identity");
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(bytes)
}

fn nibble(value: u8) -> Result<u8, &'static str> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err("identity"),
    }
}

fn parse_nonzero<T>(value: &str) -> Result<T, &'static str>
where
    T: std::str::FromStr + Default + PartialEq,
{
    let value = value.parse().map_err(|_| "number")?;
    if value == T::default() {
        return Err("number");
    }
    Ok(value)
}

fn parse_canonical_u64(value: &str) -> Result<u64, &'static str> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err("number");
    }
    value.parse().map_err(|_| "number")
}

fn arguments() -> Result<Vec<String>, &'static str> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(MAX_ARGUMENTS)
        .map_err(|_| "allocation")?;
    for argument in std::env::args_os().skip(1) {
        if output.len() == MAX_ARGUMENTS {
            return Err("arguments");
        }
        if argument.as_os_str().as_encoded_bytes().len() > MAX_ARGUMENT_BYTES {
            return Err("arguments");
        }
        output.push(argument.into_string().map_err(|_| "arguments")?);
    }
    Ok(output)
}

fn read_bounded() -> Result<Vec<u8>, &'static str> {
    let max = usize::try_from(MAX_PROFILER_SOURCE_BYTES_V4).map_err(|_| "input_limit")?;
    let mut input = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut reader = std::io::stdin().lock();
    loop {
        if input.len() == max {
            if reader.read(&mut buffer[..1]).map_err(|_| "stdin_read")? != 0 {
                return Err("input_too_large");
            }
            break;
        }
        let read_limit = buffer.len().min(max - input.len());
        let read = reader
            .read(&mut buffer[..read_limit])
            .map_err(|_| "stdin_read")?;
        if read == 0 {
            break;
        }
        input.try_reserve_exact(read).map_err(|_| "allocation")?;
        if input.capacity() > max {
            return Err("allocation");
        }
        input.extend_from_slice(&buffer[..read]);
    }
    Ok(input)
}

fn fail(code: &'static str) -> ExitCode {
    let _ = writeln!(std::io::stderr().lock(), "{{\"error\":\"{code}\"}}");
    ExitCode::FAILURE
}
