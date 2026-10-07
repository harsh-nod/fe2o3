use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct ScratchDirectory(PathBuf);

#[test]
fn protected_release_accepts_observation_names_before_the_unchanged_locale_gate() {
    for (name, value) in [
        ("FE2O3_PRODUCTION_BUILD_EXPECTED_ID_V1", "12".repeat(32)),
        ("FE2O3_PRODUCTION_BUILD_EXPECTED_ID_V2", "34".repeat(32)),
        ("FE2O3_TUTORIAL_GRAPH_CAPTURE_V92", "/private/graphs".into()),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
            .env_clear()
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env(name, value)
            .args(["authority", "release", "probe"])
            .output()
            .expect("run protected release environment admission");
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("authority release requires exact environment TZ=UTC"),
            "{stderr}"
        );
        assert!(
            !stderr.contains("unexpected inherited environment"),
            "{stderr}"
        );
    }
}

impl ScratchDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cargo-fe2o3-production-build-config-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn production_configuration_has_no_compatibility_type_alias() {
    let source = include_str!("../src/build_config.rs");
    assert!(!source.contains("type PreparedBuildConfig = PreparedProductionBuildConfig"));
    let production_api = source
        .split("impl PreparedProductionBuildConfig {")
        .nth(1)
        .expect("production configuration API exists")
        .split("fn prepare_production_manifest")
        .next()
        .expect("production parser follows its API");
    assert!(!production_api.contains("executes_worker_in_rustc"));
    assert!(!production_api.contains("into_production"));
}

#[test]
fn cargo_package_has_no_worker_v2_compiler_surface() {
    let manifest = include_str!("../Cargo.toml");
    for rejected in [
        "worker-v2-fault-injection-test-only",
        "fe2o3-worker-v2-bundle",
        "cargo-fe2o3-worker-v2-fixture",
        "cargo-fe2o3-envelope-input-fixture",
    ] {
        assert!(
            !manifest.contains(rejected),
            "Cargo package still exposes retired compiler surface {rejected}"
        );
    }
    assert!(manifest.contains("application-handoff-fault-injection-test-only"));
    assert!(manifest.contains("worker-v3-envelope-integration-test-only"));

    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for retired in [
        "src/worker_v2_artifact_container.rs",
        "src/worker_v2_envelope_mode.rs",
        "src/worker_v2_restart.rs",
        "tests/worker_v2_vertical_slice.rs",
    ] {
        assert!(
            !package.join(retired).exists(),
            "retired Cargo compiler implementation remains at {retired}"
        );
    }
}

#[test]
fn cargo_and_application_routes_use_fixed_production_types() {
    let source = include_str!("../src/main.rs");
    let cargo_route = source
        .split("fn cargo_with_backend_result(")
        .nth(1)
        .expect("Cargo production route exists")
        .split("fn authority_sha256_from_environment(")
        .next()
        .expect("Cargo production route has a bounded body");
    assert!(cargo_route.contains("PreparedProductionBuildConfig"));
    assert!(cargo_route.contains("ProductionCargoPlan"));

    let application_route = source
        .split("fn run_application_boundary_result(")
        .nth(1)
        .expect("application production route exists")
        .split("fn run_application_with_handoff(")
        .next()
        .expect("application production route has a bounded body");
    assert!(application_route.contains("RUNNER_EXPECTS_ENVELOPE"));
    assert!(application_route.contains("requires a canonical load envelope"));
}

#[test]
fn production_managed_transaction_has_no_qualification_dispatch() {
    let source = include_str!("../src/binding_wrapper.rs");
    let preparation = source
        .split("fn prepare_production_managed_attempt(")
        .nth(1)
        .expect("direct production preparation exists")
        .split("fn complete_managed_attempt(")
        .next()
        .expect("production completion follows production preparation");
    for rejected in [
        "PreparedManagedWork",
        "ManagedQualificationWork",
        "executes_worker_in_rustc",
        "WorkerV2ResumeStore",
        "row_softmax",
        "qualification",
    ] {
        assert!(
            !preparation.contains(rejected),
            "production preparation contains qualification decision {rejected}"
        );
    }
    assert!(preparation.contains("prepare_managed_production_build"));
    assert!(preparation.contains("production_build,"));
    assert!(!preparation.contains("production_build: Option"));

    let completion = source
        .split("fn complete_managed_attempt_inner(")
        .nth(1)
        .expect("direct production completion exists")
        .split("fn complete_managed_production_build(")
        .next()
        .expect("production build completion follows transaction dispatch");
    assert!(!completion.contains("finish_build_attempt"));
    assert!(!completion.contains("qualification_work"));
    assert!(completion.contains("production_build"));
    assert!(completion.contains("source_isa_observer"));
    assert!(!completion.contains("production_build.take()"));
    assert!(completion.contains("complete_managed_production_build"));

    let environment = source
        .split("fn materialize_production_child_environment(")
        .nth(1)
        .expect("direct production child environment exists")
        .split("fn materialize_closed_child_environment(")
        .next()
        .expect("closed child environment follows production selection");
    assert!(!environment.contains("GeneralGemm"));
    assert!(!environment.contains("WorkerV2"));
    assert!(!environment.contains("qualification"));
}

#[test]
fn production_capability_release_preserves_v1_and_defers_only_selected_v2() {
    let source = include_str!("../src/binding_wrapper.rs");
    let broker = include_str!("../src/capability_broker.rs");
    let intake = source
        .split("fn from_authenticated_transfer(")
        .nth(1)
        .expect("direct production capability intake exists")
        .split("fn output_dir(&self)")
        .next()
        .expect("production capability API follows capability intake");
    assert!(intake.contains("release_or_retain_invocation_authority"));
    assert!(intake.contains("retain_for_selected_source_isa_observer"));
    assert!(!intake.contains("receive_validated_compiler_capabilities"));
    assert!(!intake.contains("ROW_SOFTMAX"));
    assert!(!intake.contains("FE2O3_QUALIFICATION"));
    let selection = source
        .split("let source_isa_selection =")
        .nth(1)
        .expect("source/ISA unit selection exists")
        .split("scope_managed_rustc_arguments")
        .next()
        .expect("managed argument scoping follows capability release");
    assert!(selection.contains("prepare_production_managed_attempt"));
    assert!(selection.contains("release_invocation_with_source_isa_observer"));
    assert!(selection.contains("config, unit, managed.attempt"));
    assert!(selection.contains("retain_source_isa_observer"));

    assert!(!source.contains("from_qualification_environment"));
    assert!(!source.contains("ManagedQualificationWork"));
    assert!(!source.contains("PreparedBuildConfig"));
    assert!(!broker.contains("S09"));
    assert!(!broker.contains("fn inherit_for_child("));
    assert!(!broker.contains("pinned_cargo_image: File"));
}

fn wrapper_path_is(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path
            .segments
            .iter()
            .all(|segment| segment.arguments.is_empty())
        && path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .eq(expected.split("::"))
}

fn wrapper_expression_is(expression: &syn::Expr, expected: &str) -> bool {
    matches!(expression, syn::Expr::Path(path)
        if path.qself.is_none() && wrapper_path_is(&path.path, expected))
}

fn wrapper_local<'a>(block: &'a syn::Block, name: &str) -> Result<(usize, &'a syn::Expr), String> {
    let locals: Vec<_> = block
        .stmts
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            let syn::Stmt::Local(local) = statement else {
                return None;
            };
            let syn::Pat::Ident(pattern) = &local.pat else {
                return None;
            };
            (pattern.ident == name).then_some((index, local))
        })
        .collect();
    let [(index, local)] = locals.as_slice() else {
        return Err(format!("expected one {name} binding"));
    };
    let initializer = local
        .init
        .as_ref()
        .ok_or_else(|| format!("missing {name} initializer"))?;
    if initializer.diverge.is_some() {
        return Err(format!("unexpected {name} fallback"));
    }
    Ok((*index, &initializer.expr))
}

#[derive(Default)]
struct WrapperAuthenticationCalls<'a> {
    statement: usize,
    calls: Vec<(usize, &'a syn::ExprCall)>,
    methods: Vec<(usize, &'a syn::ExprMethodCall)>,
}

impl<'a> syn::visit::Visit<'a> for WrapperAuthenticationCalls<'a> {
    fn visit_expr_call(&mut self, call: &'a syn::ExprCall) {
        self.calls.push((self.statement, call));
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'a syn::ExprMethodCall) {
        self.methods.push((self.statement, call));
        syn::visit::visit_expr_method_call(self, call);
    }
}

impl WrapperAuthenticationCalls<'_> {
    fn stage(&self, name: &str) -> Result<usize, String> {
        let stages: Vec<_> = self
            .calls
            .iter()
            .filter_map(|(index, call)| wrapper_expression_is(&call.func, name).then_some(*index))
            .chain(
                self.methods
                    .iter()
                    .filter_map(|(index, call)| (call.method == name).then_some(*index)),
            )
            .collect();
        match stages.as_slice() {
            [index] => Ok(*index),
            _ => Err(format!("expected one production stage: {name}")),
        }
    }
}

fn wrapper_authentication_order(source: &str) -> Result<(), String> {
    wrapper_authentication_order_with_mutation(source, |_| {})
}

fn wrapper_authentication_order_with_mutation(
    source: &str,
    mutate: impl FnOnce(&mut syn::Block),
) -> Result<(), String> {
    use syn::visit::Visit;

    let file = syn::parse_file(source).map_err(|error| error.to_string())?;
    let runs: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "run" => Some(function),
            _ => None,
        })
        .collect();
    let [run] = runs.as_slice() else {
        return Err("expected one wrapper run function".into());
    };
    let invocations: Vec<_> = run
        .block
        .stmts
        .iter()
        .filter_map(|statement| {
            let syn::Stmt::Local(local) = statement else {
                return None;
            };
            let syn::Expr::Match(selection) = &*local.init.as_ref()?.expr else {
                return None;
            };
            wrapper_expression_is(&selection.expr, "invocation").then_some(selection)
        })
        .collect();
    let [invocation] = invocations.as_slice() else {
        return Err("expected one invocation selection".into());
    };
    let arms: Vec<_> = invocation
        .arms
        .iter()
        .filter(|arm| {
            matches!(&arm.pat, syn::Pat::TupleStruct(pattern)
            if wrapper_path_is(&pattern.path, "RustcInvocationV2::Compile"))
        })
        .collect();
    let [arm] = arms.as_slice() else {
        return Err("expected one compile arm".into());
    };
    let syn::Expr::Block(compile) = &*arm.body else {
        return Err("expected compile block".into());
    };
    if arm.guard.is_some() {
        return Err("unexpected compile guard".into());
    }
    let mut block = compile.block.clone();
    mutate(&mut block);
    let block = &block;

    let (family_index, family) = wrapper_local(block, "profile_family")?;
    let syn::Expr::Try(family) = family else {
        return Err("route selection must propagate failure".into());
    };
    let syn::Expr::MethodCall(mapping) = &*family.expr else {
        return Err("missing route error mapping".into());
    };
    let syn::Expr::Call(route) = &*mapping.receiver else {
        return Err("missing route selection".into());
    };
    if mapping.method != "map_err"
        || !wrapper_expression_is(
            &route.func,
            "capability_broker::broker_route_family_from_environment",
        )
        || !route.args.is_empty()
    {
        return Err("route family must come from the existing selector".into());
    }
    let (binding_index, binding) = wrapper_local(block, "capability_binding")?;
    let syn::Expr::Try(binding) = binding else {
        return Err("binding must propagate failure".into());
    };
    let syn::Expr::MethodCall(mapping) = &*binding.expr else {
        return Err("missing binding error mapping".into());
    };
    let syn::Expr::Match(selection) = &*mapping.receiver else {
        return Err("missing family parser selection".into());
    };
    if mapping.method != "map_err"
        || !wrapper_expression_is(&selection.expr, "profile_family")
        || selection.arms.len() != 2
    {
        return Err("binding must select exactly the two authenticated route parsers".into());
    }
    for (family, parser) in [
        ("LegacyV1", "from_environment_for_client"),
        ("NativeV3", "from_environment_for_client_v4"),
    ] {
        let expected_family =
            format!("capability_broker::CompilerExecutionProfileFamily::{family}");
        let arms: Vec<_> = selection.arms.iter().filter(|arm| {
            matches!(&arm.pat, syn::Pat::Path(path) if wrapper_path_is(&path.path, &expected_family))
        }).collect();
        let [arm] = arms.as_slice() else {
            return Err(format!("missing unique {family} parser arm"));
        };
        let syn::Expr::Block(body) = &*arm.body else {
            return Err("parser arm must be a block".into());
        };
        let [syn::Stmt::Expr(syn::Expr::Call(call), None)] = body.block.stmts.as_slice() else {
            return Err("parser arm must contain only its binding constructor".into());
        };
        if arm.guard.is_some()
            || !wrapper_expression_is(
                &call.func,
                &format!("capability_broker::CapabilityBindingV3::{parser}"),
            )
            || call.args.len() != 1
            || !wrapper_expression_is(
                &call.args[0],
                "capability_broker::CapabilityProfileV1::Ordinary",
            )
        {
            return Err(format!("wrong {family} binding constructor"));
        }
    }

    let (_, transfer) = wrapper_local(block, "transferred")?;
    let syn::Expr::Try(transfer) = transfer else {
        return Err("receive must propagate failure".into());
    };
    let syn::Expr::Call(receive) = &*transfer.expr else {
        return Err("receive must be called directly".into());
    };
    if !wrapper_expression_is(&receive.func, "receive_validated_compiler_capabilities")
        || receive.args.len() != 2
        || !wrapper_expression_is(&receive.args[0], "capability_binding")
        || !wrapper_expression_is(&receive.args[1], "profile_family")
    {
        return Err("receive must authenticate this binding and selected family".into());
    }

    let mut calls = WrapperAuthenticationCalls::default();
    for (index, statement) in block.stmts.iter().enumerate() {
        calls.statement = index;
        calls.visit_stmt(statement);
    }
    let propagated_call = |index: usize| -> Result<&syn::ExprCall, String> {
        let syn::Stmt::Expr(syn::Expr::Try(propagated), Some(_)) = &block.stmts[index] else {
            return Err("authentication must directly propagate its failure".into());
        };
        let syn::Expr::Call(call) = &*propagated.expr else {
            return Err("authentication must be a direct call".into());
        };
        Ok(call)
    };
    let rustc_index = calls.stage("authenticate_pinned_rustc")?;
    let rustc = propagated_call(rustc_index)?;
    if !wrapper_expression_is(&rustc.func, "authenticate_pinned_rustc")
        || rustc.args.len() != 2
        || !matches!(&rustc.args[0], syn::Expr::Reference(reference)
            if reference.mutability.is_none() && wrapper_expression_is(&reference.expr, "pinned_rustc"))
        || !matches!(&rustc.args[1], syn::Expr::MethodCall(call)
            if call.method == "rustc_executable_sha256" && call.args.is_empty()
                && wrapper_expression_is(&call.receiver, "capability_binding"))
    {
        return Err(
            "rustc authentication must bind the pinned descriptor and selected capability".into(),
        );
    }
    let descriptor_index = calls.stage("validate_rustc_lib_tree_descriptor")?;
    let descriptor = propagated_call(descriptor_index)?;
    if !wrapper_expression_is(&descriptor.func, "validate_rustc_lib_tree_descriptor")
        || descriptor.args.len() != 1
        || !wrapper_expression_is(&descriptor.args[0], "capability_binding")
    {
        return Err("library-tree validation must authenticate the selected binding".into());
    }
    let stages = [
        family_index,
        binding_index,
        rustc_index,
        descriptor_index,
        calls.stage("receive_validated_compiler_capabilities")?,
        calls.stage("PreparedProductionBuildConfig::from_environment")?,
        calls.stage("validate_expected_build_config_identity")?,
        wrapper_local(block, "source_isa_selection")?.0,
        calls.stage("CompilerCapabilities::from_authenticated_transfer")?,
        calls.stage("prepare_production_managed_attempt")?,
        calls.stage("release_invocation_with_source_isa_observer")?,
        calls.stage("scope_managed_rustc_arguments")?,
    ];
    if !stages.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err("out-of-order production authentication stage".into());
    }
    let (_, identity) = calls
        .calls
        .iter()
        .find(|(_, call)| {
            wrapper_expression_is(&call.func, "validate_expected_build_config_identity")
        })
        .unwrap();
    if identity.args.len() != 2
        || !matches!(&identity.args[1], syn::Expr::MethodCall(call)
        if call.method == "config_identity" && call.args.is_empty()
            && wrapper_expression_is(&call.receiver, "capability_binding"))
    {
        return Err("configuration must match the authenticated binding".into());
    }
    Ok(())
}

#[test]
fn wrapper_authenticates_before_configuration_io_and_attempt_creation() {
    let source = include_str!("../src/binding_wrapper.rs");
    wrapper_authentication_order(source).unwrap();
    let formatted = source.replace(
        "receive_validated_compiler_capabilities(capability_binding, profile_family)?",
        "receive_validated_compiler_capabilities(\n capability_binding, /* selected route */ profile_family\n )?",
    );
    assert_ne!(formatted, source);
    wrapper_authentication_order(&formatted).unwrap();
}

#[test]
fn wrapper_authentication_rejects_changed_routes_missing_stages_and_reordering() {
    let source = include_str!("../src/binding_wrapper.rs");
    let receive = "receive_validated_compiler_capabilities(capability_binding, profile_family)?";
    for replacement in [
        "unreachable!()",
        "receive_validated_compiler_capabilities(capability_binding)?",
        "receive_validated_compiler_capabilities(capability_binding, other_family)?",
        "receive_validated_compiler_capabilities(other_binding, profile_family)?",
        "receive_validated_compiler_capabilities(capability_binding, profile_family)",
    ] {
        let changed = source.replacen(receive, replacement, 1);
        assert_ne!(changed, source);
        assert!(
            wrapper_authentication_order(&changed).is_err(),
            "accepted {replacement}"
        );
    }
    for (from, to) in [
        (
            "broker_route_family_from_environment()",
            "untrusted_route_family()",
        ),
        (
            "let capability_binding = match profile_family",
            "let capability_binding = match other_family",
        ),
        (
            "CapabilityBindingV3::from_environment_for_client(",
            "CapabilityBindingV3::from_environment_for_client_v4(",
        ),
        (
            "CapabilityBindingV3::from_environment_for_client_v4(",
            "CapabilityBindingV3::from_environment_for_client(",
        ),
        (
            "validate_rustc_lib_tree_descriptor(capability_binding)?",
            "validate_rustc_lib_tree_descriptor(other_binding)?",
        ),
        (
            "validate_rustc_lib_tree_descriptor(capability_binding)?",
            "validate_rustc_lib_tree_descriptor(capability_binding)",
        ),
        (
            "authenticate_pinned_rustc(&pinned_rustc,",
            "authenticate_pinned_rustc(&other_rustc,",
        ),
        (
            "authenticate_pinned_rustc(&pinned_rustc, capability_binding.rustc_executable_sha256())?",
            "authenticate_pinned_rustc(&pinned_rustc, capability_binding.rustc_executable_sha256())",
        ),
        (
            "capability_binding.rustc_executable_sha256()",
            "other_binding.rustc_executable_sha256()",
        ),
        (
            "capability_binding.config_identity()",
            "other_binding.config_identity()",
        ),
    ] {
        let changed = source.replacen(from, to, 1);
        assert_ne!(changed, source);
        assert!(
            wrapper_authentication_order(&changed).is_err(),
            "accepted {to}"
        );
    }
    for parser in [
        "from_environment_for_client",
        "from_environment_for_client_v4",
    ] {
        let constructor = format!("capability_broker::CapabilityBindingV3::{parser}(");
        let (prefix, selected) = source.split_at(source.find(&constructor).unwrap());
        let changed = format!(
            "{prefix}{}",
            selected.replacen(
                "capability_broker::CapabilityProfileV1::Ordinary",
                "capability_broker::CapabilityProfileV1::Other",
                1
            )
        );
        assert_ne!(changed, source);
        assert!(
            wrapper_authentication_order(&changed).is_err(),
            "accepted changed {parser} profile"
        );
    }
    let receive_statement = format!("            let transferred =\n                {receive};\n");
    assert_eq!(source.matches(&receive_statement).count(), 1);
    let duplicate = source.replacen(
        &receive_statement,
        &format!("{receive_statement}{receive_statement}"),
        1,
    );
    assert!(wrapper_authentication_order(&duplicate).is_err());
    let without_receive = source.replacen(&receive_statement, "", 1);
    for before in [
        "            validate_expected_build_config_identity(",
        "            let source_isa_selection =",
        "            let mut release_guard =",
    ] {
        assert_eq!(without_receive.matches(before).count(), 1);
        let reordered =
            without_receive.replacen(before, &format!("{receive_statement}{before}"), 1);
        assert!(
            wrapper_authentication_order(&reordered).is_err(),
            "accepted receive before {before}"
        );
    }
    let duplicate = wrapper_authentication_order_with_mutation(source, |block| {
        let (index, _) = wrapper_local(block, "build_config").unwrap();
        block.stmts.insert(index, block.stmts[index].clone());
    });
    assert!(duplicate.is_err());
    let missing = wrapper_authentication_order_with_mutation(source, |block| {
        let (index, _) = wrapper_local(block, "build_config").unwrap();
        block.stmts.remove(index);
    });
    assert!(missing.is_err());
}

#[test]
fn source_isa_observer_lifecycle_is_one_shot_and_prepublication() {
    let source = include_str!("../src/binding_wrapper.rs");
    assert_eq!(
        source
            .matches("admit_production_source_isa_acceptance_summary_v1")
            .count(),
        0,
        "summary mapping belongs exclusively to source_isa_observation.rs"
    );
    let mapping = include_str!("../src/source_isa_observation.rs");
    assert_eq!(
        mapping
            .matches(".admit_production_source_isa_acceptance_summary_v1()")
            .count(),
        1,
        "there is one summary computation entry point"
    );

    let fresh = source
        .split("fn complete_fresh_production_artifact(")
        .nth(1)
        .expect("fresh completion exists")
        .split("fn complete_recovered_production_artifact(")
        .next()
        .expect("recovered completion follows fresh completion");
    let finalization = fresh
        .find("finalize_protected_worker_nominal_hsaco_on_budget_v89")
        .expect("fresh finalization exists");
    let observation = fresh
        .find("emit_finalized_source_isa_observation(&finalized)")
        .expect("fresh observation exists");
    let preparation = fresh
        .find("prepare_mixed_worker_publication_v89")
        .expect("publication preparation exists");
    assert!(finalization < observation && observation < preparation);
    assert!(!fresh.contains("finalize_protected_worker_v3_hsaco_v1"));
    assert!(!fresh.contains("prepare_protected_worker_v3_hsaco_publication_v1"));
    assert!(!fresh.contains("finalize_protected_worker_nominal_hsaco_on_budget_v53"));
    assert!(!fresh.contains("prepare_mixed_worker_publication_v53"));

    let recovered = source
        .split("fn complete_recovered_production_artifact(")
        .nth(1)
        .expect("recovered completion exists")
        .split("fn complete_published_production_artifact(")
        .next()
        .expect("published completion follows recovered completion");
    assert!(
        recovered.contains("emit_finalized_source_isa_observation(recovered.finalized_evidence())")
    );
    assert!(source.contains("let Some(sink) = sink.take() else"));

    let ready = source
        .split("fn complete_ready_production_artifact(")
        .nth(1)
        .expect("Ready completion exists")
        .split("fn derive_build_attempt_input_with_config_identity(")
        .next()
        .expect("Ready completion has a bounded body");
    assert!(ready.contains("record.attempt()"));
    assert!(ready.contains("record.plan().finalization().as_bytes()"));
    assert!(ready.contains("emit_ready_source_isa_observation"));
    assert!(!ready.contains("admit_production_source_isa_acceptance_summary_v1"));
}

#[test]
fn cargo_finishes_and_exposes_observer_collection_nonfatally() {
    let source = include_str!("../src/main.rs");
    let run = source
        .split("fn run_cargo_with_backend_inner(")
        .nth(1)
        .expect("Cargo backend route exists")
        .split("struct CapabilityBrokerCompletionV1")
        .next()
        .expect("capability completion follows the device route");
    assert!(run.contains("start_protected_with_source_isa_observer"));
    assert!(run.contains("CapabilityBrokerCompletionV1::new"));
    assert!(source.contains("impl<Value> Drop for ObserverFinishOnDropV1<Value>"));
    assert!(run.contains("capability_broker.finish()"));
    assert!(source.contains("finish_capability_broker_observations"));
    assert!(source.contains("source-isa-observation-collection-v1"));
    assert!(source.contains("authority=observation-only"));
    let aggregate = run
        .split("aggregate_post_spawn_results(")
        .last()
        .expect("post-spawn aggregation exists");
    assert!(!aggregate.contains("source/ISA observer collection"));
}

#[test]
fn production_run_has_one_worker_v3_application_path() {
    let source = include_str!("../src/main.rs");
    let injection = source
        .split("fn inject_production_application_runner(")
        .nth(1)
        .expect("production runner injection exists")
        .split("fn application_runner_executable(")
        .next()
        .expect("production runner injection has a bounded body");
    assert!(injection.contains("RUNNER_EXPECTS_ENVELOPE"));
    assert!(injection.contains("does not permit an intermediate Cargo runner"));
    assert!(!injection.contains("RUNNER_EXPECTS_NO_ENVELOPE"));
    assert!(!injection.contains("expects_envelope"));

    let execution = source
        .split("if runner_count != 0 || !original_runner.is_empty()")
        .nth(1)
        .expect("production runner execution exists")
        .split("fn run_application_with_handoff(")
        .next()
        .expect("production runner execution has a bounded body");
    assert!(execution.contains("requires a canonical load envelope"));
    assert!(execution.contains("run_application_with_handoff"));
    assert!(!execution.contains("RUNNER_EXPECTS_NO_ENVELOPE"));
    assert!(!execution.contains("run_qualification_application_without_handoff"));

    let handoff_source = include_str!("../src/application_handoff.rs");
    assert!(!handoff_source.contains("RUNNER_EXPECTS_NO_ENVELOPE"));
    let exec_source = include_str!("../src/application_exec.rs");
    assert!(!exec_source.contains("configure_closed_descriptor_baseline"));
}

#[test]
fn production_receipt_is_carried_by_one_v2_envelope_through_host_admission() {
    let binding = include_str!("../src/binding_wrapper.rs");
    let intake = include_str!("../src/protected_compiler_handoff_v3.rs");
    let application = include_str!("../src/application_handoff.rs");
    let host_handoff = include_str!("../../fe2o3-host/src/application_descriptor_handoff.rs");
    let host_admission = include_str!("../../fe2o3-host/src/recovered_worker_v3_admission.rs");
    let host_verification =
        include_str!("../../fe2o3-host/src/worker_v3_verification_admission.rs");

    assert!(intake.contains("recover_compiler_execution_receipt_transport_with_currentness_v1"));
    assert!(intake.contains("admit_receipt_transport(&subject, receipt_transport)"));
    let intake_body = intake
        .split("pub(crate) fn consume_after_preflight<T>(")
        .nth(1)
        .expect("fresh protected handoff intake exists");
    let receipt_recovery = intake_body
        .find("recover_compiler_execution_receipt_transport_with_currentness_v1")
        .expect("fresh intake recovers the subject-bound compiler receipt");
    let consumption = intake_body
        .find("consume_compiler_module_handoff_with_currentness_v3")
        .expect("fresh intake consumes the exact handoff");
    assert!(receipt_recovery < consumption);

    let publication = binding
        .split("fn complete_published_production_artifact(")
        .nth(1)
        .expect("published mixed completion exists")
        .split("fn complete_ready_production_artifact(")
        .next()
        .expect("Ready completion follows published completion");
    assert!(publication.contains("WorkerV3LoadEnvelopeV2::from_published_mixed_hsaco_v89"));
    assert!(!publication.contains("WorkerV3LoadEnvelopeV2::from_published_mixed_hsaco_v53"));
    assert!(!publication.contains("WorkerV3LoadEnvelopeV2::from_published_hsaco_v1"));
    assert!(binding.contains("persist_durable_replay_custody_v2"));
    assert!(binding.contains("recover_worker_v3_load_envelope_v2"));
    assert!(binding.contains("validate_compiler_execution_receipt_carriage"));
    assert!(application.contains("WorkerV3LoadEnvelopeWireV2::decode_canonical"));
    assert!(host_handoff.contains("recover_worker_v3_load_envelope_v2"));
    assert!(host_handoff.contains("WorkerV3LoadEnvelopeWireV2::decode_canonical"));
    assert!(host_admission.contains("RecoveredWorkerV3LoadEnvelopeV2"));
    assert!(host_admission.contains("compiler_execution_subject"));
    assert!(host_admission.contains("compiler_execution_receipt.canonical_bytes()"));
    assert!(host_verification.contains("compiler_execution_subject:"));
    assert!(host_verification.contains("compiler_execution_receipt:"));
    assert!(host_verification.contains("compiler_execution_receipt_bytes"));
    assert!(host_verification.contains("protected_policy_verification_sha256"));
    assert!(host_verification.contains("protected_worker_ledger_verification_sha256"));
    assert!(host_verification.contains("external_rollback_verification_sha256"));
    assert!(host_verification.contains("validate_decision::<K>(&request, &verification)"));

    for production_source in [
        binding,
        application,
        host_handoff,
        host_admission,
        host_verification,
    ] {
        for retired in [
            "WorkerV3LoadEnvelopeV1::from_published_hsaco_v1",
            "persist_durable_replay_custody_v1",
            "recover_worker_v3_load_envelope_v1",
            "WorkerV3LoadEnvelopeWireV1::decode_canonical",
            "RecoveredWorkerV3LoadEnvelopeV1",
        ] {
            assert!(
                !production_source.contains(retired),
                "production source retained receipt-free envelope path {retired}"
            );
        }
    }
}

#[test]
fn production_worker_v3_verifier_is_sealed_and_synthetic_authority_is_test_only() {
    let host = include_str!("../../fe2o3-host/src/worker_v3_verification_admission.rs");
    let host_exports = include_str!("../../fe2o3-host/src/lib.rs");
    let host_manifest = include_str!("../../fe2o3-host/Cargo.toml");
    let cargo_manifest = include_str!("../Cargo.toml");

    assert!(host.contains("verifier_seal::Sealed<K>"));
    assert!(host.contains("pub(crate) fn new("));
    assert!(host.contains("feature = \"worker-v3-verifier-test-support\""));
    assert!(host.contains("pub unsafe trait WorkerV3SyntheticVerifierV1"));
    assert!(host_exports.contains("WorkerV3SyntheticVerifierAdapterV1"));
    assert!(host_exports.contains("WorkerV3SyntheticVerifierV1"));
    assert!(host_manifest.contains("worker-v3-verifier-test-support = []"));

    let test_feature = cargo_manifest
        .split("worker-v3-envelope-integration-test-only = [")
        .nth(1)
        .expect("receipt-bearing integration feature exists")
        .split(']')
        .next()
        .expect("receipt-bearing integration feature is bounded");
    assert!(test_feature.contains("fe2o3-host/worker-v3-verifier-test-support"));

    let default_dependencies = cargo_manifest
        .split("[dependencies]")
        .nth(1)
        .expect("Cargo dependencies exist")
        .split("[dev-dependencies]")
        .next()
        .expect("Cargo dependencies are bounded");
    assert!(!default_dependencies.contains("fe2o3-host/worker-v3-verifier-test-support"));
}

fn require_fixed_production_plan_fields(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source).map_err(|error| error.to_string())?;
    let plans: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(plan) if plan.ident == "ProductionCargoPlan" => Some(plan),
            _ => None,
        })
        .collect();
    let [plan] = plans.as_slice() else {
        return Err("expected one fixed ProductionCargoPlan struct".into());
    };
    if !plan.generics.params.is_empty() || plan.generics.where_clause.is_some() {
        return Err("production plan must not have generic route parameters".into());
    }
    let syn::Fields::Named(fields) = &plan.fields else {
        return Err("production plan must have named device and host fields".into());
    };
    if fields.named.len() != 2 {
        return Err("production plan must contain only device and host".into());
    }
    for (field, expected) in fields.named.iter().zip(["device", "host"]) {
        let syn::Type::Path(path) = &field.ty else {
            return Err("production phases must use the fixed CargoPhase type".into());
        };
        if field.ident.as_ref().is_none_or(|name| name != expected)
            || !matches!(&field.vis, syn::Visibility::Inherited)
            || path.qself.is_some()
            || !wrapper_path_is(&path.path, "CargoPhase")
        {
            return Err("production phases must be private device/host CargoPhase fields".into());
        }
    }
    Ok(())
}

#[test]
fn production_build_has_one_fixed_device_then_host_plan() {
    let source = include_str!("../src/main.rs");
    let plan = include_str!("../src/production_cargo_plan.rs");
    assert!(source.contains("ProductionCargoPlan::new"));
    assert!(source.contains("run_production_host_cargo"));
    assert!(source.contains("host phase uses ordinary rustc"));
    assert!(plan.contains("command: \"build\""));
    assert!(plan.contains("PRODUCTION_GFX942_RUSTC_TARGET_V1"));
    assert!(plan.contains("reject_caller_target"));
    assert!(!plan.contains("enum Pipeline"));
    require_fixed_production_plan_fields(plan).expect("fixed device/host production plan");
}

#[test]
fn production_plan_shape_rejects_route_fields_without_banning_host_selector_checks() {
    let fixed = "struct ProductionCargoPlan { device: CargoPhase, host: CargoPhase }";
    require_fixed_production_plan_fields(fixed).unwrap();
    require_fixed_production_plan_fields(
        "struct ProductionCargoPlan { device: CargoPhase, host: CargoPhase }
         fn reject_host_selector(selector: &str) { assert_ne!(selector, \"--all-targets\"); }",
    )
    .unwrap();
    for changed in [
        "struct ProductionCargoPlan { device: CargoPhase, host: CargoPhase, selector: bool }",
        "struct ProductionCargoPlan { device: CargoPhase }",
        "struct ProductionCargoPlan { host: CargoPhase, device: CargoPhase }",
        "struct ProductionCargoPlan { device: Option<CargoPhase>, host: CargoPhase }",
        "struct ProductionCargoPlan { pub device: CargoPhase, host: CargoPhase }",
        "struct ProductionCargoPlan(CargoPhase, CargoPhase);",
        "enum ProductionCargoPlan { Device, Host }",
        "struct ProductionCargoPlan<T> { device: CargoPhase, host: CargoPhase }",
        "struct ProductionCargoPlan { device: CargoPhase, host: CargoPhase }
         struct ProductionCargoPlan { device: CargoPhase, host: CargoPhase }",
    ] {
        assert!(
            require_fixed_production_plan_fields(changed).is_err(),
            "{changed}"
        );
    }
}

#[test]
fn ordinary_host_phase_has_no_device_compiler_controls() {
    let source = include_str!("../src/main.rs");
    let host_phase = source
        .split("fn run_production_host_cargo(")
        .nth(1)
        .expect("host phase exists")
        .split("fn scrub_simulation_build_environment(")
        .next()
        .expect("host phase ends before environment scrubbing helpers");

    for removed in [
        ".env_remove(BACKEND_ENV)",
        ".env_remove(TARGET_ENV)",
        ".env_remove(build_config::PRODUCTION_BUILD_CONFIG_ENV)",
        ".env_remove(build_config::PRODUCTION_BUILD_CONFIG_V2_ENV)",
        ".env_remove(build_config::PRODUCTION_BUILD_EXPECTED_ID_ENV)",
        ".env_remove(build_config::PRODUCTION_BUILD_EXPECTED_ID_V2_ENV)",
        ".env_remove(build_config::QUALIFICATION_ORACLE_ENV)",
        ".env_remove(capability_broker::CAPABILITY_BROKER_ENV)",
    ] {
        assert!(host_phase.contains(removed), "missing {removed}");
    }
    assert!(host_phase.contains("configure_pinned_rustc_child"));
    assert!(host_phase.contains("generation.reject_if_substituted"));
    assert!(!host_phase.contains("configure_production_target_environment"));
}

#[test]
fn production_runner_rejects_no_envelope_marker() {
    let scratch = ScratchDirectory::new();
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700))
        .expect("make scratch generation private");
    let mut owner_record = b"fe2o3-owned-v1\0".to_vec();
    owner_record.extend_from_slice(&[1_u8; 16]);
    let owner_path = scratch.0.join(".fe2o3-owned-v1");
    fs::write(&owner_path, owner_record).expect("write generation owner record");
    fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600))
        .expect("make generation owner record private");
    let metadata = fs::metadata(&scratch.0).expect("stat scratch directory");
    let encoded_path = scratch
        .0
        .as_os_str()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .args([
            "__fe2o3-runner-v1".to_owned(),
            "3".to_owned(),
            encoded_path,
            metadata.dev().to_string(),
            metadata.ino().to_string(),
            "none".to_owned(),
            "0".to_owned(),
            "/bin/true".to_owned(),
        ])
        .output()
        .expect("run production application boundary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("production application runner requires the Worker V3 envelope marker"),
        "{stderr}"
    );
}

#[test]
fn production_driver_rejects_qualification_before_target_preparation() {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-v1")
        .args(["build", "--target", "host-placeholder"])
        .output()
        .expect("run cargo-fe2o3");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FE2O3_QUALIFICATION_ORACLE_V1 is unavailable")
            && stderr.contains("production compilation has no selector"),
        "{stderr}"
    );
    assert!(!stderr.contains("production compilation requires exact FE2O3_TARGET"));
}

#[test]
fn production_manifest_rejects_qualification_envelope_fields() {
    let scratch = ScratchDirectory::new();
    let manifest = scratch.0.join("build-config.json");
    fs::write(
        &manifest,
        br#"{"candidate_output_max_bytes":1,"format":"fe2o3-production-build-config-v1","limits":{},"link_options":[],"load_envelope":"required","load_envelope_inputs":{},"providers":[],"units":[],"worker":{}}"#,
    )
    .expect("write manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_PRODUCTION_BUILD_CONFIG_V1", &manifest)
        .arg("build")
        .output()
        .expect("run cargo-fe2o3");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("production configuration must contain exactly the fields")
            && stderr.contains("load_envelope"),
        "{stderr}"
    );
}

#[test]
fn production_config_versions_are_mutually_exclusive_before_either_manifest_is_read() {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .env("FE2O3_TARGET", "gfx942")
        .env(
            "FE2O3_PRODUCTION_BUILD_CONFIG_V1",
            "/does/not/exist/production-v1.json",
        )
        .env(
            "FE2O3_PRODUCTION_BUILD_CONFIG_V2",
            "/does/not/exist/production-v2.json",
        )
        .arg("build")
        .output()
        .expect("run cargo-fe2o3");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FE2O3_PRODUCTION_BUILD_CONFIG_V1")
            && stderr.contains("FE2O3_PRODUCTION_BUILD_CONFIG_V2")
            && stderr.contains("mutually exclusive"),
        "{stderr}"
    );
    assert!(!stderr.contains("does/not/exist"), "{stderr}");
}

#[test]
fn production_v2_manifest_requires_the_exact_source_isa_observation() {
    let scratch = ScratchDirectory::new();
    let manifest = scratch.0.join("build-config-v2.json");
    fs::write(
        &manifest,
        br#"{"candidate_output_max_bytes":1,"format":"fe2o3-production-build-config-v2","limits":{},"link_options":[],"observation":{"kind":"source-isa-summary-v2"},"providers":[],"units":[],"worker":{}}"#,
    )
    .expect("write manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_PRODUCTION_BUILD_CONFIG_V2", &manifest)
        .arg("build")
        .output()
        .expect("run cargo-fe2o3");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("observation.kind must be exactly \"source-isa-summary-v1\""),
        "{stderr}"
    );
    assert!(!stderr.contains("worker.path"), "{stderr}");
}

#[test]
fn production_rejects_worker_v2_namespace_before_reading_its_manifest() {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .env("FE2O3_TARGET", "gfx942")
        .env(
            "FE2O3_WORKER_V2_CONFIG_V2",
            "/does/not/exist/worker-v2-config.json",
        )
        .arg("build")
        .output()
        .expect("run cargo-fe2o3");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FE2O3_WORKER_V2_CONFIG_V2 is qualification-only")
            && stderr.contains("FE2O3_PRODUCTION_BUILD_CONFIG_V1")
            && stderr.contains("FE2O3_PRODUCTION_BUILD_CONFIG_V2"),
        "{stderr}"
    );
    assert!(!stderr.contains("does/not/exist"), "{stderr}");
}

#[test]
fn production_driver_rejects_caller_target_selection() {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .env("FE2O3_TARGET", "gfx942")
        .args(["build", "--target", "amdgcn-amd-amdhsa"])
        .output()
        .expect("run cargo-fe2o3");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("owns device and host target selection"),
        "{stderr}"
    );
}
