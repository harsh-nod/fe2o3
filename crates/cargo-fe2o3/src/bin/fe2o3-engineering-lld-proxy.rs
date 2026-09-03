//! Minimal argv-preserving proxy from a compiler driver to a sealed GNU lld image.

use std::ffi::{CString, OsString};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;

const LLD_CHILD_FD: std::os::fd::RawFd = 208;
const MAX_ARGUMENTS: usize = 65_536;
const MAX_ARGUMENT_BYTES: usize = 8 * 1024 * 1024;

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    let arguments = match proxy_arguments(&arguments) {
        Ok(arguments) => arguments,
        Err(error) => {
            eprintln!("fe2o3-engineering-lld-proxy: {error}");
            return ExitCode::from(125);
        }
    };
    let mut pointers: Vec<*mut libc::c_char> = arguments
        .iter()
        .map(|argument| argument.as_ptr().cast_mut())
        .collect();
    pointers.push(std::ptr::null_mut());
    let empty = c"";

    unsafe extern "C" {
        static mut environ: *mut *mut libc::c_char;
    }
    // SAFETY: every argument is a live NUL-terminated CString, both pointer arrays are
    // NUL-terminated, and FD 208 is supplied by the engineering parent as an executable image.
    let result = unsafe {
        libc::execveat(
            LLD_CHILD_FD,
            empty.as_ptr(),
            pointers.as_ptr(),
            environ.cast_const(),
            libc::AT_EMPTY_PATH,
        )
    };
    debug_assert_eq!(result, -1);
    let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1);
    eprintln!("fe2o3-engineering-lld-proxy: execveat FD 208 failed with errno {errno}");
    ExitCode::from(127)
}

fn proxy_arguments(arguments: &[OsString]) -> Result<Vec<CString>, String> {
    let count = arguments
        .len()
        .checked_add(1)
        .filter(|count| *count <= MAX_ARGUMENTS)
        .ok_or_else(|| "linker argument count exceeds the fixed bound".to_owned())?;
    let mut bytes = b"ld.lld".len() + 1;
    let mut output = Vec::with_capacity(count);
    output.push(CString::new("ld.lld").expect("fixed argv[0] contains no NUL"));
    for argument in arguments {
        bytes = bytes
            .checked_add(argument.as_bytes().len() + 1)
            .filter(|bytes| *bytes <= MAX_ARGUMENT_BYTES)
            .ok_or_else(|| "linker argument bytes exceed the fixed bound".to_owned())?;
        output.push(
            CString::new(argument.as_bytes())
                .map_err(|_| "linker argument contains an interior NUL".to_owned())?,
        );
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_only_argv_zero_and_preserves_remaining_bytes() {
        let input = [
            OsString::from("--version"),
            OsString::from("-z"),
            OsString::from("now"),
        ];
        let output = proxy_arguments(&input).unwrap();
        assert_eq!(output[0].as_bytes(), b"ld.lld");
        assert_eq!(output[1].as_bytes(), b"--version");
        assert_eq!(output[2].as_bytes(), b"-z");
        assert_eq!(output[3].as_bytes(), b"now");
    }

    #[test]
    fn rejects_oversized_argument_bytes() {
        let input = [OsString::from("x".repeat(MAX_ARGUMENT_BYTES))];
        assert!(proxy_arguments(&input).is_err());
    }
}
