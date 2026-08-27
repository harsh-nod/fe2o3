use std::collections::HashSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::ExitCode;

use fe2o3_kernel_ir::{
    AccessMode, FunctionId, ScalarType, VerifiedCanonicalKernelIrErrorV7,
    VerifiedCanonicalKernelIrV7,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    ScalarBitsV1, SharedBufferV1, SimulationAdmissionErrorV1, SimulationArgumentV1,
    SimulationConflictAssessmentV1, SimulationErrorV1, SimulationExecutionErrorKindV1,
    SimulationExecutionErrorV1, SimulationExecutionV1, SimulationInvocationV1, SimulationLimitsV1,
    SimulationMemoryConflictV1, SimulationPreflightErrorV1, SimulationRequestV1,
    SimulationScheduleIdentityV1, SimulationSiteV1, SimulationTargetV1, UnsupportedFeatureV1,
    UnsupportedSimulationSiteV1,
};
use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, PROC_SUPER_MAGIC, ResolveFlags, fchmod, fstat, fstatfs, fsync,
    linkat, openat, openat2, statat,
};
use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::schema::{ErrorKind, Stage};

const USAGE: &str = "usage: fe2o3-kir-sim --kir-v7 PATH --request PATH [--output PATH]";
const REQUEST_SCHEMA: &str = "fe2o3-simulation-request-v1";
const RESULT_SCHEMA: &str = "fe2o3-simulation-result-v1";
const ERROR_SCHEMA: &str = "fe2o3-simulation-error-v1";
const MAX_KIR_BYTES: usize = 16 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;
const MAX_ARGUMENTS: usize = 4_096;
const MAX_SHARED_BUFFERS: usize = 4_096;
const MAX_KERNEL_BYTES: usize = 4_096;
const MAX_BUFFER_BYTES: usize = 4 * 1024 * 1024;
const MAX_TOTAL_BUFFER_BYTES: usize = 16 * 1024 * 1024;
const MAX_SUCCESS_BYTES: usize = 64 * 1024 * 1024;
const MAX_ERROR_BYTES: usize = 1024 * 1024;
const MAX_ERROR_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_ERROR_FUNCTION_BYTES: usize = 16 * 1024;
const MAX_UNSUPPORTED_JSON_BYTES: usize = MAX_ERROR_BYTES - 128 * 1024;
const HEX_INPUT_CHUNK_BYTES: usize = 4 * 1024;
const MAX_UNSUPPORTED_FINDINGS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum UnsupportedFeatureCode {
    FloatType,
    UnsupportedType,
    MemoryIntrinsic,
    FloatConstant,
    FloatOperation,
    InvalidIntegerCast,
    ExternalCall,
    NonInternalCall,
    WorkgroupAllocation,
    NonScalarMemory,
    UnsupportedAddressSpace,
    Barrier,
    Atomic,
    Fence,
    WorkgroupBarrier,
    WorkgroupMemory,
    DynamicWorkgroupMemory,
    Matrix,
    Wave,
    Gfx950LdsTranspose,
    InlineAssembly,
    UnsupportedScalarOperation,
    TargetConstantOutOfRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum InputCode {
    KirV7,
    Request,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PublicationState {
    PublishedDurabilityUnknown,
    PublishedNameUncertain,
}

#[derive(Debug, Serialize)]
struct ErrorDocument {
    schema: &'static str,
    status: &'static str,
    stage: Stage,
    kind: ErrorKind,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<InputCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invocation: Option<InvocationDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    site: Option<SiteDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    publication_state: Option<PublicationState>,
}

#[derive(Debug, Serialize)]
struct InvocationDocument {
    global: [u64; 3],
    workgroup: [u64; 3],
    local: [u32; 3],
    workgroup_size: [u32; 3],
    workgroup_count: [u64; 3],
    launch_extent: [u64; 3],
}

#[derive(Debug, Serialize)]
struct SiteDocument {
    function: BoundedFunctionId,
    function_bytes: usize,
    function_truncated: bool,
    block: u32,
    operation: Option<u32>,
}

#[derive(Debug)]
struct BoundedFunctionId(FunctionId);

impl Serialize for BoundedFunctionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(bounded_str(self.0.as_str(), MAX_ERROR_FUNCTION_BYTES))
    }
}

#[derive(Serialize)]
struct UnsupportedDocument<'a> {
    function: &'a str,
    function_bytes: usize,
    function_truncated: bool,
    block: Option<u32>,
    operation: Option<u32>,
    feature: UnsupportedFeatureCode,
}

#[derive(Debug)]
struct UnsupportedFailure {
    total: u64,
    findings: Vec<UnsupportedFinding>,
}

#[derive(Debug)]
struct UnsupportedFinding {
    function: String,
    function_bytes: usize,
    function_truncated: bool,
    block: Option<u32>,
    operation: Option<u32>,
    feature: UnsupportedFeatureCode,
}

#[derive(Debug)]
struct Failure(Box<ErrorDocument>, Option<UnsupportedFailure>);

impl Failure {
    fn new(stage: Stage, kind: ErrorKind, message: impl Into<String>) -> Self {
        let message = bounded_error_message(message.into());
        Self(
            Box::new(ErrorDocument {
                schema: ERROR_SCHEMA,
                status: "error",
                stage,
                kind,
                message,
                input: None,
                invocation: None,
                site: None,
                publication_state: None,
            }),
            None,
        )
    }

    fn published(kind: ErrorKind, message: impl Into<String>, state: PublicationState) -> Self {
        let mut failure = Self::new(Stage::Output, kind, message);
        failure.0.publication_state = Some(state);
        failure
    }

    fn input(code: InputCode, kind: ErrorKind, message: impl Into<String>) -> Self {
        let mut failure = Self::new(Stage::Input, kind, message);
        failure.0.input = Some(code);
        failure
    }

    fn preflight(error: SimulationPreflightErrorV1) -> Self {
        match error {
            SimulationPreflightErrorV1::Unsupported(report) => {
                Self::unsupported(report.total_findings(), report.findings())
            }
            error => Self::new(
                Stage::Preflight,
                preflight_kind(&error),
                bounded_display(&error),
            ),
        }
    }

    fn unsupported(total: u64, findings: &[UnsupportedSimulationSiteV1]) -> Self {
        let mut bounded = Vec::new();
        if bounded
            .try_reserve_exact(findings.len().min(MAX_UNSUPPORTED_FINDINGS))
            .is_ok()
        {
            for finding in findings.iter().take(MAX_UNSUPPORTED_FINDINGS) {
                let source = finding.function.as_str();
                let prefix = bounded_str(source, MAX_ERROR_FUNCTION_BYTES);
                let mut function = String::new();
                if function.try_reserve_exact(prefix.len()).is_err() {
                    break;
                }
                function.push_str(prefix);
                bounded.push(UnsupportedFinding {
                    function,
                    function_bytes: source.len(),
                    function_truncated: prefix.len() != source.len(),
                    block: finding.block.map(|block| block.0),
                    operation: finding.operation,
                    feature: unsupported_code(&finding.feature),
                });
            }
        }
        Self(
            Box::new(ErrorDocument {
                schema: ERROR_SCHEMA,
                status: "error",
                stage: Stage::Preflight,
                kind: ErrorKind::PreflightUnsupported,
                message: format!(
                    "selected kernel has {total} unsupported reachable site occurrence(s)"
                ),
                input: None,
                invocation: None,
                site: None,
                publication_state: None,
            }),
            Some(UnsupportedFailure {
                total,
                findings: bounded,
            }),
        )
    }

    fn execution(error: SimulationExecutionErrorV1) -> Self {
        let message = bounded_display(&error);
        let invocation = error.invocation.map(|value| InvocationDocument {
            global: value.global,
            workgroup: value.workgroup,
            local: value.local,
            workgroup_size: value.workgroup_size,
            workgroup_count: value.workgroup_count,
            launch_extent: value.launch_extent,
        });
        let site = error.site.map(site_document);
        Self(
            Box::new(ErrorDocument {
                schema: ERROR_SCHEMA,
                status: "error",
                stage: Stage::Execution,
                kind: execution_kind(&error.kind),
                message,
                input: None,
                invocation,
                site,
                publication_state: None,
            }),
            None,
        )
    }
}

fn bounded_str(value: &str, maximum: usize) -> &str {
    if value.len() <= maximum {
        return value;
    }
    let mut end = maximum;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn bounded_display(value: &impl fmt::Display) -> String {
    let mut writer = BoundedMessageWriter::default();
    let _ = fmt::write(&mut writer, format_args!("{value}"));
    writer.finish()
}

#[derive(Default)]
struct BoundedMessageWriter {
    message: String,
    truncated: bool,
    allocation_failed: bool,
}

impl BoundedMessageWriter {
    fn finish(mut self) -> String {
        const SUFFIX: &str = " [truncated]";
        if self.truncated
            && !self.allocation_failed
            && self.message.try_reserve_exact(SUFFIX.len()).is_ok()
        {
            self.message.push_str(SUFFIX);
        }
        self.message
    }
}

impl fmt::Write for BoundedMessageWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        const SUFFIX_BYTES: usize = " [truncated]".len();
        if self.truncated || self.allocation_failed {
            return Ok(());
        }
        let remaining = MAX_ERROR_MESSAGE_BYTES
            .saturating_sub(SUFFIX_BYTES)
            .saturating_sub(self.message.len());
        let prefix = bounded_str(value, remaining);
        if self.message.try_reserve(prefix.len()).is_err() {
            self.allocation_failed = true;
            return Ok(());
        }
        self.message.push_str(prefix);
        self.truncated = prefix.len() != value.len();
        Ok(())
    }
}

fn bounded_error_message(mut message: String) -> String {
    const SUFFIX: &str = " [truncated]";
    if message.len() <= MAX_ERROR_MESSAGE_BYTES {
        return message;
    }
    let mut end = MAX_ERROR_MESSAGE_BYTES - SUFFIX.len();
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message.truncate(end);
    message.push_str(SUFFIX);
    message
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    kir_v7: OsString,
    request: OsString,
    output: Option<OsString>,
}

#[derive(Debug)]
struct RequestDocument {
    schema: String,
    kernel: String,
    grid: [u64; 3],
    workgroup: [u32; 3],
    arguments: Vec<RequestArgument>,
    shared_buffers: Vec<RequestSharedBuffer>,
}

#[derive(Debug)]
enum RequestArgument {
    Scalar {
        ty: String,
        bits: String,
    },
    Buffer {
        element: String,
        access: String,
        alignment: u32,
        bytes: String,
        initialized: Option<String>,
    },
    BufferView {
        backing: u32,
        element: String,
        access: String,
        alignment: u32,
        byte_offset: usize,
        elements: usize,
    },
}

#[derive(Debug)]
struct RequestSharedBuffer {
    id: u32,
    element: String,
    access: String,
    alignment: u32,
    bytes: String,
    initialized: Option<String>,
}

struct ArgumentVisitor;

impl<'de> Visitor<'de> for ArgumentVisitor {
    type Value = Vec<RequestArgument>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "at most {MAX_ARGUMENTS} simulation arguments")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        if sequence
            .size_hint()
            .is_some_and(|hint| hint > MAX_ARGUMENTS)
        {
            return Err(de::Error::custom("fe2o3:argument_limit"));
        }
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(sequence.size_hint().unwrap_or(0).min(MAX_ARGUMENTS))
            .map_err(|_| de::Error::custom("fe2o3:argument_allocation"))?;
        while let Some(argument) = sequence.next_element()? {
            if arguments.len() == MAX_ARGUMENTS {
                return Err(de::Error::custom("fe2o3:argument_limit"));
            }
            if arguments.len() == arguments.capacity() {
                arguments
                    .try_reserve(1)
                    .map_err(|_| de::Error::custom("fe2o3:argument_allocation"))?;
            }
            arguments.push(argument);
        }
        Ok(arguments)
    }
}

struct BoundedArguments(Vec<RequestArgument>);

impl<'de> Deserialize<'de> for BoundedArguments {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(ArgumentVisitor).map(Self)
    }
}

struct SharedBufferVisitor;

impl<'de> Visitor<'de> for SharedBufferVisitor {
    type Value = Vec<RequestSharedBuffer>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "at most {MAX_SHARED_BUFFERS} shared buffers")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        if sequence
            .size_hint()
            .is_some_and(|hint| hint > MAX_SHARED_BUFFERS)
        {
            return Err(de::Error::custom("fe2o3:shared_buffer_limit"));
        }
        let mut buffers = Vec::new();
        buffers
            .try_reserve_exact(sequence.size_hint().unwrap_or(0).min(MAX_SHARED_BUFFERS))
            .map_err(|_| de::Error::custom("fe2o3:shared_buffer_allocation"))?;
        while let Some(buffer) = sequence.next_element()? {
            if buffers.len() == MAX_SHARED_BUFFERS {
                return Err(de::Error::custom("fe2o3:shared_buffer_limit"));
            }
            if buffers.len() == buffers.capacity() {
                buffers
                    .try_reserve(1)
                    .map_err(|_| de::Error::custom("fe2o3:shared_buffer_allocation"))?;
            }
            buffers.push(buffer);
        }
        Ok(buffers)
    }
}

struct BoundedSharedBuffers(Vec<RequestSharedBuffer>);

impl<'de> Deserialize<'de> for BoundedSharedBuffers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(SharedBufferVisitor).map(Self)
    }
}

struct NonNull<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for NonNull<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer)
            .map(|value| value.map(Self))
            .and_then(|value| value.ok_or_else(|| de::Error::custom("fe2o3:null")))
    }
}

impl<'de> Deserialize<'de> for RequestDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(RequestDocumentVisitor)
    }
}

struct RequestDocumentVisitor;

impl<'de> Visitor<'de> for RequestDocumentVisitor {
    type Value = RequestDocument;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a simulation request object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut schema = None;
        let mut kernel = None;
        let mut grid = None;
        let mut workgroup = None;
        let mut arguments = None;
        let mut shared_buffers = None;
        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                "schema" => assign_once(&mut schema, map.next_value::<NonNull<_>>()?.0)?,
                "kernel" => assign_once(&mut kernel, map.next_value::<NonNull<_>>()?.0)?,
                "grid" => assign_once(&mut grid, map.next_value::<NonNull<_>>()?.0)?,
                "workgroup" => assign_once(&mut workgroup, map.next_value::<NonNull<_>>()?.0)?,
                "arguments" => {
                    let value: BoundedArguments = map.next_value::<NonNull<_>>()?.0;
                    assign_once(&mut arguments, value.0)?;
                }
                "shared_buffers" => {
                    let value: BoundedSharedBuffers = map.next_value::<NonNull<_>>()?.0;
                    assign_once(&mut shared_buffers, value.0)?;
                }
                _ => {
                    let _: IgnoredAny = map.next_value()?;
                    return Err(de::Error::custom("fe2o3:unknown_field"));
                }
            }
        }
        Ok(RequestDocument {
            schema: required(schema)?,
            kernel: required(kernel)?,
            grid: required(grid)?,
            workgroup: required(workgroup)?,
            arguments: required(arguments)?,
            shared_buffers: shared_buffers.unwrap_or_default(),
        })
    }
}

impl<'de> Deserialize<'de> for RequestArgument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(RequestArgumentVisitor)
    }
}

struct RequestArgumentVisitor;

impl<'de> Visitor<'de> for RequestArgumentVisitor {
    type Value = RequestArgument;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a tagged simulation argument object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut kind = None;
        let mut ty = None;
        let mut bits = None;
        let mut element = None;
        let mut access = None;
        let mut alignment = None;
        let mut bytes = None;
        let mut initialized = None;
        let mut backing = None;
        let mut byte_offset = None;
        let mut elements = None;
        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                "kind" => assign_once(&mut kind, map.next_value::<NonNull<_>>()?.0)?,
                "type" => assign_once(&mut ty, map.next_value::<NonNull<_>>()?.0)?,
                "bits" => assign_once(&mut bits, map.next_value::<NonNull<_>>()?.0)?,
                "element" => assign_once(&mut element, map.next_value::<NonNull<_>>()?.0)?,
                "access" => assign_once(&mut access, map.next_value::<NonNull<_>>()?.0)?,
                "alignment" => assign_once(&mut alignment, map.next_value::<NonNull<_>>()?.0)?,
                "bytes" => assign_once(&mut bytes, map.next_value::<NonNull<_>>()?.0)?,
                "initialized" => {
                    let value: Option<String> = map.next_value()?;
                    let value = value.ok_or_else(|| de::Error::custom("fe2o3:null"))?;
                    assign_once(&mut initialized, value)?;
                }
                "backing" => assign_once(&mut backing, map.next_value::<NonNull<_>>()?.0)?,
                "byte_offset" => assign_once(&mut byte_offset, map.next_value::<NonNull<_>>()?.0)?,
                "elements" => assign_once(&mut elements, map.next_value::<NonNull<_>>()?.0)?,
                _ => {
                    let _: IgnoredAny = map.next_value()?;
                    return Err(de::Error::custom("fe2o3:unknown_field"));
                }
            }
        }
        match required::<String, A::Error>(kind)?.as_str() {
            "scalar" => {
                if element.is_some()
                    || access.is_some()
                    || alignment.is_some()
                    || bytes.is_some()
                    || initialized.is_some()
                    || backing.is_some()
                    || byte_offset.is_some()
                    || elements.is_some()
                {
                    return Err(de::Error::custom("fe2o3:unknown_field"));
                }
                Ok(RequestArgument::Scalar {
                    ty: required(ty)?,
                    bits: required(bits)?,
                })
            }
            "buffer" => {
                if ty.is_some()
                    || bits.is_some()
                    || backing.is_some()
                    || byte_offset.is_some()
                    || elements.is_some()
                {
                    return Err(de::Error::custom("fe2o3:unknown_field"));
                }
                Ok(RequestArgument::Buffer {
                    element: required(element)?,
                    access: required(access)?,
                    alignment: required(alignment)?,
                    bytes: required(bytes)?,
                    initialized,
                })
            }
            "buffer_view" => {
                if ty.is_some() || bits.is_some() || bytes.is_some() || initialized.is_some() {
                    return Err(de::Error::custom("fe2o3:unknown_field"));
                }
                Ok(RequestArgument::BufferView {
                    backing: required(backing)?,
                    element: required(element)?,
                    access: required(access)?,
                    alignment: required(alignment)?,
                    byte_offset: required(byte_offset)?,
                    elements: required(elements)?,
                })
            }
            _ => Err(de::Error::custom("fe2o3:invalid_tag")),
        }
    }
}

impl<'de> Deserialize<'de> for RequestSharedBuffer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(RequestSharedBufferVisitor)
    }
}

struct RequestSharedBufferVisitor;

impl<'de> Visitor<'de> for RequestSharedBufferVisitor {
    type Value = RequestSharedBuffer;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a shared buffer object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut id = None;
        let mut element = None;
        let mut access = None;
        let mut alignment = None;
        let mut bytes = None;
        let mut initialized = None;
        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                "id" => assign_once(&mut id, map.next_value::<NonNull<_>>()?.0)?,
                "element" => assign_once(&mut element, map.next_value::<NonNull<_>>()?.0)?,
                "access" => assign_once(&mut access, map.next_value::<NonNull<_>>()?.0)?,
                "alignment" => assign_once(&mut alignment, map.next_value::<NonNull<_>>()?.0)?,
                "bytes" => assign_once(&mut bytes, map.next_value::<NonNull<_>>()?.0)?,
                "initialized" => {
                    let value: Option<String> = map.next_value()?;
                    let value = value.ok_or_else(|| de::Error::custom("fe2o3:null"))?;
                    assign_once(&mut initialized, value)?;
                }
                _ => {
                    let _: IgnoredAny = map.next_value()?;
                    return Err(de::Error::custom("fe2o3:unknown_field"));
                }
            }
        }
        Ok(RequestSharedBuffer {
            id: required(id)?,
            element: required(element)?,
            access: required(access)?,
            alignment: required(alignment)?,
            bytes: required(bytes)?,
            initialized,
        })
    }
}

fn assign_once<T, E: de::Error>(slot: &mut Option<T>, value: T) -> Result<(), E> {
    if slot.replace(value).is_some() {
        Err(E::custom("fe2o3:duplicate_field"))
    } else {
        Ok(())
    }
}

fn required<T, E: de::Error>(value: Option<T>) -> Result<T, E> {
    value.ok_or_else(|| E::custom("fe2o3:missing_field"))
}

fn request_json_kind(error: &serde_json::Error) -> ErrorKind {
    let detail = error.to_string();
    let marker = detail
        .split_once(" at line ")
        .map_or(detail.as_str(), |(marker, _)| marker);
    if marker == "fe2o3:duplicate_field" {
        ErrorKind::RequestJsonDuplicateField
    } else if marker == "fe2o3:unknown_field" {
        ErrorKind::RequestJsonUnknownField
    } else if marker == "fe2o3:missing_field" {
        ErrorKind::RequestJsonMissingField
    } else if marker == "fe2o3:invalid_tag" {
        ErrorKind::RequestJsonInvalidTag
    } else if marker == "fe2o3:null" {
        ErrorKind::RequestJsonNull
    } else if marker == "fe2o3:argument_limit" {
        ErrorKind::RequestJsonArgumentLimit
    } else if marker == "fe2o3:shared_buffer_limit" {
        ErrorKind::RequestJsonSharedBufferLimit
    } else if matches!(
        marker,
        "fe2o3:argument_allocation" | "fe2o3:shared_buffer_allocation"
    ) {
        ErrorKind::RequestJsonAllocationFailure
    } else {
        match error.classify() {
            serde_json::error::Category::Syntax | serde_json::error::Category::Eof => {
                ErrorKind::RequestJsonSyntax
            }
            serde_json::error::Category::Data | serde_json::error::Category::Io => {
                ErrorKind::RequestJsonTypeMismatch
            }
        }
    }
}

pub(crate) fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let first = arguments.next();
    if first
        .as_deref()
        .is_some_and(|argument| argument == OsStr::new("--help") || argument == OsStr::new("-h"))
        && arguments.next().is_none()
    {
        let mut stdout = io::stdout().lock();
        return match stdout
            .write_all(USAGE.as_bytes())
            .and_then(|()| stdout.write_all(b"\n"))
        {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                write_error(&Failure::new(
                    Stage::Output,
                    ErrorKind::OutputWriteFailed,
                    format!("cannot write help: {error}"),
                ));
                ExitCode::FAILURE
            }
        };
    }
    let result = match first {
        Some(first) => run(std::iter::once(first).chain(arguments)),
        None => run(std::iter::empty()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            write_error(&error);
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn run_captured_kir_v7(
    canonical_kir_v7: &[u8],
    request: OsString,
    expected_request: Option<crate::SimulationRequestIdentityV1>,
    output: Option<OsString>,
) -> ExitCode {
    let result = run_with_captured_kir(
        canonical_kir_v7,
        Options {
            kir_v7: OsString::new(),
            request,
            output,
        },
        expected_request,
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            write_error(&error);
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn bind_request_v1(
    request: OsString,
) -> Result<crate::SimulationRequestIdentityV1, String> {
    let bytes = secure_read(
        Path::new(&request),
        MAX_REQUEST_BYTES,
        InputCode::Request,
        "simulation request",
    )
    .map_err(|failure| failure.0.message.clone())?;
    let document: RequestDocument = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "request JSON is invalid at line {} column {}",
            error.line(),
            error.column()
        )
    })?;
    prepare_request(document).map_err(|failure| failure.0.message.clone())?;
    Ok(crate::SimulationRequestIdentityV1 {
        sha256: Sha256::digest(&bytes).into(),
        length: bytes.len(),
    })
}

pub(crate) fn load_debug_simulation_input_v1(
    kir_v7: OsString,
    request: OsString,
) -> Result<crate::AdmittedSimulationInputV1, crate::SimulationInputErrorV1> {
    let result = (|| {
        let kir = secure_read(
            Path::new(&kir_v7),
            MAX_KIR_BYTES,
            InputCode::KirV7,
            "canonical KIR V7",
        )?;
        load_admitted_input(&kir, Path::new(&request), None)
    })();
    result.map_err(|failure: Failure| crate::SimulationInputErrorV1 {
        stage: serialized_tag(failure.0.stage),
        code: serialized_tag(failure.0.kind),
        message: failure.0.message.clone(),
    })
}

fn serialized_tag(value: impl Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(tag)) => tag,
        _ => "internal_serialization_failure".to_owned(),
    }
}

fn run(arguments: impl Iterator<Item = OsString>) -> Result<(), Failure> {
    let options = parse_options(arguments)?;
    let kir = secure_read(
        Path::new(&options.kir_v7),
        MAX_KIR_BYTES,
        InputCode::KirV7,
        "canonical KIR V7",
    )?;
    run_with_captured_kir(&kir, options, None)
}

fn run_with_captured_kir(
    kir: &[u8],
    options: Options,
    expected_request: Option<crate::SimulationRequestIdentityV1>,
) -> Result<(), Failure> {
    let input = load_admitted_input(kir, Path::new(&options.request), expected_request)?;
    let execution = input
        .module
        .simulate(
            &input.request,
            SimulationTargetV1::amdgpu_64(),
            input.simulation_limits,
        )
        .map_err(|error| match error {
            SimulationErrorV1::Preflight(error) => Failure::preflight(error),
            SimulationErrorV1::Execution(error) => Failure::execution(error),
        })?;
    drop(input);
    let maximum = measure_success_bytes(&execution)?;
    match options.output {
        Some(path) => publish_transactionally(Path::new(&path), &execution, maximum),
        None => write_success_stdout(&execution, maximum),
    }
}

fn load_admitted_input(
    kir: &[u8],
    request: &Path,
    expected_request: Option<crate::SimulationRequestIdentityV1>,
) -> Result<crate::AdmittedSimulationInputV1, Failure> {
    if kir.len() > MAX_KIR_BYTES {
        return Err(Failure::input(
            InputCode::KirV7,
            ErrorKind::InputTooLarge,
            format!(
                "canonical KIR V7 input is {} bytes; maximum is {MAX_KIR_BYTES}",
                kir.len()
            ),
        ));
    }
    let limits = cli_simulation_limits();
    let canonical =
        VerifiedCanonicalKernelIrV7::from_canonical_bytes(kir.to_vec()).map_err(|error| {
            Failure::new(
                Stage::KirAdmission,
                kir_error_kind(&error),
                bounded_display(&error),
            )
        })?;
    let admitted = AdmittedSimulationModuleV1::admit(canonical, limits).map_err(|error| {
        Failure::new(
            Stage::SimulatorAdmission,
            admission_error_kind(&error),
            bounded_display(&error),
        )
    })?;
    let request_bytes = secure_read(
        request,
        MAX_REQUEST_BYTES,
        InputCode::Request,
        "simulation request",
    )?;
    if expected_request.is_some_and(|expected| {
        expected.length != request_bytes.len()
            || expected.sha256 != <[u8; 32]>::from(Sha256::digest(&request_bytes))
    }) {
        return Err(Failure::input(
            InputCode::Request,
            ErrorKind::InputChanged,
            "simulation request changed after its pre-build admission",
        ));
    }
    let document: RequestDocument = serde_json::from_slice(&request_bytes).map_err(|error| {
        Failure::new(
            Stage::Request,
            request_json_kind(&error),
            format!(
                "request JSON is invalid at line {} column {}",
                error.line(),
                error.column()
            ),
        )
    })?;
    let request = prepare_request(document)?;
    Ok(crate::AdmittedSimulationInputV1 {
        kir_sha256: *admitted.identity().digest(),
        request_sha256: Sha256::digest(&request_bytes).into(),
        module: admitted,
        request,
        simulation_limits: limits,
    })
}

const fn cli_simulation_limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: MAX_KIR_BYTES,
        max_reachable_functions: 4_096,
        max_reachable_operations: 1 << 20,
        max_invocations: 1 << 20,
        max_workgroups: 1 << 20,
        max_scheduled_slots: 1 << 22,
        max_steps: 1 << 27,
        max_call_depth: 64,
        max_ssa_values: 4_096,
        max_allocations: 16_384,
        max_allocation_bytes: MAX_TOTAL_BUFFER_BYTES,
        max_total_bytes: 64 * 1024 * 1024,
        max_resident_bytes: 256 * 1024 * 1024,
        max_events: 1,
        max_memory_access_records: 65_536,
    }
}

fn parse_options(arguments: impl Iterator<Item = OsString>) -> Result<Options, Failure> {
    let mut kir_v7 = None;
    let mut request = None;
    let mut output = None;
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        let (slot, name) = if argument == OsStr::new("--kir-v7") {
            (&mut kir_v7, "--kir-v7")
        } else if argument == OsStr::new("--request") {
            (&mut request, "--request")
        } else if argument == OsStr::new("--output") {
            (&mut output, "--output")
        } else {
            return Err(Failure::new(
                Stage::Arguments,
                ErrorKind::InvalidCommandLine,
                format!("unknown option {argument:?}; {USAGE}"),
            ));
        };
        let value = arguments.next().ok_or_else(|| {
            Failure::new(
                Stage::Arguments,
                ErrorKind::InvalidCommandLine,
                format!("{name} requires a path; {USAGE}"),
            )
        })?;
        if value.is_empty() || slot.replace(value).is_some() {
            return Err(Failure::new(
                Stage::Arguments,
                ErrorKind::InvalidCommandLine,
                format!("{name} requires one nonempty path; {USAGE}"),
            ));
        }
    }
    Ok(Options {
        kir_v7: kir_v7.ok_or_else(|| {
            Failure::new(
                Stage::Arguments,
                ErrorKind::InvalidCommandLine,
                format!("--kir-v7 is required; {USAGE}"),
            )
        })?,
        request: request.ok_or_else(|| {
            Failure::new(
                Stage::Arguments,
                ErrorKind::InvalidCommandLine,
                format!("--request is required; {USAGE}"),
            )
        })?,
        output,
    })
}

fn secure_read(
    path: &Path,
    maximum: usize,
    code: InputCode,
    label: &str,
) -> Result<Vec<u8>, Failure> {
    secure_read_with_hook(path, maximum, code, label, || {})
}

fn secure_read_with_hook(
    path: &Path,
    maximum: usize,
    code: InputCode,
    label: &str,
    after_read: impl FnOnce(),
) -> Result<Vec<u8>, Failure> {
    let descriptor = openat2(
        rustix::fs::CWD,
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|error| {
        Failure::input(
            code,
            ErrorKind::InputOpenFailed,
            format!("cannot securely open {label}: {error}"),
        )
    })?;
    let before = fstat(&descriptor).map_err(|error| {
        Failure::input(
            code,
            ErrorKind::InputReadFailed,
            format!("cannot inspect {label}: {error}"),
        )
    })?;
    if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile {
        return Err(Failure::input(
            code,
            ErrorKind::InputNotRegular,
            format!("{label} is not a regular file"),
        ));
    }
    let length = usize::try_from(before.st_size)
        .ok()
        .filter(|length| *length <= maximum)
        .ok_or_else(|| {
            Failure::input(
                code,
                ErrorKind::InputTooLarge,
                format!("{label} exceeds its {maximum}-byte bound"),
            )
        })?;
    let capacity = length.checked_add(1).ok_or_else(|| {
        Failure::input(
            code,
            ErrorKind::InputAllocationFailed,
            format!("cannot size bounded storage for {label}"),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|_| {
        Failure::input(
            code,
            ErrorKind::InputAllocationFailed,
            format!("cannot allocate bounded storage for {label}"),
        )
    })?;
    let mut file = File::from(descriptor);
    let read_limit = u64::try_from(maximum)
        .ok()
        .and_then(|limit| limit.checked_add(1))
        .ok_or_else(|| {
            Failure::input(
                code,
                ErrorKind::InputTooLarge,
                format!("{label} exceeds the platform input-size range"),
            )
        })?;
    Read::by_ref(&mut file)
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            Failure::input(
                code,
                ErrorKind::InputReadFailed,
                format!("cannot read {label}: {error}"),
            )
        })?;
    if bytes.len() > maximum {
        return Err(Failure::input(
            code,
            ErrorKind::InputTooLarge,
            format!("{label} exceeds its {maximum}-byte bound"),
        ));
    }
    after_read();
    let after = fstat(&file).map_err(|error| {
        Failure::input(
            code,
            ErrorKind::InputReadFailed,
            format!("cannot reinspect {label}: {error}"),
        )
    })?;
    if !same_input_snapshot(&before, &after) || bytes.len() != length {
        return Err(Failure::input(
            code,
            ErrorKind::InputChanged,
            format!("{label} changed while it was read"),
        ));
    }
    Ok(bytes)
}

fn same_input_snapshot(left: &rustix::fs::Stat, right: &rustix::fs::Stat) -> bool {
    left.st_dev == right.st_dev
        && left.st_ino == right.st_ino
        && left.st_mode == right.st_mode
        && left.st_nlink == right.st_nlink
        && left.st_size == right.st_size
        && left.st_mtime == right.st_mtime
        && left.st_mtime_nsec == right.st_mtime_nsec
        && left.st_ctime == right.st_ctime
        && left.st_ctime_nsec == right.st_ctime_nsec
}

fn prepare_request(document: RequestDocument) -> Result<SimulationRequestV1, Failure> {
    if document.schema != REQUEST_SCHEMA {
        return Err(Failure::new(
            Stage::Request,
            ErrorKind::RequestSchemaUnsupported,
            format!("request schema must be {REQUEST_SCHEMA}"),
        ));
    }
    if document.kernel.is_empty() || document.kernel.len() > MAX_KERNEL_BYTES {
        return Err(Failure::new(
            Stage::Request,
            ErrorKind::RequestKernelInvalid,
            format!("kernel identity must contain 1 to {MAX_KERNEL_BYTES} UTF-8 bytes"),
        ));
    }

    let target = SimulationTargetV1::amdgpu_64();
    let mut total_buffer_bytes = 0_usize;
    let mut backing_ids = HashSet::new();
    backing_ids
        .try_reserve(document.shared_buffers.len())
        .map_err(|_| request_limit("cannot allocate shared backing identity set"))?;
    let mut shared_buffers = Vec::new();
    shared_buffers
        .try_reserve_exact(document.shared_buffers.len())
        .map_err(|_| request_limit("cannot allocate shared buffer table"))?;
    for shared in document.shared_buffers {
        if !backing_ids.insert(shared.id) {
            return Err(Failure::new(
                Stage::Request,
                ErrorKind::RequestDuplicateBacking,
                format!("shared backing {} is duplicated", shared.id),
            ));
        }
        let buffer = prepare_buffer(
            shared.element,
            shared.access,
            shared.alignment,
            shared.bytes,
            shared.initialized,
            target,
            &mut total_buffer_bytes,
        )?;
        shared_buffers.push(SharedBufferV1 {
            id: BufferBackingIdV1(shared.id),
            buffer,
        });
    }
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(document.arguments.len())
        .map_err(|_| request_limit("cannot allocate simulation arguments"))?;
    for argument in document.arguments {
        arguments.push(prepare_argument(argument, target, &mut total_buffer_bytes)?);
    }
    Ok(SimulationRequestV1::new(
        document.kernel,
        document.grid,
        document.workgroup,
        arguments,
    )
    .with_shared_buffers(shared_buffers))
}

fn prepare_argument(
    argument: RequestArgument,
    target: SimulationTargetV1,
    total_buffer_bytes: &mut usize,
) -> Result<SimulationArgumentV1, Failure> {
    match argument {
        RequestArgument::Scalar { ty, bits } => {
            let scalar = scalar_type(&ty).ok_or_else(|| {
                Failure::new(
                    Stage::Request,
                    ErrorKind::RequestScalarTypeUnsupported,
                    format!("unsupported scalar type {ty}"),
                )
            })?;
            let width = scalar_width(scalar);
            let value = decode_scalar_bits(&bits, width)?;
            let scalar = ScalarBitsV1::new(scalar, value, target).map_err(|error| {
                Failure::new(
                    Stage::Request,
                    ErrorKind::RequestScalarBitsInvalid,
                    error.to_string(),
                )
            })?;
            Ok(SimulationArgumentV1::Scalar(scalar))
        }
        RequestArgument::Buffer {
            element,
            access,
            alignment,
            bytes,
            initialized,
        } => prepare_buffer(
            element,
            access,
            alignment,
            bytes,
            initialized,
            target,
            total_buffer_bytes,
        )
        .map(SimulationArgumentV1::Buffer),
        RequestArgument::BufferView {
            backing,
            element,
            access,
            alignment,
            byte_offset,
            elements,
        } => {
            let element = buffer_element(&element)?;
            let access = access_mode(&access)?;
            BufferViewArgumentV1::new(
                BufferBackingIdV1(backing),
                element,
                access,
                alignment,
                byte_offset,
                elements,
                target,
            )
            .map(SimulationArgumentV1::BufferView)
            .map_err(|error| {
                Failure::new(
                    Stage::Request,
                    ErrorKind::RequestBufferViewInvalid,
                    error.to_string(),
                )
            })
        }
    }
}

fn prepare_buffer(
    element: String,
    access: String,
    alignment: u32,
    bytes: String,
    initialized: Option<String>,
    target: SimulationTargetV1,
    total_buffer_bytes: &mut usize,
) -> Result<BufferArgumentV1, Failure> {
    let element = buffer_element(&element)?;
    let access = access_mode(&access)?;
    let bytes = decode_buffer_hex(&bytes)?;
    if bytes.len() > MAX_BUFFER_BYTES {
        return Err(request_limit(format!(
            "one buffer exceeds its {MAX_BUFFER_BYTES}-byte bound"
        )));
    }
    *total_buffer_bytes = total_buffer_bytes
        .checked_add(bytes.len())
        .filter(|total| *total <= MAX_TOTAL_BUFFER_BYTES)
        .ok_or_else(|| {
            request_limit(format!(
                "buffers exceed their {MAX_TOTAL_BUFFER_BYTES}-byte aggregate bound"
            ))
        })?;
    let initialized = match initialized {
        Some(bits) => decode_initialization(&bits, bytes.len())?,
        None => {
            let mut state = Vec::new();
            state
                .try_reserve_exact(bytes.len())
                .map_err(|_| request_limit("cannot allocate initialization state"))?;
            state.resize(bytes.len(), true);
            state
        }
    };
    BufferArgumentV1::new(element, access, alignment, bytes, initialized, target).map_err(|error| {
        Failure::new(
            Stage::Request,
            ErrorKind::RequestInitializationInvalid,
            error.to_string(),
        )
    })
}

fn buffer_element(element: &str) -> Result<ScalarType, Failure> {
    scalar_type(element).ok_or_else(|| {
        Failure::new(
            Stage::Request,
            ErrorKind::RequestBufferElementUnsupported,
            format!("unsupported buffer element type {element}"),
        )
    })
}

fn access_mode(access: &str) -> Result<AccessMode, Failure> {
    match access {
        "read_only" => Ok(AccessMode::ReadOnly),
        "read_write" => Ok(AccessMode::ReadWrite),
        _ => Err(Failure::new(
            Stage::Request,
            ErrorKind::RequestBufferAccessInvalid,
            "buffer access must be read_only or read_write",
        )),
    }
}

fn request_limit(message: impl Into<String>) -> Failure {
    Failure::new(Stage::Request, ErrorKind::RequestResourceLimit, message)
}

fn scalar_type(value: &str) -> Option<ScalarType> {
    match value {
        "bool" => Some(ScalarType::Bool),
        "i8" => Some(ScalarType::I8),
        "i16" => Some(ScalarType::I16),
        "i32" => Some(ScalarType::I32),
        "i64" => Some(ScalarType::I64),
        "i128" => Some(ScalarType::I128),
        "u8" => Some(ScalarType::U8),
        "u16" => Some(ScalarType::U16),
        "u32" => Some(ScalarType::U32),
        "u64" => Some(ScalarType::U64),
        "u128" => Some(ScalarType::U128),
        "index" => Some(ScalarType::Index),
        _ => None,
    }
}

fn scalar_width(ty: ScalarType) -> u16 {
    match ty {
        ScalarType::Bool => 1,
        ScalarType::I8 | ScalarType::U8 => 8,
        ScalarType::I16 | ScalarType::U16 => 16,
        ScalarType::I32 | ScalarType::U32 => 32,
        ScalarType::I64 | ScalarType::U64 | ScalarType::Index => 64,
        ScalarType::I128 | ScalarType::U128 => 128,
        ScalarType::F16 | ScalarType::Bf16 | ScalarType::F32 | ScalarType::F64 => {
            unreachable!("floating-point types are rejected by scalar_type")
        }
    }
}

fn scalar_type_name(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Bool => "bool",
        ScalarType::I8 => "i8",
        ScalarType::I16 => "i16",
        ScalarType::I32 => "i32",
        ScalarType::I64 => "i64",
        ScalarType::I128 => "i128",
        ScalarType::U8 => "u8",
        ScalarType::U16 => "u16",
        ScalarType::U32 => "u32",
        ScalarType::U64 => "u64",
        ScalarType::U128 => "u128",
        ScalarType::Index => "index",
        ScalarType::F16 | ScalarType::Bf16 | ScalarType::F32 | ScalarType::F64 => {
            unreachable!("simulator output cannot contain floating-point scalars")
        }
    }
}

fn decode_scalar_bits(text: &str, width: u16) -> Result<u128, Failure> {
    let digits = usize::from(width.div_ceil(4));
    let payload = canonical_hex_payload(text, digits, false).map_err(|message| {
        Failure::new(Stage::Request, ErrorKind::RequestScalarBitsInvalid, message)
    })?;
    u128::from_str_radix(payload, 16).map_err(|_| {
        Failure::new(
            Stage::Request,
            ErrorKind::RequestScalarBitsInvalid,
            "scalar bits are outside the supported 128-bit width",
        )
    })
}

fn decode_buffer_hex(text: &str) -> Result<Vec<u8>, Failure> {
    let payload = text.strip_prefix("0x").ok_or_else(|| {
        Failure::new(
            Stage::Request,
            ErrorKind::RequestHexInvalid,
            "buffer bytes must start with 0x",
        )
    })?;
    if payload.len() % 2 != 0 || !payload.bytes().all(is_lower_hex) {
        return Err(Failure::new(
            Stage::Request,
            ErrorKind::RequestHexInvalid,
            "buffer bytes must use an even number of lowercase hexadecimal digits",
        ));
    }
    if payload.len() / 2 > MAX_BUFFER_BYTES {
        return Err(request_limit(format!(
            "one buffer exceeds its {MAX_BUFFER_BYTES}-byte bound"
        )));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(payload.len() / 2)
        .map_err(|_| request_limit("cannot allocate buffer bytes"))?;
    for pair in payload.as_bytes().chunks_exact(2) {
        bytes.push((hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]));
    }
    Ok(bytes)
}

fn decode_initialization(text: &str, byte_count: usize) -> Result<Vec<bool>, Failure> {
    let bitset_bytes = byte_count.div_ceil(8);
    let payload = canonical_hex_payload(text, bitset_bytes * 2, true).map_err(|message| {
        Failure::new(
            Stage::Request,
            ErrorKind::RequestInitializationInvalid,
            message,
        )
    })?;
    let mut packed = Vec::new();
    packed
        .try_reserve_exact(bitset_bytes)
        .map_err(|_| request_limit("cannot allocate initialization bitset"))?;
    for pair in payload.as_bytes().chunks_exact(2) {
        packed.push((hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]));
    }
    if !byte_count.is_multiple_of(8)
        && packed
            .last()
            .is_some_and(|last| last & !((1_u8 << (byte_count % 8)) - 1) != 0)
    {
        return Err(Failure::new(
            Stage::Request,
            ErrorKind::RequestInitializationInvalid,
            "unused initialization bits must be zero",
        ));
    }
    let mut initialized = Vec::new();
    initialized
        .try_reserve_exact(byte_count)
        .map_err(|_| request_limit("cannot allocate initialization state"))?;
    for index in 0..byte_count {
        initialized.push(packed[index / 8] & (1 << (index % 8)) != 0);
    }
    Ok(initialized)
}

fn canonical_hex_payload(
    text: &str,
    exact_digits: usize,
    initialization: bool,
) -> Result<&str, &'static str> {
    let payload = text.strip_prefix("0x").ok_or(if initialization {
        "initialization bitset must start with 0x"
    } else {
        "scalar bits must start with 0x"
    })?;
    if payload.len() != exact_digits || !payload.bytes().all(is_lower_hex) {
        return Err(if initialization {
            "initialization bitset has the wrong width or non-lowercase digits"
        } else {
            "scalar bits must have the exact type width in lowercase hexadecimal"
        });
    }
    Ok(payload)
}

const fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("caller validates lowercase hexadecimal"),
    }
}

fn kir_error_kind(error: &VerifiedCanonicalKernelIrErrorV7) -> ErrorKind {
    match error {
        VerifiedCanonicalKernelIrErrorV7::Encode(_) => ErrorKind::KirV7EncodeFailed,
        VerifiedCanonicalKernelIrErrorV7::Decode(_) => ErrorKind::KirV7DecodeFailed,
        VerifiedCanonicalKernelIrErrorV7::Verification(_) => ErrorKind::KirV7VerificationFailed,
        VerifiedCanonicalKernelIrErrorV7::NotExactV7 { .. } => ErrorKind::KirV7WrongVersion,
        VerifiedCanonicalKernelIrErrorV7::RoundTripMismatch => ErrorKind::KirV7RoundTripMismatch,
        VerifiedCanonicalKernelIrErrorV7::IdentityMismatch => ErrorKind::KirV7IdentityMismatch,
    }
}

fn admission_error_kind(error: &SimulationAdmissionErrorV1) -> ErrorKind {
    match error {
        SimulationAdmissionErrorV1::InvalidLimits(_) => ErrorKind::SimulatorAdmissionInvalidLimits,
        SimulationAdmissionErrorV1::CanonicalBytesLimit { .. } => {
            ErrorKind::SimulatorAdmissionCanonicalBytesLimit
        }
        SimulationAdmissionErrorV1::DecodeAfterAdmission(_) => {
            ErrorKind::SimulatorAdmissionDecodeFailed
        }
        SimulationAdmissionErrorV1::EncodeAfterAdmission(_) => {
            ErrorKind::SimulatorAdmissionEncodeFailed
        }
        SimulationAdmissionErrorV1::ResidentBytesOverflow => {
            ErrorKind::SimulatorAdmissionResidentBytesOverflow
        }
        SimulationAdmissionErrorV1::ResidentBytesLimit { .. } => {
            ErrorKind::SimulatorAdmissionResidentBytesLimit
        }
    }
}

fn preflight_kind(error: &SimulationPreflightErrorV1) -> ErrorKind {
    match error {
        SimulationPreflightErrorV1::InvalidLimits(_) => ErrorKind::PreflightInvalidLimits,
        SimulationPreflightErrorV1::UnknownKernel(_) => ErrorKind::PreflightUnknownKernel,
        SimulationPreflightErrorV1::MissingEntry(_) => ErrorKind::PreflightMissingEntry,
        SimulationPreflightErrorV1::InvalidLaunch(_) => ErrorKind::PreflightInvalidLaunch,
        SimulationPreflightErrorV1::StaticLaunchMismatch { .. } => {
            ErrorKind::PreflightStaticLaunchMismatch
        }
        SimulationPreflightErrorV1::WorkgroupMismatch { .. } => {
            ErrorKind::PreflightWorkgroupMismatch
        }
        SimulationPreflightErrorV1::ResourceLimit { .. } => ErrorKind::PreflightResourceLimit,
        SimulationPreflightErrorV1::Unsupported(_) => ErrorKind::PreflightUnsupported,
        SimulationPreflightErrorV1::ArgumentCount { .. } => ErrorKind::PreflightArgumentCount,
        SimulationPreflightErrorV1::ArgumentType { .. } => ErrorKind::PreflightArgumentType,
        SimulationPreflightErrorV1::BufferAccess { .. } => ErrorKind::PreflightBufferAccess,
        SimulationPreflightErrorV1::TargetLayout { .. } => ErrorKind::PreflightTargetLayout,
        SimulationPreflightErrorV1::TargetValueOutOfRange { .. } => {
            ErrorKind::PreflightTargetValueOutOfRange
        }
        SimulationPreflightErrorV1::SharedTargetLayout(_) => ErrorKind::PreflightSharedTargetLayout,
        SimulationPreflightErrorV1::DuplicateBacking(_) => ErrorKind::PreflightDuplicateBacking,
        SimulationPreflightErrorV1::MissingBacking { .. } => ErrorKind::PreflightMissingBacking,
        SimulationPreflightErrorV1::BufferViewBounds { .. } => ErrorKind::PreflightBufferViewBounds,
        SimulationPreflightErrorV1::AllocationFailure => ErrorKind::PreflightAllocationFailure,
    }
}

fn execution_kind(error: &SimulationExecutionErrorKindV1) -> ErrorKind {
    match error {
        SimulationExecutionErrorKindV1::StepLimit { .. } => ErrorKind::ExecutionStepLimit,
        SimulationExecutionErrorKindV1::EventLimit { .. } => ErrorKind::ExecutionEventLimit,
        SimulationExecutionErrorKindV1::CallDepthLimit { .. } => ErrorKind::ExecutionCallDepthLimit,
        SimulationExecutionErrorKindV1::SsaValueLimit { .. } => ErrorKind::ExecutionSsaValueLimit,
        SimulationExecutionErrorKindV1::AllocationLimit { .. } => {
            ErrorKind::ExecutionAllocationLimit
        }
        SimulationExecutionErrorKindV1::AllocationBytesLimit { .. } => {
            ErrorKind::ExecutionAllocationBytesLimit
        }
        SimulationExecutionErrorKindV1::TotalBytesLimit { .. } => {
            ErrorKind::ExecutionTotalBytesLimit
        }
        SimulationExecutionErrorKindV1::AllocationFailure => ErrorKind::ExecutionAllocationFailure,
        SimulationExecutionErrorKindV1::MissingFunction(_) => ErrorKind::ExecutionMissingFunction,
        SimulationExecutionErrorKindV1::MissingBody(_) => ErrorKind::ExecutionMissingBody,
        SimulationExecutionErrorKindV1::UnknownBlock(_) => ErrorKind::ExecutionUnknownBlock,
        SimulationExecutionErrorKindV1::MissingTerminator(_) => {
            ErrorKind::ExecutionMissingTerminator
        }
        SimulationExecutionErrorKindV1::UndefinedValue(_) => ErrorKind::ExecutionUndefinedValue,
        SimulationExecutionErrorKindV1::RuntimeType { .. } => ErrorKind::ExecutionRuntimeType,
        SimulationExecutionErrorKindV1::ResultArity { .. } => ErrorKind::ExecutionResultArity,
        SimulationExecutionErrorKindV1::BlockArgumentArity { .. } => {
            ErrorKind::ExecutionBlockArgumentArity
        }
        SimulationExecutionErrorKindV1::UndefinedIntegerOperation(_) => {
            ErrorKind::ExecutionUndefinedIntegerOperation
        }
        SimulationExecutionErrorKindV1::IntegerOutOfRange => ErrorKind::ExecutionIntegerOutOfRange,
        SimulationExecutionErrorKindV1::PointerOffsetOverflow => {
            ErrorKind::ExecutionPointerOffsetOverflow
        }
        SimulationExecutionErrorKindV1::DanglingPointer { .. } => {
            ErrorKind::ExecutionDanglingPointer
        }
        SimulationExecutionErrorKindV1::AddressSpaceMismatch => {
            ErrorKind::ExecutionAddressSpaceMismatch
        }
        SimulationExecutionErrorKindV1::ReadOnlyWrite => ErrorKind::ExecutionReadOnlyWrite,
        SimulationExecutionErrorKindV1::MisalignedAccess { .. } => {
            ErrorKind::ExecutionMisalignedAccess
        }
        SimulationExecutionErrorKindV1::OutOfBounds { .. } => ErrorKind::ExecutionOutOfBounds,
        SimulationExecutionErrorKindV1::UninitializedRead { .. } => {
            ErrorKind::ExecutionUninitializedRead
        }
        SimulationExecutionErrorKindV1::WorkgroupUseBeforePublish { .. } => {
            ErrorKind::ExecutionWorkgroupUseBeforePublish
        }
        SimulationExecutionErrorKindV1::DivergentWorkgroupBarrier(_) => {
            ErrorKind::ExecutionDivergentWorkgroupBarrier
        }
        SimulationExecutionErrorKindV1::MismatchedWorkgroupBarrier(_) => {
            ErrorKind::ExecutionMismatchedWorkgroupBarrier
        }
        SimulationExecutionErrorKindV1::WorkgroupSchedulerNoProgress { .. } => {
            ErrorKind::ExecutionWorkgroupSchedulerNoProgress
        }
        SimulationExecutionErrorKindV1::ReachedUnreachable => {
            ErrorKind::ExecutionReachedUnreachable
        }
        SimulationExecutionErrorKindV1::InternalInvariant(_) => {
            ErrorKind::ExecutionInternalInvariant
        }
        SimulationExecutionErrorKindV1::EventSinkFailure(_) => ErrorKind::ExecutionEventSinkFailure,
    }
}

fn unsupported_code(feature: &UnsupportedFeatureV1) -> UnsupportedFeatureCode {
    match feature {
        UnsupportedFeatureV1::FloatType(_) => UnsupportedFeatureCode::FloatType,
        UnsupportedFeatureV1::UnsupportedType => UnsupportedFeatureCode::UnsupportedType,
        UnsupportedFeatureV1::MemoryIntrinsic => UnsupportedFeatureCode::MemoryIntrinsic,
        UnsupportedFeatureV1::FloatConstant => UnsupportedFeatureCode::FloatConstant,
        UnsupportedFeatureV1::FloatOperation => UnsupportedFeatureCode::FloatOperation,
        UnsupportedFeatureV1::InvalidIntegerCast { .. } => {
            UnsupportedFeatureCode::InvalidIntegerCast
        }
        UnsupportedFeatureV1::ExternalCall(_) => UnsupportedFeatureCode::ExternalCall,
        UnsupportedFeatureV1::NonInternalCall { .. } => UnsupportedFeatureCode::NonInternalCall,
        UnsupportedFeatureV1::WorkgroupAllocation => UnsupportedFeatureCode::WorkgroupAllocation,
        UnsupportedFeatureV1::NonScalarMemory => UnsupportedFeatureCode::NonScalarMemory,
        UnsupportedFeatureV1::UnsupportedAddressSpace(_) => {
            UnsupportedFeatureCode::UnsupportedAddressSpace
        }
        UnsupportedFeatureV1::Barrier => UnsupportedFeatureCode::Barrier,
        UnsupportedFeatureV1::Atomic => UnsupportedFeatureCode::Atomic,
        UnsupportedFeatureV1::Fence => UnsupportedFeatureCode::Fence,
        UnsupportedFeatureV1::WorkgroupBarrier => UnsupportedFeatureCode::WorkgroupBarrier,
        UnsupportedFeatureV1::WorkgroupMemory => UnsupportedFeatureCode::WorkgroupMemory,
        UnsupportedFeatureV1::DynamicWorkgroupMemory => {
            UnsupportedFeatureCode::DynamicWorkgroupMemory
        }
        UnsupportedFeatureV1::Matrix => UnsupportedFeatureCode::Matrix,
        UnsupportedFeatureV1::Wave => UnsupportedFeatureCode::Wave,
        UnsupportedFeatureV1::Gfx950LdsTranspose => UnsupportedFeatureCode::Gfx950LdsTranspose,
        UnsupportedFeatureV1::InlineAssembly => UnsupportedFeatureCode::InlineAssembly,
        UnsupportedFeatureV1::UnsupportedScalarOperation => {
            UnsupportedFeatureCode::UnsupportedScalarOperation
        }
        UnsupportedFeatureV1::TargetConstantOutOfRange => {
            UnsupportedFeatureCode::TargetConstantOutOfRange
        }
    }
}

fn unsupported_document(site: &UnsupportedFinding) -> UnsupportedDocument<'_> {
    UnsupportedDocument {
        function: &site.function,
        function_bytes: site.function_bytes,
        function_truncated: site.function_truncated,
        block: site.block,
        operation: site.operation,
        feature: site.feature,
    }
}

fn site_document(site: SimulationSiteV1) -> SiteDocument {
    let function_bytes = site.function.as_str().len();
    SiteDocument {
        function: BoundedFunctionId(site.function),
        function_bytes,
        function_truncated: function_bytes > MAX_ERROR_FUNCTION_BYTES,
        block: site.block.0,
        operation: site.operation,
    }
}

fn measure_success_bytes(execution: &SimulationExecutionV1) -> Result<usize, Failure> {
    let mut bounded = BoundedWriter::new(
        CountingWriter::default(),
        MAX_SUCCESS_BYTES.saturating_sub(1),
    );
    if let Err(error) = write_success(&mut bounded, execution) {
        return if error.kind() == io::ErrorKind::FileTooLarge {
            Err(output_too_large())
        } else {
            Err(output_write_failure(error))
        };
    }
    bounded
        .into_inner()
        .0
        .checked_add(1)
        .ok_or_else(output_too_large)
}

fn output_too_large() -> Failure {
    Failure::new(
        Stage::Output,
        ErrorKind::OutputTooLarge,
        format!("result exceeds its {MAX_SUCCESS_BYTES}-byte bound"),
    )
}

struct BoundedWriter<W> {
    inner: W,
    written: usize,
    limit: usize,
}

impl<W> BoundedWriter<W> {
    const fn new(inner: W, limit: usize) -> Self {
        Self {
            inner,
            written: 0,
            limit,
        }
    }

    fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for BoundedWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.limit.saturating_sub(self.written) {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "bounded JSON output limit exceeded",
            ));
        }
        let count = self.inner.write(buffer)?;
        self.written = self
            .written
            .checked_add(count)
            .ok_or_else(|| io::Error::other("bounded JSON byte count overflow"))?;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn write_success_stdout(execution: &SimulationExecutionV1, maximum: usize) -> Result<(), Failure> {
    let stdout = io::stdout();
    let bounded = BoundedWriter::new(stdout.lock(), maximum);
    let mut output = BufWriter::with_capacity(32 * 1024, bounded);
    write_success(&mut output, execution)
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush())
        .map_err(output_write_failure)
}

fn write_success<W: Write + ?Sized>(
    writer: &mut W,
    execution: &SimulationExecutionV1,
) -> io::Result<()> {
    writer.write_all(b"{\"schema\":\"")?;
    writer.write_all(RESULT_SCHEMA.as_bytes())?;
    writer.write_all(b"\",\"status\":\"ok\",\"authority\":\"observation_only\",\"simulated\":true,\"hardware_observed\":false,\"hardware_validation\":false,\"performance_prediction\":false,\"target_profile\":{\"identity\":\"amdgpu_64_little_endian_v1\",\"index_bits\":64,\"max_workgroup_invocations\":1024},\"kir\":{\"sha256\":\"")?;
    write_lower_hex(writer, execution.identity().digest(), false)?;
    write!(
        writer,
        "\",\"canonical_bytes\":{}}},\"counts\":{{\"arguments\":{},\"shared_buffers\":{},\"invocations_executed\":{},\"workgroups_visited\":{},\"scheduled_slots_visited\":{},\"steps_executed\":{},\"events_emitted\":{}}},\"schedule\":{{\"identity\":\"{}\"}},\"conflict_assessment\":",
        execution.identity().canonical_length(),
        execution.arguments().len(),
        execution.shared_buffers().len(),
        execution.invocations_executed(),
        execution.workgroups_visited(),
        execution.scheduled_slots_visited(),
        execution.steps_executed(),
        execution.events_emitted(),
        schedule_name(execution.schedule()),
    )?;
    write_conflict_assessment(writer, execution.conflict_assessment())?;
    writer.write_all(b",\"arguments\":[")?;
    for (index, argument) in execution.arguments().iter().enumerate() {
        if index != 0 {
            writer.write_all(b",")?;
        }
        write_argument(writer, argument)?;
    }
    writer.write_all(b"],\"shared_buffers\":[")?;
    for (index, shared) in execution.shared_buffers().iter().enumerate() {
        if index != 0 {
            writer.write_all(b",")?;
        }
        write!(writer, "{{\"id\":{},\"buffer\":", shared.id.0)?;
        write_buffer(writer, &shared.buffer)?;
        writer.write_all(b"}")?;
    }
    writer.write_all(b"]}")
}

const fn schedule_name(schedule: SimulationScheduleIdentityV1) -> &'static str {
    match schedule {
        SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxSerialV1 => {
            "workgroup_major_local_zyx_serial_v1"
        }
        SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1 => {
            "workgroup_major_local_zyx_cooperative_v1"
        }
    }
}

fn write_conflict_assessment<W: Write + ?Sized>(
    writer: &mut W,
    assessment: &SimulationConflictAssessmentV1,
) -> io::Result<()> {
    match assessment {
        SimulationConflictAssessmentV1::NoConflictsObserved => {
            writer.write_all(b"{\"status\":\"no_conflicts_observed\"}")
        }
        SimulationConflictAssessmentV1::ConflictsObserved {
            conflicting_bytes,
            first,
        } => {
            write!(
                writer,
                "{{\"status\":\"conflicts_observed\",\"conflicting_bytes\":{conflicting_bytes},\"first\":"
            )?;
            write_memory_conflict(writer, first)?;
            writer.write_all(b"}")
        }
        SimulationConflictAssessmentV1::Incomplete {
            conflicting_bytes,
            first,
            record_limit,
        } => {
            write!(
                writer,
                "{{\"status\":\"incomplete\",\"conflicting_bytes\":{conflicting_bytes},\"record_limit\":{record_limit},\"first\":"
            )?;
            match first {
                Some(first) => write_memory_conflict(writer, first)?,
                None => writer.write_all(b"null")?,
            }
            writer.write_all(b"}")
        }
    }
}

fn write_memory_conflict<W: Write + ?Sized>(
    writer: &mut W,
    conflict: &SimulationMemoryConflictV1,
) -> io::Result<()> {
    write!(
        writer,
        "{{\"allocation\":{},\"offset\":{},\"earlier\":",
        conflict.allocation, conflict.offset
    )?;
    write_invocation(writer, &conflict.earlier)?;
    writer.write_all(b",\"later\":")?;
    write_invocation(writer, &conflict.later)?;
    writer.write_all(b",\"earlier_site\":")?;
    write_site(writer, &conflict.earlier_site)?;
    writer.write_all(b",\"later_site\":")?;
    write_site(writer, &conflict.later_site)?;
    writer.write_all(b"}")
}

fn write_invocation<W: Write + ?Sized>(
    writer: &mut W,
    invocation: &SimulationInvocationV1,
) -> io::Result<()> {
    write!(
        writer,
        "{{\"global\":[{},{},{}],\"workgroup\":[{},{},{}],\"local\":[{},{},{}],\"workgroup_size\":[{},{},{}],\"workgroup_count\":[{},{},{}],\"launch_extent\":[{},{},{}]}}",
        invocation.global[0],
        invocation.global[1],
        invocation.global[2],
        invocation.workgroup[0],
        invocation.workgroup[1],
        invocation.workgroup[2],
        invocation.local[0],
        invocation.local[1],
        invocation.local[2],
        invocation.workgroup_size[0],
        invocation.workgroup_size[1],
        invocation.workgroup_size[2],
        invocation.workgroup_count[0],
        invocation.workgroup_count[1],
        invocation.workgroup_count[2],
        invocation.launch_extent[0],
        invocation.launch_extent[1],
        invocation.launch_extent[2],
    )
}

fn write_site<W: Write + ?Sized>(writer: &mut W, site: &SimulationSiteV1) -> io::Result<()> {
    writer.write_all(b"{\"function\":")?;
    serde_json::to_writer(&mut *writer, site.function.as_str()).map_err(io::Error::other)?;
    write!(writer, ",\"block\":{},\"operation\":", site.block.0)?;
    match site.operation {
        Some(operation) => write!(writer, "{operation}")?,
        None => writer.write_all(b"null")?,
    }
    writer.write_all(b"}")
}

fn write_argument<W: Write + ?Sized>(
    writer: &mut W,
    argument: &SimulationArgumentV1,
) -> io::Result<()> {
    match argument {
        SimulationArgumentV1::Scalar(scalar) => {
            write!(
                writer,
                "{{\"kind\":\"scalar\",\"type\":\"{}\",\"bits\":\"0x{:0width$x}\"}}",
                scalar_type_name(scalar.ty()),
                scalar.bits(),
                width = usize::from(scalar_width(scalar.ty()).div_ceil(4)),
            )
        }
        SimulationArgumentV1::Buffer(buffer) => {
            writer.write_all(b"{\"kind\":\"buffer\",\"value\":")?;
            write_buffer(writer, buffer)?;
            writer.write_all(b"}")
        }
        SimulationArgumentV1::BufferView(view) => {
            write!(
                writer,
                "{{\"kind\":\"buffer_view\",\"backing\":{},\"element\":\"{}\",\"access\":\"{}\",\"alignment\":{},\"byte_offset\":{},\"elements\":{}}}",
                view.backing().0,
                scalar_type_name(view.element()),
                access_name(view.access()),
                view.alignment(),
                view.byte_offset(),
                view.elements(),
            )
        }
    }
}

fn write_buffer<W: Write + ?Sized>(writer: &mut W, buffer: &BufferArgumentV1) -> io::Result<()> {
    write!(
        writer,
        "{{\"element\":\"{}\",\"access\":\"{}\",\"alignment\":{},\"bytes\":\"",
        scalar_type_name(buffer.element()),
        access_name(buffer.access()),
        buffer.alignment(),
    )?;
    write_lower_hex(writer, buffer.bytes(), true)?;
    writer.write_all(b"\",\"initialized\":\"")?;
    write_initialization(writer, buffer.initialized())?;
    writer.write_all(b"\"}")
}

const fn access_name(access: AccessMode) -> &'static str {
    match access {
        AccessMode::ReadOnly => "read_only",
        AccessMode::ReadWrite => "read_write",
    }
}

fn write_lower_hex<W: Write + ?Sized>(
    writer: &mut W,
    bytes: &[u8],
    prefix: bool,
) -> io::Result<()> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    if prefix {
        writer.write_all(b"0x")?;
    }
    let mut encoded = [0_u8; HEX_INPUT_CHUNK_BYTES * 2];
    for chunk in bytes.chunks(HEX_INPUT_CHUNK_BYTES) {
        for (index, byte) in chunk.iter().enumerate() {
            encoded[index * 2] = HEX[usize::from(byte >> 4)];
            encoded[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
        }
        writer.write_all(&encoded[..chunk.len() * 2])?;
    }
    Ok(())
}

fn write_initialization<W: Write + ?Sized>(writer: &mut W, initialized: &[bool]) -> io::Result<()> {
    writer.write_all(b"0x")?;
    let mut packed = [0_u8; HEX_INPUT_CHUNK_BYTES];
    for chunk in initialized.chunks(HEX_INPUT_CHUNK_BYTES * 8) {
        let packed_len = chunk.len().div_ceil(8);
        packed[..packed_len].fill(0);
        for (index, initialized) in chunk.iter().copied().enumerate() {
            if initialized {
                packed[index / 8] |= 1 << (index % 8);
            }
        }
        write_lower_hex(writer, &packed[..packed_len], false)?;
    }
    Ok(())
}

fn output_write_failure(error: io::Error) -> Failure {
    let kind = if error.kind() == io::ErrorKind::FileTooLarge {
        ErrorKind::OutputTooLarge
    } else {
        ErrorKind::OutputWriteFailed
    };
    Failure::new(
        Stage::Output,
        kind,
        format!("cannot write result JSON: {error}"),
    )
}

fn publish_transactionally(
    path: &Path,
    execution: &SimulationExecutionV1,
    maximum: usize,
) -> Result<(), Failure> {
    publish_payload(path, maximum, |writer| {
        write_success(writer, execution)?;
        writer.write_all(b"\n")
    })
}

fn publish_payload(
    path: &Path,
    maximum: usize,
    emit: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(), Failure> {
    if path.as_os_str().as_bytes().ends_with(b"/") {
        return Err(Failure::new(
            Stage::Output,
            ErrorKind::OutputInvalidPath,
            "output path must not end with a directory separator",
        ));
    }
    let target = path
        .file_name()
        .filter(|name| *name != OsStr::new(".") && *name != OsStr::new("..") && !name.is_empty());
    let target = target.ok_or_else(|| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputInvalidPath,
            "output path must name a file beneath an existing directory",
        )
    })?;
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_descriptor = openat2(
        rustix::fs::CWD,
        parent_path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|error| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputParentOpenFailed,
            format!("cannot securely open output parent: {error}"),
        )
    })?;
    let parent = File::from(parent_descriptor);
    let stat = fstat(&parent).map_err(|error| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputParentOpenFailed,
            format!("cannot inspect output parent: {error}"),
        )
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return Err(Failure::new(
            Stage::Output,
            ErrorKind::OutputParentOpenFailed,
            "output parent is not a directory",
        ));
    }

    let descriptor = openat(
        &parent,
        ".",
        OFlags::RDWR | OFlags::TMPFILE | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|error| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputTemporaryCreateFailed,
            format!("output filesystem does not support retained anonymous files: {error}"),
        )
    })?;
    let mut temporary = File::from(descriptor);
    fchmod(&temporary, Mode::from_raw_mode(0o600)).map_err(|error| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputTemporaryCreateFailed,
            format!("cannot set anonymous result mode: {error}"),
        )
    })?;
    let stat = fstat(&temporary).map_err(|error| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputTemporaryCreateFailed,
            format!("cannot inspect anonymous result: {error}"),
        )
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_mode & 0o777 != 0o600
        || stat.st_nlink != 0
    {
        return Err(Failure::new(
            Stage::Output,
            ErrorKind::OutputTemporaryCreateFailed,
            "anonymous result is not a retained private regular inode",
        ));
    }
    {
        let bounded = BoundedWriter::new(&mut temporary, maximum);
        let mut buffered = BufWriter::with_capacity(32 * 1024, bounded);
        emit(&mut buffered).map_err(output_write_failure)?;
        buffered.flush().map_err(output_write_failure)?;
    }
    fsync(&temporary).map_err(|error| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputSyncFailed,
            format!("cannot fsync anonymous result: {error}"),
        )
    })?;
    let proc_fd = open_authenticated_proc_fd_directory(Path::new("/proc"))?;
    link_retained_anonymous(&temporary, &parent, target, &proc_fd)?;
    let linked = match fstat(&temporary) {
        Ok(linked) => linked,
        Err(error) => {
            return Err(Failure::published(
                ErrorKind::OutputPublishFailed,
                format!("cannot reinspect published result descriptor: {error}"),
                PublicationState::PublishedNameUncertain,
            ));
        }
    };
    let named = match statat(&parent, target, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(named) => named,
        Err(error) => {
            return Err(Failure::published(
                ErrorKind::OutputPublishFailed,
                format!("cannot inspect published result name: {error}"),
                PublicationState::PublishedNameUncertain,
            ));
        }
    };
    if !same_published_inode(&stat, &linked, &named) {
        return Err(Failure::published(
            ErrorKind::OutputPublishFailed,
            "published result is not the retained anonymous inode",
            PublicationState::PublishedNameUncertain,
        ));
    }
    if let Err(error) = fsync(&parent) {
        return Err(Failure::published(
            ErrorKind::OutputDirectorySyncFailed,
            format!("cannot fsync output directory: {error}"),
            PublicationState::PublishedDurabilityUnknown,
        ));
    }
    let final_named = match statat(&parent, target, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(named) => named,
        Err(error) => {
            return Err(Failure::published(
                ErrorKind::OutputPublishFailed,
                format!("cannot revalidate published result name: {error}"),
                PublicationState::PublishedNameUncertain,
            ));
        }
    };
    if !same_published_inode(&stat, &linked, &final_named) {
        return Err(Failure::published(
            ErrorKind::OutputPublishFailed,
            "published result name changed during directory sync",
            PublicationState::PublishedNameUncertain,
        ));
    }
    Ok(())
}

fn link_retained_anonymous(
    temporary: &File,
    parent: &File,
    target: &OsStr,
    proc_fd_directory: &File,
) -> Result<(), Failure> {
    let source = temporary.as_raw_fd().to_string();
    let descriptor = fstat(temporary).map_err(|error| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputProcFdUnavailable,
            format!("cannot inspect retained result before publication: {error}"),
        )
    })?;
    let proc_source = statat(proc_fd_directory, &source, AtFlags::empty()).map_err(|error| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputProcFdUnavailable,
            format!("cannot inspect pinned procfs fd entry: {error}"),
        )
    })?;
    if descriptor.st_nlink != 0
        || proc_source.st_dev != descriptor.st_dev
        || proc_source.st_ino != descriptor.st_ino
        || proc_source.st_mode != descriptor.st_mode
        || proc_source.st_size != descriptor.st_size
        || proc_source.st_nlink != descriptor.st_nlink
    {
        return Err(Failure::new(
            Stage::Output,
            ErrorKind::OutputProcFdUnavailable,
            "pinned procfs fd entry does not name the retained anonymous inode",
        ));
    }
    linkat(
        proc_fd_directory,
        &source,
        parent,
        target,
        AtFlags::SYMLINK_FOLLOW,
    )
    .map_err(|error| {
        if error == rustix::io::Errno::EXIST {
            Failure::new(
                Stage::Output,
                ErrorKind::OutputAlreadyExists,
                "output already exists",
            )
        } else {
            Failure::new(
                Stage::Output,
                ErrorKind::OutputProcFdUnavailable,
                format!("cannot publish retained inode through trusted procfs fd link: {error}"),
            )
        }
    })
}

fn open_authenticated_proc_fd_directory(proc_path: &Path) -> Result<File, Failure> {
    let proc_descriptor = openat2(
        rustix::fs::CWD,
        proc_path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|error| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputProcFdUnavailable,
            format!("cannot pin procfs root: {error}"),
        )
    })?;
    let proc_root = File::from(proc_descriptor);
    authenticate_procfs(&proc_root)?;

    let pid_fd = format!("{}/fd", std::process::id());
    let fd_descriptor = openat2(
        &proc_root,
        Path::new(&pid_fd),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|error| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputProcFdUnavailable,
            format!("cannot pin current process procfs fd directory: {error}"),
        )
    })?;
    let fd_directory = File::from(fd_descriptor);
    authenticate_procfs(&fd_directory)?;
    Ok(fd_directory)
}

fn authenticate_procfs(directory: &File) -> Result<(), Failure> {
    let filesystem = fstatfs(directory).map_err(|error| {
        Failure::new(
            Stage::Output,
            ErrorKind::OutputProcFdUnavailable,
            format!("cannot authenticate procfs: {error}"),
        )
    })?;
    if filesystem.f_type != PROC_SUPER_MAGIC {
        return Err(Failure::new(
            Stage::Output,
            ErrorKind::OutputProcFdUnavailable,
            "retained fd publication requires an authenticated procfs mount",
        ));
    }
    Ok(())
}

fn same_published_inode(
    anonymous: &rustix::fs::Stat,
    descriptor: &rustix::fs::Stat,
    named: &rustix::fs::Stat,
) -> bool {
    descriptor.st_dev == anonymous.st_dev
        && descriptor.st_ino == anonymous.st_ino
        && descriptor.st_nlink == 1
        && named.st_dev == descriptor.st_dev
        && named.st_ino == descriptor.st_ino
        && named.st_mode == descriptor.st_mode
        && named.st_size == descriptor.st_size
}

#[derive(Serialize)]
struct ErrorDocumentView<'a> {
    schema: &'static str,
    status: &'static str,
    stage: Stage,
    kind: ErrorKind,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<InputCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invocation: Option<&'a InvocationDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    site: Option<&'a SiteDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    publication_state: Option<PublicationState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unsupported: Option<UnsupportedSummary<'a>>,
}

#[derive(Serialize)]
struct UnsupportedSummary<'a> {
    total: u64,
    emitted: usize,
    truncated: bool,
    sites: UnsupportedSites<'a>,
}

struct UnsupportedSites<'a>(&'a [UnsupportedFinding]);

impl Serialize for UnsupportedSites<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for site in self.0 {
            sequence.serialize_element(&unsupported_document(site))?;
        }
        sequence.end()
    }
}

#[derive(Default)]
struct CountingWriter(usize);

impl Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::other("JSON byte count overflow"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn unsupported_prefix_len(sites: &[UnsupportedFinding]) -> usize {
    let mut bytes = 0_usize;
    let mut emitted = 0_usize;
    for site in sites.iter().take(MAX_UNSUPPORTED_FINDINGS) {
        let mut counter = CountingWriter::default();
        if serde_json::to_writer(&mut counter, &unsupported_document(site)).is_err() {
            break;
        }
        let separator = usize::from(emitted != 0);
        let Some(next) = bytes
            .checked_add(separator)
            .and_then(|value| value.checked_add(counter.0))
        else {
            break;
        };
        if next > MAX_UNSUPPORTED_JSON_BYTES {
            break;
        }
        bytes = next;
        emitted += 1;
    }
    emitted
}

fn error_document_view(error: &Failure) -> ErrorDocumentView<'_> {
    let unsupported = error.1.as_ref().map(|report| {
        let emitted = unsupported_prefix_len(&report.findings);
        UnsupportedSummary {
            total: report.total,
            emitted,
            truncated: emitted as u64 != report.total,
            sites: UnsupportedSites(&report.findings[..emitted]),
        }
    });
    ErrorDocumentView {
        schema: error.0.schema,
        status: error.0.status,
        stage: error.0.stage,
        kind: error.0.kind,
        message: &error.0.message,
        input: error.0.input,
        invocation: error.0.invocation.as_ref(),
        site: error.0.site.as_ref(),
        publication_state: error.0.publication_state,
        unsupported,
    }
}

fn write_error(error: &Failure) {
    const FALLBACK: &[u8] = b"{\"schema\":\"fe2o3-simulation-error-v1\",\"status\":\"error\",\"stage\":\"output\",\"kind\":\"output_serialization_failed\",\"message\":\"cannot serialize bounded error JSON\"}\n";
    let mut guarded = BoundedWriter::new(FallibleVecWriter::default(), MAX_ERROR_BYTES);
    let serialized = serde_json::to_writer(&mut guarded, &error_document_view(error))
        .map_err(|_| ())
        .and_then(|()| guarded.write_all(b"\n").map_err(|_| ()));
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    match serialized {
        Ok(()) => {
            let _ = stderr.write_all(&guarded.into_inner().0);
        }
        Err(()) => {
            let _ = stderr.write_all(FALLBACK);
        }
    }
    let _ = stderr.flush();
}

#[derive(Default)]
struct FallibleVecWriter(Vec<u8>);

impl Write for FallibleVecWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0
            .try_reserve(buffer.len())
            .map_err(|_| io::Error::other("cannot allocate bounded error JSON"))?;
        self.0.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use fe2o3_kernel_ir::{
        AddressSpace, BasicBlock, BlockId, Function, FunctionId, Kernel, LaunchDomain,
        LaunchExtent, Module, Signature, Terminator, Type, VerifiedCanonicalKernelIrV7,
    };

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "fe2o3-kir-sim-cli-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn request(json: &str) -> Result<RequestDocument, serde_json::Error> {
        serde_json::from_str(json)
    }

    fn base_request(arguments: &str) -> String {
        format!(
            "{{\"schema\":\"{REQUEST_SCHEMA}\",\"kernel\":\"kernel\",\"grid\":[1,1,1],\"workgroup\":[1,1,1],\"arguments\":[{arguments}]}}"
        )
    }

    #[test]
    fn command_line_paths_remain_os_strings() {
        let options = parse_options(
            [
                OsString::from("--kir-v7"),
                OsString::from("kernel"),
                OsString::from("--request"),
                OsString::from("request"),
                OsString::from("--output"),
                OsString::from("output"),
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(options.kir_v7, OsString::from("kernel"));
        assert_eq!(options.request, OsString::from("request"));
        assert_eq!(options.output, Some(OsString::from("output")));
    }

    #[test]
    fn strict_json_rejects_duplicates_unknowns_and_null_initialization() {
        let duplicate = format!(
            "{{\"schema\":\"{REQUEST_SCHEMA}\",\"kernel\":\"a\",\"kernel\":\"b\",\"grid\":[1,1,1],\"workgroup\":[1,1,1],\"arguments\":[]}}"
        );
        assert_eq!(
            request_json_kind(&request(&duplicate).unwrap_err()),
            ErrorKind::RequestJsonDuplicateField
        );

        let nested_duplicate = base_request(
            "{\"kind\":\"scalar\",\"kind\":\"scalar\",\"type\":\"u32\",\"bits\":\"0x00000000\"}",
        );
        assert_eq!(
            request_json_kind(&request(&nested_duplicate).unwrap_err()),
            ErrorKind::RequestJsonDuplicateField
        );

        let unknown = base_request(
            "{\"kind\":\"scalar\",\"type\":\"u32\",\"bits\":\"0x00000000\",\"extra\":0}",
        );
        assert_eq!(
            request_json_kind(&request(&unknown).unwrap_err()),
            ErrorKind::RequestJsonUnknownField
        );

        let null = base_request(
            "{\"kind\":\"buffer\",\"element\":\"u8\",\"access\":\"read_only\",\"alignment\":1,\"bytes\":\"0x00\",\"initialized\":null}",
        );
        assert_eq!(
            request_json_kind(&request(&null).unwrap_err()),
            ErrorKind::RequestJsonNull
        );

        let null_schema = "{\"schema\":null,\"kernel\":\"k\",\"grid\":[1,1,1],\"workgroup\":[1,1,1],\"arguments\":[]}";
        assert_eq!(
            request_json_kind(&request(null_schema).unwrap_err()),
            ErrorKind::RequestJsonNull
        );

        let invalid_tag = base_request("{\"kind\":\"float\",\"type\":\"u32\",\"bits\":\"0\"}");
        assert_eq!(
            request_json_kind(&request(&invalid_tag).unwrap_err()),
            ErrorKind::RequestJsonInvalidTag
        );

        let syntax = "{";
        assert_eq!(
            request_json_kind(&request(syntax).unwrap_err()),
            ErrorKind::RequestJsonSyntax
        );

        let spoofed_marker = format!(
            "{{\"schema\":\"{REQUEST_SCHEMA}\",\"kernel\":\"k\",\"grid\":\"fe2o3:null\",\"workgroup\":[1,1,1],\"arguments\":[]}}"
        );
        assert_eq!(
            request_json_kind(&request(&spoofed_marker).unwrap_err()),
            ErrorKind::RequestJsonTypeMismatch
        );
    }

    #[test]
    fn typed_scalars_and_initialization_are_exact() {
        let json = base_request(
            "{\"kind\":\"scalar\",\"type\":\"bool\",\"bits\":\"0x1\"},\
             {\"kind\":\"scalar\",\"type\":\"i8\",\"bits\":\"0xff\"},\
             {\"kind\":\"scalar\",\"type\":\"index\",\"bits\":\"0x000000000000002a\"},\
             {\"kind\":\"scalar\",\"type\":\"i128\",\"bits\":\"0xffffffffffffffffffffffffffffffff\"},\
             {\"kind\":\"buffer\",\"element\":\"u8\",\"access\":\"read_write\",\"alignment\":1,\"bytes\":\"0x000102030405060708\",\"initialized\":\"0x0101\"}",
        );
        let prepared = prepare_request(request(&json).unwrap()).unwrap();
        assert_eq!(prepared.arguments.len(), 5);
        match &prepared.arguments[0] {
            SimulationArgumentV1::Scalar(value) => assert_eq!(value.as_bool(), Some(true)),
            _ => panic!("expected scalar"),
        }
        match &prepared.arguments[1] {
            SimulationArgumentV1::Scalar(value) => assert_eq!(value.bits(), 0xff),
            _ => panic!("expected scalar"),
        }
        match &prepared.arguments[2] {
            SimulationArgumentV1::Scalar(value) => {
                assert_eq!(value.ty(), ScalarType::Index);
                assert_eq!(value.bits(), 42);
            }
            _ => panic!("expected scalar"),
        }
        match &prepared.arguments[3] {
            SimulationArgumentV1::Scalar(value) => assert_eq!(value.bits(), u128::MAX),
            _ => panic!("expected scalar"),
        }
        match &prepared.arguments[4] {
            SimulationArgumentV1::Buffer(buffer) => {
                assert_eq!(
                    buffer.initialized(),
                    &[true, false, false, false, false, false, false, false, true]
                );
            }
            _ => panic!("expected buffer"),
        }
    }

    #[test]
    fn shared_backings_and_overlapping_views_are_preserved() {
        let json = format!(
            "{{\"schema\":\"{REQUEST_SCHEMA}\",\"kernel\":\"kernel\",\"grid\":[1,1,1],\"workgroup\":[1,1,1],\"arguments\":[{{\"kind\":\"buffer_view\",\"backing\":7,\"element\":\"u8\",\"access\":\"read_write\",\"alignment\":1,\"byte_offset\":0,\"elements\":3}},{{\"kind\":\"buffer_view\",\"backing\":7,\"element\":\"u8\",\"access\":\"read_write\",\"alignment\":1,\"byte_offset\":1,\"elements\":3}}],\"shared_buffers\":[{{\"id\":7,\"element\":\"u8\",\"access\":\"read_write\",\"alignment\":1,\"bytes\":\"0x00010203\"}}]}}"
        );
        let prepared = prepare_request(request(&json).unwrap()).unwrap();
        assert_eq!(prepared.shared_buffers.len(), 1);
        assert_eq!(prepared.arguments.len(), 2);
        assert!(matches!(
            &prepared.arguments[0],
            SimulationArgumentV1::BufferView(view)
                if view.backing() == BufferBackingIdV1(7)
                    && view.byte_offset() == 0
                    && view.elements() == 3
        ));
        assert!(matches!(
            &prepared.arguments[1],
            SimulationArgumentV1::BufferView(view)
                if view.backing() == BufferBackingIdV1(7)
                    && view.byte_offset() == 1
                    && view.elements() == 3
        ));

        let slice = Type::slice(
            Type::Scalar(ScalarType::U8),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        );
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = Function::kernel_entry(
            "entry",
            Signature::new(vec![slice.clone(), slice], vec![]),
            vec![fe2o3_kernel_ir::ValueId(0), fe2o3_kernel_ir::ValueId(1)],
            vec![block],
        );
        let mut module = Module::new("view-test");
        module.functions.push(entry);
        module.kernels.push(Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        let admitted = AdmittedSimulationModuleV1::admit(
            VerifiedCanonicalKernelIrV7::from_module(module).unwrap(),
            cli_simulation_limits(),
        )
        .unwrap();
        admitted
            .preflight(
                &prepared,
                SimulationTargetV1::amdgpu_64(),
                cli_simulation_limits(),
            )
            .unwrap();

        let missing = json.replacen("\"backing\":7", "\"backing\":8", 1);
        let missing = prepare_request(request(&missing).unwrap()).unwrap();
        assert!(matches!(
            admitted.preflight(
                &missing,
                SimulationTargetV1::amdgpu_64(),
                cli_simulation_limits(),
            ),
            Err(SimulationPreflightErrorV1::MissingBacking {
                argument: 0,
                backing: 8
            })
        ));

        let out_of_bounds = json.replacen("\"elements\":3", "\"elements\":5", 1);
        let out_of_bounds = prepare_request(request(&out_of_bounds).unwrap()).unwrap();
        assert!(matches!(
            admitted.preflight(
                &out_of_bounds,
                SimulationTargetV1::amdgpu_64(),
                cli_simulation_limits(),
            ),
            Err(SimulationPreflightErrorV1::BufferViewBounds { argument: 0 })
        ));

        let duplicate = json.replace(
            "\"shared_buffers\":[",
            "\"shared_buffers\":[{\"id\":7,\"element\":\"u8\",\"access\":\"read_write\",\"alignment\":1,\"bytes\":\"0x00\"},",
        );
        assert_eq!(
            prepare_request(request(&duplicate).unwrap())
                .unwrap_err()
                .0
                .kind,
            ErrorKind::RequestDuplicateBacking
        );
        assert_eq!(
            preflight_kind(&SimulationPreflightErrorV1::MissingBacking {
                argument: 0,
                backing: 7,
            }),
            ErrorKind::PreflightMissingBacking
        );
        assert_eq!(
            preflight_kind(&SimulationPreflightErrorV1::BufferViewBounds { argument: 0 }),
            ErrorKind::PreflightBufferViewBounds
        );
    }

    #[test]
    fn noncanonical_hex_floats_and_bounds_fail_closed() {
        for argument in [
            "{\"kind\":\"scalar\",\"type\":\"bool\",\"bits\":\"0x2\"}",
            "{\"kind\":\"scalar\",\"type\":\"u8\",\"bits\":\"0xFf\"}",
            "{\"kind\":\"scalar\",\"type\":\"u8\",\"bits\":\"0x0\"}",
            "{\"kind\":\"scalar\",\"type\":\"f32\",\"bits\":\"0x00000000\"}",
            "{\"kind\":\"buffer\",\"element\":\"u8\",\"access\":\"read_only\",\"alignment\":1,\"bytes\":\"0x0\"}",
            "{\"kind\":\"buffer\",\"element\":\"u8\",\"access\":\"read_only\",\"alignment\":1,\"bytes\":\"0x00\",\"initialized\":\"0x80\"}",
        ] {
            let document = request(&base_request(argument)).unwrap();
            assert!(prepare_request(document).is_err(), "{argument}");
        }

        let oversized = format!("0x{}", "00".repeat(MAX_BUFFER_BYTES + 1));
        let argument = format!(
            "{{\"kind\":\"buffer\",\"element\":\"u8\",\"access\":\"read_only\",\"alignment\":1,\"bytes\":\"{oversized}\"}}"
        );
        let document = request(&base_request(&argument)).unwrap();
        assert_eq!(
            prepare_request(document).unwrap_err().0.kind,
            ErrorKind::RequestResourceLimit
        );

        let mut total = MAX_TOTAL_BUFFER_BYTES;
        assert_eq!(
            prepare_buffer(
                "u8".to_owned(),
                "read_only".to_owned(),
                1,
                "0x00".to_owned(),
                None,
                SimulationTargetV1::amdgpu_64(),
                &mut total,
            )
            .unwrap_err()
            .0
            .kind,
            ErrorKind::RequestResourceLimit
        );

        let repeated =
            "{\"kind\":\"scalar\",\"type\":\"u8\",\"bits\":\"0x00\"},".repeat(MAX_ARGUMENTS);
        let oversized_arguments = base_request(&format!(
            "{repeated}{{\"kind\":\"scalar\",\"type\":\"u8\",\"bits\":\"0x00\"}}"
        ));
        let error = request(&oversized_arguments).unwrap_err();
        assert_eq!(
            request_json_kind(&error),
            ErrorKind::RequestJsonArgumentLimit
        );
    }

    #[test]
    fn secure_inputs_reject_devices_fifos_and_symlinks() {
        let device =
            secure_read(Path::new("/dev/null"), 16, InputCode::Request, "device").unwrap_err();
        assert_eq!(device.0.kind, ErrorKind::InputNotRegular);
        assert_eq!(device.0.input, Some(InputCode::Request));

        let directory = TestDirectory::new();
        let fifo = directory.path().join("fifo");
        rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, Mode::from_raw_mode(0o600)).unwrap();
        let failure = secure_read(&fifo, 16, InputCode::Request, "fifo").unwrap_err();
        assert_eq!(failure.0.kind, ErrorKind::InputNotRegular);

        let regular = directory.path().join("regular");
        fs::write(&regular, b"data").unwrap();
        let linked = directory.path().join("linked");
        symlink(&regular, &linked).unwrap();
        assert_eq!(
            secure_read(&linked, 16, InputCode::Request, "symlink")
                .unwrap_err()
                .0
                .kind,
            ErrorKind::InputOpenFailed
        );

        let actual = directory.path().join("actual");
        fs::create_dir(&actual).unwrap();
        fs::write(actual.join("input"), b"data").unwrap();
        let parent_link = directory.path().join("parent-link");
        symlink(&actual, &parent_link).unwrap();
        assert_eq!(
            secure_read(
                &parent_link.join("input"),
                16,
                InputCode::Request,
                "parent symlink",
            )
            .unwrap_err()
            .0
            .kind,
            ErrorKind::InputOpenFailed
        );
    }

    #[test]
    fn secure_input_enforces_size_before_reading() {
        let directory = TestDirectory::new();
        let path = directory.path().join("large");
        let file = File::create(&path).unwrap();
        file.set_len(17).unwrap();
        assert_eq!(
            secure_read(&path, 16, InputCode::Request, "large")
                .unwrap_err()
                .0
                .kind,
            ErrorKind::InputTooLarge
        );

        let changed = directory.path().join("changed");
        fs::write(&changed, b"before").unwrap();
        let mutate = changed.clone();
        let failure =
            secure_read_with_hook(&changed, 16, InputCode::Request, "changed", move || {
                fs::write(mutate, b"after-is-longer").unwrap()
            })
            .unwrap_err();
        assert_eq!(failure.0.kind, ErrorKind::InputChanged);
    }

    #[test]
    fn publication_is_private_noreplace_and_cleans_exact_temp() {
        let directory = TestDirectory::new();
        let output = directory.path().join("result.json");
        publish_payload(&output, 16, |writer| writer.write_all(b"ok")).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"ok");
        assert_eq!(
            fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let existing = publish_payload(&output, 16, |writer| writer.write_all(b"new")).unwrap_err();
        assert_eq!(existing.0.kind, ErrorKind::OutputAlreadyExists);
        assert_eq!(fs::read(&output).unwrap(), b"ok");

        let failed_output = directory.path().join("failed.json");
        let failure = publish_payload(&failed_output, 16, |_writer| {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected"))
        })
        .unwrap_err();
        assert_eq!(failure.0.kind, ErrorKind::OutputWriteFailed);
        assert!(!failed_output.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);

        let substituted = directory.path().join("substituted.json");
        let attacker = substituted.clone();
        let failure = publish_payload(&substituted, 16, |writer| {
            fs::write(&attacker, b"attacker")?;
            writer.write_all(b"retained")
        })
        .unwrap_err();
        assert_eq!(failure.0.kind, ErrorKind::OutputAlreadyExists);
        assert_eq!(fs::read(&substituted).unwrap(), b"attacker");

        let final_symlink = directory.path().join("symlink.json");
        symlink(&output, &final_symlink).unwrap();
        let failure = publish_payload(&final_symlink, 16, |writer| writer.write_all(b"retained"))
            .unwrap_err();
        assert_eq!(failure.0.kind, ErrorKind::OutputAlreadyExists);
        assert_eq!(fs::read_link(&final_symlink).unwrap(), output);
    }

    #[test]
    fn publication_rejects_symlink_parent_and_bounded_writer_reports_io() {
        let directory = TestDirectory::new();
        let trailing = PathBuf::from(format!("{}/", directory.path().join("result").display()));
        assert_eq!(
            publish_payload(&trailing, 16, |writer| writer.write_all(b"ok"))
                .unwrap_err()
                .0
                .kind,
            ErrorKind::OutputInvalidPath
        );
        let actual = directory.path().join("actual");
        fs::create_dir(&actual).unwrap();
        let linked = directory.path().join("linked");
        symlink(&actual, &linked).unwrap();
        let failure = publish_payload(&linked.join("result"), 16, |writer| writer.write_all(b"ok"))
            .unwrap_err();
        assert_eq!(failure.0.kind, ErrorKind::OutputParentOpenFailed);

        let parent = File::open(&actual).unwrap();
        let anonymous = File::from(
            openat(
                &parent,
                ".",
                OFlags::RDWR | OFlags::TMPFILE | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .unwrap(),
        );
        let proc_failure = open_authenticated_proc_fd_directory(directory.path()).unwrap_err();
        assert_eq!(proc_failure.0.kind, ErrorKind::OutputProcFdUnavailable);

        let wrong_proc = actual.join("wrong-proc");
        fs::create_dir(&wrong_proc).unwrap();
        fs::write(
            wrong_proc.join(anonymous.as_raw_fd().to_string()),
            b"wrong inode",
        )
        .unwrap();
        let wrong_proc = File::open(wrong_proc).unwrap();
        let proc_failure = link_retained_anonymous(
            &anonymous,
            &parent,
            OsStr::new("never-published"),
            &wrong_proc,
        )
        .unwrap_err();
        assert_eq!(proc_failure.0.kind, ErrorKind::OutputProcFdUnavailable);
        assert!(!actual.join("never-published").exists());

        let mut bounded = BoundedWriter::new(Vec::new(), 1);
        let error = bounded.write_all(b"xx").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);

        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut broken = BoundedWriter::new(Broken, 16);
        assert_eq!(
            broken.write_all(b"x").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
    }

    #[test]
    fn error_schema_uses_closed_stage_kind_and_feature_codes() {
        let failure = Failure::new(Stage::Request, ErrorKind::RequestHexInvalid, "invalid");
        let value = serde_json::to_value(&failure.0).unwrap();
        assert_eq!(value["schema"], ERROR_SCHEMA);
        assert_eq!(value["stage"], "request");
        assert_eq!(value["kind"], "request_hex_invalid");
        assert_eq!(
            serde_json::to_value(UnsupportedFeatureCode::InlineAssembly).unwrap(),
            "inline_assembly"
        );
        assert_eq!(
            serde_json::to_value(unsupported_code(&UnsupportedFeatureV1::Gfx950LdsTranspose))
                .unwrap(),
            "gfx950_lds_transpose"
        );
        assert_eq!(
            serde_json::to_value(unsupported_code(
                &UnsupportedFeatureV1::DynamicWorkgroupMemory
            ))
            .unwrap(),
            "dynamic_workgroup_memory"
        );
        assert_eq!(
            execution_kind(&SimulationExecutionErrorKindV1::WorkgroupUseBeforePublish {
                allocation: 1,
                offset: 2,
                bytes: 3,
            }),
            ErrorKind::ExecutionWorkgroupUseBeforePublish
        );
        assert_eq!(
            execution_kind(
                &SimulationExecutionErrorKindV1::WorkgroupSchedulerNoProgress { phase: 4 }
            ),
            ErrorKind::ExecutionWorkgroupSchedulerNoProgress
        );
        for (kind, expected) in [
            (
                ErrorKind::ExecutionWorkgroupUseBeforePublish,
                "execution_workgroup_use_before_publish",
            ),
            (
                ErrorKind::ExecutionDivergentWorkgroupBarrier,
                "execution_divergent_workgroup_barrier",
            ),
            (
                ErrorKind::ExecutionMismatchedWorkgroupBarrier,
                "execution_mismatched_workgroup_barrier",
            ),
            (
                ErrorKind::ExecutionWorkgroupSchedulerNoProgress,
                "execution_workgroup_scheduler_no_progress",
            ),
        ] {
            assert_eq!(serde_json::to_value(kind).unwrap(), expected);
        }
        assert_eq!(serialized_tag(Stage::KirAdmission), "kir_admission");
        assert_eq!(
            serialized_tag(ErrorKind::InputOpenFailed),
            "input_open_failed"
        );
        assert_eq!(
            schedule_name(SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1),
            "workgroup_major_local_zyx_cooperative_v1"
        );
        assert_eq!(
            preflight_kind(&SimulationPreflightErrorV1::TargetValueOutOfRange { argument: 7 }),
            ErrorKind::PreflightTargetValueOutOfRange
        );
        assert_eq!(
            preflight_kind(&SimulationPreflightErrorV1::SharedTargetLayout(3)),
            ErrorKind::PreflightSharedTargetLayout
        );
        assert_eq!(
            admission_error_kind(&SimulationAdmissionErrorV1::ResidentBytesOverflow),
            ErrorKind::SimulatorAdmissionResidentBytesOverflow
        );
        assert_eq!(
            admission_error_kind(&SimulationAdmissionErrorV1::ResidentBytesLimit {
                phase: "test",
                actual: 2,
                limit: 1,
            }),
            ErrorKind::SimulatorAdmissionResidentBytesLimit
        );

        let failure = Failure::published(
            ErrorKind::OutputPublishFailed,
            "failed",
            PublicationState::PublishedNameUncertain,
        );
        let value = serde_json::to_value(error_document_view(&failure)).unwrap();
        assert_eq!(value["publication_state"], "published_name_uncertain");
    }

    #[test]
    fn hex_streaming_uses_bounded_chunk_count() {
        struct CountingWriter {
            calls: usize,
            bytes: usize,
        }
        impl Write for CountingWriter {
            fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
                self.calls += 1;
                self.bytes += buffer.len();
                Ok(buffer.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let input = vec![0xa5; HEX_INPUT_CHUNK_BYTES * 3 + 1];
        let mut writer = CountingWriter { calls: 0, bytes: 0 };
        write_lower_hex(&mut writer, &input, true).unwrap();
        assert_eq!(writer.calls, 5);
        assert_eq!(writer.bytes, input.len() * 2 + 2);
    }

    #[test]
    fn conflict_output_is_complete_and_machine_readable() {
        let invocation = SimulationInvocationV1 {
            global: [1, 2, 3],
            workgroup: [4, 5, 6],
            local: [7, 8, 9],
            workgroup_size: [10, 11, 12],
            workgroup_count: [13, 14, 15],
            launch_extent: [16, 17, 18],
        };
        let assessment = SimulationConflictAssessmentV1::ConflictsObserved {
            conflicting_bytes: 2,
            first: SimulationMemoryConflictV1 {
                allocation: 3,
                offset: 4,
                earlier: invocation,
                later: invocation,
                earlier_site: SimulationSiteV1 {
                    function: FunctionId::new("entry"),
                    block: BlockId(1),
                    operation: Some(2),
                },
                later_site: SimulationSiteV1 {
                    function: FunctionId::new("entry"),
                    block: BlockId(1),
                    operation: Some(3),
                },
            },
        };
        let mut bytes = Vec::new();
        write_conflict_assessment(&mut bytes, &assessment).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["status"], "conflicts_observed");
        assert_eq!(value["conflicting_bytes"], 2);
        assert_eq!(
            value["first"]["earlier"]["global"],
            serde_json::json!([1, 2, 3])
        );
        assert_eq!(value["first"]["later_site"]["operation"], 3);
    }

    #[test]
    fn unsupported_error_reports_bounded_borrowed_prefix_truthfully() {
        let sites: Vec<_> = (0..(MAX_UNSUPPORTED_FINDINGS + 10_000))
            .map(|index| UnsupportedSimulationSiteV1 {
                function: FunctionId::new(format!("function-{index:04}")),
                block: Some(BlockId(index as u32)),
                operation: Some(index as u32),
                feature: UnsupportedFeatureV1::Barrier,
            })
            .collect();
        let total = sites.len() as u64;
        let failure = Failure::unsupported(total, &sites);
        let mut bytes = BoundedWriter::new(Vec::new(), MAX_ERROR_BYTES);
        serde_json::to_writer(&mut bytes, &error_document_view(&failure)).unwrap();
        let bytes = bytes.into_inner();
        assert!(bytes.len() < MAX_ERROR_BYTES);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            value["unsupported"]["total"],
            MAX_UNSUPPORTED_FINDINGS + 10_000
        );
        assert_eq!(value["unsupported"]["emitted"], MAX_UNSUPPORTED_FINDINGS);
        assert_eq!(value["unsupported"]["truncated"], true);
        assert_eq!(
            value["unsupported"]["sites"].as_array().unwrap().len(),
            MAX_UNSUPPORTED_FINDINGS
        );
    }

    #[test]
    fn unsupported_prefix_is_bounded_by_encoded_bytes() {
        let escaped = "\u{0000}".repeat(4_096);
        let sites: Vec<_> = (0..MAX_UNSUPPORTED_FINDINGS)
            .map(|index| UnsupportedSimulationSiteV1 {
                function: FunctionId::new(escaped.clone()),
                block: Some(BlockId(index as u32)),
                operation: Some(index as u32),
                feature: UnsupportedFeatureV1::ExternalCall(FunctionId::new(escaped.clone())),
            })
            .collect();
        let failure = Failure::unsupported((MAX_UNSUPPORTED_FINDINGS as u64) + 10_000, &sites);
        let mut bytes = BoundedWriter::new(Vec::new(), MAX_ERROR_BYTES);
        serde_json::to_writer(&mut bytes, &error_document_view(&failure)).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes.into_inner()).unwrap();
        let emitted = value["unsupported"]["emitted"].as_u64().unwrap() as usize;
        assert!(emitted > 0);
        assert!(emitted < MAX_UNSUPPORTED_FINDINGS);
        assert_eq!(value["unsupported"]["truncated"], true);
        assert_eq!(
            value["unsupported"]["sites"].as_array().unwrap().len(),
            emitted
        );
        assert_eq!(value["unsupported"]["sites"][0]["feature"], "external_call");
    }

    #[test]
    fn post_link_failure_never_unlinks_a_mutable_target_name() {
        let directory = TestDirectory::new();
        let source = directory.path().join("source");
        let target = directory.path().join("target");
        fs::write(&source, b"published").unwrap();
        fs::hard_link(&source, &target).unwrap();
        let failure = Failure::published(
            ErrorKind::OutputPublishFailed,
            "injected post-link failure",
            PublicationState::PublishedNameUncertain,
        );
        assert_eq!(
            failure.0.publication_state,
            Some(PublicationState::PublishedNameUncertain)
        );
        assert_eq!(fs::read(&target).unwrap(), b"published");

        fs::remove_file(&target).unwrap();
        fs::write(&target, b"replacement").unwrap();
        let failure = Failure::published(
            ErrorKind::OutputPublishFailed,
            "injected substitution",
            PublicationState::PublishedNameUncertain,
        );
        assert_eq!(
            failure.0.publication_state,
            Some(PublicationState::PublishedNameUncertain)
        );
        assert_eq!(fs::read(target).unwrap(), b"replacement");
    }

    #[test]
    fn dynamic_error_keeps_typed_context_with_long_control_character_identity() {
        let function = "\u{0000}".repeat(MAX_ERROR_FUNCTION_BYTES + 1);
        let function_bytes = function.len();
        let error = SimulationExecutionErrorV1 {
            invocation: Some(SimulationInvocationV1 {
                global: [1, 2, 3],
                workgroup: [4, 5, 6],
                local: [7, 8, 9],
                workgroup_size: [10, 11, 12],
                workgroup_count: [13, 14, 15],
                launch_extent: [16, 17, 18],
            }),
            site: Some(SimulationSiteV1 {
                function: FunctionId::new(function),
                block: BlockId(2),
                operation: Some(3),
            }),
            kind: SimulationExecutionErrorKindV1::ReachedUnreachable,
            observation_failure: None,
        };
        let failure = Failure::execution(error);
        let mut bytes = BoundedWriter::new(FallibleVecWriter::default(), MAX_ERROR_BYTES);
        serde_json::to_writer(&mut bytes, &error_document_view(&failure)).unwrap();
        let bytes = bytes.into_inner().0;
        assert!(bytes.len() < MAX_ERROR_BYTES);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["stage"], "execution");
        assert_eq!(value["kind"], "execution_reached_unreachable");
        assert_eq!(value["invocation"]["global"], serde_json::json!([1, 2, 3]));
        assert_eq!(value["site"]["block"], 2);
        assert_eq!(value["site"]["operation"], 3);
        assert_eq!(value["site"]["function_bytes"], function_bytes);
        assert_eq!(value["site"]["function_truncated"], true);
        assert_eq!(
            value["site"]["function"].as_str().unwrap().len(),
            MAX_ERROR_FUNCTION_BYTES
        );
    }

    #[test]
    fn cli_limits_are_an_immutable_bounded_host_profile() {
        let limits = cli_simulation_limits();
        assert_eq!(limits.max_canonical_bytes, MAX_KIR_BYTES);
        assert_eq!(limits.max_allocation_bytes, MAX_TOTAL_BUFFER_BYTES);
        assert_eq!(limits.max_total_bytes, 64 * 1024 * 1024);
        assert_eq!(limits.max_resident_bytes, 256 * 1024 * 1024);
        assert_eq!(limits.max_call_depth, 64);
        assert_eq!(limits.max_ssa_values, 4_096);
        assert!(limits.max_steps < SimulationLimitsV1::default().max_steps);
        assert!(limits.max_allocations < SimulationLimitsV1::default().max_allocations);
        limits.validate().unwrap();
    }

    #[test]
    fn cli_limit_exhaustion_is_enforced_end_to_end() {
        fn admitted(loop_forever: bool) -> AdmittedSimulationModuleV1 {
            let mut block = BasicBlock::new(BlockId(0));
            block.terminator = Some(if loop_forever {
                Terminator::Branch {
                    target: BlockId(0),
                    arguments: vec![],
                }
            } else {
                Terminator::Return { values: vec![] }
            });
            let entry = Function::kernel_entry(
                "entry",
                Signature::new(vec![], vec![]),
                vec![],
                vec![block],
            );
            let mut module = Module::new("cli-limit-test");
            module.functions.push(entry);
            module.kernels.push(Kernel::new(
                "kernel",
                "entry",
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            ));
            let canonical = VerifiedCanonicalKernelIrV7::from_module(module).unwrap();
            AdmittedSimulationModuleV1::admit(canonical, cli_simulation_limits()).unwrap()
        }

        let limits = cli_simulation_limits();
        let request = SimulationRequestV1::new(
            "kernel",
            [limits.max_invocations + 1, 1, 1],
            [1, 1, 1],
            vec![],
        );
        let error = admitted(false)
            .simulate(&request, SimulationTargetV1::amdgpu_64(), limits)
            .unwrap_err();
        assert!(matches!(
            error,
            SimulationErrorV1::Preflight(SimulationPreflightErrorV1::ResourceLimit { .. })
        ));

        let mut constrained = limits;
        constrained.max_steps = 4;
        let request = SimulationRequestV1::new("kernel", [1, 1, 1], [1, 1, 1], vec![]);
        let loop_kernel = admitted(true);
        loop_kernel
            .preflight(&request, SimulationTargetV1::amdgpu_64(), limits)
            .expect("minimal request fits the coherent CLI profile");
        let error = loop_kernel
            .simulate(&request, SimulationTargetV1::amdgpu_64(), constrained)
            .unwrap_err();
        assert!(
            matches!(
                error,
                SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::StepLimit { limit: 4 },
                    ..
                })
            ),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn dependency_closure_excludes_gpu_runtime_packages() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let metadata = Command::new(env!("CARGO"))
            .args([
                "metadata",
                "--locked",
                "--format-version=1",
                "--manifest-path",
            ])
            .arg(&manifest)
            .output()
            .unwrap();
        assert!(
            metadata.status.success(),
            "{}",
            String::from_utf8_lossy(&metadata.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
        let packages = value["packages"].as_array().unwrap();
        let names: BTreeMap<&str, &str> = packages
            .iter()
            .map(|package| {
                (
                    package["id"].as_str().unwrap(),
                    package["name"].as_str().unwrap(),
                )
            })
            .collect();
        let root = packages
            .iter()
            .find(|package| package["name"] == "fe2o3-kir-sim-cli")
            .unwrap()["id"]
            .as_str()
            .unwrap();
        let nodes = value["resolve"]["nodes"].as_array().unwrap();
        let dependencies: BTreeMap<&str, Vec<&str>> = nodes
            .iter()
            .map(|node| {
                (
                    node["id"].as_str().unwrap(),
                    node["dependencies"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|dependency| dependency.as_str().unwrap())
                        .collect(),
                )
            })
            .collect();
        let mut pending = vec![root];
        let mut closure = BTreeSet::new();
        while let Some(package) = pending.pop() {
            if closure.insert(package) {
                pending.extend(dependencies.get(package).into_iter().flatten().copied());
            }
        }
        let forbidden = [
            "fe2o3-aql",
            "fe2o3-amdhsa-loader",
            "fe2o3-hip-sys",
            "fe2o3-hsa-runtime",
            "fe2o3-kfd",
            "fe2o3-kfd-uapi",
        ];
        for package in closure {
            assert!(
                !forbidden.contains(names.get(package).unwrap()),
                "GPU runtime package {} entered the standalone CLI closure",
                names[package]
            );
        }
    }
}
