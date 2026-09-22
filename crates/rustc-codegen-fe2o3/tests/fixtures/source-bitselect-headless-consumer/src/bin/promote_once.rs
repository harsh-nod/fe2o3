//! Fixed-profile normal-dependency fixture, not a public compiler command.
//!
//! ORIGINAL CANDIDATE LOWERCASE_SHA256 -- COMPLETE_RUSTC_ARGV
//! The sole positive profile uses scratch v4, output v5, and inputs v0/v1/v2.
//! Only the public driver's successful return can produce the observation line.
#![feature(rustc_private)]

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Write as _;

use fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1;
use rustc_codegen_fe2o3::{BitselectPromotionRequestV1, run_bitselect_source_promotion_driver_v1};

const RUSTC_ARG_CAP: usize = 4096;
const RUSTC_BYTE_CAP: usize = 1024 * 1024;
const ENVELOPE_BYTE_CAP: usize = 2 * 1024 + 64 + 2;
const PREFIX: &str = "FE2O3_HEADLESS_NORMAL_PUBLISHED ";

fn arguments(
    raw: impl IntoIterator<Item = OsString>,
) -> Result<(BitselectPromotionRequestV1, Vec<String>), &'static str> {
    let mut args = Vec::new();
    let mut bytes = 0usize;
    for value in raw {
        if args.len() == RUSTC_ARG_CAP + 4 {
            return Err("fixture argument count exceeds its closed bound");
        }
        bytes = bytes
            .checked_add(value.len())
            .ok_or("argument byte overflow")?;
        if bytes > RUSTC_BYTE_CAP + ENVELOPE_BYTE_CAP {
            return Err("fixture argument bytes exceed its closed bound");
        }
        args.push(
            value
                .into_string()
                .map_err(|_| "fixture arguments must be UTF-8")?,
        );
    }
    if args.len() < 5 || args[3] != "--" {
        return Err("expected ORIGINAL CANDIDATE LOWERCASE_SHA256 -- COMPLETE_RUSTC_ARGV");
    }
    let digest = digest(&args[2])?;
    let registers = Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2])
        .expect("fixed distinct low-register profile");
    let request = BitselectPromotionRequestV1::new(&args[0], &args[1], digest, registers)
        .map_err(|_| "fixture source paths are invalid")?;
    let rustc_args = args.split_off(4);
    // The normal API independently repeats these bounds before source access.
    if rustc_args.iter().map(String::len).sum::<usize>() > RUSTC_BYTE_CAP {
        return Err("fixture rustc arguments exceed their byte bound");
    }
    Ok((request, rustc_args))
}

fn digest(text: &str) -> Result<[u8; 32], &'static str> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("expected exactly 64 lowercase hexadecimal revision bytes");
    }
    let mut result = [0; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| "invalid revision byte")?;
    }
    Ok(result)
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut result = String::with_capacity(64);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").unwrap();
    }
    result
}

fn run() -> Result<(), String> {
    let (request, args) = arguments(std::env::args_os().skip(1)).map_err(str::to_owned)?;
    let attempt = run_bitselect_source_promotion_driver_v1(&args, request);
    let (_, result) = attempt.into_parts();
    let published = result.map_err(|error| {
        format!(
            "phase={:?} publication={:?} compiler_fatal={} diagnostic={:?}",
            error.phase(),
            error.publication(),
            error.compiler_fatal(),
            error.diagnostic()
        )
    })?;
    let report = format!(
        "{PREFIX}{} {} {}\n",
        hex(published.original_sha256()),
        hex(published.candidate_sha256()),
        published.candidate_bytes()
    );
    assert!(report.len() <= 256);
    std::io::stdout()
        .lock()
        .write_all(report.as_bytes())
        .map_err(|error| {
            // Do not delete or retry a candidate after any attempted publication.
            format!("candidate was published but observation write failed: {error}")
        })
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("headless normal fixture refused: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> Vec<OsString> {
        [
            "original.rs",
            "candidate.rs",
            &"07".repeat(32),
            "--",
            "rustc",
        ]
        .into_iter()
        .map(OsString::from)
        .collect()
    }

    #[test]
    fn fixed_request_preserves_complete_rustc_arguments() {
        let (request, args) = arguments(valid()).unwrap();
        assert_eq!(request.original_path(), "original.rs");
        assert_eq!(request.candidate_path(), "candidate.rs");
        assert_eq!(request.expected_original_sha256(), &[7; 32]);
        assert_eq!(
            request.registers(),
            Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2]).unwrap()
        );
        assert_eq!(args, ["rustc"]);
        assert_eq!(hex(&[7; 32]), "07".repeat(32));
    }

    #[test]
    fn envelope_and_revision_refusals_precede_driver() {
        assert!(arguments(Vec::<OsString>::new()).is_err());
        for (index, replacement) in [(0, "../escape.rs"), (1, "original.rs"), (2, "AA"), (3, "-")] {
            let mut args = valid();
            args[index] = replacement.into();
            assert!(arguments(args).is_err());
        }
        assert!(digest(&"AA".repeat(32)).is_err());
        let mut args = valid();
        args.extend(std::iter::repeat_n(OsString::from(""), RUSTC_ARG_CAP));
        assert!(arguments(args).is_err());
        let mut args = valid();
        args[4] = "a".repeat(RUSTC_BYTE_CAP + 1).into();
        assert!(arguments(args).is_err());
    }
}
