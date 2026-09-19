//! Fixed production analyses over owner-custodied Pliron graphs.
//!
//! Report and model types are public, but every operation that traverses a
//! raw Pliron `Context`/`FuncOp` graph is crate-private. The only public
//! execution route is `ProductionPlironSessionV1`, which authenticates the
//! owner, stage, registered root, and exact graph snapshot before entering
//! this module.

mod pliron_analysis_manager;
mod pliron_analysis_witness;
mod pliron_atomic_legality;
mod pliron_barrier;
mod pliron_control_edges_v1;
mod pliron_effect_refinement;
mod pliron_function_inventory;
mod pliron_hierarchical_ownership;
mod pliron_invocation_trace;
mod pliron_ir_identity;
mod pliron_launch_contract;
mod pliron_memory_order;
mod pliron_pass_contract;
mod pliron_pipeline;
mod pliron_pipeline_protocol;
mod pliron_presburger_adapter;
mod pliron_progress;
mod pliron_provenance_alias;
mod pliron_race;
mod pliron_ranked_bounds;
mod pliron_report_payload_receipt;
mod pliron_report_validation;
mod pliron_resource_envelope;
mod pliron_semantic_refinement;
mod pliron_simt_protocol;
mod pliron_sparse_index;
mod pliron_switch_verification_v1;
mod pliron_tensor_layout;
mod pliron_workgroup_memory;

pub(crate) use pliron_analysis_manager::PlironAnalysisManagerV1;
#[cfg(test)]
pub(crate) use pliron_analysis_manager::panic_next_analysis_manager_prepare_for_test_v1;
pub use pliron_analysis_witness::*;
pub use pliron_atomic_legality::*;
pub use pliron_barrier::*;
pub use pliron_effect_refinement::*;
pub use pliron_hierarchical_ownership::*;
pub use pliron_ir_identity::*;
pub use pliron_launch_contract::*;
pub use pliron_memory_order::*;
pub use pliron_pass_contract::*;
#[cfg(test)]
pub(crate) use pliron_pipeline::panic_next_production_analysis_for_test_v1;
pub use pliron_pipeline::*;
pub(crate) use pliron_pipeline::{
    ProductionPlironPreloweringOutcomeV1,
    require_production_pliron_checks_before_lowering_with_resource_limits_v1,
    require_production_pliron_checks_with_atomic_target_and_resource_limits_v1,
};
#[cfg(test)]
pub(crate) use pliron_pipeline::{
    structurally_mutate_next_production_analysis_for_test_v1,
    transiently_mutate_next_production_analysis_for_test_v1,
};
pub use pliron_pipeline_protocol::*;
#[cfg(test)]
pub(crate) use pliron_presburger_adapter::*;
pub use pliron_progress::*;
pub use pliron_provenance_alias::*;
pub use pliron_race::*;
pub use pliron_ranked_bounds::*;
pub use pliron_report_validation::*;
pub use pliron_resource_envelope::ProductionAnalysisResourcePhaseV1;
pub(crate) use pliron_resource_envelope::*;
pub use pliron_semantic_refinement::*;
pub use pliron_simt_protocol::*;
pub use pliron_sparse_index::*;
pub use pliron_tensor_layout::*;
pub use pliron_workgroup_memory::*;

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        fs,
        path::{Path, PathBuf},
    };
    use syn::{
        Fields, FnArg, GenericArgument, ImplItem, Item, PathArguments, ReturnType, Signature,
        TraitItem, Type, TypeParamBound, UseTree, Visibility, WherePredicate,
    };

    const RAW_PLIRON_TYPES: [&str; 5] = ["Context", "FuncOp", "OpRef", "Operation", "Ptr"];

    fn is_raw_name(name: &str, aliases: &BTreeSet<String>) -> bool {
        RAW_PLIRON_TYPES.contains(&name) || aliases.contains(name)
    }

    fn path_contains_raw(path: &syn::Path, aliases: &BTreeSet<String>) -> bool {
        path.segments.iter().any(|segment| {
            is_raw_name(&segment.ident.to_string(), aliases)
                || match &segment.arguments {
                    PathArguments::AngleBracketed(arguments) => {
                        arguments.args.iter().any(|argument| match argument {
                            GenericArgument::Type(ty) => type_contains_raw(ty, aliases),
                            GenericArgument::AssocType(binding) => {
                                type_contains_raw(&binding.ty, aliases)
                            }
                            GenericArgument::Constraint(constraint) => constraint
                                .bounds
                                .iter()
                                .any(|bound| bound_contains_raw(bound, aliases)),
                            GenericArgument::Lifetime(_)
                            | GenericArgument::Const(_)
                            | GenericArgument::AssocConst(_) => false,
                            _ => false,
                        })
                    }
                    PathArguments::Parenthesized(arguments) => {
                        arguments
                            .inputs
                            .iter()
                            .any(|ty| type_contains_raw(ty, aliases))
                            || matches!(
                                &arguments.output,
                                ReturnType::Type(_, ty) if type_contains_raw(ty, aliases)
                            )
                    }
                    PathArguments::None => false,
                }
        })
    }

    fn bound_contains_raw(bound: &TypeParamBound, aliases: &BTreeSet<String>) -> bool {
        match bound {
            TypeParamBound::Trait(bound) => path_contains_raw(&bound.path, aliases),
            _ => false,
        }
    }

    fn tokens_contain_raw(tokens: &impl ToString, aliases: &BTreeSet<String>) -> bool {
        tokens
            .to_string()
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .any(|token| is_raw_name(token, aliases))
    }

    fn type_contains_raw(ty: &Type, aliases: &BTreeSet<String>) -> bool {
        match ty {
            Type::Path(path) => {
                path.qself
                    .as_ref()
                    .is_some_and(|qualified| type_contains_raw(&qualified.ty, aliases))
                    || path_contains_raw(&path.path, aliases)
            }
            Type::Reference(reference) => type_contains_raw(&reference.elem, aliases),
            Type::Array(array) => type_contains_raw(&array.elem, aliases),
            Type::Slice(slice) => type_contains_raw(&slice.elem, aliases),
            Type::Tuple(tuple) => tuple.elems.iter().any(|ty| type_contains_raw(ty, aliases)),
            Type::Ptr(pointer) => type_contains_raw(&pointer.elem, aliases),
            Type::Paren(paren) => type_contains_raw(&paren.elem, aliases),
            Type::Group(group) => type_contains_raw(&group.elem, aliases),
            Type::BareFn(function) => {
                function
                    .inputs
                    .iter()
                    .any(|argument| type_contains_raw(&argument.ty, aliases))
                    || matches!(
                        &function.output,
                        ReturnType::Type(_, ty) if type_contains_raw(ty, aliases)
                    )
            }
            Type::ImplTrait(bounds) => bounds
                .bounds
                .iter()
                .any(|bound| bound_contains_raw(bound, aliases)),
            Type::TraitObject(bounds) => bounds
                .bounds
                .iter()
                .any(|bound| bound_contains_raw(bound, aliases)),
            Type::Macro(r#macro) => tokens_contain_raw(&r#macro.mac.tokens, aliases),
            Type::Verbatim(tokens) => tokens_contain_raw(tokens, aliases),
            _ => false,
        }
    }

    fn signature_contains_raw(signature: &Signature, aliases: &BTreeSet<String>) -> bool {
        signature.inputs.iter().any(|argument| match argument {
            FnArg::Receiver(receiver) => type_contains_raw(&receiver.ty, aliases),
            FnArg::Typed(argument) => type_contains_raw(&argument.ty, aliases),
        }) || matches!(
            &signature.output,
            ReturnType::Type(_, ty) if type_contains_raw(ty, aliases)
        ) || signature
            .generics
            .params
            .iter()
            .any(|parameter| match parameter {
                syn::GenericParam::Type(parameter) => parameter
                    .bounds
                    .iter()
                    .any(|bound| bound_contains_raw(bound, aliases)),
                _ => false,
            })
            || signature
                .generics
                .where_clause
                .as_ref()
                .is_some_and(|clause| {
                    clause.predicates.iter().any(|predicate| match predicate {
                        WherePredicate::Type(predicate) => {
                            type_contains_raw(&predicate.bounded_ty, aliases)
                                || predicate
                                    .bounds
                                    .iter()
                                    .any(|bound| bound_contains_raw(bound, aliases))
                        }
                        WherePredicate::Lifetime(_) => false,
                        _ => false,
                    })
                })
    }

    fn assert_public_type_is_not_raw(
        path: &Path,
        visibility: &Visibility,
        ty: &Type,
        aliases: &BTreeSet<String>,
    ) {
        assert!(
            !matches!(visibility, Visibility::Public(_)) || !type_contains_raw(ty, aliases),
            "{} exposes a safe public raw Pliron type",
            path.display(),
        );
    }

    fn assert_signature_is_not_public_raw(
        path: &Path,
        visibility: &Visibility,
        signature: &Signature,
        aliases: &BTreeSet<String>,
    ) {
        assert!(
            !matches!(visibility, Visibility::Public(_))
                || !signature_contains_raw(signature, aliases),
            "{} exposes safe public raw Pliron entry point `{}`",
            path.display(),
            signature.ident
        );
    }

    fn assert_public_struct_fields_are_not_raw(
        path: &Path,
        fields: &Fields,
        aliases: &BTreeSet<String>,
    ) {
        match fields {
            Fields::Named(fields) => {
                for field in &fields.named {
                    assert_public_type_is_not_raw(path, &field.vis, &field.ty, aliases);
                }
            }
            Fields::Unnamed(fields) => assert_all_fields_are_not_public_raw(
                path,
                &Fields::Unnamed(fields.clone()),
                aliases,
            ),
            Fields::Unit => {}
        }
    }

    fn assert_all_fields_are_not_public_raw(
        path: &Path,
        fields: &Fields,
        aliases: &BTreeSet<String>,
    ) {
        for field in fields {
            assert!(
                !type_contains_raw(&field.ty, aliases),
                "{} exposes a public raw Pliron field",
                path.display()
            );
        }
    }

    fn use_tree_contains_raw(tree: &UseTree, aliases: &BTreeSet<String>) -> bool {
        match tree {
            UseTree::Path(path) => {
                is_raw_name(&path.ident.to_string(), aliases)
                    || use_tree_contains_raw(&path.tree, aliases)
            }
            UseTree::Name(name) => is_raw_name(&name.ident.to_string(), aliases),
            UseTree::Rename(rename) => {
                is_raw_name(&rename.ident.to_string(), aliases)
                    || is_raw_name(&rename.rename.to_string(), aliases)
            }
            UseTree::Glob(_) => true,
            UseTree::Group(group) => group
                .items
                .iter()
                .any(|tree| use_tree_contains_raw(tree, aliases)),
        }
    }

    fn local_module_glob(tree: &UseTree) -> bool {
        match tree {
            UseTree::Path(path) => {
                path.ident.to_string().starts_with("pliron_") && local_module_glob(&path.tree)
            }
            UseTree::Glob(_) => true,
            _ => false,
        }
    }

    fn collect_raw_aliases(items: &[Item], aliases: &mut BTreeSet<String>) {
        for item in items {
            match item {
                Item::Use(use_item) => collect_raw_use_aliases(&use_item.tree, false, aliases),
                Item::Mod(module) => {
                    if let Some((_, items)) = &module.content {
                        collect_raw_aliases(items, aliases);
                    }
                }
                _ => {}
            }
        }
        loop {
            let before = aliases.len();
            for item in items {
                match item {
                    Item::Type(alias) if type_contains_raw(&alias.ty, aliases) => {
                        aliases.insert(alias.ident.to_string());
                    }
                    Item::Mod(module) => {
                        if let Some((_, items)) = &module.content {
                            collect_raw_aliases(items, aliases);
                        }
                    }
                    _ => {}
                }
            }
            if aliases.len() == before {
                break;
            }
        }
    }

    fn collect_raw_use_aliases(tree: &UseTree, raw_prefix: bool, aliases: &mut BTreeSet<String>) {
        match tree {
            UseTree::Path(path) => collect_raw_use_aliases(
                &path.tree,
                raw_prefix || RAW_PLIRON_TYPES.contains(&path.ident.to_string().as_str()),
                aliases,
            ),
            UseTree::Name(name) if raw_prefix || is_raw_name(&name.ident.to_string(), aliases) => {
                aliases.insert(name.ident.to_string());
            }
            UseTree::Rename(rename)
                if raw_prefix || is_raw_name(&rename.ident.to_string(), aliases) =>
            {
                aliases.insert(rename.rename.to_string());
            }
            UseTree::Group(group) => {
                for tree in &group.items {
                    collect_raw_use_aliases(tree, raw_prefix, aliases);
                }
            }
            UseTree::Name(_) | UseTree::Rename(_) | UseTree::Glob(_) => {}
        }
    }

    fn assert_items_have_no_public_raw_api(
        path: &Path,
        items: &[Item],
        aliases: &BTreeSet<String>,
    ) {
        for item in items {
            match item {
                Item::Fn(function) => {
                    assert_signature_is_not_public_raw(path, &function.vis, &function.sig, aliases)
                }
                Item::Struct(structure) if matches!(structure.vis, Visibility::Public(_)) => {
                    assert_public_struct_fields_are_not_raw(path, &structure.fields, aliases)
                }
                Item::Enum(enumeration) if matches!(enumeration.vis, Visibility::Public(_)) => {
                    for variant in &enumeration.variants {
                        assert_all_fields_are_not_public_raw(path, &variant.fields, aliases);
                    }
                }
                Item::Union(union) if matches!(union.vis, Visibility::Public(_)) => {
                    assert_all_fields_are_not_public_raw(
                        path,
                        &Fields::Named(union.fields.clone()),
                        aliases,
                    )
                }
                Item::Type(alias) => {
                    assert_public_type_is_not_raw(path, &alias.vis, &alias.ty, aliases)
                }
                Item::Const(constant) => {
                    assert_public_type_is_not_raw(path, &constant.vis, &constant.ty, aliases)
                }
                Item::Static(static_item) => {
                    assert_public_type_is_not_raw(path, &static_item.vis, &static_item.ty, aliases)
                }
                Item::Use(use_item) if matches!(use_item.vis, Visibility::Public(_)) => {
                    assert!(
                        local_module_glob(&use_item.tree)
                            || !use_tree_contains_raw(&use_item.tree, aliases),
                        "{} re-exports a raw Pliron type",
                        path.display()
                    );
                }
                Item::Trait(trait_item) => {
                    for item in &trait_item.items {
                        match item {
                            TraitItem::Fn(function) => assert_signature_is_not_public_raw(
                                path,
                                &Visibility::Public(syn::token::Pub::default()),
                                &function.sig,
                                aliases,
                            ),
                            TraitItem::Type(ty) => {
                                for bound in &ty.bounds {
                                    assert!(
                                        !bound_contains_raw(bound, aliases),
                                        "{} exposes a raw Pliron associated type bound",
                                        path.display()
                                    );
                                }
                                if let Some((_, default)) = &ty.default {
                                    assert_public_type_is_not_raw(
                                        path,
                                        &Visibility::Public(syn::token::Pub::default()),
                                        default,
                                        aliases,
                                    );
                                }
                            }
                            TraitItem::Const(constant) => assert_public_type_is_not_raw(
                                path,
                                &Visibility::Public(syn::token::Pub::default()),
                                &constant.ty,
                                aliases,
                            ),
                            _ => {}
                        }
                    }
                }
                Item::Impl(implementation) => {
                    for item in &implementation.items {
                        match item {
                            ImplItem::Fn(function) => assert_signature_is_not_public_raw(
                                path,
                                &function.vis,
                                &function.sig,
                                aliases,
                            ),
                            ImplItem::Type(ty) => {
                                assert_public_type_is_not_raw(path, &ty.vis, &ty.ty, aliases)
                            }
                            ImplItem::Const(constant) => assert_public_type_is_not_raw(
                                path,
                                &constant.vis,
                                &constant.ty,
                                aliases,
                            ),
                            _ => {}
                        }
                    }
                }
                Item::Mod(module) => {
                    if let Some((_, items)) = &module.content {
                        assert_items_have_no_public_raw_api(path, items, aliases);
                    }
                }
                Item::Macro(r#macro)
                    if tokens_contain_raw(&r#macro.mac.tokens, aliases)
                        && r#macro
                            .mac
                            .tokens
                            .to_string()
                            .split_whitespace()
                            .any(|token| token == "pub" || token.starts_with("pub(")) =>
                {
                    panic!(
                        "{} may generate a public raw Pliron API from a macro",
                        path.display()
                    );
                }
                _ => {}
            }
        }
    }

    fn collect_rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(directory).expect("production analysis source directory") {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                collect_rust_sources(&path, sources);
            } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                sources.push(path);
            }
        }
    }

    #[test]
    fn raw_context_analysis_entry_points_remain_private() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/production_analysis");
        let mut sources = Vec::new();
        collect_rust_sources(&directory, &mut sources);
        sources.sort();
        for path in sources {
            let source = fs::read_to_string(&path).expect("production analysis source");
            let syntax = syn::parse_file(&source).expect("valid production analysis Rust source");
            let mut aliases = BTreeSet::new();
            collect_raw_aliases(&syntax.items, &mut aliases);
            assert_items_have_no_public_raw_api(&path, &syntax.items, &aliases);
        }
    }

    #[test]
    fn raw_context_api_audit_rejects_alias_generic_nested_and_macro_bypasses() {
        let hostile_sources = [
            r#"
                use pliron::context::Context as Arena;
                type Hidden = Arena;
                pub fn leak() -> Option<Hidden> { None }
            "#,
            r#"
                use pliron::context::Context;
                pub trait Leak {
                    type Raw = Context;
                    fn leak(&self) -> impl Iterator<Item = Context>;
                }
            "#,
            r#"
                mod nested {
                    use pliron::context::Ptr as ArenaPointer;
                    use pliron::operation::Operation;
                    pub fn leak(pointer: ArenaPointer<Operation>) {}
                }
            "#,
            r#"
                use pliron::context::Context;
                macro_rules! raw_api {
                    () => { pub fn leak(context: &Context) {} };
                }
            "#,
            r#"pub use pliron::context::Context as PublicArena;"#,
        ];
        let path = Path::new("hostile-production-analysis-api.rs");
        for source in hostile_sources {
            let syntax = syn::parse_file(source).expect("valid hostile Rust source");
            let mut aliases = BTreeSet::new();
            collect_raw_aliases(&syntax.items, &mut aliases);
            assert!(
                std::panic::catch_unwind(|| {
                    assert_items_have_no_public_raw_api(path, &syntax.items, &aliases);
                })
                .is_err(),
                "raw Pliron API bypass was not rejected: {source}"
            );
        }
    }

    #[test]
    fn audited_raw_analysis_roster_is_not_public() {
        const RETIRED: [&str; 33] = [
            "analyze_pliron_provenance_alias_v1",
            "analyze_pliron_sparse_indices_v1",
            "derive_pliron_ir_structural_identity_v1",
            "require_pliron_ir_structural_identity_preserved_v1",
            "run_pliron_atomic_legality_check_v1",
            "run_pliron_atomic_legality_check_with_target_v1",
            "require_pliron_atomic_legality_before_lowering_v1",
            "require_pliron_atomic_legality_with_target_before_lowering_v1",
            "run_pliron_barrier_convergence_check_v1",
            "require_pliron_barrier_convergence_before_lowering_v1",
            "run_pliron_effect_refinement_check_v1",
            "require_pliron_effect_refinement_before_lowering_v1",
            "run_pliron_hierarchical_ownership_check_v1",
            "require_pliron_hierarchical_ownership_before_lowering_v1",
            "run_pliron_launch_contract_check_v1",
            "require_pliron_launch_contract_before_lowering_v1",
            "run_pliron_pipeline_protocol_check_v1",
            "require_pliron_pipeline_protocol_v1",
            "run_pliron_progress_check_v1",
            "run_pliron_ranked_bounds_check_v1",
            "require_pliron_ranked_bounds_before_lowering_v1",
            "run_pliron_ranked_race_check_v1",
            "require_pliron_ranked_race_freedom_before_lowering_v1",
            "run_pliron_semantic_refinement_check_v1",
            "require_pliron_semantic_refinement_before_lowering_v1",
            "run_pliron_tensor_layout_check_v1",
            "require_pliron_tensor_layout_before_lowering_v1",
            "run_pliron_workgroup_memory_check_v1",
            "require_pliron_workgroup_memory_safety_before_lowering_v1",
            "require_production_pliron_checks_before_lowering_v2",
            "require_production_pliron_checks_with_atomic_target_before_lowering_v2",
            "require_production_pliron_checks_with_target_before_lowering_v2",
            "require_production_pliron_checks_with_atomic_and_target_before_lowering_v2",
        ];
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/production_analysis");
        let mut pending = vec![directory];
        let mut sources = Vec::new();
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory).expect("production analysis source directory") {
                let path = entry.expect("production analysis source entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                    sources.push(fs::read_to_string(path).expect("production analysis source"));
                }
            }
        }
        for name in RETIRED {
            let public_spelling = format!("pub fn {name}");
            assert!(
                sources
                    .iter()
                    .all(|source| !source.contains(&public_spelling)),
                "audited raw analysis entry point became public: {name}"
            );
        }
    }

    #[path = "pliron_atomic_legality_tests.rs"]
    mod pliron_atomic_legality;
    #[path = "pliron_barrier_tests.rs"]
    mod pliron_barrier;
    #[path = "pliron_collective_semantics_tests.rs"]
    mod pliron_collective_semantics;
    #[path = "pliron_effect_refinement_tests.rs"]
    mod pliron_effect_refinement;
    #[path = "pliron_hierarchical_ownership_tests.rs"]
    mod pliron_hierarchical_ownership;
    #[path = "pliron_ir_identity_tests.rs"]
    mod pliron_ir_identity;
    #[path = "pliron_launch_contract_tests.rs"]
    mod pliron_launch_contract;
    #[path = "pliron_lit_tests.rs"]
    mod pliron_lit;
    #[path = "pliron_pipeline_protocol_tests.rs"]
    mod pliron_pipeline_protocol;
    #[path = "pliron_presburger_tests.rs"]
    mod pliron_presburger;
    #[path = "pliron_progress_tests.rs"]
    mod pliron_progress;
    #[path = "pliron_protocol_lit_tests.rs"]
    mod pliron_protocol_lit;
    #[path = "pliron_provenance_alias_tests.rs"]
    mod pliron_provenance_alias;
    #[path = "pliron_race_tests.rs"]
    mod pliron_race;
    #[path = "pliron_ranked_bounds_tests.rs"]
    mod pliron_ranked_bounds;
    #[path = "pliron_semantic_refinement_tests.rs"]
    mod pliron_semantic_refinement;
    #[path = "pliron_sparse_index_tests.rs"]
    mod pliron_sparse_index;
    #[path = "pliron_tensor_layout_tests.rs"]
    mod pliron_tensor_layout;
    #[path = "pliron_workgroup_memory_tests.rs"]
    mod pliron_workgroup_memory;
}
