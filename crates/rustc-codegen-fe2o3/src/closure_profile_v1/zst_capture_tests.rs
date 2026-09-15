// These are compiler custody components, not capability issuers or launch proofs.
#[test]
fn optimized_captured_zst_constant_remains_rejected_without_environment_custody() {
    #[derive(Default)]
    struct Check {
        completed: bool,
    }
    impl Callbacks for Check {
        fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            let caller = Instance::mono(tcx, local_function(tcx, "constant_captured_zst"));
            let body = tcx.instance_mir(caller.def);
            let argument = body
                .basic_blocks
                .iter()
                .find_map(|block| {
                    let TerminatorKind::Call {
                        func: Operand::Constant(function),
                        args,
                        ..
                    } = &block.terminator().kind
                    else {
                        return None;
                    };
                    let TyKind::FnDef(definition, _) = function.const_.ty().kind() else {
                        return None;
                    };
                    (tcx.item_name(*definition).as_str() == "host_ref_apply").then(|| &args[0])
                })
                .expect("original higher-order call");
            let Operand::Constant(value) = &argument.node else {
                panic!("optimized ZST capture must actually be constant")
            };
            let closure = value.const_.ty();
            let TyKind::Closure(_, args) = closure.kind() else {
                panic!("concrete closure")
            };
            assert_eq!(args.as_closure().upvar_tys().len(), 1);
            assert!(!closure.needs_drop(tcx, TypingEnv::fully_monomorphized()));
            let layout = LayoutCx::new(tcx, TypingEnv::fully_monomorphized())
                .layout_of(closure)
                .unwrap();
            assert_eq!(layout.size.bytes(), 0);
            assert_eq!(layout.fields.count(), 1);
            let error = CollectedClosureCustodyV1::default()
                .analyze(tcx, caller, false, "gfx950")
                .unwrap_err();
            assert_eq!(
                error.to_string(),
                "production closure profile rejected MIR: only a zero-capture, no-drop closure may use constant custody"
            );
            self.completed = true;
            Compilation::Stop
        }
    }
    let mut check = Check::default();
    run_fixture(&mut check, 3);
    assert!(check.completed);
}

#[test]
fn optimized_runtime_gated_zst_capture_has_exact_live_environment_transfer() {
    #[derive(Default)]
    struct Check {
        completed: bool,
    }
    impl Callbacks for Check {
        fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            let caller = Instance::mono(tcx, local_function(tcx, "runtime_captured_zst"));
            let mut custody = CollectedClosureCustodyV1::default();
            let plan = custody
                .analyze(tcx, caller, false, "gfx950")
                .expect("retained runtime environment");
            let [call] = plan.transport_calls() else {
                panic!("one transfer")
            };
            let ClosureCustodyV1::EnvironmentLocal(local) = call.closure_custody else {
                panic!("a captured token cannot use constant custody")
            };
            let [environment] = plan.environments() else {
                panic!("one environment")
            };
            assert_eq!(environment.local, local);
            assert_eq!(environment.call_kind, ClosureCallKindV1::FnOnce);
            assert_eq!(environment.size_bytes, 4);
            assert_eq!(environment.captures.len(), 2);
            assert!(
                environment
                    .captures
                    .iter()
                    .all(|capture| capture.mode == ClosureCaptureModeV1::ByValue)
            );
            let mut sizes = environment
                .captures
                .iter()
                .map(|capture| capture.layout.size_bytes)
                .collect::<Vec<_>>();
            sizes.sort_unstable();
            assert_eq!(sizes, [0, 4]);
            custody
                .observe_calls(tcx, caller, Some(&plan))
                .expect("exact caller custody");
            let callee = resolved_call_named(tcx, caller, "host_ref_apply");
            let received = custody
                .analyze(tcx, callee, true, "gfx950")
                .expect("exact callee custody");
            assert_eq!(
                received.environments()[0].closure_type_identity,
                environment.closure_type_identity
            );
            assert_eq!(received.environments()[0].captures, environment.captures);
            let mut changed = plan.clone();
            changed.transport_calls[0].closure_custody =
                ClosureCustodyV1::EnvironmentLocal(local + 1);
            assert!(custody.observe_calls(tcx, caller, Some(&changed)).is_err());
            self.completed = true;
            Compilation::Stop
        }
    }
    let mut check = Check::default();
    run_fixture(&mut check, 3);
    assert!(check.completed);
}
