#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::ExitCode;

use fe2o3_semantic_import::*;
use fe2o3_semantic_trace::*;
use serde::Serialize;

const MAX_ARGUMENTS: usize = 32;
const MAX_ARGUMENT_BYTES: usize = 256;

fn main() -> ExitCode {
    match run() {
        Ok(output) => {
            if std::io::stdout().lock().write_all(&output).is_ok() {
                ExitCode::SUCCESS
            } else {
                emit_error(
                    "stdout_write",
                    "could not publish the complete bounded trace",
                )
            }
        }
        Err(error) => emit_error(error.code, error.message),
    }
}

fn run() -> Result<Vec<u8>, CliErrorV1> {
    let request = parse_arguments()?;
    let limits = ImportLimitsV1::default();
    let input = read_bounded_stdin(limits.max_source_bytes())?;
    let imported = match request.command {
        CommandV1::Rocprofv3Json => import_rocprofv3_json_v1(
            &input,
            RocprofBindingV1 {
                kernel_ir_claim: request.kernel_ir_claim()?,
                artifact: request.artifact_claim()?,
                wave_width: request.wave_width()?,
                selection: RocprofDispatchSelectionV1 {
                    process_index: request
                        .required_usize(request.process_index, "--process-index")?,
                    dispatch_index: request
                        .required_usize(request.dispatch_index, "--dispatch-index")?,
                },
            },
            limits,
        ),
        CommandV1::Rocprofv3AttManifest | CommandV1::NormalizedRocgdbS09 => {
            let binding = SparseImportBindingV1 {
                kernel_ir_claim: request.kernel_ir_claim()?,
                artifact: request.artifact_claim()?,
                launch: request.launch()?,
            };
            if request.command == CommandV1::Rocprofv3AttManifest {
                import_rocprofv3_att_manifest_v1(&input, binding, limits)
            } else {
                import_normalized_rocgdb_s09_v1(&input, binding, limits)
            }
        }
    }
    .map_err(|_| CliErrorV1::new("import", "source evidence failed bounded import validation"))?;
    drop(input);
    let output = encode_trace_v1(imported.trace())
        .map_err(|_| CliErrorV1::new("encode", "could not encode canonical Semantic Trace V1"))?;
    if output.len() as u64 > MAX_IMPORT_OUTPUT_BYTES_V1 {
        return Err(CliErrorV1::new(
            "output_limit",
            "canonical Semantic Trace V1 exceeded the import output limit",
        ));
    }
    Ok(output)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandV1 {
    Rocprofv3Json,
    Rocprofv3AttManifest,
    NormalizedRocgdbS09,
}

#[derive(Debug)]
struct ParsedRequestV1 {
    command: CommandV1,
    kir_sha256: Option<[u8; 32]>,
    kir_len: Option<u64>,
    artifact_sha256: Option<[u8; 32]>,
    artifact_len: Option<u64>,
    artifact_format: Option<u16>,
    wave_width: Option<WaveWidthV1>,
    process_index: Option<usize>,
    dispatch_index: Option<usize>,
    logical_grid: Option<[u64; 3]>,
    grid_workgroups: Option<[u32; 3]>,
    workgroup_size: Option<[u32; 3]>,
}

impl ParsedRequestV1 {
    fn kernel_ir_claim(&self) -> Result<KernelIrIdentityClaimV1, CliErrorV1> {
        let digest = required_identity(self.kir_sha256, "--kir-sha256")?;
        let len = self
            .kir_len
            .ok_or_else(|| CliErrorV1::new("arguments", "missing --kir-len"))?;
        KernelIrIdentityClaimV1::canonical_v6_claim(digest, len)
            .map_err(|_| CliErrorV1::new("arguments", "invalid KIR identity claim"))
    }

    fn artifact_claim(&self) -> Result<Option<ArtifactClaimV1>, CliErrorV1> {
        match (
            self.artifact_sha256,
            self.artifact_len,
            self.artifact_format,
        ) {
            (None, None, None) => Ok(None),
            (Some(digest), Some(canonical_len), Some(format_version)) => {
                Ok(Some(ArtifactClaimV1 {
                    identity: OpaqueIdentityV1::new(digest).map_err(|_| {
                        CliErrorV1::new("arguments", "artifact digest cannot be all zero")
                    })?,
                    canonical_len,
                    format_version,
                }))
            }
            _ => Err(CliErrorV1::new(
                "arguments",
                "artifact flags must be supplied together",
            )),
        }
    }

    fn wave_width(&self) -> Result<WaveWidthV1, CliErrorV1> {
        self.wave_width
            .ok_or_else(|| CliErrorV1::new("arguments", "missing --wave-width"))
    }

    fn required_usize(
        &self,
        value: Option<usize>,
        flag: &'static str,
    ) -> Result<usize, CliErrorV1> {
        value.ok_or_else(|| CliErrorV1::new("arguments", format!("missing {flag}")))
    }

    fn launch(&self) -> Result<LaunchGeometryV1, CliErrorV1> {
        let logical_grid = self
            .logical_grid
            .ok_or_else(|| CliErrorV1::new("arguments", "missing --grid"))?;
        let groups = self
            .grid_workgroups
            .ok_or_else(|| CliErrorV1::new("arguments", "missing --grid-workgroups"))?;
        let workgroup = self
            .workgroup_size
            .ok_or_else(|| CliErrorV1::new("arguments", "missing --workgroup"))?;
        LaunchGeometryV1::new_exact(logical_grid, groups, workgroup, self.wave_width()?)
            .map_err(|_| CliErrorV1::new("arguments", "invalid launch geometry"))
    }
}

fn parse_arguments() -> Result<ParsedRequestV1, CliErrorV1> {
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(MAX_ARGUMENTS)
        .map_err(|_| CliErrorV1::new("allocation", "could not reserve bounded arguments"))?;
    for argument in std::env::args_os().skip(1) {
        if arguments.len() == MAX_ARGUMENTS {
            return Err(CliErrorV1::new(
                "arguments",
                "too many command-line arguments",
            ));
        }
        validate_argument(&argument)?;
        arguments.push(argument);
    }
    let command = match arguments.first().and_then(|value| value.to_str()) {
        Some("rocprofv3-json") => CommandV1::Rocprofv3Json,
        Some("rocprofv3-att-manifest") => CommandV1::Rocprofv3AttManifest,
        Some("rocgdb-s09") => CommandV1::NormalizedRocgdbS09,
        _ => return Err(CliErrorV1::new("arguments", usage())),
    };
    let mut request = ParsedRequestV1 {
        command,
        kir_sha256: None,
        kir_len: None,
        artifact_sha256: None,
        artifact_len: None,
        artifact_format: None,
        wave_width: None,
        process_index: None,
        dispatch_index: None,
        logical_grid: None,
        grid_workgroups: None,
        workgroup_size: None,
    };
    let mut seen = 0_u16;
    let mut position = 1;
    while position < arguments.len() {
        let flag = arguments[position]
            .to_str()
            .ok_or_else(|| CliErrorV1::new("arguments", "arguments must be UTF-8"))?;
        position += 1;
        let value = arguments
            .get(position)
            .and_then(|value| value.to_str())
            .ok_or_else(|| CliErrorV1::new("arguments", "every flag requires one value"))?;
        position += 1;
        let bit = flag_bit(flag)?;
        if seen & bit != 0 {
            return Err(CliErrorV1::new(
                "arguments",
                "each import flag may appear at most once",
            ));
        }
        seen |= bit;
        match flag {
            "--kir-sha256" => request.kir_sha256 = Some(parse_sha256(value)?),
            "--kir-len" => request.kir_len = Some(parse_u64(value)?),
            "--artifact-sha256" => request.artifact_sha256 = Some(parse_sha256(value)?),
            "--artifact-len" => request.artifact_len = Some(parse_u64(value)?),
            "--artifact-format" => request.artifact_format = Some(parse_u16(value)?),
            "--wave-width" => {
                request.wave_width = Some(match value {
                    "32" => WaveWidthV1::Wave32,
                    "64" => WaveWidthV1::Wave64,
                    _ => return Err(CliErrorV1::new("arguments", "wave width must be 32 or 64")),
                });
            }
            "--process-index" => request.process_index = Some(parse_usize(value)?),
            "--dispatch-index" => request.dispatch_index = Some(parse_usize(value)?),
            "--grid" => request.logical_grid = Some(parse_u64x3(value)?),
            "--grid-workgroups" => request.grid_workgroups = Some(parse_u32x3(value)?),
            "--workgroup" => request.workgroup_size = Some(parse_u32x3(value)?),
            _ => return Err(CliErrorV1::new("arguments", "unknown import flag")),
        }
    }
    let rocprof_only = request.process_index.is_some() || request.dispatch_index.is_some();
    let sparse_only = request.logical_grid.is_some()
        || request.grid_workgroups.is_some()
        || request.workgroup_size.is_some();
    if command == CommandV1::Rocprofv3Json && sparse_only
        || command != CommandV1::Rocprofv3Json && rocprof_only
    {
        return Err(CliErrorV1::new(
            "arguments",
            "command-specific flags were supplied to the wrong importer",
        ));
    }
    Ok(request)
}

fn read_bounded_stdin(max: u64) -> Result<Vec<u8>, CliErrorV1> {
    let max = usize::try_from(max)
        .map_err(|_| CliErrorV1::new("input_limit", "input limit is not representable"))?;
    let mut input = Vec::new();
    let mut reader = std::io::stdin().lock();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        if input.len() == max {
            if reader
                .read(&mut buffer[..1])
                .map_err(|_| CliErrorV1::new("stdin_read", "could not read bounded stdin"))?
                != 0
            {
                return Err(CliErrorV1::new(
                    "input_too_large",
                    "stdin exceeds the importer source limit",
                ));
            }
            break;
        }
        let read_limit = (max - input.len()).min(buffer.len());
        let read = reader
            .read(&mut buffer[..read_limit])
            .map_err(|_| CliErrorV1::new("stdin_read", "could not read bounded stdin"))?;
        if read == 0 {
            break;
        }
        reserve_input(&mut input, read, max)?;
        input.extend_from_slice(&buffer[..read]);
    }
    Ok(input)
}

fn reserve_input(input: &mut Vec<u8>, additional: usize, max: usize) -> Result<(), CliErrorV1> {
    let required = input
        .len()
        .checked_add(additional)
        .ok_or_else(|| CliErrorV1::new("input_too_large", "stdin size overflowed"))?;
    if required > max {
        return Err(CliErrorV1::new(
            "input_too_large",
            "stdin exceeds source limit",
        ));
    }
    if required > input.capacity() {
        let doubled = input.capacity().checked_mul(2).unwrap_or(max);
        let target = required.max(doubled).min(max);
        input
            .try_reserve_exact(target.saturating_sub(input.capacity()))
            .map_err(|_| CliErrorV1::new("allocation", "could not grow bounded stdin"))?;
        if input.capacity() > max {
            return Err(CliErrorV1::new(
                "allocation",
                "stdin allocation exceeded its bound",
            ));
        }
    }
    Ok(())
}

fn validate_argument(argument: &OsString) -> Result<(), CliErrorV1> {
    if argument.as_os_str().as_encoded_bytes().len() > MAX_ARGUMENT_BYTES {
        return Err(CliErrorV1::new(
            "arguments",
            "one command-line argument is too long",
        ));
    }
    if argument.to_str().is_none() {
        return Err(CliErrorV1::new(
            "arguments",
            "arguments must be valid UTF-8",
        ));
    }
    Ok(())
}

fn flag_bit(flag: &str) -> Result<u16, CliErrorV1> {
    let bit = match flag {
        "--kir-sha256" => 0,
        "--kir-len" => 1,
        "--artifact-sha256" => 2,
        "--artifact-len" => 3,
        "--artifact-format" => 4,
        "--wave-width" => 5,
        "--process-index" => 6,
        "--dispatch-index" => 7,
        "--grid" => 8,
        "--grid-workgroups" => 9,
        "--workgroup" => 10,
        _ => return Err(CliErrorV1::new("arguments", "unknown import flag")),
    };
    Ok(1_u16 << bit)
}

fn parse_sha256(value: &str) -> Result<[u8; 32], CliErrorV1> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CliErrorV1::new(
            "arguments",
            "SHA-256 must contain exactly 64 hex digits",
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Result<u8, CliErrorV1> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(CliErrorV1::new(
            "arguments",
            "SHA-256 contains non-hex data",
        )),
    }
}

fn required_identity(value: Option<[u8; 32]>, flag: &str) -> Result<OpaqueIdentityV1, CliErrorV1> {
    OpaqueIdentityV1::new(
        value.ok_or_else(|| CliErrorV1::new("arguments", format!("missing {flag}")))?,
    )
    .map_err(|_| CliErrorV1::new("arguments", "identity digest cannot be all zero"))
}

fn parse_u16(value: &str) -> Result<u16, CliErrorV1> {
    value
        .parse()
        .map_err(|_| CliErrorV1::new("arguments", "expected an unsigned 16-bit integer"))
}

fn parse_u64(value: &str) -> Result<u64, CliErrorV1> {
    value
        .parse()
        .map_err(|_| CliErrorV1::new("arguments", "expected an unsigned 64-bit integer"))
}

fn parse_usize(value: &str) -> Result<usize, CliErrorV1> {
    value
        .parse()
        .map_err(|_| CliErrorV1::new("arguments", "expected a nonnegative index"))
}

fn parse_u32x3(value: &str) -> Result<[u32; 3], CliErrorV1> {
    let values = parse_triplet(value)?;
    Ok([
        u32::try_from(values[0])
            .map_err(|_| CliErrorV1::new("arguments", "dimension exceeds u32"))?,
        u32::try_from(values[1])
            .map_err(|_| CliErrorV1::new("arguments", "dimension exceeds u32"))?,
        u32::try_from(values[2])
            .map_err(|_| CliErrorV1::new("arguments", "dimension exceeds u32"))?,
    ])
}

fn parse_u64x3(value: &str) -> Result<[u64; 3], CliErrorV1> {
    parse_triplet(value)
}

fn parse_triplet(value: &str) -> Result<[u64; 3], CliErrorV1> {
    let mut parts = value.split(',');
    let values = [
        parts.next().and_then(|v| v.parse().ok()),
        parts.next().and_then(|v| v.parse().ok()),
        parts.next().and_then(|v| v.parse().ok()),
    ];
    if parts.next().is_some() || values.contains(&None) {
        return Err(CliErrorV1::new("arguments", "expected X,Y,Z dimensions"));
    }
    Ok([values[0].unwrap(), values[1].unwrap(), values[2].unwrap()])
}

const fn usage() -> &'static str {
    "usage: fe2o3-trace-import {rocprofv3-json|rocprofv3-att-manifest|rocgdb-s09} --kir-sha256 HEX --kir-len N --wave-width {32|64} [command-specific flags]"
}

#[derive(Debug)]
struct CliErrorV1 {
    code: &'static str,
    message: String,
}

impl CliErrorV1 {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Serialize)]
struct CliErrorResponseV1<'a> {
    error: &'a str,
    message: &'a str,
}

fn emit_error(code: &'static str, message: impl AsRef<str>) -> ExitCode {
    let message = message.as_ref();
    let response = CliErrorResponseV1 {
        error: code,
        message,
    };
    let mut stderr = std::io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, &response);
    let _ = stderr.write_all(b"\n");
    ExitCode::FAILURE
}
