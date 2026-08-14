//! Lossless classification of rustc command-line invocations.
//!
//! This module separates source-free terminal and query invocations from
//! compilations before extracting compile metadata. Every successful
//! classification borrows the caller's complete argument vector, including
//! `argv[0]`; no argument is normalized, reordered, decoded, or reconstructed.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::Path;

/// Rustc options that consume the following argument when written separately.
///
/// The classifier uses this explicit table to avoid mistaking an option value
/// ending in `.rs` for a source input. Joined forms such as `--target=...` and
/// `-Copt-level=3` do not consume the next argument. This V2 table is frozen;
/// incompatible grammar changes require a new classifier version.
pub const RUSTC_SEPARATE_VALUE_OPTIONS_V2: &[&str] = &[
    "-A",
    "-C",
    "-D",
    "-F",
    "-L",
    "-W",
    "-Z",
    "-l",
    "-o",
    "--allow",
    "--cap-lints",
    "--cfg",
    "--check-cfg",
    "--codegen",
    "--color",
    "--crate-name",
    "--crate-type",
    "--deny",
    "--diagnostic-width",
    "--edition",
    "--emit",
    "--error-format",
    "--explain",
    "--extern",
    "--forbid",
    "--force-warn",
    "--json",
    "--lint",
    "--out-dir",
    "--output-format",
    "--pretty",
    "--print",
    "--remap-path-prefix",
    "--remap-path-scope",
    "--sysroot",
    "--target",
    "--unpretty",
    "--warn",
];

const CRATE_NAME: &str = "--crate-name";
const CRATE_NAME_JOINED_PREFIX: &[u8] = b"--crate-name=";
const PRINT_JOINED_PREFIX: &[u8] = b"--print=";
const EXPLAIN_JOINED_PREFIX: &[u8] = b"--explain=";
const PASSTHROUGH_PRINT_KINDS_V2: &[&[u8]] = &[
    b"cfg",
    b"crate-name",
    b"file-names",
    b"split-debuginfo",
    b"sysroot",
];

/// Reports whether one rustc argument begins a codegen-backend selector.
///
/// This recognizes the joined and split hyphen/underscore spellings accepted
/// by rustc's `-Z` option grammar, plus the fail-closed `-Z=...` spelling used
/// by the wrappers' rejection policy. It is a syntax recognizer, not an
/// authority or an assertion that rustc will accept the complete invocation.
#[must_use]
pub fn is_rustc_codegen_backend_selector_v2(argument: &OsStr, following: Option<&OsStr>) -> bool {
    let bytes = argument.as_encoded_bytes();
    if bytes == b"-Z" {
        return following.is_some_and(is_rustc_codegen_backend_option_value_v2);
    }

    bytes.strip_prefix(b"-Z").is_some_and(|value| {
        is_rustc_codegen_backend_option_value_bytes_v2(value.strip_prefix(b"=").unwrap_or(value))
    })
}

/// Reports whether one value names rustc's codegen-backend unstable option.
///
/// Both rustc spellings are recognized, with or without an assigned value.
/// Callers use this together with [`is_rustc_codegen_backend_selector_v2`] to
/// reject ambiguous or duplicate backend selection without decoding paths.
#[must_use]
pub fn is_rustc_codegen_backend_option_value_v2(value: &OsStr) -> bool {
    is_rustc_codegen_backend_option_value_bytes_v2(value.as_encoded_bytes())
}

/// Reports whether one argument is rustc's option terminator.
///
/// Rustc treats every following token as an input path, including tokens that
/// otherwise spell options. A wrapper that appends managed options must reject
/// this token instead of recording those options as semantically effective.
#[must_use]
pub fn is_rustc_option_terminator_v2(argument: &OsStr) -> bool {
    argument == "--"
}

fn is_rustc_codegen_backend_option_value_bytes_v2(value: &[u8]) -> bool {
    [b"codegen-backend".as_slice(), b"codegen_backend".as_slice()]
        .iter()
        .any(|name| {
            value == *name
                || value
                    .strip_prefix(*name)
                    .is_some_and(|rest| rest.starts_with(b"="))
        })
}

/// The lossless classification of one rustc argument vector.
///
/// Terminal and query invocations are passthrough forms. Their arguments may
/// contain arbitrary platform-native, non-UTF-8 data. A compile invocation has
/// exactly one crate name and one `.rs` source input, both valid UTF-8.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RustcInvocationV2<'a> {
    /// An invocation that asks rustc to print terminal information and exit.
    Terminal(RustcPassthroughInvocationV2<'a>),
    /// An invocation that queries rustc rather than compiling one source.
    Query(RustcPassthroughInvocationV2<'a>),
    /// An invocation that compiles exactly one Rust source input.
    Compile(RustcCompileInvocationV2<'a>),
}

impl<'a> RustcInvocationV2<'a> {
    /// Returns the complete, unchanged argument vector including `argv[0]`.
    #[must_use]
    pub fn argv(self) -> &'a [OsString] {
        match self {
            Self::Terminal(invocation) | Self::Query(invocation) => invocation.argv(),
            Self::Compile(invocation) => invocation.argv(),
        }
    }

    /// Returns the rustc executable from `argv[0]` without decoding it.
    #[must_use]
    pub fn executable(self) -> &'a OsStr {
        match self {
            Self::Terminal(invocation) | Self::Query(invocation) => invocation.executable(),
            Self::Compile(invocation) => invocation.executable(),
        }
    }

    /// Returns all arguments after `argv[0]` without modifying them.
    #[must_use]
    pub fn forwarded_args(self) -> &'a [OsString] {
        match self {
            Self::Terminal(invocation) | Self::Query(invocation) => invocation.forwarded_args(),
            Self::Compile(invocation) => invocation.forwarded_args(),
        }
    }

    /// Returns whether this invocation belongs to the minimal bootstrap
    /// passthrough grammar used before pinned compiler execution is available.
    ///
    /// This is only a syntax gate. It does not authenticate the executable or
    /// process environment and must not be treated as an artifact-authority
    /// decision.
    #[must_use]
    pub fn is_bootstrap_passthrough_approved(self) -> bool {
        match self {
            Self::Terminal(invocation) => bootstrap_terminal_v2(invocation.argv()),
            Self::Query(invocation) => bootstrap_query_v2(invocation.argv()),
            Self::Compile(_) => false,
        }
    }
}

/// A terminal or query invocation whose arguments must remain unchanged.
///
/// This type deliberately exposes no compile metadata. It can therefore retain
/// arbitrary non-UTF-8 arguments without a lossy conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustcPassthroughInvocationV2<'a> {
    argv: &'a [OsString],
}

impl<'a> RustcPassthroughInvocationV2<'a> {
    /// Returns the complete, unchanged argument vector including `argv[0]`.
    #[must_use]
    pub fn argv(self) -> &'a [OsString] {
        self.argv
    }

    /// Returns the rustc executable from `argv[0]` without decoding it.
    #[must_use]
    pub fn executable(self) -> &'a OsStr {
        &self.argv[0]
    }

    /// Returns all arguments after `argv[0]` without modifying them.
    #[must_use]
    pub fn forwarded_args(self) -> &'a [OsString] {
        &self.argv[1..]
    }
}

/// A validated, lossless rustc compile invocation.
///
/// The complete argument vector remains the source of truth. The crate name and
/// source path are borrowed views into that vector and are never substituted
/// back into it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustcCompileInvocationV2<'a> {
    argv: &'a [OsString],
    crate_name: &'a str,
    crate_name_argument_index: usize,
    source_path: &'a Path,
    source_argument_index: usize,
}

impl<'a> RustcCompileInvocationV2<'a> {
    /// Returns the complete, unchanged argument vector including `argv[0]`.
    #[must_use]
    pub fn argv(self) -> &'a [OsString] {
        self.argv
    }

    /// Returns the rustc executable from `argv[0]` without decoding it.
    #[must_use]
    pub fn executable(self) -> &'a OsStr {
        &self.argv[0]
    }

    /// Returns all arguments after `argv[0]` without modifying them.
    #[must_use]
    pub fn forwarded_args(self) -> &'a [OsString] {
        &self.argv[1..]
    }

    /// Returns the UTF-8 crate name selected by `--crate-name`.
    #[must_use]
    pub fn crate_name(self) -> &'a str {
        self.crate_name
    }

    /// Returns the index of the argv entry containing the crate-name value.
    ///
    /// For `--crate-name value`, this is the index of `value`. For
    /// `--crate-name=value`, this is the index of the joined option.
    #[must_use]
    pub fn crate_name_argument_index(self) -> usize {
        self.crate_name_argument_index
    }

    /// Returns the single UTF-8 `.rs` source path.
    #[must_use]
    pub fn source_path(self) -> &'a Path {
        self.source_path
    }

    /// Returns the index of the argv entry containing the source path.
    #[must_use]
    pub fn source_argument_index(self) -> usize {
        self.source_argument_index
    }
}

/// An error found while structurally classifying rustc arguments.
///
/// Argument indexes refer to the original input slice, including `argv[0]`, so
/// callers can report the exact platform-native argument without this error
/// type copying or lossily decoding it.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RustcArgsErrorV2 {
    /// The argument vector did not contain `argv[0]`.
    MissingExecutable,
    /// The `argv[0]` entry was empty.
    EmptyExecutable,
    /// A response-file argument was present.
    ResponseFile {
        /// Index of the argument beginning with `@`.
        argument_index: usize,
    },
    /// A known separate-value option had no nonempty following value.
    MissingOptionValue {
        /// The option whose value was missing.
        option: &'static str,
        /// Index of the option in the original argument vector.
        option_index: usize,
    },
    /// A compile crate name was not valid UTF-8.
    NonUtf8CrateName {
        /// Index of the argv entry containing the crate-name value.
        argument_index: usize,
    },
    /// The same compile crate name was specified more than once.
    DuplicateCrateName {
        /// Index of the first argv entry containing the crate-name value.
        first_argument_index: usize,
        /// Index of the duplicate argv entry containing the crate-name value.
        duplicate_argument_index: usize,
    },
    /// Two different compile crate names were specified.
    ConflictingCrateNames {
        /// Index of the first argv entry containing a crate-name value.
        first_argument_index: usize,
        /// Index of the conflicting argv entry containing a crate-name value.
        conflicting_argument_index: usize,
    },
    /// A potential compile source input was not valid UTF-8.
    NonUtf8SourceInput {
        /// Index of the non-UTF-8 positional argument.
        argument_index: usize,
    },
    /// A positional compile argument was not an `.rs` source path.
    AmbiguousSourceInput {
        /// Index of the ambiguous positional argument.
        argument_index: usize,
    },
    /// More than one `.rs` compile source input was present.
    MultipleSourceInputs {
        /// Index of the first `.rs` source input.
        first_argument_index: usize,
        /// Index of the additional `.rs` source input.
        additional_argument_index: usize,
    },
    /// Exactly one of the crate name and source input was present.
    PartialCompile {
        /// Index of the crate-name value, when one was present.
        crate_name_argument_index: Option<usize>,
        /// Index of the source path, when one was present.
        source_argument_index: Option<usize>,
    },
}

impl fmt::Display for RustcArgsErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingExecutable => formatter.write_str("rustc argv is missing argv[0]"),
            Self::EmptyExecutable => formatter.write_str("rustc argv[0] is empty"),
            Self::ResponseFile { argument_index } => write!(
                formatter,
                "rustc response-file argument at argv[{argument_index}] is not supported"
            ),
            Self::MissingOptionValue {
                option,
                option_index,
            } => write!(
                formatter,
                "rustc option `{option}` at argv[{option_index}] is missing its value"
            ),
            Self::NonUtf8CrateName { argument_index } => write!(
                formatter,
                "rustc crate name at argv[{argument_index}] is not valid UTF-8"
            ),
            Self::DuplicateCrateName {
                first_argument_index,
                duplicate_argument_index,
            } => write!(
                formatter,
                "rustc crate name at argv[{duplicate_argument_index}] duplicates argv[{first_argument_index}]"
            ),
            Self::ConflictingCrateNames {
                first_argument_index,
                conflicting_argument_index,
            } => write!(
                formatter,
                "rustc crate name at argv[{conflicting_argument_index}] conflicts with argv[{first_argument_index}]"
            ),
            Self::NonUtf8SourceInput { argument_index } => write!(
                formatter,
                "rustc source input at argv[{argument_index}] is not valid UTF-8"
            ),
            Self::AmbiguousSourceInput { argument_index } => write!(
                formatter,
                "positional rustc argument at argv[{argument_index}] is not an `.rs` source"
            ),
            Self::MultipleSourceInputs {
                first_argument_index,
                additional_argument_index,
            } => write!(
                formatter,
                "rustc source at argv[{additional_argument_index}] is additional to argv[{first_argument_index}]"
            ),
            Self::PartialCompile {
                crate_name_argument_index,
                source_argument_index,
            } => write!(
                formatter,
                "partial rustc compile invocation (crate name: {crate_name_argument_index:?}, source: {source_argument_index:?})"
            ),
        }
    }
}

impl Error for RustcArgsErrorV2 {}

/// Classifies a complete rustc argument vector without changing any argument.
///
/// The input must include the rustc executable as `argv[0]`. Source-free
/// terminal selectors such as `--version` take precedence over compile metadata
/// and compile-only validation. A fixed set of source-free print kinds also
/// permits Cargo probes such as
/// `rustc - --crate-name ___ --print=file-names ...` to pass through unchanged.
///
/// A call with no terminal/query selector and no compile metadata is treated as
/// an opaque query. A call with only one of a crate name and source is rejected
/// as a partial compile. The classifier intentionally does not require an
/// edition, crate type, codegen backend, or any fe2o3-specific option; a schema
/// validator may impose those constraints after classification.
///
/// # Errors
///
/// Returns [`RustcArgsErrorV2`] for a missing executable or a response file, and
/// for missing option values and malformed metadata in a compile candidate.
pub fn classify_rustc_invocation_v2(
    argv: &[OsString],
) -> Result<RustcInvocationV2<'_>, RustcArgsErrorV2> {
    let executable = argv.first().ok_or(RustcArgsErrorV2::MissingExecutable)?;
    if executable.is_empty() {
        return Err(RustcArgsErrorV2::EmptyExecutable);
    }
    reject_response_files(argv)?;

    let invocation = RustcPassthroughInvocationV2 { argv };
    match detect_passthrough_form(argv) {
        Some(PassthroughForm::Terminal) => return Ok(RustcInvocationV2::Terminal(invocation)),
        Some(PassthroughForm::Query) => return Ok(RustcInvocationV2::Query(invocation)),
        None => {}
    }

    let passthrough = classify_passthrough_form(argv)?;
    match passthrough {
        Some(PassthroughForm::Terminal) => return Ok(RustcInvocationV2::Terminal(invocation)),
        Some(PassthroughForm::Query) => return Ok(RustcInvocationV2::Query(invocation)),
        None => {}
    }

    classify_compile_form(argv)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PassthroughForm {
    Terminal,
    Query,
}

fn detect_passthrough_form(argv: &[OsString]) -> Option<PassthroughForm> {
    let mut saw_query = false;
    let mut saw_source_input = false;
    let mut options = true;
    let mut index = 1;
    while index < argv.len() {
        let argument = &argv[index];
        if options && argument == OsStr::new("--") {
            options = false;
            index += 1;
            continue;
        }

        if options
            && (is_terminal_selector(argument)
                || separate_value_option(argument).is_some_and(|option| {
                    argv.get(index + 1)
                        .is_some_and(|value| is_terminal_pair(option, value))
                }))
        {
            return Some(PassthroughForm::Terminal);
        }

        if options {
            saw_query |= is_passthrough_query_selector(argument, argv.get(index + 1));
            if separate_value_option(argument).is_some() && index + 1 < argv.len() {
                index += 2;
                continue;
            }
            if is_option(argument) {
                index += 1;
                continue;
            }
        }

        saw_source_input = true;
        index += 1;
    }
    (saw_query && (!saw_source_input || is_canonical_cargo_probe_v2(argv)))
        .then_some(PassthroughForm::Query)
}

fn reject_response_files(argv: &[OsString]) -> Result<(), RustcArgsErrorV2> {
    if let Some(argument_index) = argv
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, argument)| is_response_file(argument).then_some(index))
    {
        return Err(RustcArgsErrorV2::ResponseFile { argument_index });
    }
    Ok(())
}

fn classify_passthrough_form(
    argv: &[OsString],
) -> Result<Option<PassthroughForm>, RustcArgsErrorV2> {
    let mut saw_terminal = false;
    let mut saw_query = false;
    let mut saw_source_input = false;
    let mut options = true;
    let mut index = 1;

    while index < argv.len() {
        let argument = &argv[index];
        if options && argument == OsStr::new("--") {
            options = false;
            index += 1;
            continue;
        }
        if !options {
            saw_source_input = true;
            index += 1;
            continue;
        }

        if let Some(option) = empty_joined_value_option(argument) {
            return Err(RustcArgsErrorV2::MissingOptionValue {
                option,
                option_index: index,
            });
        }
        if is_terminal_selector(argument) {
            saw_terminal = true;
        }
        saw_query |= is_passthrough_query_selector(argument, argv.get(index + 1));

        if let Some(option) = separate_value_option(argument) {
            let value_index = index + 1;
            let value = argv
                .get(value_index)
                .filter(|value| !value.is_empty())
                .ok_or(RustcArgsErrorV2::MissingOptionValue {
                    option,
                    option_index: index,
                })?;
            if is_terminal_pair(option, value) {
                saw_terminal = true;
            }
            index += 2;
        } else if is_option(argument) {
            index += 1;
        } else {
            saw_source_input = true;
            index += 1;
        }
    }

    Ok(if saw_terminal {
        Some(PassthroughForm::Terminal)
    } else if saw_query && (!saw_source_input || is_canonical_cargo_probe_v2(argv)) {
        Some(PassthroughForm::Query)
    } else {
        None
    })
}

fn classify_compile_form(argv: &[OsString]) -> Result<RustcInvocationV2<'_>, RustcArgsErrorV2> {
    let mut crate_name: Option<(&str, usize)> = None;
    let mut source_path: Option<(&Path, usize)> = None;
    let mut options = true;
    let mut index = 1;

    while index < argv.len() {
        let argument = &argv[index];
        if options && argument == OsStr::new("--") {
            options = false;
            index += 1;
            continue;
        }

        if options {
            if argument == OsStr::new(CRATE_NAME) {
                let value_index = index + 1;
                let value = &argv[value_index];
                record_crate_name(&mut crate_name, value, value_index)?;
                index += 2;
                continue;
            }
            if has_encoded_prefix(argument, CRATE_NAME_JOINED_PREFIX) {
                let value = argument
                    .to_str()
                    .ok_or(RustcArgsErrorV2::NonUtf8CrateName {
                        argument_index: index,
                    })?
                    .strip_prefix("--crate-name=")
                    .expect("the encoded crate-name prefix was checked");
                record_crate_name_str(&mut crate_name, value, index)?;
                index += 1;
                continue;
            }
            if separate_value_option(argument).is_some() {
                index += 2;
                continue;
            }
            if is_option(argument) {
                index += 1;
                continue;
            }
        }

        record_source_path(&mut source_path, argument, index)?;
        index += 1;
    }

    match (crate_name, source_path) {
        (
            Some((crate_name, crate_name_argument_index)),
            Some((source_path, source_argument_index)),
        ) => Ok(RustcInvocationV2::Compile(RustcCompileInvocationV2 {
            argv,
            crate_name,
            crate_name_argument_index,
            source_path,
            source_argument_index,
        })),
        (None, None) => Ok(RustcInvocationV2::Query(RustcPassthroughInvocationV2 {
            argv,
        })),
        (crate_name, source_path) => Err(RustcArgsErrorV2::PartialCompile {
            crate_name_argument_index: crate_name.map(|(_, index)| index),
            source_argument_index: source_path.map(|(_, index)| index),
        }),
    }
}

fn record_crate_name<'a>(
    slot: &mut Option<(&'a str, usize)>,
    value: &'a OsStr,
    argument_index: usize,
) -> Result<(), RustcArgsErrorV2> {
    let value = value
        .to_str()
        .ok_or(RustcArgsErrorV2::NonUtf8CrateName { argument_index })?;
    record_crate_name_str(slot, value, argument_index)
}

fn record_crate_name_str<'a>(
    slot: &mut Option<(&'a str, usize)>,
    value: &'a str,
    argument_index: usize,
) -> Result<(), RustcArgsErrorV2> {
    if let Some((first, first_argument_index)) = *slot {
        if first == value {
            return Err(RustcArgsErrorV2::DuplicateCrateName {
                first_argument_index,
                duplicate_argument_index: argument_index,
            });
        }
        return Err(RustcArgsErrorV2::ConflictingCrateNames {
            first_argument_index,
            conflicting_argument_index: argument_index,
        });
    }
    *slot = Some((value, argument_index));
    Ok(())
}

fn record_source_path<'a>(
    slot: &mut Option<(&'a Path, usize)>,
    argument: &'a OsStr,
    argument_index: usize,
) -> Result<(), RustcArgsErrorV2> {
    argument
        .to_str()
        .ok_or(RustcArgsErrorV2::NonUtf8SourceInput { argument_index })?;
    let path = Path::new(argument);
    if path.extension() != Some(OsStr::new("rs")) {
        return Err(RustcArgsErrorV2::AmbiguousSourceInput { argument_index });
    }
    if let Some((_, first_argument_index)) = *slot {
        return Err(RustcArgsErrorV2::MultipleSourceInputs {
            first_argument_index,
            additional_argument_index: argument_index,
        });
    }
    *slot = Some((path, argument_index));
    Ok(())
}

fn separate_value_option(argument: &OsStr) -> Option<&'static str> {
    RUSTC_SEPARATE_VALUE_OPTIONS_V2
        .iter()
        .copied()
        .find(|option| argument == OsStr::new(option))
}

fn empty_joined_value_option(argument: &OsStr) -> Option<&'static str> {
    let bytes = argument.as_encoded_bytes();
    RUSTC_SEPARATE_VALUE_OPTIONS_V2
        .iter()
        .copied()
        .filter(|option| option.starts_with("--"))
        .find(|option| {
            bytes.len() == option.len() + 1
                && bytes.starts_with(option.as_bytes())
                && bytes.last() == Some(&b'=')
        })
}

fn bootstrap_terminal_v2(argv: &[OsString]) -> bool {
    if argv.len() == 2 {
        let selector = &argv[1];
        return matches_os(
            selector,
            &[
                "-h",
                "--help",
                "-V",
                "--version",
                "-vV",
                "-Vv",
                "--explain",
                "-Chelp",
                "-Whelp",
                "-Zhelp",
            ],
        ) || has_encoded_prefix(selector, EXPLAIN_JOINED_PREFIX);
    }
    argv.len() == 3
        && ((argv[1] == OsStr::new("--explain") && !argv[2].is_empty())
            || (matches_os(&argv[1], &["-C", "-W", "-Z"]) && argv[2] == OsStr::new("help")))
}

fn bootstrap_query_v2(argv: &[OsString]) -> bool {
    simple_print_query_v2(argv) || is_canonical_cargo_probe_v2(argv)
}

fn simple_print_query_v2(argv: &[OsString]) -> bool {
    let mut saw_print = false;
    let mut index = 1;
    while index < argv.len() {
        let argument = &argv[index];
        if let Some(kind) = argument
            .as_encoded_bytes()
            .strip_prefix(PRINT_JOINED_PREFIX)
        {
            if !kind.is_empty() && !is_passthrough_print_kind(kind) {
                return false;
            }
            saw_print = true;
            index += 1;
            continue;
        }
        if argument == OsStr::new("--print") {
            saw_print = true;
            let Some(kind) = argv.get(index + 1) else {
                index += 1;
                continue;
            };
            if !kind.is_empty() && !is_passthrough_print_kind(kind.as_encoded_bytes()) {
                return false;
            }
            index += 2;
            continue;
        }
        return false;
    }
    saw_print
}

fn is_terminal_selector(argument: &OsStr) -> bool {
    matches_os(argument, &["-h", "--help", "-V", "--version", "-vV", "-Vv"])
        || has_encoded_prefix(argument, EXPLAIN_JOINED_PREFIX)
        || argument == OsStr::new("--explain")
        || matches_os(argument, &["-Chelp", "-Whelp", "-Zhelp"])
}

fn is_passthrough_query_selector(argument: &OsStr, next: Option<&OsString>) -> bool {
    if argument == OsStr::new("--print") {
        return next.is_none_or(|value| {
            value.is_empty() || is_passthrough_print_kind(value.as_encoded_bytes())
        });
    }
    argument
        .as_encoded_bytes()
        .strip_prefix(PRINT_JOINED_PREFIX)
        .is_some_and(|value| value.is_empty() || is_passthrough_print_kind(value))
}

fn is_passthrough_print_kind(value: &[u8]) -> bool {
    PASSTHROUGH_PRINT_KINDS_V2.contains(&value)
}

fn is_canonical_cargo_probe_v2(argv: &[OsString]) -> bool {
    if argv
        .get(1)
        .is_none_or(|argument| argument != OsStr::new("-"))
        || argv
            .get(2)
            .is_none_or(|argument| argument != OsStr::new("--crate-name"))
        || argv
            .get(3)
            .is_none_or(|argument| argument != OsStr::new("___"))
    {
        return false;
    }

    let mut saw_file_names = false;
    let mut index = 4;
    while index < argv.len() {
        let argument = &argv[index];
        if let Some(kind) = argument
            .as_encoded_bytes()
            .strip_prefix(PRINT_JOINED_PREFIX)
        {
            if !is_passthrough_print_kind(kind) {
                return false;
            }
            saw_file_names |= kind == b"file-names";
            index += 1;
            continue;
        }
        if argument == OsStr::new("--print") {
            let Some(kind) = argv.get(index + 1).map(|value| value.as_encoded_bytes()) else {
                return false;
            };
            if !is_passthrough_print_kind(kind) {
                return false;
            }
            saw_file_names |= kind == b"file-names";
            index += 2;
            continue;
        }
        if let Some(crate_type) = argument.as_encoded_bytes().strip_prefix(b"--crate-type=") {
            if !is_cargo_probe_crate_type(crate_type) {
                return false;
            }
            index += 1;
            continue;
        }
        if argument == OsStr::new("--crate-type") {
            if argv
                .get(index + 1)
                .is_none_or(|value| !is_cargo_probe_crate_type(value.as_encoded_bytes()))
            {
                return false;
            }
            index += 2;
            continue;
        }
        return false;
    }
    saw_file_names
}

fn is_cargo_probe_crate_type(value: &[u8]) -> bool {
    [
        b"bin".as_slice(),
        b"rlib",
        b"dylib",
        b"cdylib",
        b"staticlib",
        b"proc-macro",
    ]
    .contains(&value)
}

fn is_terminal_pair(option: &str, value: &OsStr) -> bool {
    matches!(option, "-C" | "-W" | "-Z") && value == OsStr::new("help")
}

fn matches_os(argument: &OsStr, choices: &[&str]) -> bool {
    choices.iter().any(|choice| argument == OsStr::new(choice))
}

fn is_option(argument: &OsStr) -> bool {
    let bytes = argument.as_encoded_bytes();
    bytes.len() > 1 && bytes.first() == Some(&b'-')
}

fn is_response_file(argument: &OsStr) -> bool {
    argument.as_encoded_bytes().first() == Some(&b'@')
}

fn has_encoded_prefix(argument: &OsStr, prefix: &[u8]) -> bool {
    argument.as_encoded_bytes().starts_with(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn codegen_backend_selector_grammar_covers_rustc_spellings() {
        for (argument, following) in [
            ("-Zcodegen-backend=/backend.so", None),
            ("-Zcodegen_backend=/backend.so", None),
            ("-Z=codegen-backend=/backend.so", None),
            ("-Z=codegen_backend=/backend.so", None),
            ("-Zcodegen-backend", None),
            ("-Zcodegen_backend", None),
            ("-Z", Some("codegen-backend=/backend.so")),
            ("-Z", Some("codegen_backend=/backend.so")),
            ("-Z", Some("codegen-backend")),
            ("-Z", Some("codegen_backend")),
        ] {
            assert!(
                is_rustc_codegen_backend_selector_v2(
                    OsStr::new(argument),
                    following.map(OsStr::new),
                ),
                "missed {argument:?} {following:?}"
            );
        }

        for (argument, following) in [
            ("-Ccodegen-units=1", None),
            ("-Zunstable-options", None),
            ("-Z", Some("mir-opt-level=2")),
            ("codegen-backend=/backend.so", None),
            ("--codegen-backend=/backend.so", None),
        ] {
            assert!(
                !is_rustc_codegen_backend_selector_v2(
                    OsStr::new(argument),
                    following.map(OsStr::new),
                ),
                "misclassified {argument:?} {following:?}"
            );
        }
    }

    fn expect_terminal(argv: &[OsString]) -> RustcPassthroughInvocationV2<'_> {
        match classify_rustc_invocation_v2(argv).expect("terminal invocation should classify") {
            RustcInvocationV2::Terminal(invocation) => invocation,
            invocation => panic!("expected terminal invocation, got {invocation:?}"),
        }
    }

    fn expect_query(argv: &[OsString]) -> RustcPassthroughInvocationV2<'_> {
        match classify_rustc_invocation_v2(argv).expect("query invocation should classify") {
            RustcInvocationV2::Query(invocation) => invocation,
            invocation => panic!("expected query invocation, got {invocation:?}"),
        }
    }

    fn expect_compile(argv: &[OsString]) -> RustcCompileInvocationV2<'_> {
        match classify_rustc_invocation_v2(argv).expect("compile invocation should classify") {
            RustcInvocationV2::Compile(invocation) => invocation,
            invocation => panic!("expected compile invocation, got {invocation:?}"),
        }
    }

    #[test]
    fn classifies_terminal_forms_before_compile_metadata() {
        for selector in ["-h", "--help", "-V", "--version", "-vV", "-Vv"] {
            let argv = args(&["rustc", "--crate-name", "partial", selector]);
            assert_eq!(expect_terminal(&argv).argv(), argv);
        }

        for argv in [
            args(&["rustc", "--explain", "E0308"]),
            args(&["rustc", "--explain=E0308", "src/ignored.rs"]),
            args(&["rustc", "-C", "help"]),
            args(&["rustc", "-Whelp"]),
            args(&["rustc", "-Z", "help"]),
        ] {
            assert_eq!(expect_terminal(&argv).forwarded_args(), &argv[1..]);
        }
    }

    #[test]
    fn source_bearing_print_forms_remain_compile_invocations() {
        let joined = args(&[
            "rustc",
            "--crate-name",
            "looks_like_compile",
            "src/lib.rs",
            "--print=file-names",
        ]);
        assert_eq!(expect_compile(&joined).argv(), joined);

        let separate = args(&["rustc", "--print", "sysroot"]);
        assert_eq!(expect_query(&separate).forwarded_args(), &separate[1..]);
    }

    #[test]
    fn classifies_the_real_cargo_probe_shape_as_query() {
        let argv = args(&[
            "/toolchain/bin/rustc",
            "-",
            "--crate-name",
            "___",
            "--print=file-names",
            "--crate-type",
            "bin",
            "--crate-type",
            "rlib",
            "--crate-type",
            "dylib",
            "--crate-type",
            "cdylib",
            "--crate-type",
            "staticlib",
            "--crate-type",
            "proc-macro",
            "--print=sysroot",
            "--print=split-debuginfo",
            "--print=crate-name",
            "--print=cfg",
        ]);
        let query = expect_query(&argv);
        assert_eq!(query.executable(), OsStr::new("/toolchain/bin/rustc"));
        assert_eq!(query.argv(), argv);
    }

    #[test]
    fn stdin_queries_require_the_canonical_cargo_probe_shape() {
        for argv in [
            args(&["rustc", "-", "--print=crate-name"]),
            args(&["rustc", "-", "--crate-name", "kernel", "--print=file-names"]),
            args(&[
                "rustc",
                "-",
                "--crate-name",
                "___",
                "--print=file-names",
                "--extern",
                "proc_macro=libproc_macro.so",
            ]),
        ] {
            assert!(
                !matches!(
                    classify_rustc_invocation_v2(&argv),
                    Ok(RustcInvocationV2::Query(_))
                ),
                "noncanonical stdin invocation became a query: {argv:?}"
            );
        }
    }

    #[test]
    fn bootstrap_passthrough_policy_rejects_code_loading_and_mixed_forms() {
        for argv in [
            args(&["rustc", "--version"]),
            args(&["rustc", "--print=sysroot"]),
            args(&[
                "rustc",
                "-",
                "--crate-name",
                "___",
                "--print=file-names",
                "--crate-type=bin",
            ]),
        ] {
            assert!(
                classify_rustc_invocation_v2(&argv)
                    .unwrap()
                    .is_bootstrap_passthrough_approved(),
                "approved passthrough was rejected: {argv:?}"
            );
        }

        for argv in [
            args(&[
                "rustc",
                "-Zcodegen-backend=/tmp/untrusted.so",
                "--print=sysroot",
            ]),
            args(&["rustc", "-Zcodegen-backend=/tmp/untrusted.so", "--help"]),
            args(&["rustc", "--verbose"]),
            args(&["rustc", "--crate-name", "kernel", "src/lib.rs"]),
        ] {
            assert!(
                !classify_rustc_invocation_v2(&argv)
                    .unwrap()
                    .is_bootstrap_passthrough_approved(),
                "unsafe passthrough was approved: {argv:?}"
            );
        }
    }

    #[test]
    fn terminal_takes_precedence_over_query() {
        let argv = args(&["rustc", "--print=sysroot", "--version"]);
        assert!(matches!(
            classify_rustc_invocation_v2(&argv),
            Ok(RustcInvocationV2::Terminal(_))
        ));
    }

    #[test]
    fn selector_shaped_option_values_are_not_reclassified() {
        let compile = args(&[
            "rustc",
            "--cfg",
            "--version",
            "--crate-name",
            "kernel",
            "src/lib.rs",
        ]);
        assert!(matches!(
            classify_rustc_invocation_v2(&compile),
            Ok(RustcInvocationV2::Compile(_))
        ));

        let query = args(&["rustc", "--print", "--version"]);
        assert!(matches!(
            classify_rustc_invocation_v2(&query),
            Ok(RustcInvocationV2::Query(_))
        ));
    }

    #[test]
    fn query_selectors_after_double_dash_are_positional_inputs() {
        let argv = args(&["rustc", "--crate-name", "demo", "--", "--print=sysroot"]);
        assert_eq!(
            classify_rustc_invocation_v2(&argv),
            Err(RustcArgsErrorV2::AmbiguousSourceInput { argument_index: 4 })
        );
    }

    #[test]
    fn classifies_an_opaque_probe_without_compile_metadata_as_query() {
        let argv = args(&["rustc", "--verbose"]);
        assert_eq!(expect_query(&argv).argv(), argv);
    }

    #[test]
    fn preserves_normal_compile_arguments_and_reports_metadata() {
        let argv = args(&[
            "/toolchain/bin/rustc",
            "--crate-name",
            "demo_kernel",
            "--edition=2024",
            "crates/demo/src/lib.rs",
            "--error-format=json",
            "--crate-type",
            "lib",
            "--emit=dep-info,metadata,link",
            "-C",
            "embed-bitcode=no",
            "--out-dir",
            "/workspace/target/debug/deps",
            "--extern",
            "dependency=/workspace/target/debug/deps/libdependency.rmeta",
        ]);
        let compile = expect_compile(&argv);

        assert_eq!(compile.argv(), argv);
        assert_eq!(compile.executable(), OsStr::new("/toolchain/bin/rustc"));
        assert_eq!(compile.forwarded_args(), &argv[1..]);
        assert_eq!(compile.crate_name(), "demo_kernel");
        assert_eq!(compile.crate_name_argument_index(), 2);
        assert_eq!(compile.source_path(), Path::new("crates/demo/src/lib.rs"));
        assert_eq!(compile.source_argument_index(), 4);
    }

    #[test]
    fn accepts_joined_crate_name_and_minimal_compile_shape() {
        let argv = args(&["rustc", "src/lib.rs", "--crate-name=joined"]);
        let compile = expect_compile(&argv);
        assert_eq!(compile.crate_name(), "joined");
        assert_eq!(compile.crate_name_argument_index(), 2);
        assert_eq!(compile.source_argument_index(), 1);
    }

    #[test]
    fn does_not_require_backend_edition_or_crate_type() {
        let argv = args(&["rustc", "--crate-name", "minimal", "src/minimal.rs"]);
        assert!(matches!(
            classify_rustc_invocation_v2(&argv),
            Ok(RustcInvocationV2::Compile(_))
        ));
    }

    #[test]
    fn every_separate_value_option_consumes_one_argument() {
        for option in RUSTC_SEPARATE_VALUE_OPTIONS_V2 {
            if *option == "--crate-name" {
                let argv = args(&["rustc", "--crate-name", "actual", "src/actual.rs"]);
                let compile = expect_compile(&argv);
                assert_eq!(compile.crate_name_argument_index(), 2);
                assert_eq!(compile.source_argument_index(), 3);
                continue;
            }
            let argv = vec![
                OsString::from("rustc"),
                OsString::from(*option),
                OsString::from("decoy.rs"),
                OsString::from("--crate-name=actual"),
                OsString::from("src/actual.rs"),
            ];
            if *option == "--explain" {
                assert!(matches!(
                    classify_rustc_invocation_v2(&argv),
                    Ok(RustcInvocationV2::Terminal(_))
                ));
                continue;
            }
            let compile = expect_compile(&argv);
            assert_eq!(compile.source_argument_index(), 4, "option {option}");
        }
    }

    #[test]
    fn separate_value_option_table_is_unique_and_contains_metadata_options() {
        let mut sorted = RUSTC_SEPARATE_VALUE_OPTIONS_V2.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), RUSTC_SEPARATE_VALUE_OPTIONS_V2.len());
        assert!(RUSTC_SEPARATE_VALUE_OPTIONS_V2.contains(&"--crate-name"));
        assert!(RUSTC_SEPARATE_VALUE_OPTIONS_V2.contains(&"--print"));
        assert!(RUSTC_SEPARATE_VALUE_OPTIONS_V2.contains(&"-Z"));
    }

    #[test]
    fn option_values_ending_in_rs_are_not_sources() {
        let argv = args(&[
            "rustc",
            "--sysroot",
            "/toolchain/sysroot.rs",
            "-o",
            "target/output.rs",
            "--crate-name",
            "actual",
            "src/actual.rs",
        ]);
        let compile = expect_compile(&argv);
        assert_eq!(compile.source_argument_index(), 7);
        assert_eq!(compile.source_path(), Path::new("src/actual.rs"));
    }

    #[test]
    fn double_dash_allows_an_option_shaped_source() {
        let argv = args(&["rustc", "--crate-name", "generated", "--", "-generated.rs"]);
        let compile = expect_compile(&argv);
        assert_eq!(compile.source_path(), Path::new("-generated.rs"));
    }

    #[test]
    fn rejects_missing_or_empty_executable() {
        assert_eq!(
            classify_rustc_invocation_v2(&[]),
            Err(RustcArgsErrorV2::MissingExecutable)
        );
        assert_eq!(
            classify_rustc_invocation_v2(&[OsString::new()]),
            Err(RustcArgsErrorV2::EmptyExecutable)
        );
    }

    #[test]
    fn rejects_response_files_in_every_invocation_class() {
        for argv in [
            args(&["rustc", "@rustc.args"]),
            args(&["rustc", "--version", "@terminal.args"]),
            args(&["rustc", "--print=sysroot", "@query.args"]),
            args(&["rustc", "--sysroot", "@sysroot.args"]),
            args(&["rustc", "--", "@source.rs"]),
        ] {
            assert!(matches!(
                classify_rustc_invocation_v2(&argv),
                Err(RustcArgsErrorV2::ResponseFile { .. })
            ));
        }
    }

    #[test]
    fn passthrough_selectors_preserve_rustc_diagnostics_for_malformed_options() {
        assert!(matches!(
            classify_rustc_invocation_v2(&args(&["rustc", "--print"])),
            Ok(RustcInvocationV2::Query(_))
        ));
        assert!(matches!(
            classify_rustc_invocation_v2(&args(&["rustc", "--version", "--target"])),
            Ok(RustcInvocationV2::Terminal(_))
        ));
        assert_eq!(
            classify_rustc_invocation_v2(&[
                OsString::from("rustc"),
                OsString::from("--cfg"),
                OsString::new(),
            ]),
            Err(RustcArgsErrorV2::MissingOptionValue {
                option: "--cfg",
                option_index: 1,
            })
        );
    }

    #[test]
    fn code_producing_print_kinds_and_source_processing_modes_do_not_bypass_compile() {
        for selector in [
            "--print=native-static-libs",
            "--print=link-args",
            "-Zunpretty=expanded",
            "-Zno-analysis",
            "-Zno-codegen",
            "-Zparse-crate-root-only",
            "--pretty=expanded",
            "--unpretty=expanded",
        ] {
            let argv = args(&["rustc", "--crate-name", "kernel", "src/lib.rs", selector]);
            assert!(
                matches!(
                    classify_rustc_invocation_v2(&argv),
                    Ok(RustcInvocationV2::Compile(_))
                ),
                "{selector} bypassed compile classification"
            );
        }
    }

    #[test]
    fn non_rust_suffix_source_input_does_not_become_a_query() {
        let argv = args(&[
            "rustc",
            "--crate-name",
            "kernel",
            "src/kernel.input",
            "--print=file-names",
        ]);
        assert_eq!(
            classify_rustc_invocation_v2(&argv),
            Err(RustcArgsErrorV2::AmbiguousSourceInput { argument_index: 3 })
        );
    }

    #[test]
    fn rejects_empty_joined_long_option_values() {
        for (option, joined) in [("--crate-name", "--crate-name="), ("--target", "--target=")] {
            assert_eq!(
                classify_rustc_invocation_v2(&args(&["rustc", joined])),
                Err(RustcArgsErrorV2::MissingOptionValue {
                    option,
                    option_index: 1,
                })
            );
        }
        assert!(matches!(
            classify_rustc_invocation_v2(&args(&["rustc", "--print="])),
            Ok(RustcInvocationV2::Query(_))
        ));
        assert!(matches!(
            classify_rustc_invocation_v2(&args(&["rustc", "--explain="])),
            Ok(RustcInvocationV2::Terminal(_))
        ));
    }

    #[test]
    fn rejects_duplicate_and_conflicting_compile_crate_names() {
        let duplicate = args(&[
            "rustc",
            "--crate-name",
            "same",
            "--crate-name=same",
            "src/lib.rs",
        ]);
        assert_eq!(
            classify_rustc_invocation_v2(&duplicate),
            Err(RustcArgsErrorV2::DuplicateCrateName {
                first_argument_index: 2,
                duplicate_argument_index: 3,
            })
        );

        let conflicting = args(&[
            "rustc",
            "--crate-name=first",
            "--crate-name",
            "second",
            "src/lib.rs",
        ]);
        assert_eq!(
            classify_rustc_invocation_v2(&conflicting),
            Err(RustcArgsErrorV2::ConflictingCrateNames {
                first_argument_index: 1,
                conflicting_argument_index: 3,
            })
        );
    }

    #[test]
    fn query_precedence_does_not_parse_crate_metadata() {
        let duplicate = args(&[
            "rustc",
            "--crate-name",
            "first",
            "--crate-name",
            "second",
            "--print=file-names",
        ]);
        assert!(matches!(
            classify_rustc_invocation_v2(&duplicate),
            Ok(RustcInvocationV2::Query(_))
        ));
    }

    #[test]
    fn rejects_partial_compile_shapes() {
        assert_eq!(
            classify_rustc_invocation_v2(&args(&["rustc", "src/lib.rs"])),
            Err(RustcArgsErrorV2::PartialCompile {
                crate_name_argument_index: None,
                source_argument_index: Some(1),
            })
        );
        assert_eq!(
            classify_rustc_invocation_v2(&args(&["rustc", "--crate-name=demo"])),
            Err(RustcArgsErrorV2::PartialCompile {
                crate_name_argument_index: Some(1),
                source_argument_index: None,
            })
        );
    }

    #[test]
    fn rejects_ambiguous_and_multiple_source_inputs() {
        assert_eq!(
            classify_rustc_invocation_v2(&args(&["rustc", "--crate-name=demo", "not-a-source"])),
            Err(RustcArgsErrorV2::AmbiguousSourceInput { argument_index: 2 })
        );

        assert_eq!(
            classify_rustc_invocation_v2(&args(&[
                "rustc",
                "--crate-name=demo",
                "src/lib.rs",
                "build/generated.rs",
            ])),
            Err(RustcArgsErrorV2::MultipleSourceInputs {
                first_argument_index: 2,
                additional_argument_index: 3,
            })
        );

        assert_eq!(
            classify_rustc_invocation_v2(&args(&["rustc", "--crate-name=demo", "-"])),
            Err(RustcArgsErrorV2::AmbiguousSourceInput { argument_index: 2 })
        );
    }

    #[test]
    fn common_rust_source_path_forms_are_preserved() {
        for source in [
            "./src/lib.rs",
            "../generated/out.rs",
            "/workspace/src/main.rs",
        ] {
            let argv = args(&["rustc", "--crate-name=demo", source]);
            assert_eq!(expect_compile(&argv).source_path(), Path::new(source));
        }
    }

    #[test]
    fn enum_accessors_return_the_original_argv() {
        let argv = args(&["rustc", "--crate-name=demo", "src/lib.rs"]);
        let invocation = classify_rustc_invocation_v2(&argv).expect("compile should classify");
        assert_eq!(invocation.argv(), argv);
        assert_eq!(invocation.executable(), OsStr::new("rustc"));
        assert_eq!(invocation.forwarded_args(), &argv[1..]);
    }

    #[cfg(unix)]
    mod unix {
        use super::*;
        use std::os::unix::ffi::OsStringExt;

        #[test]
        fn query_passthrough_preserves_arbitrary_non_utf8_arguments() {
            let executable = OsString::from_vec(b"/toolchain/ru\xffstc".to_vec());
            let opaque = OsString::from_vec(b"opaque-\xff-value".to_vec());
            let argv = vec![
                executable.clone(),
                OsString::from("--cfg"),
                opaque,
                OsString::from("--print=sysroot"),
            ];
            let query = expect_query(&argv);
            assert_eq!(query.argv(), argv);
            assert_eq!(query.executable(), executable);
        }

        #[test]
        fn terminal_passthrough_preserves_arbitrary_non_utf8_arguments() {
            let opaque = OsString::from_vec(b"opaque-\xff-value".to_vec());
            let argv = vec![OsString::from("rustc"), OsString::from("--version"), opaque];
            assert_eq!(expect_terminal(&argv).argv(), argv);
        }

        #[test]
        fn compile_rejects_non_utf8_crate_metadata() {
            let separate = vec![
                OsString::from("rustc"),
                OsString::from("--crate-name"),
                OsString::from_vec(b"bad-\xff-name".to_vec()),
                OsString::from("src/lib.rs"),
            ];
            assert_eq!(
                classify_rustc_invocation_v2(&separate),
                Err(RustcArgsErrorV2::NonUtf8CrateName { argument_index: 2 })
            );

            let joined = vec![
                OsString::from("rustc"),
                OsString::from_vec(b"--crate-name=bad-\xff-name".to_vec()),
                OsString::from("src/lib.rs"),
            ];
            assert_eq!(
                classify_rustc_invocation_v2(&joined),
                Err(RustcArgsErrorV2::NonUtf8CrateName { argument_index: 1 })
            );
        }

        #[test]
        fn compile_rejects_non_utf8_source_metadata() {
            let argv = vec![
                OsString::from("rustc"),
                OsString::from("--crate-name=demo"),
                OsString::from_vec(b"src/bad-\xff.rs".to_vec()),
            ];
            assert_eq!(
                classify_rustc_invocation_v2(&argv),
                Err(RustcArgsErrorV2::NonUtf8SourceInput { argument_index: 2 })
            );
        }

        #[test]
        fn compile_preserves_non_utf8_opaque_option_values() {
            let opaque = OsString::from_vec(b"/sysroot/\xff".to_vec());
            let argv = vec![
                OsString::from_vec(b"/toolchain/ru\xffstc".to_vec()),
                OsString::from("--sysroot"),
                opaque,
                OsString::from("--crate-name=demo"),
                OsString::from("src/lib.rs"),
            ];
            let compile = expect_compile(&argv);
            assert_eq!(compile.argv(), argv);
        }

        #[test]
        fn rejects_non_utf8_response_file_without_decoding_it() {
            let argv = vec![
                OsString::from("rustc"),
                OsString::from_vec(b"@bad-\xff.args".to_vec()),
            ];
            assert_eq!(
                classify_rustc_invocation_v2(&argv),
                Err(RustcArgsErrorV2::ResponseFile { argument_index: 1 })
            );
        }
    }
}
