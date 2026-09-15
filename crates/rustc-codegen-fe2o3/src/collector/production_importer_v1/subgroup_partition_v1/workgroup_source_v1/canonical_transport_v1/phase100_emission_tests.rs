//! Independent three-phase input through the existing registered rustc harness.
use super::*;
use source_tests::three_phase_source_tests as three;

struct ThreePhaseProbe {
    completed: bool,
}

impl Callbacks for ThreePhaseProbe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("phase100_three_phase_source.rs".into()),
            input: three::SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            crate::collector::session_crate_binding(tcx),
            Some(registration_binding_v1())
        );
        three::check(tcx);
        self.completed = true;
        Compilation::Stop
    }
}

fn probe(test: &str, cpu: &'static str) {
    run(test, |original| {
        let mut args = original.to_vec();
        let mut replaced = 0;
        for arg in &mut args {
            if arg == "-Ctarget-cpu=gfx950" {
                *arg = format!("-Ctarget-cpu={cpu}");
                replaced += 1;
            }
        }
        assert_eq!(replaced, 1);
        let mut probe = ThreePhaseProbe { completed: false };
        rustc_driver::run_compiler(&args, &mut probe);
        assert!(probe.completed, "all three-phase source/KIR assertions ran");
    });
}

macro_rules! emission {
    ($name:ident, $cpu:literal) => {
        #[test]
        #[ignore = "requires cached authenticated AMD metadata; never runs Cargo"]
        fn $name() {
            probe(
                concat!(module_path!(), "::", stringify!($name))
                    .strip_prefix("rustc_codegen_fe2o3::")
                    .unwrap(),
                $cpu,
            );
        }
    };
}

emission!(phase100_three_phase_source_kir_gfx942, "gfx942");
emission!(phase100_three_phase_source_kir_gfx950, "gfx950");
