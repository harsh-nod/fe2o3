
#[test]
fn actual_sibling_compile_retains_cargo_argv_and_own_binding_without_extraction_outputs() {
    let identity = PortablePackageIdentityV1::new("shared-package", "0.1.0", [0x61; 32]).unwrap();
    for name in ["shared_name", "differently_named_library"] {
        let args = vec![
            "actual-rustc".into(), "--crate-name".into(), name.into(),
            "--crate-type".into(), "lib".into(), "-Cmetadata=cargo-library-token".into(),
            "--emit=dep-info,metadata".into(), "src/lib.rs".into(),
        ];
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&args).unwrap() else { panic!() };
        let PreparedExtractionV1::BindingOnly { executable, forwarded_args, crate_binding } =
            prepare_binding_only_compile(compile, &identity).unwrap() else { panic!("binding only") };
        let portable = portable_rustc_metadata_v1(compile, &identity).unwrap();
        assert_eq!(crate_binding, derive_crate_binding_id_v1(name, [portable.as_str()]));
        let mut bin_args = args.clone();
        bin_args[2] = "shared_name".into();
        bin_args[4] = "bin".into();
        bin_args[7] = "src/main.rs".into();
        let RustcInvocationV2::Compile(bin) = classify_rustc_invocation_v2(&bin_args).unwrap() else { panic!() };
        let bin_metadata = portable_rustc_metadata_v1(bin, &identity).unwrap();
        assert_ne!(crate_binding, derive_crate_binding_id_v1("shared_name", [bin_metadata.as_str()]));
        let inherited = [
            EXTRACT_CRATE_ENV_V1, EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V1,
            EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V6, EXTRACT_CRATE_BINDING_PATH_ENV_V1,
            EXTRACT_DIAGNOSTIC_KIR_PATH_ENV_V19, cargo_target_selection_v1::ENV,
        ].map(OsString::from);
        let command = binding_only_command(executable, forwarded_args, crate_binding, inherited.clone());
        assert_eq!(command.get_program(), &args[0]);
        assert_eq!(command.get_args().collect::<Vec<_>>(), args[1..].iter().map(OsString::as_os_str).collect::<Vec<_>>());
        assert!(command.get_args().any(|arg| arg == "-Cmetadata=cargo-library-token"));
        let environment = command.get_envs().collect::<std::collections::BTreeMap<_, _>>();
        for name in &inherited { assert_eq!(environment.get(name.as_os_str()), Some(&None)); }
        assert_eq!(environment.get(std::ffi::OsStr::new(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2)), Some(&None));
        let expected_binding = OsString::from(crate_binding.to_hex());
        assert_eq!(environment.get(std::ffi::OsStr::new(CRATE_BINDING_ID_ENV_V1)), Some(&Some(expected_binding.as_os_str())));
        let mut missing_metadata = args.clone();
        missing_metadata.remove(5);
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&missing_metadata).unwrap() else { panic!() };
        assert!(prepare_binding_only_compile(compile, &identity).is_err());
    }
}
