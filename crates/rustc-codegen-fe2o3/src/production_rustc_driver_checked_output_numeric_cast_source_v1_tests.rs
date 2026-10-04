use super::*;
use simulation::numeric_cast::{Integer, OperationCase};

pub(super) struct Config {
    pub(super) operation: OperationCase,
    pub(super) retained: bool,
}

impl Config {
    pub(super) fn name(&self) -> String {
        format!(
            "{}-{}",
            if self.retained { "retained" } else { "normal" },
            self.operation.name()
        )
    }

    pub(super) fn configure(&self, args: &mut Vec<String>) {
        args.push(format!(
            "--cfg=feature=\"numeric-cast-{}\"",
            self.operation.integer.name()
        ));
        if self.operation.to_integer {
            args.push("--cfg=feature=\"numeric-cast-to-int\"".into());
        }
        if self.retained {
            args.push("--cfg=feature=\"numeric-cast-retained\"".into());
            args.push("-Zinline-mir=no".into());
        }
    }
}

const PROFILES: [fe2o3_amd_target::ProductionAmdTargetProfileV1; 2] = [
    fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
];

fn cases() -> Vec<OrdinarySourceCase> {
    let mut cases = Vec::new();
    for integer in Integer::ALL {
        for to_integer in [false, true] {
            for retained in [false, true] {
                cases.push(OrdinarySourceCase::NumericCast(Config {
                    operation: OperationCase {
                        integer,
                        to_integer,
                    },
                    retained,
                }));
            }
        }
    }
    cases
}

#[test]
fn numeric_cast_source_matrix_has_64_unique_targeted_configurations() {
    let cases = cases();
    assert_eq!(cases.len(), 32);
    let mut seen = std::collections::BTreeSet::new();
    for profile in PROFILES {
        for case in &cases {
            let OrdinarySourceCase::NumericCast(config) = case else {
                panic!("numeric cast matrix contains a different fixture");
            };
            assert!(seen.insert((profile.cpu(), config.name())));
        }
        assert_eq!(
            seen.iter().filter(|(cpu, _)| *cpu == profile.cpu()).count(),
            32
        );
    }
    assert_eq!(seen.len(), 64);
}

#[test]
#[ignore = "requires pinned rust-src, AMD dependencies and ordinary-source numeric cast qualification"]
fn ordinary_rust_numeric_casts_all_widths_reach_actual_o_and_cpu_oracle() {
    let cases = cases();
    for profile in PROFILES {
        ordinary_rust_checked_output_cases_for_profile(&cases, profile);
    }
}
