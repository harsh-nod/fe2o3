use super::*;

/// Identifies an implemented trait, not the source owner of its implementation.
/// Core membership comes from rustc's language items. This grants no source
/// observation to the trait, its defaults, or any reachable core helper.
pub(super) fn nominal_trait_path(tcx: TyCtxt<'_>, definition: DefId) -> Result<String, String> {
    if tcx.def_kind(definition) != DefKind::Trait {
        return Err("provider implementation does not name a trait".into());
    }
    if tcx.crate_name(definition.krate).as_str() == "fe2o3_device" {
        return stable_nominal_provider_path_v1(tcx, definition);
    }
    let core = tcx
        .lang_items()
        .sized_trait()
        .ok_or_else(|| "provider trait has no pinned core owner".to_owned())?
        .krate;
    if definition.krate != core {
        return Err("provider trait belongs to an unreviewed dependency".into());
    }
    named_external_provider_as(tcx, core, "core")?;
    let path = tcx.def_path(definition).to_string_no_crate_verbose();
    if path.is_empty() || !path.starts_with("::") || path.contains(['{', '}']) {
        return Err("provider trait has no stable nominal definition path".into());
    }
    Ok(format!("core{path}"))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use rustc_driver::{Callbacks, Compilation};
    use rustc_interface::interface::{Compiler, Config};
    use rustc_session::config::Input;
    use rustc_span::FileName;

    // Called by the cached-metadata device fixture after its concrete Result
    // conversion has resolved. This observes structure and rustc source owners,
    // not the managed Cargo/source-closure evidence required for admission.
    pub(in crate::trusted_device_items) fn assert_actual_device_from_structure<'tcx>(
        tcx: TyCtxt<'tcx>,
        conversion: Instance<'tcx>,
    ) {
        let definition = conversion.def_id();
        assert!(matches!(conversion.def, InstanceKind::Item(_)));
        assert_eq!(tcx.def_kind(definition), DefKind::AssocFn);
        assert_eq!(tcx.item_name(definition).as_str(), "from");
        assert_eq!(
            named_external_provider(tcx, definition.krate).unwrap(),
            "fe2o3_device"
        );
        let implementation = tcx.impl_of_assoc(definition).unwrap();
        assert_eq!(implementation.krate, definition.krate);
        let from_trait = tcx.get_diagnostic_item(Symbol::intern("From")).unwrap();
        assert_eq!(tcx.impl_trait_id(implementation), from_trait);
        assert_eq!(
            nominal_trait_path(tcx, from_trait).unwrap(),
            "core::convert::From"
        );

        let signature = tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(definition).instantiate(tcx, conversion.args),
        );
        assert_eq!(signature.safety, Safety::Safe);
        assert_eq!(signature.abi, ExternAbi::Rust);
        assert!(!signature.c_variadic);
        let [error] = signature.inputs() else {
            panic!("the device conversion must have exactly one source argument")
        };
        let kernel_error = signature.output();
        let TyKind::Adt(owner, _) = kernel_error.kind() else {
            panic!("the device conversion result must be KernelError")
        };
        let TyKind::Adt(source, _) = error.kind() else {
            panic!("the device conversion input must be Bf16MatrixViewError")
        };
        assert_eq!(owner.did().krate, definition.krate);
        assert_eq!(source.did().krate, definition.krate);
        assert_eq!(
            stable_nominal_provider_path_v1(tcx, owner.did()).unwrap(),
            "fe2o3_device::kernel_result::KernelError"
        );
        assert_eq!(
            stable_nominal_provider_path_v1(tcx, source.did()).unwrap(),
            "fe2o3_device::tensor::Bf16MatrixViewError"
        );
        let trait_ref = tcx.impl_trait_ref(implementation).instantiate_identity();
        assert_eq!(trait_ref.self_ty(), kernel_error);
        assert_eq!(
            trait_ref.args,
            tcx.mk_args(&[kernel_error.into(), (*error).into()])
        );
        for source_owner in [definition, implementation, owner.did(), source.did()] {
            let span = tcx.def_span(source_owner);
            assert!(!span.is_dummy());
            assert_eq!(
                tcx.sess.source_map().lookup_source_file(span.lo()).cnum,
                definition.krate
            );
        }
        let structure = stable_provider_structure_v1(tcx, definition).unwrap();
        assert_eq!(
            structure.canonical_definition_path,
            "fe2o3_device::kernel_result::KernelError::from"
        );
        assert_ne!(structure.identity().unwrap(), [0; 32]);
        assert_eq!(
            structure,
            stable_provider_structure_v1(tcx, definition).unwrap()
        );

        // Same genuine receiver, but local implementation/trait ownership.
        // Bound the inventory before issuing any structure or trait queries.
        let definitions = tcx.iter_local_def_id().take(513).collect::<Vec<_>>();
        assert!(definitions.len() <= 512, "device fixture definition budget");
        let mut local_methods = 0;
        let mut marker = None;
        for definition in definitions.into_iter().map(|id| id.to_def_id()) {
            if tcx.def_kind(definition) != DefKind::AssocFn {
                continue;
            }
            let Some(implementation) = tcx.impl_of_assoc(definition) else {
                continue;
            };
            local_methods += 1;
            assert!(local_methods <= 3, "device fixture implementation budget");
            let trait_ref = tcx.impl_trait_ref(implementation).instantiate_identity();
            assert_eq!(trait_ref.self_ty(), kernel_error);
            assert_eq!(definition.krate, LOCAL_CRATE);
            assert_eq!(
                stable_provider_structure_v1(tcx, definition).unwrap_err(),
                "provider is the local compilation crate"
            );
            assert!(!authenticate_reviewed_safe_external_helper_v1(tcx, definition).unwrap());
            let implemented_trait = tcx.impl_trait_id(implementation);
            match tcx.item_name(definition).as_str() {
                "from" => {
                    assert_eq!(implemented_trait, from_trait);
                    assert_eq!(
                        nominal_trait_path(tcx, implemented_trait).unwrap(),
                        "core::convert::From"
                    );
                    assert!(marker.replace(trait_ref.args.type_at(1)).is_none());
                }
                "eq" => {
                    assert_eq!(implemented_trait, tcx.lang_items().eq_trait().unwrap());
                    assert_eq!(
                        nominal_trait_path(tcx, implemented_trait).unwrap(),
                        "core::cmp::PartialEq"
                    );
                }
                "unreviewed" => {
                    assert_eq!(implemented_trait.krate, LOCAL_CRATE);
                    assert_eq!(
                        nominal_trait_path(tcx, implemented_trait).unwrap_err(),
                        "provider trait belongs to an unreviewed dependency"
                    );
                }
                name => panic!("unexpected device fixture implementation method: {name}"),
            }
        }
        assert_eq!(local_methods, 3);
        let marker = marker.unwrap();
        let TyKind::Adt(marker_owner, _) = marker.kind() else {
            panic!("the negative conversion must retain its local marker type")
        };
        assert_eq!(marker_owner.did().krate, LOCAL_CRATE);
        assert!(stable_nominal_provider_path_v1(tcx, marker_owner.did()).is_err());

        // The fixture supplies eq but not ne, so resolution must reach the core
        // default even though Self is the genuine device KernelError type.
        let eq_trait = tcx.lang_items().eq_trait().unwrap();
        let items = tcx
            .associated_items(eq_trait)
            .in_definition_order()
            .take(9)
            .collect::<Vec<_>>();
        assert!(items.len() <= 8, "core comparison trait item budget");
        let defaults = items
            .into_iter()
            .filter(|item| item.is_fn() && item.name().as_str() == "ne")
            .collect::<Vec<_>>();
        let [default] = defaults.as_slice() else {
            panic!("core PartialEq must expose one ne default")
        };
        let resolved = Instance::try_resolve(
            tcx,
            TypingEnv::fully_monomorphized(),
            default.def_id,
            tcx.mk_args(&[kernel_error.into(), marker.into()]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(resolved.def_id(), default.def_id);
        assert_eq!(resolved.def_id().krate, eq_trait.krate);
        assert_ne!(resolved.def_id().krate, owner.did().krate);
        assert!(tcx.impl_of_assoc(resolved.def_id()).is_none());
        assert_eq!(
            stable_provider_structure_v1(tcx, resolved.def_id()).unwrap_err(),
            "provider crate name is `core`"
        );
        assert!(!authenticate_reviewed_safe_external_helper_v1(tcx, resolved.def_id()).unwrap());
    }

    struct Probe(bool);
    impl Callbacks for Probe {
        fn config(&mut self, config: &mut Config) {
            config.input = Input::Str {
                name: FileName::Custom("provider_trait_identity.rs".into()),
                input: "#![no_std]\npub trait From {}\npub struct Owner;\npub fn from() {}".into(),
            };
        }

        fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            for (trait_id, expected) in [
                (
                    tcx.get_diagnostic_item(Symbol::intern("From")).unwrap(),
                    "core::convert::From",
                ),
                (tcx.lang_items().eq_trait().unwrap(), "core::cmp::PartialEq"),
            ] {
                assert_eq!(nominal_trait_path(tcx, trait_id).unwrap(), expected);
                // Trait membership is not an authentication of a device source.
                assert!(!authenticate_reviewed_safe_external_helper_v1(tcx, trait_id).unwrap());
            }
            let mut rejected = 0;
            for definition in tcx.iter_local_def_id().map(|id| id.to_def_id()) {
                if matches!(
                    tcx.def_kind(definition),
                    DefKind::Fn | DefKind::Struct | DefKind::Trait
                ) {
                    assert!(nominal_trait_path(tcx, definition).is_err());
                    rejected += 1;
                }
            }
            assert_eq!(rejected, 3);
            self.0 = true;
            Compilation::Stop
        }
    }

    #[test]
    fn core_trait_identity_does_not_reassign_implementation_source_ownership() {
        let sysroot = std::process::Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .unwrap();
        assert!(sysroot.status.success());
        let args = vec![
            "rustc".into(),
            "--crate-name=provider_trait_identity".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "-Zno-codegen".into(),
            "--sysroot".into(),
            String::from_utf8(sysroot.stdout).unwrap().trim().into(),
            "-".into(),
        ];
        let mut probe = Probe(false);
        rustc_driver::run_compiler(&args, &mut probe);
        assert!(probe.0);
    }
}
