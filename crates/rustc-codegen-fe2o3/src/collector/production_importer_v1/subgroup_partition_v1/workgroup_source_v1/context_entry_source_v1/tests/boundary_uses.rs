use super::*;

mod adjustments;

fn id(index: u32) -> HirId {
    HirId {
        local_id: rustc_hir::ItemLocalId::from_u32(index),
        ..rustc_hir::CRATE_HIR_ID
    }
}

fn boundary(index: u32) -> BoundaryBorrowV1 {
    BoundaryBorrowV1 {
        binding: id(1),
        use_site: id(index),
        call: id(index + 10),
        callee: rustc_hir::def_id::CRATE_DEF_ID.to_def_id(),
    }
}

#[test]
fn context_entry_accepts_completed_boundary_borrows_and_one_transfer() {
    assert!(exact_source_uses(&[id(9)], id(9), &[]));
    assert!(exact_source_uses(
        &[id(2), id(3), id(9)],
        id(9),
        &[boundary(2), boundary(3)]
    ));
}

#[test]
fn context_entry_use_roster_rejects_extra_missing_or_reused_source_edges() {
    for uses in [
        vec![id(2), id(3), id(9), id(8)],
        vec![id(2), id(3)],
        vec![id(2), id(3), id(9), id(9)],
        vec![id(2), id(9)],
    ] {
        assert!(!exact_source_uses(
            &uses,
            id(9),
            &[boundary(2), boundary(3)]
        ));
    }
    assert!(!exact_source_uses(
        &[id(2), id(9)],
        id(9),
        &[boundary(2), boundary(2)]
    ));
    assert!(!exact_source_uses(&[id(2), id(9)], id(9), &[boundary(9)]));
}

#[test]
fn context_entry_boundary_binder_set_is_closed() {
    use crate::trusted_device_items::TrustedDeviceItem as Item;
    for item in [
        Item::CapabilityGlobalBindReadOnly,
        Item::CapabilityGlobalBindDisjointWrite,
        Item::CapabilityGlobalBindExclusiveReadWrite,
        Item::CapabilityGlobalBindAtomic,
    ] {
        assert!(is_context_boundary_binder(Some(item)));
    }
    for item in [
        None,
        Some(Item::KernelContextIssue),
        Some(Item::CapabilityGlobalLoad),
    ] {
        assert!(!is_context_boundary_binder(item));
    }
}

// Real HIR use classification only. Local roles below are deliberately not
// trusted device items and never create an authenticated Context receipt.
const SOURCE: &str = r#"
#![no_std]
pub struct Context;
pub fn issue() -> Context { Context }
pub fn helper(_: Context) {}
pub fn __compiler_bind_read_only(_: &Context, _: u32) -> u32 { 0 }
pub fn custom(_: &Context) {}
pub fn custom_mut(_: &mut Context) {}
pub fn custom_raw(_: *const Context) {}
pub fn plain() { let context = issue(); helper(context); }
pub fn custom_borrow() { let context = issue(); custom(&context); helper(context); }
pub fn lookalike_binder() {
    let context = issue(); let _ = __compiler_bind_read_only(&context, 0); helper(context);
}
pub fn stored_borrow() { let context = issue(); let alias = &context; custom(alias); helper(context); }
pub fn mutable_borrow() { let mut context = issue(); custom_mut(&mut context); helper(context); }
pub fn raw_borrow() { let context = issue(); custom_raw(&raw const context); helper(context); }
pub fn reassignment() { let mut context = issue(); context = Context; helper(context); }
pub fn copied_owner() { let context = issue(); let moved = context; helper(moved); }
pub fn captured_owner() { let context = issue(); let read = || custom(&context); read(); helper(context); }
"#;

struct Probe(bool);

impl rustc_driver::Callbacks for Probe {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        config.input = rustc_session::config::Input::Str {
            name: rustc_span::FileName::Custom("context_entry_source_uses.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(
        &mut self,
        _: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        let find = |name: &str| {
            tcx.iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == rustc_hir::def::DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .unwrap()
        };
        let helper = find("helper").to_def_id();
        let issuer = find("issue").to_def_id();
        for name in [
            "plain",
            "custom_borrow",
            "lookalike_binder",
            "stored_borrow",
            "mutable_borrow",
            "raw_borrow",
            "reassignment",
            "copied_owner",
            "captured_owner",
        ] {
            let root = find(name);
            let mut work = 65_536;
            let mut source = SourceEdge {
                tcx,
                typeck: tcx.typeck(root),
                helper,
                issuer,
                argument: 0,
                bindings: HashMap::new(),
                local_uses: HashMap::new(),
                uses: Vec::new(),
                boundary_uses: Vec::new(),
                issue_count: 0,
                malformed: false,
                exhausted: false,
                work: &mut work,
            };
            source.visit_expr(tcx.hir_body_owned_by(root).value);
            let [edge] = source.uses.as_slice() else {
                panic!("one helper use for {name}");
            };
            let (binding, site) = *edge;
            let accepted = !source.malformed
                && !source.exhausted
                && source.issue_count == 1
                && source.bindings.get(&binding) == Some(&issuer)
                && source.checked_uses(binding, site).is_ok();
            assert_eq!(accepted, name == "plain", "HIR use predicate for {name}");
            assert!(
                source.boundary_uses.is_empty(),
                "local lookalike cannot become a trusted binder"
            );
        }
        self.0 = true;
        rustc_driver::Compilation::Stop
    }
}

#[test]
fn context_entry_hir_rejects_custom_borrows_aliases_capture_and_reassignment() {
    let mut probe = Probe(false);
    run_probe(&mut probe);
    assert!(probe.0);
}

fn run_probe(probe: &mut (impl rustc_driver::Callbacks + Send)) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=context_entry_source_uses".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-".into(),
    ];
    rustc_driver::run_compiler(&args, probe);
}
