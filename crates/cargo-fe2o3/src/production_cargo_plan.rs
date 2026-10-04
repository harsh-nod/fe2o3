use std::ffi::OsString;

const PRODUCTION_DEVICE_BUILD_STD_V1: &str = "-Zbuild-std=core";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CargoPhase {
    command: &'static str,
    args: Vec<OsString>,
}

impl CargoPhase {
    pub(crate) const fn command(&self) -> &'static str {
        self.command
    }

    pub(crate) fn args(&self) -> &[OsString] {
        &self.args
    }

    pub(crate) fn args_mut(&mut self) -> &mut Vec<OsString> {
        &mut self.args
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProductionCargoPlan {
    device: CargoPhase,
    host: CargoPhase,
}

impl ProductionCargoPlan {
    pub(crate) fn new(
        command: &str,
        args: &[OsString],
        host_target: &str,
        locked: bool,
    ) -> Result<Self, String> {
        if !matches!(command, "build" | "run") {
            return Err(format!(
                "production Cargo plan does not support command {command:?}"
            ));
        }
        validate_target(host_target, "host rustc target")?;
        crate::reject_caller_target(args)?;
        reject_caller_build_std(args)?;

        let separator = args.iter().position(|argument| argument == "--");
        if command == "build" && separator.is_some() {
            return Err("cargo fe2o3 build does not accept arguments after `--`".to_owned());
        }
        let cargo_args_end = separator.unwrap_or(args.len());

        let mut device_args = if command == "run" {
            device_library_args(&args[..cargo_args_end])?
        } else {
            args[..cargo_args_end].to_vec()
        };
        device_args.push(OsString::from(PRODUCTION_DEVICE_BUILD_STD_V1));
        append_target(
            &mut device_args,
            fe2o3_amd_target::PRODUCTION_GFX942_RUSTC_TARGET_V1,
        );
        insert_locked_flags(&mut device_args, locked);

        let mut host_args = args.to_vec();
        insert_target(&mut host_args, host_target);
        insert_locked_flags(&mut host_args, locked);

        Ok(Self {
            device: CargoPhase {
                command: "build",
                args: device_args,
            },
            host: CargoPhase {
                command: if command == "run" { "run" } else { "build" },
                args: host_args,
            },
        })
    }

    pub(crate) const fn device(&self) -> &CargoPhase {
        &self.device
    }

    pub(crate) const fn host(&self) -> &CargoPhase {
        &self.host
    }

    pub(crate) fn host_mut(&mut self) -> &mut CargoPhase {
        &mut self.host
    }
}

fn device_library_args(args: &[OsString]) -> Result<Vec<OsString>, String> {
    let mut device = Vec::with_capacity(args.len() + 1);
    let mut selected_bin = false;
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        let bytes = crate::os_bytes(argument);
        if bytes == b"--bin" || bytes.starts_with(b"--bin=") {
            if selected_bin {
                return Err("cargo fe2o3 run accepts only one host --bin selection".to_owned());
            }
            selected_bin = true;
            let name = if bytes == b"--bin" {
                index += 1;
                crate::os_bytes(args.get(index).ok_or("--bin requires a host binary name")?)
            } else {
                &bytes[b"--bin=".len()..]
            };
            if name.is_empty() || name.starts_with(b"-") {
                return Err("--bin requires a nonempty host binary name".to_owned());
            }
        } else {
            for selector in [
                "--lib",
                "--bins",
                "--example",
                "--examples",
                "--test",
                "--tests",
                "--bench",
                "--benches",
                "--all-targets",
            ] {
                if bytes == selector.as_bytes()
                    || bytes
                        .strip_prefix(selector.as_bytes())
                        .is_some_and(|tail| tail.starts_with(b"="))
                {
                    return Err(format!(
                        "cargo fe2o3 run compiles the package library for the device and selects a host binary; {selector} is not supported"
                    ));
                }
            }
            device.push(argument.clone());
        }
        index += 1;
    }
    device.push(OsString::from("--lib"));
    Ok(device)
}

fn reject_caller_build_std(args: &[OsString]) -> Result<(), String> {
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--" {
            return Ok(());
        }
        if argument == "-Z" {
            let value = args
                .get(index + 1)
                .ok_or_else(|| "Cargo -Z requires an argument".to_owned())?;
            if value == "build-std" || crate::os_bytes(value).starts_with(b"build-std=") {
                return Err(
                    "cargo fe2o3 owns the production device build-std selection; remove caller -Zbuild-std"
                        .to_owned(),
                );
            }
            index += 1;
        } else {
            let bytes = crate::os_bytes(argument);
            if bytes == b"-Zbuild-std" || bytes.starts_with(b"-Zbuild-std=") {
                return Err(
                    "cargo fe2o3 owns the production device build-std selection; remove caller -Zbuild-std"
                        .to_owned(),
                );
            }
        }
        index += 1;
    }
    Ok(())
}

fn validate_target(target: &str, label: &str) -> Result<(), String> {
    if target.is_empty()
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(format!(
            "{label} is not a canonical Cargo target: {target:?}"
        ));
    }
    Ok(())
}

fn append_target(args: &mut Vec<OsString>, target: &str) {
    args.push(OsString::from("--target"));
    args.push(OsString::from(target));
}

fn insert_target(args: &mut Vec<OsString>, target: &str) {
    let position = separator_position(args);
    args.insert(position, OsString::from("--target"));
    args.insert(position + 1, OsString::from(target));
}

fn insert_locked_flags(args: &mut Vec<OsString>, locked: bool) {
    if !locked {
        return;
    }
    let mut position = separator_position(args);
    for required in ["--offline", "--frozen"] {
        if !args[..position].iter().any(|argument| argument == required) {
            args.insert(position, OsString::from(required));
            position += 1;
        }
    }
}

fn separator_position(args: &[OsString]) -> usize {
    args.iter()
        .position(|argument| argument == "--")
        .unwrap_or(args.len())
}

#[cfg(test)]
mod tests {
    use super::ProductionCargoPlan;
    use crate::reject_caller_target;
    use std::ffi::OsString;

    fn strings(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn build_has_one_fixed_device_phase_then_one_fixed_host_phase() {
        let plan = ProductionCargoPlan::new(
            "build",
            &strings(&["--package", "kernel", "--release"]),
            "x86_64-unknown-linux-gnu",
            false,
        )
        .unwrap();

        assert_eq!(plan.device().command(), "build");
        assert_eq!(
            plan.device().args(),
            strings(&[
                "--package",
                "kernel",
                "--release",
                "-Zbuild-std=core",
                "--target",
                "amdgcn-amd-amdhsa",
            ])
        );
        assert_eq!(plan.host().command(), "build");
        assert_eq!(
            plan.host().args(),
            strings(&[
                "--package",
                "kernel",
                "--release",
                "--target",
                "x86_64-unknown-linux-gnu",
            ])
        );
    }

    #[test]
    fn run_arguments_exist_only_in_the_host_phase() {
        let plan = ProductionCargoPlan::new(
            "run",
            &strings(&["--bin", "app", "--", "input", "--target=application-data"]),
            "x86_64-unknown-linux-gnu",
            true,
        )
        .unwrap();

        assert_eq!(plan.device().command(), "build");
        assert_eq!(
            plan.device().args(),
            strings(&[
                "--lib",
                "-Zbuild-std=core",
                "--target",
                "amdgcn-amd-amdhsa",
                "--offline",
                "--frozen",
            ])
        );
        assert_eq!(plan.host().command(), "run");
        assert_eq!(
            plan.host().args(),
            strings(&[
                "--bin",
                "app",
                "--target",
                "x86_64-unknown-linux-gnu",
                "--offline",
                "--frozen",
                "--",
                "input",
                "--target=application-data",
            ])
        );
    }

    #[test]
    fn run_selects_device_library_without_changing_host_binary_arguments() {
        for selection in [vec![], strings(&["--bin", "app"]), strings(&["--bin=app"])] {
            let mut args = strings(&["--package", "kernels", "--features", "fill", "--release"]);
            args.extend(selection);
            args.extend(strings(&[
                "--",
                "--bin",
                "application-data",
                "--example=test",
            ]));
            let plan =
                ProductionCargoPlan::new("run", &args, "x86_64-unknown-linux-gnu", false).unwrap();
            assert_eq!(
                plan.device().args(),
                strings(&[
                    "--package",
                    "kernels",
                    "--features",
                    "fill",
                    "--release",
                    "--lib",
                    "-Zbuild-std=core",
                    "--target",
                    "amdgcn-amd-amdhsa"
                ])
            );
            let mut expected_host = args.clone();
            let separator = expected_host.iter().position(|arg| arg == "--").unwrap();
            expected_host.splice(
                separator..separator,
                strings(&["--target", "x86_64-unknown-linux-gnu"]),
            );
            assert_eq!(plan.host().args(), expected_host);
        }
        let plan = ProductionCargoPlan::new(
            "build",
            &strings(&["--lib"]),
            "x86_64-unknown-linux-gnu",
            false,
        )
        .unwrap();
        assert_eq!(
            plan.device().args(),
            strings(&["--lib", "-Zbuild-std=core", "--target", "amdgcn-amd-amdhsa"])
        );
    }

    #[test]
    fn run_rejects_malformed_or_conflicting_host_target_selections() {
        for args in [
            vec!["--bin"],
            vec!["--bin="],
            vec!["--bin", ""],
            vec!["--bin", "--release"],
            vec!["--bin", "a", "--bin=b"],
            vec!["--lib"],
            vec!["--bins"],
            vec!["--example", "app"],
            vec!["--example=app"],
            vec!["--examples"],
            vec!["--test", "app"],
            vec!["--test=app"],
            vec!["--tests"],
            vec!["--bench", "app"],
            vec!["--bench=app"],
            vec!["--benches"],
            vec!["--all-targets"],
        ] {
            assert!(
                ProductionCargoPlan::new("run", &strings(&args), "x86_64-unknown-linux-gnu", false)
                    .is_err(),
                "{args:?}"
            );
        }
    }

    #[test]
    fn caller_target_selection_and_build_payloads_are_rejected() {
        for args in [
            strings(&["--target", "amdgcn-amd-amdhsa"]),
            strings(&["--target=x86_64-unknown-linux-gnu"]),
        ] {
            assert!(reject_caller_target(&args).is_err());
        }
        assert!(
            ProductionCargoPlan::new(
                "build",
                &strings(&["--release", "--", "payload"]),
                "x86_64-unknown-linux-gnu",
                false,
            )
            .is_err()
        );
        for args in [
            strings(&["-Zbuild-std=std"]),
            strings(&["-Z", "build-std=core,alloc"]),
        ] {
            assert!(
                ProductionCargoPlan::new("build", &args, "x86_64-unknown-linux-gnu", false,)
                    .is_err()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_cargo_and_application_arguments_are_preserved() {
        use std::os::unix::ffi::OsStringExt as _;

        let cargo = OsString::from_vec(b"feature-\xff".to_vec());
        let application = OsString::from_vec(b"payload-\xfe".to_vec());
        let plan = ProductionCargoPlan::new(
            "run",
            &[
                OsString::from("--features"),
                cargo.clone(),
                OsString::from("--"),
                application.clone(),
            ],
            "x86_64-unknown-linux-gnu",
            false,
        )
        .unwrap();

        assert!(plan.device().args().contains(&cargo));
        assert!(!plan.device().args().contains(&application));
        assert!(plan.host().args().contains(&cargo));
        assert!(plan.host().args().contains(&application));
    }
}
