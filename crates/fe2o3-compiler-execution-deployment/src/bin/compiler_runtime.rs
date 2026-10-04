use fe2o3_compiler_execution_deployment::{
    CompilerRuntimePackagePlanV1, encode_sha256_lower_hex_v1,
    install_compiler_runtime_deployment_v1, package_compiler_runtime_deployment_v1,
    verify_compiler_runtime_deployment_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{ffi::OsStr, path::Path};

const USAGE: &str = "usage: fe2o3-compiler-runtime-deployment verify|install BUNDLE POLICY_SHA256 MANIFEST_SHA256 [OFFLINE_ROOT]\n       fe2o3-compiler-runtime-deployment package RECIPE SOURCE_ROOT PROFILE_ROOT DESTINATION WORK STORAGE";

fn pin(value: &OsStr) -> Result<[u8; 32], &'static str> {
    let value = value.to_str().ok_or("pin must be lowercase hexadecimal")?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("pin must be exactly 64 lowercase hexadecimal characters");
    }
    let mut result = [0; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte =
            u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| "invalid pin")?;
    }
    Ok(result)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().take(9).collect();
    if args.get(1).is_some_and(|value| value == "package") {
        return package(&args);
    }
    let install = args.get(1).is_some_and(|value| value == "install");
    let verify = args.get(1).is_some_and(|value| value == "verify");
    if (!install && !verify) || args.len() != if install { 6 } else { 5 } {
        return Err(USAGE.into());
    }
    let verified =
        verify_compiler_runtime_deployment_v1(Path::new(&args[2]), pin(&args[3])?, pin(&args[4])?)?;
    if install {
        let installed = install_compiler_runtime_deployment_v1(verified, Path::new(&args[5]))?;
        println!(
            "installed_policy_sha256={}",
            encode_sha256_lower_hex_v1(installed.policy_sha256())
        );
        println!(
            "installed_manifest_sha256={}",
            encode_sha256_lower_hex_v1(installed.manifest_sha256())
        );
        println!("installed_file_count={}", installed.file_count());
    } else {
        println!(
            "verified_file_count={}",
            verified.manifest().entries().len()
        );
    }
    println!("compiler_execution_authority=false");
    Ok(())
}

fn allowance(value: &OsStr) -> Result<usize, &'static str> {
    let text = value
        .to_str()
        .ok_or("allowance must be canonical positive decimal")?;
    if text.is_empty()
        || text.len() > 20
        || text.starts_with('0')
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("allowance must be canonical positive decimal");
    }
    text.parse().map_err(|_| "allowance exceeds usize")
}

fn package(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() != 8 {
        return Err(USAGE.into());
    }
    let mut work = Work::new(allowance(&args[6])?);
    let mut budget = Budget::new(&mut work, allowance(&args[7])?);
    let plan = CompilerRuntimePackagePlanV1::read(Path::new(&args[2]), &mut budget)?;
    budget.reserve_storage(CompilerRuntimePackagePlanV1::STORAGE)?;
    let quote = plan.quota()?;
    let result = package_compiler_runtime_deployment_v1(
        &plan,
        Path::new(&args[3]),
        Path::new(&args[4]),
        Path::new(&args[5]),
        &mut budget,
    )?;
    println!("release_approval=UNAPPROVED");
    println!(
        "unapproved_policy_sha256={}",
        encode_sha256_lower_hex_v1(result.policy_sha256())
    );
    println!(
        "unapproved_manifest_sha256={}",
        encode_sha256_lower_hex_v1(result.manifest_sha256())
    );
    println!("packaged_file_count={}", result.file_count());
    println!("package_work_quote={}", quote.work());
    println!(
        "work_used={} peak_storage={}",
        budget.work(),
        budget.peak_storage()
    );
    println!("compiler_execution_authority=false");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("compiler runtime deployment: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    #[test]
    fn pins_require_exact_lowercase_hex() {
        assert_eq!(pin(OsStr::new(&"ab".repeat(32))).unwrap(), [0xab; 32]);
        for value in [
            "",
            &"0".repeat(63),
            &"0".repeat(65),
            &"AB".repeat(32),
            &"g0".repeat(32),
        ] {
            assert!(pin(OsStr::new(value)).is_err());
        }
    }

    #[test]
    fn pins_refuse_non_ascii_without_slicing_utf8() {
        assert!(pin(OsStr::from_bytes(&[0xff; 64])).is_err());
        let multibyte = [0xc3, 0xa9].repeat(32);
        assert!(pin(OsStr::from_bytes(&multibyte)).is_err());
    }

    #[test]
    fn package_budget_and_argument_gates_are_closed() {
        assert_eq!(allowance(OsStr::new("123")).unwrap(), 123);
        for text in [
            "",
            "0",
            "01",
            "+1",
            "-1",
            " 1",
            "1 ",
            "18446744073709551616",
        ] {
            assert!(allowance(OsStr::new(text)).is_err());
        }
        assert!(allowance(OsStr::from_bytes(&[0xff])).is_err());
        assert!(USAGE.contains("verify|install"));
        assert!(USAGE.contains("package RECIPE SOURCE_ROOT PROFILE_ROOT DESTINATION WORK STORAGE"));
        for count in [0, 2, 7, 9] {
            let args = vec![std::ffi::OsString::from("unused"); count];
            assert_eq!(package(&args).unwrap_err().to_string(), USAGE);
        }
    }
}
