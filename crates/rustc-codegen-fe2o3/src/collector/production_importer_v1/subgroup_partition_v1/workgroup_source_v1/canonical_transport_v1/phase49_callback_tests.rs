//! Child of the existing registered Workgroup import test harness. It keeps
//! that harness's real provider/registration, cwd and scratch output handling.
use super::*;
use crate::collector::production_importer_v1::reusable_phase_v26::source_tests;

#[path = "phase49_duplicate_rust_tests.rs"]
mod duplicate_rust_tests;

#[path = "phase67_emission_tests.rs"]
mod emission_tests;

#[path = "phase100_emission_tests.rs"]
mod three_phase_emission_tests;

struct PhaseProbe {
    require_ssa: bool,
    require_emission: bool,
    rejection: Option<&'static str>,
    completed: bool,
}
impl Callbacks for PhaseProbe {
    fn config(&mut self, config: &mut Config) {
        let source=match self.rejection {
            Some("phase owner, phase, storage or lease escaped its exact source protocol")=>source_tests::SOURCE.replacen(
                "let _lease = phase.bind_reusable_lds(&mut storage);",
                "let lease = phase.bind_reusable_lds(&mut storage); core::mem::drop(lease);",1),
            None=>source_tests::SOURCE.into(),
            Some(_)=>panic!("closed fixture rejection roster"),
        };
        config.input = Input::Str {
            name: FileName::Custom("phase49_actual_source.rs".into()),
            input: source,
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            crate::collector::session_crate_binding(tcx),
            Some(registration_binding_v1())
        );
        if self.require_emission {
            assert!(self.require_ssa && self.rejection.is_none());
            source_tests::check_emission(tcx);
        } else {
            source_tests::check(tcx, self.require_ssa,self.rejection);
        }
        self.completed = true;
        Compilation::Stop
    }
}
fn probe(test: &str, cpu: &'static str, require_ssa: bool, rejection: Option<&'static str>) {
    probe_with_emission(test, cpu, require_ssa, rejection, false);
}

fn probe_with_emission(test: &str, cpu: &'static str, require_ssa: bool,
    rejection: Option<&'static str>, require_emission: bool) {
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
        let mut probe = PhaseProbe {
            require_ssa,
            require_emission,
            rejection,
            completed: false,
        };
        rustc_driver::run_compiler(&args, &mut probe);
        assert!(probe.completed, "all actual phase assertions must execute");
    });
}
macro_rules! case {
    ($name:ident, $cpu:literal, $ssa:literal) => {
        #[test]
        #[ignore = "requires cached authenticated AMD metadata; never runs Cargo"]
        fn $name() {
            probe(
                concat!(module_path!(), "::", stringify!($name))
                    .strip_prefix("rustc_codegen_fe2o3::")
                    .unwrap(),
                $cpu,
                $ssa,
                None,
            );
        }
    };
}
case!(phase49_two_phase_source_carriage_gfx942, "gfx942", false);
case!(phase49_two_phase_source_carriage_gfx950, "gfx950", false);
case!(phase49_two_phase_source_ssa_gfx942, "gfx942", true);
case!(phase49_two_phase_source_ssa_gfx950, "gfx950", true);
macro_rules! negative {
    ($name:ident,$cpu:literal,$detail:literal)=> {
        #[test]
        #[ignore="requires cached authenticated AMD metadata; never runs Cargo"]
        fn $name() {
            probe(concat!(module_path!(),"::",stringify!($name)).strip_prefix("rustc_codegen_fe2o3::").unwrap(),
                $cpu,false,Some($detail));
        }
    };
}
negative!(phase49_lease_escape_source_gfx942,"gfx942","phase owner, phase, storage or lease escaped its exact source protocol");
negative!(phase49_lease_escape_source_gfx950,"gfx950","phase owner, phase, storage or lease escaped its exact source protocol");
