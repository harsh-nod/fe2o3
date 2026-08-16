use crate::error::{HostLinkError, HostLinkErrorCodeV1};
use crate::model::{
    LibraryPreferenceV1, LinkerZPolicyV1, PlanArgumentV1, RootInputKindV1, validate_relative_path,
};

const MAX_CONTROL_BYTES: usize = 1024 * 1024;
const MAX_CONTROL_TOKENS: usize = 4096;

const EXACT_STATIC_TOOL_FLAGS_V1: &[&[u8]] = &[
    b"-static",
    b"--static",
    b"-Bstatic",
    b"--build-id=none",
    b"--no-dynamic-linker",
    b"--fatal-warnings",
    b"--no-undefined",
    b"--gc-sections",
    b"--eh-frame-hdr",
    b"--hash-style=gnu",
    b"--strip-debug",
    b"--discard-all",
    b"--discard-locals",
    b"-O0",
    b"-O1",
    b"-O2",
    b"-O3",
    b"--no-undefined-version",
    b"--no-allow-shlib-undefined",
];

pub(crate) fn validate_literal(value: &[u8]) -> Result<(), HostLinkError> {
    if value.is_empty() || !value.is_ascii() || value.contains(&0) {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidText,
            "literal LLD argument must be nonempty non-NUL ASCII",
        ));
    }
    if value[0] != b'-' {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidPath,
            "non-option linker inputs must use typed descriptor arguments",
        ));
    }
    if value.starts_with(b"-L") {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnresolvedSearch,
            "raw -L search directives are not admitted",
        ));
    }
    if value.starts_with(b"-l") {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnresolvedLibrary,
            "raw -l library directives are not admitted",
        ));
    }
    if value.starts_with(b"@") {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NestedResponseFile,
            "raw response-file arguments are not admitted",
        ));
    }
    if value.starts_with(b"--fe2o3-") {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnsupportedArgument,
            "static-tool protocol controls are synthesized by HostLinkClosureV1",
        ));
    }
    let lower = value.to_ascii_lowercase();
    if lower.starts_with(b"--plugin")
        || lower.starts_with(b"-plugin")
        || lower.starts_with(b"--load-pass-plugin")
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::Plugin,
            "linker plugin options are outside HostLinkClosureV1",
        ));
    }
    if lower.starts_with(b"--lto")
        || lower.starts_with(b"--thinlto")
        || lower.starts_with(b"-flto")
        || lower
            .windows(b"lto-cache".len())
            .any(|window| window == b"lto-cache")
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::Lto,
            "LTO options and caches are outside HostLinkClosureV1",
        ));
    }
    if lower == b"-o"
        || lower.starts_with(b"-o=")
        || lower.starts_with(b"--output")
        || lower.starts_with(b"--sysroot")
        || lower.starts_with(b"--chroot")
        || lower == b"-t"
        || lower.starts_with(b"--script")
        || lower.starts_with(b"--version-script")
        || lower.starts_with(b"--dynamic-list")
        || lower.starts_with(b"--retain-symbols-file")
        || lower.starts_with(b"--dependency-file")
        || lower.starts_with(b"--reproduce")
        || lower.starts_with(b"--map")
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidPath,
            "path-bearing linker option must use a typed V1 field",
        ));
    }
    if value.contains(&b'/') {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidPath,
            "slash-bearing linker options require a typed V1 field",
        ));
    }
    if EXACT_STATIC_TOOL_FLAGS_V1.contains(&value) {
        Ok(())
    } else {
        Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnsupportedArgument,
            "literal option is outside the exact fe2o3-host-lld V1 allowlist",
        ))
    }
}

pub(crate) fn validate_undefined_symbol(symbol: &str) -> Result<(), HostLinkError> {
    if symbol.is_empty()
        || symbol.len() > 512
        || !symbol
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$' | b'@'))
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnsupportedArgument,
            "undefined symbol is outside the canonical fe2o3-host-lld V1 grammar",
        ));
    }
    Ok(())
}

pub(crate) fn parse_response_file(bytes: &[u8]) -> Result<Vec<PlanArgumentV1>, HostLinkError> {
    if bytes.is_empty()
        || bytes.len() > MAX_CONTROL_BYTES
        || !bytes.is_ascii()
        || bytes.contains(&0)
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::ResponseFile,
            "response file must contain 1 through 1048576 non-NUL ASCII bytes",
        ));
    }
    let tokens = bytes
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.len() > MAX_CONTROL_TOKENS {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::ResponseFile,
            "response file exceeds its token bound",
        ));
    }
    let mut preference = LibraryPreferenceV1::DynamicOnly;
    let mut arguments = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        index += 1;
        if token.contains(&b'\'') || token.contains(&b'"') || token.contains(&b'\\') {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ResponseFile,
                "response-file quoting and escaping are outside the V1 grammar",
            ));
        }
        if token.starts_with(b"@") {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::NestedResponseFile,
                "nested response files are outside HostLinkClosureV1",
            ));
        }
        if token == b"-Bstatic" {
            preference = LibraryPreferenceV1::StaticOnly;
            continue;
        }
        if token == b"-Bdynamic" {
            preference = LibraryPreferenceV1::DynamicOnly;
            continue;
        }
        if token == b"-z" {
            let value = tokens.get(index).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ResponseFile,
                    "response-file -z option has no value",
                )
            })?;
            index += 1;
            let value = std::str::from_utf8(value).map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ResponseFile,
                    "response-file -z policy is not UTF-8",
                )
            })?;
            arguments.push(PlanArgumentV1::ZPolicy(LinkerZPolicyV1::from_str(value)?));
            continue;
        }
        if token == b"-u" {
            let symbol = tokens.get(index).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ResponseFile,
                    "response-file -u option has no symbol",
                )
            })?;
            index += 1;
            let symbol = std::str::from_utf8(symbol).map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ResponseFile,
                    "response-file undefined symbol is not UTF-8",
                )
            })?;
            validate_undefined_symbol(symbol)?;
            arguments.push(PlanArgumentV1::UndefinedSymbol(symbol.to_owned()));
            continue;
        }
        if let Some(symbol) = token.strip_prefix(b"--undefined=") {
            let symbol = std::str::from_utf8(symbol).map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ResponseFile,
                    "response-file undefined symbol is not UTF-8",
                )
            })?;
            validate_undefined_symbol(symbol)?;
            arguments.push(PlanArgumentV1::UndefinedSymbol(symbol.to_owned()));
            continue;
        }
        if let Some(root) = token.strip_prefix(b"-L@") {
            let root = std::str::from_utf8(root).map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ResponseFile,
                    "response-file root label is not UTF-8",
                )
            })?;
            arguments.push(PlanArgumentV1::SearchRoot(root.to_owned()));
            continue;
        }
        if token.starts_with(b"-L") {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::UnresolvedSearch,
                "response-file -L must name a retained root as -L@label",
            ));
        }
        if let Some(name) = token.strip_prefix(b"-l") {
            let name = std::str::from_utf8(name).map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ResponseFile,
                    "response-file library name is not UTF-8",
                )
            })?;
            arguments.push(PlanArgumentV1::Library {
                name: name.to_owned(),
                preference,
            });
            continue;
        }
        if token[0] != b'-' {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ResponseFile,
                "response-file path inputs must be represented by typed plan records",
            ));
        }
        validate_literal(token)?;
        arguments.push(PlanArgumentV1::Literal(token.to_vec()));
    }
    if arguments.is_empty() {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::ResponseFile,
            "response file expands to no arguments",
        ));
    }
    Ok(arguments)
}

pub(crate) fn parse_linker_script(bytes: &[u8]) -> Result<Vec<Vec<u8>>, HostLinkError> {
    if bytes.is_empty()
        || bytes.len() > MAX_CONTROL_BYTES
        || !bytes.is_ascii()
        || bytes.contains(&0)
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::LinkerScript,
            "linker script must contain 1 through 1048576 non-NUL ASCII bytes",
        ));
    }
    let upper = bytes.to_ascii_uppercase();
    if contains_word(&upper, b"SEARCH_DIR") {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::ScriptSearchDir,
            "SEARCH_DIR is outside the V1 linker-script grammar",
        ));
    }
    if contains_word(&upper, b"INCLUDE") {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::ScriptInclude,
            "INCLUDE is outside the V1 linker-script grammar",
        ));
    }
    let stripped = strip_comments(bytes)?;
    let tokens = script_tokens(&stripped)?;
    let mut cursor = 0;
    let mut paths = Vec::new();
    while cursor < tokens.len() {
        let directive = &tokens[cursor];
        cursor += 1;
        if matches!(directive.as_slice(), b"GROUP" | b"group") {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::LinkerScript,
                "GROUP semantics are not implemented by HostLinkClosureV1",
            ));
        }
        if !matches!(directive.as_slice(), b"INPUT" | b"input") {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::LinkerScript,
                "only INPUT(...) linker-script directives are admitted",
            ));
        }
        if tokens.get(cursor).map(Vec::as_slice) != Some(b"(") {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::LinkerScript,
                "linker-script directive has no opening parenthesis",
            ));
        }
        cursor += 1;
        let before = paths.len();
        loop {
            let token = tokens.get(cursor).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::LinkerScript,
                    "unterminated linker-script directive",
                )
            })?;
            cursor += 1;
            if token == b")" {
                break;
            }
            if matches!(token.as_slice(), b"(" | b"," | b";") {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::LinkerScript,
                    "nested or empty linker-script token is outside the V1 grammar",
                ));
            }
            if token.starts_with(b"/") {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::AbsoluteNestedPath,
                    "absolute nested linker-script paths are not admitted",
                ));
            }
            if token.starts_with(b"-") || token.starts_with(b"@") {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::LinkerScript,
                    "linker-script entries must be exact relative files",
                ));
            }
            validate_relative_path(token)?;
            paths.push(token.clone());
            if paths.len() > MAX_CONTROL_TOKENS {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::LinkerScript,
                    "linker script exceeds its input bound",
                ));
            }
            if tokens.get(cursor).map(Vec::as_slice) == Some(b",") {
                cursor += 1;
            }
        }
        if paths.len() == before {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::LinkerScript,
                "linker-script directive has no inputs",
            ));
        }
        if tokens.get(cursor).map(Vec::as_slice) == Some(b";") {
            cursor += 1;
        }
    }
    Ok(paths)
}

pub(crate) fn classify_script_input(path: &[u8]) -> Result<RootInputKindV1, HostLinkError> {
    if path.ends_with(b".a") {
        Ok(RootInputKindV1::RegularArchive)
    } else if path.ends_with(b".rlib") {
        Ok(RootInputKindV1::Rlib)
    } else if path.ends_with(b".o") {
        Ok(RootInputKindV1::Object)
    } else if path.ends_with(b".so") || path.windows(4).any(|window| window == b".so.") {
        Ok(RootInputKindV1::Dso)
    } else {
        Err(HostLinkError::new(
            HostLinkErrorCodeV1::LinkerScript,
            "linker-script input has no admitted object/archive/rlib/DSO suffix",
        ))
    }
}

fn contains_word(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn strip_comments(bytes: &[u8]) -> Result<Vec<u8>, HostLinkError> {
    let mut output = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes.get(cursor..cursor + 2) == Some(b"/*") {
            let Some(end) = bytes[cursor + 2..]
                .windows(2)
                .position(|window| window == b"*/")
            else {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::LinkerScript,
                    "unterminated linker-script comment",
                ));
            };
            output.push(b' ');
            cursor += end + 4;
        } else {
            output.push(bytes[cursor]);
            cursor += 1;
        }
    }
    Ok(output)
}

fn script_tokens(bytes: &[u8]) -> Result<Vec<Vec<u8>>, HostLinkError> {
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if matches!(bytes[cursor], b'(' | b')' | b',' | b';') {
            tokens.push(vec![bytes[cursor]]);
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < bytes.len()
            && !bytes[cursor].is_ascii_whitespace()
            && !matches!(bytes[cursor], b'(' | b')' | b',' | b';')
        {
            cursor += 1;
        }
        let token = &bytes[start..cursor];
        if token.contains(&b'\'') || token.contains(&b'"') || token.contains(&b'\\') {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::LinkerScript,
                "linker-script quoting and escaping are outside the V1 grammar",
            ));
        }
        tokens.push(token.to_vec());
        if tokens.len() > MAX_CONTROL_TOKENS * 3 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::LinkerScript,
                "linker script exceeds its token bound",
            ));
        }
    }
    if tokens.is_empty() {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::LinkerScript,
            "linker script has no tokens",
        ));
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_language_is_an_exact_static_tool_compatible_subset() {
        for flag in EXACT_STATIC_TOOL_FLAGS_V1 {
            validate_literal(flag).unwrap();
        }
        for rejected in [
            b"--unknown-option".as_slice(),
            b"--color-diagnostics".as_slice(),
            b"-znow".as_slice(),
            b"-z=now".as_slice(),
            b"-uentry".as_slice(),
            b"--undefined=entry".as_slice(),
            b"--gc-sections=true".as_slice(),
            b"--threads".as_slice(),
            b"--threads=1".as_slice(),
            b"-Bdynamic".as_slice(),
            b"--as-needed".as_slice(),
            b"--no-as-needed".as_slice(),
            b"-pie".as_slice(),
            b"--pie".as_slice(),
            b"-shared".as_slice(),
            b"--shared".as_slice(),
            b"--export-dynamic".as_slice(),
            b"-export-dynamic".as_slice(),
            b"--start-group".as_slice(),
            b"--whole-archive".as_slice(),
            b"--start-lib".as_slice(),
            b"--fe2o3-result-socket-v1=91:1:2".as_slice(),
        ] {
            assert_eq!(
                validate_literal(rejected).unwrap_err().code(),
                HostLinkErrorCodeV1::UnsupportedArgument,
                "unexpectedly accepted {}",
                String::from_utf8_lossy(rejected)
            );
        }
    }

    #[test]
    fn response_values_are_typed_and_alternate_attached_forms_fail() {
        assert_eq!(
            parse_response_file(b"-z now -u entry --undefined=other").unwrap(),
            vec![
                PlanArgumentV1::ZPolicy(LinkerZPolicyV1::Now),
                PlanArgumentV1::UndefinedSymbol("entry".to_owned()),
                PlanArgumentV1::UndefinedSymbol("other".to_owned()),
            ]
        );
        for bytes in [b"-znow".as_slice(), b"-z=now", b"-uentry", b"-u=entry"] {
            assert!(
                parse_response_file(bytes).is_err(),
                "unexpectedly accepted {}",
                String::from_utf8_lossy(bytes)
            );
        }
    }
}
