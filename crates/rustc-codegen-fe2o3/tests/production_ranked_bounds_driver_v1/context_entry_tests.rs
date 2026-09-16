fn check_kernel_context_source_protocol() {
    use fe2o3_rustc_front::{
        KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1,
        KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1,
        KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1,
        MAX_KERNEL_CONTEXT_FRONTEND_CONTRACT_BYTES_V1,
        encode_generated_kernel_context_frontend_contract_v1,
    };

    let target = ScratchTarget::new();
    let build_dir = target.path().join("context-protocol-target");
    let mut failures = Vec::new();
    for case in [
        "valid_zst",
        "reordered",
        "substituted_zst",
        "discarded",
        "other_issuer",
        "loop",
        "reentry",
        "wrong_abi",
        "foreign_helper",
        "foreign_marker",
        "rebound_marker",
        "cross_kernel_marker",
        "shared_marker",
        "module_marker",
        "foreign_root",
        "orphan",
        "duplicate",
        "malformed",
        "oversized",
    ] {
        let mut parameters = "a: u32, b: u32, tag: Tag";
        let mut pointer = "fn(u32, u32, Tag)";
        let mut helper_parameters = "context: Context<'_>, a: u32, b: u32, tag: Tag";
        let mut helper_body = "let _ = (context, a, b, tag);";
        let mut helper_abi = "";
        let mut helper_prefix = "";
        let mut helper_suffix = "";
        let mut body =
            "context_probe_body(KernelContext::<'_, Marker>::__compiler_issue(), a, b, tag);";
        let mut options = "launch(required = [64, 1, 1], max = [64, 1, 1])";
        let mut extra = "";
        let mut logical = "context_probe";
        let mut marker = "__fe2o3_kernel_marker_context_probe";
        let mut marker_type = marker;
        let mut root = "fe2o3_kernel_context_probe";
        let expected = match case {
            "valid_zst" => "without production expansion: KernelContextIssue",
            "reordered" => {
                body = "context_probe_body(KernelContext::<'_, Marker>::__compiler_issue(), b, a, tag);";
                "helper must consume the issued context and identity-forward every physical argument"
            }
            "substituted_zst" => {
                parameters = "supplied: Context<'static>, a: u32, b: u32, tag: Tag";
                pointer = "fn(Context<'static>, u32, u32, Tag)";
                helper_parameters =
                    "context: Context<'_>, supplied: Context<'static>, a: u32, b: u32, tag: Tag";
                helper_body = "let _ = (context, supplied, a, b, tag);";
                body = "context_probe_body(supplied, KernelContext::<'static, Marker>::__compiler_issue(), a, b, tag);";
                "helper must consume the issued context and identity-forward every physical argument"
            }
            "discarded" => {
                body = "let _: Context<'_> = KernelContext::__compiler_issue();";
                "issued context is discarded before the helper"
            }
            "other_issuer" => {
                body = "context_probe_body(other_issue(), a, b, tag);";
                extra = "#[inline(never)] fn other_issue() -> Context<'static> { KernelContext::__compiler_issue() }";
                "entry protocol calls a function other than its issuer or logical helper"
            }
            "loop" => {
                options =
                    "control_flow(loop_bounds(2)), launch(required = [64, 1, 1], max = [64, 1, 1])";
                body = "loop { context_probe_body(KernelContext::<'_, Marker>::__compiler_issue(), a, b, tag); }";
                "unit assignment is only allowed for the final physical return"
            }
            "reentry" => {
                helper_body = "fe2o3_kernel_context_probe(a, b, tag);";
                "physical context root is reachable as a callee"
            }
            "wrong_abi" => {
                helper_abi = "extern \"C\"";
                "physical/logical signatures differ beyond context ordinal zero"
            }
            "foreign_helper" => {
                helper_prefix = "mod foreign { use super::*;";
                helper_suffix = "}";
                body = "foreign::context_probe_body(KernelContext::<'_, Marker>::__compiler_issue(), a, b, tag);";
                "declared sibling context_probe_body is absent"
            }
            "foreign_marker" => {
                helper_parameters =
                    "context: KernelContext<'_, foreign::Marker>, a: u32, b: u32, tag: Tag";
                body = "context_probe_body(KernelContext::<'_, foreign::Marker>::__compiler_issue(), a, b, tag);";
                extra = "mod foreign { pub enum Marker {} }";
                "logical context kernel/target/launch brands differ"
            }
            "module_marker" => "nominal marker is not a nongeneric enum",
            "rebound_marker" => {
                marker = "ForgedMarker";
                marker_type = marker;
                extra = "enum ForgedMarker {}";
                "nominal marker does not belong to the declared logical kernel"
            }
            "shared_marker" => "nominal kernel marker is shared by distinct physical roots",
            "cross_kernel_marker" => {
                "nominal marker does not belong to the declared logical kernel"
            }
            "foreign_root" => {
                root = "foreign_root";
                pointer = "unsafe extern \"C\" fn(u32, u32, Tag)";
                extra = "unsafe extern \"C\" { fn foreign_root(a: u32, b: u32, tag: Tag); }";
                "context producer requires an ordinary local function body"
            }
            "orphan" => {
                logical = "orphan";
                marker = "__fe2o3_kernel_marker_orphan";
                marker_type = marker;
                extra = "enum __fe2o3_kernel_marker_orphan {}";
                "orphan kernel-context declaration has no registered root"
            }
            "duplicate" => "duplicate declared context root",
            "malformed" => "invalid kernel-context contract",
            "oversized" => "kernel-context declaration payload exceeds its byte limit",
            _ => unreachable!(),
        };
        let mut bytes = encode_generated_kernel_context_frontend_contract_v1(
            root,
            "context_probe_body",
            marker,
        )
        .unwrap();
        match case {
            "malformed" => bytes[0] ^= 1,
            "oversized" => bytes.resize(MAX_KERNEL_CONTEXT_FRONTEND_CONTRACT_BYTES_V1 + 1, 0),
            _ => {}
        }
        let registration = format!(
            r#"
#[used]
static __fe2o3_kernel_context_contract_v1_{logical}:
    (u64, u16, u16, &str, &[u8], {pointer}) =
    ({magic}, {version}, {kind}, "{logical}", CONTEXT_BYTES, {root});
"#,
            magic = KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1,
            version = KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1,
            kind = KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1,
        );
        let duplicate = if case == "duplicate" {
            format!(
                "#[kernel] pub fn spare() {{}} mod duplicate {{ use super::*; {registration} }}"
            )
        } else {
            String::new()
        };
        let shared_bytes_path = target.path().join("shared-marker.bin");
        let shared = if matches!(case, "shared_marker" | "cross_kernel_marker") {
            let shared_label = if case == "cross_kernel_marker" {
                "other_probe"
            } else {
                "context_probe"
            };
            std::fs::write(
                &shared_bytes_path,
                encode_generated_kernel_context_frontend_contract_v1(
                    "fe2o3_kernel_other_probe",
                    "other_probe_body",
                    marker,
                )
                .unwrap(),
            )
            .unwrap();
            format!(
                r#"
#[inline(never)]
fn other_probe_body(context: Context<'_>, a: u32, b: u32, tag: Tag) {{
    let _ = (context, a, b, tag);
}}
#[kernel(launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn other_probe(a: u32, b: u32, tag: Tag) {{
    other_probe_body(KernelContext::<'_, Marker>::__compiler_issue(), a, b, tag);
}}
mod shared {{
    use super::*;
    const BYTES: &[u8] = include_bytes!(env!("FE2O3_CONTEXT_SHARED_BYTES"));
    #[used]
    static __fe2o3_kernel_context_contract_v1_{shared_label}:
        (u64, u16, u16, &str, &[u8], fn(u32, u32, Tag)) =
        ({magic}, {version}, {kind}, "{shared_label}", BYTES, fe2o3_kernel_other_probe);
}}
"#,
                magic = KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1,
                version = KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1,
                kind = KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1,
            )
        } else {
            String::new()
        };
        let mut source = format!(
            r#"
use fe2o3_device::{{kernel, KernelContext}};
type Marker = {marker_type};
type Context<'a> = KernelContext<'a, Marker>;
type Tag = core::marker::PhantomData<&'static u32>;
{helper_prefix}
#[inline(never)]
pub(super) {helper_abi} fn context_probe_body({helper_parameters}) {{ {helper_body} }}
{helper_suffix}
#[kernel({options})]
pub fn context_probe({parameters}) {{ {body} }}
{extra}
const CONTEXT_BYTES: &[u8] = include_bytes!(env!("FE2O3_CONTEXT_PROTOCOL_BYTES"));
{registration}
{duplicate}
{shared}
"#,
        );
        if case == "module_marker" {
            source = format!(
                r#"
type Tag = core::marker::PhantomData<&'static u32>;
mod __fe2o3_kernel_marker_context_probe {{}}
#[inline(never)]
fn context_probe_body(_: (), _: u32, _: u32, _: Tag) {{}}
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_context_probe(a: u32, b: u32, tag: Tag) {{ let _ = (a, b, tag); }}
#[used]
static __fe2o3_kernel_registration_context_probe:
    (u64, u16, u16, &str, &str, fn(u32, u32, Tag)) =
    ({magic}, {version}, {kind}, "context_probe", "context_probe", fe2o3_kernel_context_probe);
const CONTEXT_BYTES: &[u8] = include_bytes!(env!("FE2O3_CONTEXT_PROTOCOL_BYTES"));
{registration}
"#,
                magic = reserved_fe2o3_symbols::KERNEL_REGISTRATION_MAGIC,
                version = reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V1,
                kind = reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_KERNEL,
            );
        }
        let source_path = target.path().join(format!("{case}.rs"));
        let bytes_path = target.path().join(format!("{case}.bin"));
        let bundle = target.path().join(format!("{case}.fe2sim"));
        std::fs::write(&source_path, source).unwrap();
        std::fs::write(&bytes_path, bytes).unwrap();
        let mut command = simulation_export_command_for_feature(
            "gfx942",
            &bundle,
            &build_dir,
            Some(5),
            "provider_context_protocol",
        );
        command
            .env("FE2O3_CONTEXT_PROTOCOL_SOURCE", &source_path)
            .env("FE2O3_CONTEXT_SHARED_BYTES", &shared_bytes_path)
            .env("FE2O3_CONTEXT_PROTOCOL_BYTES", &bytes_path);
        let result = output(
            command,
            "authenticate generated context entry source protocol",
        );
        if let Some(diagnostic) = result
            .stderr
            .lines()
            .find(|line| line.contains("fe2o3 rustc extraction:"))
        {
            eprintln!("context protocol {case}: {diagnostic}");
        }
        if result.status.success() || !result.stderr.contains(expected) {
            failures.push(format!(
                "{case} did not reach {expected:?}:\n{}",
                result.stderr
            ));
        }
        assert!(!bundle.exists(), "{case} emitted a simulation bundle");
        if matches!(case, "valid_zst" | "substituted_zst" | "foreign_root") {
            let llvm = target.path().join(format!("{case}.ll"));
            let mut command = base_command("check", &build_dir);
            command
                .env_remove("FE2O3_EXTRACT_RANKED_MEMORY_V1")
                .env(
                    "RUSTC_WORKSPACE_WRAPPER",
                    env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
                )
                .env(
                    "FE2O3_EXTRACT_CRATE_V1",
                    "fe2o3_production_ranked_bounds_fixture",
                )
                .env("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1", &llvm)
                .env("FE2O3_CONTEXT_PROTOCOL_SOURCE", &source_path)
                .env("FE2O3_CONTEXT_PROTOCOL_BYTES", &bytes_path)
                .args(["--features", "provider_context_protocol"]);
            let result = output(command, "reject context authority before LLVM output");
            if result.status.success() || !result.stderr.contains(expected) {
                failures.push(format!(
                    "{case} LLVM request did not reach {expected:?}:\n{}",
                    result.stderr
                ));
            }
            assert!(!llvm.exists(), "{case} emitted LLVM");
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
