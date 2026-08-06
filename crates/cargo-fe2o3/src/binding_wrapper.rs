use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::process::{Command, ExitStatus};

use fe2o3_rustc_invocation::{RustcArgsErrorV2, RustcInvocationV2, classify_rustc_invocation_v2};
use reserved_fe2o3_symbols::{CRATE_BINDING_ID_ENV_V1, derive_crate_binding_id_v1};

#[derive(Debug)]
pub(crate) enum BindingWrapperError {
    Arguments(RustcArgsErrorV2),
    MissingMetadata { crate_name: String },
    InvalidCodegenOption { argument_index: usize },
    EmptyMetadata { argument_index: usize },
    UnsupportedInvocation,
    Spawn(std::io::Error),
}

impl fmt::Display for BindingWrapperError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arguments(error) => write!(formatter, "invalid rustc invocation: {error}"),
            Self::MissingMetadata { crate_name } => write!(
                formatter,
                "rustc compile for crate `{crate_name}` has no explicit -C metadata value"
            ),
            Self::InvalidCodegenOption { argument_index } => write!(
                formatter,
                "rustc codegen option at argv[{argument_index}] is not valid UTF-8"
            ),
            Self::EmptyMetadata { argument_index } => write!(
                formatter,
                "rustc metadata value at argv[{argument_index}] is empty"
            ),
            Self::UnsupportedInvocation => {
                formatter.write_str("unsupported future rustc invocation classification")
            }
            Self::Spawn(error) => write!(formatter, "failed to execute rustc: {error}"),
        }
    }
}

impl Error for BindingWrapperError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Arguments(error) => Some(error),
            Self::Spawn(error) => Some(error),
            Self::MissingMetadata { .. }
            | Self::InvalidCodegenOption { .. }
            | Self::EmptyMetadata { .. }
            | Self::UnsupportedInvocation => None,
        }
    }
}

impl From<RustcArgsErrorV2> for BindingWrapperError {
    fn from(value: RustcArgsErrorV2) -> Self {
        Self::Arguments(value)
    }
}

pub(crate) fn run(argv: Vec<OsString>) -> Result<ExitStatus, BindingWrapperError> {
    let invocation = match classify_rustc_invocation_v2(&argv) {
        Ok(invocation) => invocation,
        Err(_) if is_cargo_stdin_probe(&argv) => {
            let mut command = Command::new(&argv[0]);
            command.args(&argv[1..]);
            command.env_remove(CRATE_BINDING_ID_ENV_V1);
            return command.status().map_err(BindingWrapperError::Spawn);
        }
        Err(error) => return Err(error.into()),
    };
    let crate_binding = match invocation {
        RustcInvocationV2::Compile(compile) => {
            let metadata = ordered_metadata_values(compile.argv())?;
            if metadata.is_empty() {
                return Err(BindingWrapperError::MissingMetadata {
                    crate_name: compile.crate_name().to_owned(),
                });
            }
            Some(derive_crate_binding_id_v1(
                compile.crate_name(),
                metadata.iter().map(String::as_str),
            ))
        }
        RustcInvocationV2::Terminal(_) | RustcInvocationV2::Query(_) => None,
        _ => return Err(BindingWrapperError::UnsupportedInvocation),
    };

    let mut command = Command::new(invocation.executable());
    command.args(invocation.forwarded_args());
    if let Some(crate_binding) = crate_binding {
        command.env(CRATE_BINDING_ID_ENV_V1, crate_binding.to_hex());
    } else {
        command.env_remove(CRATE_BINDING_ID_ENV_V1);
    }
    command.status().map_err(BindingWrapperError::Spawn)
}

fn is_cargo_stdin_probe(argv: &[OsString]) -> bool {
    argv.get(1).is_some_and(|argument| argument == "-")
        && argv.iter().skip(2).any(|argument| {
            argument == "--print"
                || argument
                    .to_str()
                    .is_some_and(|argument| argument.starts_with("--print="))
        })
}

fn ordered_metadata_values(argv: &[OsString]) -> Result<Vec<String>, BindingWrapperError> {
    let mut metadata = Vec::new();
    let mut index = 1;
    while index < argv.len() {
        let argument = &argv[index];
        if argument == "-C" || argument == "--codegen" {
            let value_index = index + 1;
            let value = argv
                .get(value_index)
                .expect("the invocation classifier checked separate option values");
            inspect_codegen_value(value, value_index, &mut metadata)?;
            index += 2;
            continue;
        }

        if let Some(argument) = argument.to_str() {
            if let Some(value) = argument.strip_prefix("-C") {
                inspect_codegen_text(value, index, &mut metadata)?;
            } else if let Some(value) = argument.strip_prefix("--codegen=") {
                inspect_codegen_text(value, index, &mut metadata)?;
            }
        }
        index += 1;
    }
    Ok(metadata)
}

fn inspect_codegen_value(
    value: &OsStr,
    argument_index: usize,
    metadata: &mut Vec<String>,
) -> Result<(), BindingWrapperError> {
    let value = value
        .to_str()
        .ok_or(BindingWrapperError::InvalidCodegenOption { argument_index })?;
    inspect_codegen_text(value, argument_index, metadata)
}

fn inspect_codegen_text(
    value: &str,
    argument_index: usize,
    metadata: &mut Vec<String>,
) -> Result<(), BindingWrapperError> {
    let Some(value) = value.strip_prefix("metadata=") else {
        return Ok(());
    };
    if value.is_empty() {
        return Err(BindingWrapperError::EmptyMetadata { argument_index });
    }
    metadata.push(value.to_owned());
    Ok(())
}

pub(crate) fn exit_code(status: ExitStatus) -> u8 {
    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::{BindingWrapperError, is_cargo_stdin_probe, ordered_metadata_values};
    use reserved_fe2o3_symbols::derive_crate_binding_id_v1;
    use std::ffi::OsString;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn extracts_every_supported_metadata_form_in_order() {
        let argv = args(&[
            "rustc",
            "--crate-name",
            "unit",
            "unit.rs",
            "-C",
            "metadata=first",
            "-Cmetadata=second",
            "--codegen",
            "metadata=third",
            "--codegen=metadata=fourth",
            "-Copt-level=2",
        ]);

        assert_eq!(
            ordered_metadata_values(&argv).unwrap(),
            ["first", "second", "third", "fourth"]
        );
    }

    #[test]
    fn crate_name_and_ordered_metadata_distinguish_compilation_units() {
        let first = derive_crate_binding_id_v1("unit", ["first", "second"]);
        let reordered = derive_crate_binding_id_v1("unit", ["second", "first"]);
        let renamed = derive_crate_binding_id_v1("other", ["first", "second"]);
        assert_ne!(first, reordered);
        assert_ne!(first, renamed);
    }

    #[test]
    fn rejects_empty_metadata() {
        let error = ordered_metadata_values(&args(&[
            "rustc",
            "--crate-name",
            "unit",
            "unit.rs",
            "-Cmetadata=",
        ]))
        .unwrap_err();
        assert!(matches!(
            error,
            BindingWrapperError::EmptyMetadata { argument_index: 4 }
        ));
    }

    #[test]
    fn recognizes_only_stdin_print_probe_shape() {
        assert!(is_cargo_stdin_probe(&args(&[
            "rustc",
            "-",
            "--crate-name",
            "___",
            "--print=file-names",
        ])));
        assert!(!is_cargo_stdin_probe(&args(&[
            "rustc",
            "source.rs",
            "--print=file-names",
        ])));
        assert!(!is_cargo_stdin_probe(&args(&[
            "rustc",
            "-",
            "--crate-name",
            "real_compile",
        ])));
    }
}
