use fe2o3_compiler_execution_deployment::{
    encode_sha256_lower_hex_v1, install_compiler_runtime_deployment_v1,
    verify_compiler_runtime_deployment_v1,
};
use std::{ffi::OsStr, path::Path};

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
    let args: Vec<_> = std::env::args_os().take(8).collect();
    let install = args.get(1).is_some_and(|value| value == "install");
    let verify = args.get(1).is_some_and(|value| value == "verify");
    if (!install && !verify) || args.len() != if install { 6 } else { 5 } {
        return Err("usage: fe2o3-compiler-runtime-deployment verify|install BUNDLE POLICY_SHA256 MANIFEST_SHA256 [OFFLINE_ROOT]".into());
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
}
