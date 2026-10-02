use super::*;
use syn::visit::{self, Visit};

fn host_guard(module: &syn::ItemMod) -> bool {
    let guards: Vec<_> = module
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg"))
        .collect();
    if guards.len() != 1 || !matches!(guards[0].style, syn::AttrStyle::Outer) {
        return false;
    }
    let Ok(syn::Meta::List(not)) = guards[0].parse_args::<syn::Meta>() else {
        return false;
    };
    let Ok(syn::Meta::NameValue(arch)) = not.parse_args::<syn::Meta>() else {
        return false;
    };
    let syn::Expr::Lit(value) = arch.value else {
        return false;
    };
    matches!(value.lit, syn::Lit::Str(value) if value.value() == "amdgpu")
        && not.path.is_ident("not")
        && arch.path.is_ident("target_arch")
}

#[derive(Default)]
struct HostBoundary<'a> {
    module: &'a str,
    inside: bool,
    invalid: bool,
    modules: usize,
    imports: usize,
    arguments: usize,
    adapters: usize,
    expectations: usize,
}

impl<'ast> Visit<'ast> for HostBoundary<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let previous = self.inside;
        if item.ident == self.module {
            self.modules += 1;
            self.inside = host_guard(item) && item.content.is_some();
            self.invalid |= !self.inside;
        }
        visit::visit_item_mod(self, item);
        self.inside = previous;
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        if item
            .rename
            .as_ref()
            .is_some_and(|(_, name)| name == "__fe2o3_kernel_host")
        {
            self.imports += 1;
            self.invalid |= !self.inside;
        }
        visit::visit_item_extern_crate(self, item);
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if item.ident == "Arguments" {
            self.arguments += 1;
            self.invalid |= !self.inside;
        }
        visit::visit_item_struct(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if let Some((_, path, _)) = &item.trait_
            && let Some(segment) = path.segments.last()
        {
            if segment.ident == "CompilerGeneratedKfdArguments" {
                self.adapters += 1;
                self.invalid |= !self.inside;
            } else if segment.ident == "CompilerGeneratedKernelExpectationV1" {
                self.expectations += 1;
                self.invalid |= !self.inside;
            }
        }
        visit::visit_item_impl(self, item);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        if path
            .segments
            .iter()
            .any(|s| s.ident == "__fe2o3_kernel_host")
        {
            self.invalid |= !self.inside;
        }
        visit::visit_path(self, path);
    }
}

fn valid_boundary(file: &syn::File, module: &str) -> bool {
    let mut boundary = HostBoundary {
        module,
        ..Default::default()
    };
    boundary.visit_file(file);
    !boundary.invalid
        && (
            boundary.modules,
            boundary.imports,
            boundary.arguments,
            boundary.adapters,
            boundary.expectations,
        ) == (1, 1, 1, 1, 1)
}

fn expansion(input: ItemFn) -> (syn::File, String) {
    let name = input.sig.ident.to_string();
    let device = FoundCrate::Name("gpu_device".into());
    let device_path = device_path_for(FoundCrate::Name("gpu_device".into()));
    let tokens = expand_kernel_with_imports(
        input,
        parse_kernel_options(quote!(typed)).unwrap(),
        &device_import_for(device),
        Some(&device_path),
        Some(&host_import_for(FoundCrate::Name("gpu_host".into()))),
        Some(reserved_fe2o3_symbols::derive_crate_binding_id_v1(
            "fixture",
            ["host-exclusion"],
        )),
    )
    .unwrap();
    (syn::parse2(tokens).unwrap(), format!("{name}_gpu"))
}

#[test]
fn all_generated_host_contracts_inherit_the_outer_module_exclusion() {
    let inputs: [ItemFn; 3] = [
        parse_quote!(
            pub fn fill(output: DisjointSlice<u32>) {}
        ),
        parse_quote!(
            pub fn transform(scale: f32, input: &[f32], output: DisjointSlice<f32>) {}
        ),
        parse_quote!(
            pub fn empty() {}
        ),
    ];
    for input in inputs {
        let name = input.sig.ident.to_string();
        let (file, module) = expansion(input);
        assert!(valid_boundary(&file, &module));
        let registration = format!("{KERNEL_REGISTRATION_PREFIX}{name}");
        assert!(
            file.items.iter().any(|item| {
                matches!(item, syn::Item::Static(item) if item.ident == registration)
            })
        );
    }
}

#[test]
fn host_boundary_regression_control_rejects_removed_changed_and_relocated_guards() {
    let (file, name) = expansion(parse_quote!(
        pub fn fill(output: DisjointSlice<u32>) {}
    ));
    for replacement in [
        vec![],
        vec![parse_quote!(#![cfg(not(target_arch = "amdgpu"))])],
        vec![parse_quote!(#[cfg(target_arch = "amdgpu")])],
        vec![parse_quote!(#[cfg(not(target_arch = "x86_64"))])],
        vec![
            parse_quote!(#[cfg(not(target_arch = "amdgpu"))]),
            parse_quote!(#[cfg(not(target_arch = "amdgpu"))]),
        ],
    ] {
        let mut changed = file.clone();
        let module = changed
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Mod(item) if item.ident == name => Some(item),
                _ => None,
            })
            .unwrap();
        module.attrs = replacement;
        assert!(!valid_boundary(&changed, &name));
    }
    let mut changed = file.clone();
    let module = changed
        .items
        .iter_mut()
        .find_map(|item| match item {
            syn::Item::Mod(item) if item.ident == name => Some(item),
            _ => None,
        })
        .unwrap();
    let guard = module.attrs.remove(0);
    let constant = module
        .content
        .as_mut()
        .unwrap()
        .1
        .iter_mut()
        .find_map(|item| match item {
            syn::Item::Const(item) => Some(item),
            _ => None,
        })
        .unwrap();
    constant.attrs.push(guard);
    assert!(!valid_boundary(&changed, &name));
}
