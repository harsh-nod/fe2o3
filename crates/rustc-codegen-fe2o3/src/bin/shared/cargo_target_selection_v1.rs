//! Private extraction selection, not a source or executable authority.
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use fe2o3_rustc_invocation::{RUSTC_SEPARATE_VALUE_OPTIONS_V2, RustcCompileInvocationV2};
use serde::{Deserialize, Serialize};

pub(super) const ENV: &str = "FE2O3_EXTRACT_CARGO_TARGET_V1";
const MAX_REQUEST_BYTES: usize = 8192;

include!("cargo_target_primary_v1.rs");

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    kind: Kind,
    name: String,
    source: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    primary: Option<PrimaryPackage>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Bin,
}

impl Selection {
    pub(super) fn bin(name: String, source: PathBuf, crate_name: &str) -> Result<Self, String> {
        let selection = Self {
            kind: Kind::Bin,
            name,
            source: std::fs::canonicalize(&source)
                .map_err(|error| format!("canonicalize selected bin source: {error}"))?,
            primary: None,
        };
        selection.validate(crate_name)?;
        Ok(selection)
    }

    fn validate(&self, crate_name: &str) -> Result<(), String> {
        if self.name.is_empty()
            || self.name.len() > 256
            || !self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            || self.name.replace('-', "_") != crate_name
        {
            return Err("selected bin name differs from the exact rustc crate name".into());
        }
        let source_text = self
            .source
            .to_str()
            .ok_or("selected source path must be UTF-8")?;
        if source_text.is_empty() || source_text.len() > 4096 || !self.source.is_absolute() {
            return Err("selected source must be a bounded absolute path".into());
        }
        let actual = std::fs::canonicalize(&self.source)
            .map_err(|error| format!("canonicalize selected source request: {error}"))?;
        if actual != self.source
            || !std::fs::metadata(&actual)
                .map_err(|error| format!("inspect selected source: {error}"))?
                .is_file()
        {
            return Err("selected source must be an exact canonical regular file".into());
        }
        self.validate_primary()
    }

    pub(super) fn decode(value: &OsStr, crate_name: &str) -> Result<Self, String> {
        let text = value
            .to_str()
            .ok_or("selected Cargo target request must be UTF-8")?;
        if text.is_empty() || text.len() > MAX_REQUEST_BYTES {
            return Err("selected Cargo target request is empty or oversized".into());
        }
        let selection: Self = serde_json::from_str(text)
            .map_err(|error| format!("invalid selected Cargo target request: {error}"))?;
        selection.validate(crate_name)?;
        Ok(selection)
    }

    pub(super) fn encode(&self) -> Result<OsString, String> {
        let encoded = serde_json::to_string(self)
            .map_err(|error| format!("encode selected Cargo target request: {error}"))?;
        if encoded.len() > MAX_REQUEST_BYTES {
            return Err("selected Cargo target request exceeds its byte bound".into());
        }
        Ok(encoded.into())
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn matches(&self, compile: RustcCompileInvocationV2<'_>) -> Result<bool, String> {
        if compile.crate_name() != self.name.replace('-', "_") {
            return Ok(false);
        }
        if compile_crate_type(compile)? != Some("bin") {
            return Ok(false);
        }
        let source = std::fs::canonicalize(compile.source_path())
            .map_err(|error| format!("canonicalize actual selected compile source: {error}"))?;
        Ok(source == self.source)
    }
}

fn compile_crate_type<'a>(
    compile: RustcCompileInvocationV2<'a>,
) -> Result<Option<&'a str>, String> {
    // Reuse the classifier's frozen option/value grammar. Never interpret
    // an option value or a source token as a crate-type option.
    let argv = compile.argv();
    let mut index = 1;
    let mut crate_type = None;
    let mut test = false;
    while index < argv.len() {
        if index == compile.source_argument_index() {
            index += 1;
            continue;
        }
        let argument = &argv[index];
        if argument == "--" {
            break;
        }
        let separate = argument
            .to_str()
            .is_some_and(|value| RUSTC_SEPARATE_VALUE_OPTIONS_V2.contains(&value));
        let value = if argument == "--crate-type" {
            Some(
                argv.get(index + 1)
                    .ok_or("missing selected rustc crate type")?
                    .to_str()
                    .ok_or("selected rustc crate type must be UTF-8")?,
            )
        } else {
            argument
                .to_str()
                .and_then(|value| value.strip_prefix("--crate-type="))
        };
        if let Some(value) = value {
            if value.is_empty() || crate_type.replace(value).is_some() {
                return Err("selected rustc crate type is empty or repeated".into());
            }
        }
        test |= argument == "--test";
        index += if separate { 2 } else { 1 };
    }
    Ok(if test { None } else { crate_type })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_rustc_invocation::{RustcInvocationV2, classify_rustc_invocation_v2};

    include!("cargo_target_primary_v1_tests.rs");

    struct Files(PathBuf);
    impl Files {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fe2o3-target-selection-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
            ));
            std::fs::create_dir(&path).unwrap();
            for name in ["lib.rs", "main.rs"] {
                std::fs::write(path.join(name), b"").unwrap();
            }
            Self(std::fs::canonicalize(path).unwrap())
        }
        fn selection(&self) -> Selection {
            Selection::bin("shared-name".into(), self.0.join("main.rs"), "shared_name").unwrap()
        }
        fn args(&self, source: &str, kind: &str) -> Vec<OsString> {
            vec![
                "rustc".into(),
                "--crate-name".into(),
                "shared_name".into(),
                "--crate-type".into(),
                kind.into(),
                self.0.join(source).into_os_string(),
            ]
        }
    }
    impl Drop for Files {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn matches(selection: &Selection, args: &[OsString]) -> Result<bool, String> {
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(args).unwrap()
        else {
            panic!("compile fixture");
        };
        selection.matches(compile)
    }

    #[test]
    fn exact_bin_selection_distinguishes_same_name_and_same_file_library() {
        let files = Files::new();
        let selection = files.selection();
        assert_eq!(matches(&selection, &files.args("main.rs", "bin")), Ok(true));
        assert_eq!(matches(&selection, &files.args("lib.rs", "lib")), Ok(false));
        assert_eq!(
            matches(&selection, &files.args("main.rs", "lib")),
            Ok(false)
        );
        assert_eq!(matches(&selection, &files.args("lib.rs", "bin")), Ok(false));
        let encoded = selection.encode().unwrap();
        assert_eq!(
            Selection::decode(&encoded, "shared_name").unwrap(),
            selection
        );
    }

    #[test]
    fn exact_bin_selection_rejects_malformed_requests_without_fallback() {
        let files = Files::new();
        let original = serde_json::to_value(files.selection()).unwrap();
        for changed in [
            serde_json::json!({}),
            serde_json::json!({"kind":"lib","name":"shared-name","source":files.0.join("main.rs")}),
            serde_json::json!({"kind":"bin","name":"foreign","source":files.0.join("main.rs")}),
            serde_json::json!({"kind":"bin","name":"shared-name","source":"main.rs"}),
            serde_json::json!({"kind":"bin","name":"shared-name","source":files.0.join("absent.rs")}),
            serde_json::json!({"kind":"bin","name":"shared-name","source":files.0}),
            {
                let mut value = original.clone();
                value["extra"] = true.into();
                value
            },
        ] {
            assert!(Selection::decode(OsStr::new(&changed.to_string()), "shared_name").is_err());
        }
        for text in ["", "not-json"] {
            assert!(Selection::decode(OsStr::new(text), "shared_name").is_err());
        }
        assert!(
            Selection::decode(
                OsStr::new(&" ".repeat(MAX_REQUEST_BYTES + 1)),
                "shared_name"
            )
            .is_err()
        );
    }

    #[test]
    fn exact_bin_selection_uses_original_option_roles_and_refuses_ambiguity() {
        let files = Files::new();
        let selection = files.selection();
        let mut args = files.args("main.rs", "bin");
        args.extend(["--cfg", "--crate-type=lib"].map(OsString::from));
        assert_eq!(matches(&selection, &args), Ok(true));
        args.extend(["--crate-type", "bin"].map(OsString::from));
        assert!(matches(&selection, &args).is_err());
        let mut args = files.args("main.rs", "bin");
        args.push("--test".into());
        assert_eq!(matches(&selection, &args), Ok(false));
        assert_eq!(
            matches(&selection, &files.args("main.rs", "bin,lib")),
            Ok(false)
        );
    }
}
