//! Explicit CLI selection for the separately versioned inert origin report.
use super::{ExportFormat, Path, PathBuf};
pub(super) const OUTPUT_ENV: &str = "FE2O3_DIAGNOSTIC_ORDERED_ORIGIN_PATH_V1";
pub(super) fn validate_output(
    requested: Option<PathBuf>,
    format: &ExportFormat,
    current_dir: &Path,
    raw: &Path,
) -> Result<Option<PathBuf>, String> {
    let Some(requested) = requested else {
        return Ok(None);
    };
    if *format != ExportFormat::DiagnosticKirV17 {
        return Err("--diagnostic-ordered-origin-v1 requires --diagnostic-kir-v17".to_owned());
    }
    if requested.as_os_str().is_empty() {
        return Err("--diagnostic-ordered-origin-v1 must name a fresh file".to_owned());
    }
    let output = super::absolute_path(current_dir, requested);
    let name = output
        .file_name()
        .ok_or("ordered origin output must name a file")?;
    let parent = output
        .parent()
        .ok_or("ordered origin output has no parent")?
        .canonicalize()
        .map_err(|_| "ordered origin output parent is unavailable")?;
    if !parent.is_dir() {
        return Err("ordered origin output parent is not a directory".to_owned());
    }
    let output = parent.join(name);
    let raw_parent = raw
        .parent()
        .ok_or("raw KIR output has no parent")?
        .canonicalize()
        .map_err(|_| "raw KIR output parent is unavailable")?;
    let raw_name = raw.file_name().ok_or("raw KIR output must name a file")?;
    if output == raw_parent.join(raw_name) {
        return Err("ordered origin and raw KIR output paths must be distinct".to_owned());
    }
    match std::fs::symlink_metadata(&output) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Some(output)),
        Ok(_) => Err("ordered origin output already exists".to_owned()),
        Err(_) => Err("ordered origin output path is unavailable".to_owned()),
    }
}
pub(super) fn require_published(output: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(output)
        .map_err(|_| "Cargo succeeded without publishing ordered origin output")?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 16 * 1024 {
        return Err("ordered origin output is not a bounded regular file".to_owned());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    fn args() -> Vec<OsString> {
        [
            "--diagnostic-kir-v17".to_owned(),
            "--crate".to_owned(),
            "kernel_crate".to_owned(),
            "--output".to_owned(),
            format!(
                "fe2o3-origin-{}-{}.kir",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
        ]
        .into_iter()
        .map(OsString::from)
        .collect()
    }
    fn fresh_report(raw: &[OsString]) -> OsString {
        let mut value = raw[4].clone();
        value.push(".origin.json");
        value
    }
    #[test]
    fn omitted_option_leaves_old_v17_mode_and_flags_unchanged() {
        let options = super::super::parse(args(), &std::env::temp_dir()).unwrap();
        assert_eq!(options.format, ExportFormat::DiagnosticKirV17);
        assert_eq!(options.diagnostic_ordered_origin, None);
    }
    #[test]
    fn explicit_option_only_adds_distinct_origin_output() {
        let mut values = args();
        let report = fresh_report(&values);
        values.extend(["--diagnostic-ordered-origin-v1".into(), report]);
        let options = super::super::parse(values, &std::env::temp_dir()).unwrap();
        assert_ne!(
            options.diagnostic_ordered_origin.as_deref(),
            Some(options.output.as_path())
        );
        assert!(options.diagnostic_ordered_origin.is_some());
        assert!(super::super::conflicting_extraction_environment().contains(&OUTPUT_ENV));
    }
    #[test]
    fn duplicate_empty_missing_and_same_file_are_refused() {
        for variant in 0..4 {
            let mut values = args();
            match variant {
                0 => {
                    let report = fresh_report(&values);
                    values.extend([
                        "--diagnostic-ordered-origin-v1".into(),
                        report.clone(),
                        "--diagnostic-ordered-origin-v1".into(),
                        report,
                    ]);
                }
                1 => values.extend(["--diagnostic-ordered-origin-v1".into(), "".into()]),
                2 => values.push("--diagnostic-ordered-origin-v1".into()),
                _ => {
                    let raw = values[4].clone();
                    values.extend(["--diagnostic-ordered-origin-v1".into(), raw]);
                }
            }
            assert!(super::super::parse(values, &std::env::temp_dir()).is_err());
        }
    }
    #[test]
    fn v16_and_all_bundle_versions_refuse_origin() {
        for version in 0..=6 {
            let mut values = args();
            let report = fresh_report(&values);
            if version == 0 {
                values[0] = "--diagnostic-kir-v16".into();
            } else {
                values.remove(0);
                values.extend(["--bundle-version".into(), version.to_string().into()]);
            }
            values.extend(["--diagnostic-ordered-origin-v1".into(), report]);
            assert!(super::super::parse(values, &std::env::temp_dir()).is_err());
        }
    }
    #[test]
    fn missing_or_nonregular_published_output_is_refused() {
        assert!(require_published(&std::env::temp_dir()).is_err());
        let values = args();
        assert!(require_published(&std::env::temp_dir().join(&values[4])).is_err());
    }
}
