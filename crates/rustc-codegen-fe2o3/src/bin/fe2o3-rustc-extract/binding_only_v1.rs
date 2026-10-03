// Binding-only siblings run the real compiler with its original Cargo argv.
// Their own namespace binding does not authorize extraction or output files.
fn prepare_binding_only_compile(
    compile: fe2o3_rustc_invocation::RustcCompileInvocationV2<'_>,
    identity: &PortablePackageIdentityV1,
) -> Result<PreparedExtractionV1, String> {
    let metadata = ordered_rustc_codegen_metadata_v1(compile).map_err(|error| error.to_string())?;
    if metadata.is_empty() { return Err("binding-only Cargo compile has no explicit metadata".into()); }
    let portable = portable_rustc_metadata_v1(compile, identity).map_err(|error| error.to_string())?;
    Ok(PreparedExtractionV1::BindingOnly {
        executable: compile.argv()[0].clone(),
        forwarded_args: compile.argv()[1..].to_vec(),
        crate_binding: derive_crate_binding_id_v1(compile.crate_name(), [portable.as_str()]),
    })
}

fn binding_only_command(
    executable: OsString,
    forwarded_args: Vec<OsString>,
    crate_binding: CrateBindingIdV1,
    inherited: impl IntoIterator<Item = OsString>,
) -> Command {
    let mut command = passthrough_command(executable, forwarded_args);
    for name in inherited {
        if name.as_encoded_bytes().starts_with(b"FE2O3_EXTRACT_") {
            command.env_remove(name);
        }
    }
    command.env(CRATE_BINDING_ID_ENV_V1, crate_binding.to_hex());
    command
}

fn execute_binding_only(
    executable: OsString,
    forwarded_args: Vec<OsString>,
    crate_binding: CrateBindingIdV1,
) -> Result<i32, String> {
    let mut command = binding_only_command(
        executable, forwarded_args, crate_binding, env::vars_os().map(|(name, _)| name),
    );
    let status = fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn())
        .and_then(|mut child| child.wait())
        .map_err(|error| format!("failed to execute binding-only rustc: {error}"))?;
    Ok(exit_code(status))
}
