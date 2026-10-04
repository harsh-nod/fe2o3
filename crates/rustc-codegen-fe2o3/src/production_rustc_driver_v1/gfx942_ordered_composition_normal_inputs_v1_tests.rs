//! Actual multi-cfg rustc invocation; metadata is rederived after adding the
//! finite-grid source feature, never copied from a different invocation.
use super::*;
use fe2o3_rustc_invocation::{
    PortablePackageIdentityV1, RustcInvocationV2, classify_rustc_invocation_v2,
    derive_cargo_metadata_build_observation_v2, ordered_rustc_codegen_metadata_v1,
    portable_rustc_metadata_v1,
};
use reserved_fe2o3_symbols::derive_crate_binding_id_v1;
use std::ffi::OsString;

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Invocation {
    original: super::super::inputs::PreparedInvocation,
    pub(super) finite: bool,
    pub(super) args: Vec<String>,
    pub(super) crate_binding: String,
    pub(super) cargo_observation: String,
}
pub(super) fn derive(directory: &Path, feature: &str, finite: bool) -> Invocation {
    let original = super::super::inputs::derive_record(directory, feature);
    if !finite {
        return Invocation {
            args: original.args.clone(),
            crate_binding: original.crate_binding.clone(),
            cargo_observation: original.cargo_observation.clone(),
            original,
            finite,
        };
    }
    let metadata: Value = serde_json::from_slice(
        &read_bounded(&directory.join("metadata.stdout"), 16 * 1024 * 1024).unwrap(),
    )
    .unwrap();
    let package = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["name"] == PACKAGE)
        .collect::<Vec<_>>();
    assert_eq!(package.len(), 1);
    assert_eq!(
        package[0]["features"]["ordered-composition-finite-grid"],
        json!([])
    );
    let mut args = original.args.clone();
    assert!(args.pop().unwrap().starts_with("-Cmetadata="));
    assert!(!args.iter().any(|a| a.starts_with("-Cmetadata=")));
    args.extend([
        "--cfg".to_owned(),
        "feature=\"ordered-composition-finite-grid\"".to_owned(),
    ]);
    let identity = PortablePackageIdentityV1::new(
        PACKAGE,
        "0.1.0",
        Sha256::digest(read_bounded(&fixture().join("Cargo.toml"), 1024 * 1024).unwrap()).into(),
    )
    .unwrap();
    let raw = args.iter().map(OsString::from).collect::<Vec<_>>();
    let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&raw).unwrap() else {
        panic!("actual compile");
    };
    let portable = portable_rustc_metadata_v1(compile, &identity).unwrap();
    args.push(format!("-Cmetadata={portable}"));
    let raw = args.iter().map(OsString::from).collect::<Vec<_>>();
    let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&raw).unwrap() else {
        panic!("actual compile");
    };
    let ordered = ordered_rustc_codegen_metadata_v1(compile).unwrap();
    assert_eq!(ordered, [portable.as_str()]);
    let cargo_observation = derive_cargo_metadata_build_observation_v2(&ordered).to_hex();
    let crate_binding = derive_crate_binding_id_v1(CRATE_NAME, [portable.as_str()]).to_hex();
    super::super::super::require_canonical_overflow_checks_v1(&args).unwrap();
    Invocation {
        original,
        finite,
        args,
        crate_binding,
        cargo_observation,
    }
}
