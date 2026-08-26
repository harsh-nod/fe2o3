use std::ffi::OsString;

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

    #[cfg(not(any(test, feature = "qualification-oracles-test-only")))]
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

        let separator = args.iter().position(|argument| argument == "--");
        if command == "build" && separator.is_some() {
            return Err("cargo fe2o3 build does not accept arguments after `--`".to_owned());
        }
        let cargo_args_end = separator.unwrap_or(args.len());

        let mut device_args = args[..cargo_args_end].to_vec();
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

    #[cfg(not(any(test, feature = "qualification-oracles-test-only")))]
    pub(crate) fn host_mut(&mut self) -> &mut CargoPhase {
        &mut self.host
    }
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
                "--bin",
                "app",
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
