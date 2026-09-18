use super::*;
use simulation::saturating_integer::{Integer, OperationCase};

pub(super) struct Config {
    pub(super) operation: OperationCase,
    pub(super) retained: bool,
}

impl Config {
    pub(super) fn name(&self) -> String {
        format!(
            "{}-{}-{}",
            if self.retained { "retained" } else { "normal" },
            self.operation.integer.rust_name(),
            if self.operation.subtract {
                "saturating-sub"
            } else {
                "saturating-add"
            }
        )
    }

    pub(super) fn configure(&self, args: &mut Vec<String>) {
        args.push(format!(
            "--cfg=feature=\"sat-{}\"",
            self.operation.integer.rust_name()
        ));
        if self.operation.subtract {
            args.push("--cfg=feature=\"sat-sub\"".into());
        }
        if self.retained {
            args.push("-Zinline-mir=no".into());
        }
    }

    pub(super) fn check(&self, result: &Observation) {
        assert_eq!(
            dispatch::check_private_helper_route(result, false).unwrap(),
            dispatch::Route::DirectRawEmpty
        );
        assert_eq!(result.internal_helpers, usize::from(self.retained));
        assert_eq!(result.helper_calls, usize::from(self.retained));
        eprintln!(
            "ordinary saturation {}: actual O helpers={}, calls={}; test-only -Zinline-mir=no={}",
            self.name(),
            result.internal_helpers,
            result.helper_calls,
            self.retained
        );
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD dependencies, typed saturation and ordinary-source compilation"]
fn ordinary_rust_saturation_all_integer_widths_reaches_actual_o() {
    let mut cases = Vec::new();
    for integer in Integer::ALL {
        for subtract in [false, true] {
            for retained in [false, true] {
                cases.push(OrdinarySourceCase::SaturatingInteger(Config {
                    operation: OperationCase { integer, subtract },
                    retained,
                }));
            }
        }
    }
    ordinary_rust_checked_output_cases(&cases);
}
