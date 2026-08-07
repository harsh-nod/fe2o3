use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;
use std::time::Duration;

use fe2o3_artifacts::DigestAlgorithm;

const CORRELATION: &str = "32323232323232323232323232323232";
const COMPLETE_CORRELATION_BYTES: [u8; 16] = [51; 16];
const COMPLETE_CORRELATION: &str = "33333333333333333333333333333333";
const COMPLETE_PROPERTIES: &str = "bounds,address-overflow-freedom,memory-safety,initialization,race-freedom,launch-validity,functional-correctness";

fn main() {
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.len() == 25 {
        authenticated_execution(&arguments);
        return;
    }
    if arguments.get(1).map(String::as_str) == Some("--pipe-holder") {
        std::thread::sleep(Duration::from_millis(1200));
        return;
    }
    if arguments.len() != 11
        || arguments[1] != "--request"
        || arguments[3] != "--result"
        || arguments[5] != "--verifier"
        || arguments[7] != "--solver"
        || arguments[9] != "--timeout-seconds"
    {
        process::exit(97);
    }

    let request = Path::new(&arguments[2]);
    let result = Path::new(&arguments[4]);
    let mode = arguments[6]
        .strip_prefix("/fixture/mode/")
        .unwrap_or("invalid-mode");
    let solver = &arguments[8];
    let timeout = &arguments[10];
    if solver != "/fixture/solver" || timeout.parse::<u32>().is_err() {
        process::exit(96);
    }
    if fs::read(request).map_or(true, |bytes| bytes.is_empty()) {
        process::exit(95);
    }

    match mode {
        "success" => {
            if std::env::vars_os().next().is_some()
                || std::env::current_dir()
                    .map(|directory| directory != Path::new("/"))
                    .unwrap_or(true)
            {
                process::exit(94);
            }
            let mut stdin = [0_u8; 1];
            if io::stdin().read(&mut stdin).unwrap_or(1) != 0 {
                process::exit(93);
            }
            write_result(result, &envelope(CORRELATION, "proved"));
        }
        "failed" => write_result(result, &envelope(CORRELATION, "failed")),
        "exit" => {
            eprint!("bounded failure");
            process::exit(17);
        }
        "signal" => process::abort(),
        "timeout" => std::thread::sleep(Duration::from_secs(10)),
        "inherited-pipe" => {
            spawn_pipe_holder();
            write_result(result, &envelope(CORRELATION, "proved"));
        }
        "stdout-oversize" => {
            io::stdout().write_all(&vec![b'o'; 32 * 1024]).unwrap();
            io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_secs(10));
        }
        "stderr-oversize" => {
            io::stderr().write_all(&vec![b'e'; 32 * 1024]).unwrap();
            io::stderr().flush().unwrap();
            std::thread::sleep(Duration::from_secs(10));
        }
        "nonutf8-result" => write_result(result, b"not utf8: \xff"),
        "nonutf8-stdout" => {
            io::stdout().write_all(&[0xff, 0xfe]).unwrap();
            write_result(result, &envelope(CORRELATION, "proved"));
        }
        "result-oversize" => write_result(result, &vec![b'x'; 64 * 1024 + 1]),
        "result-directory" => fs::create_dir(result).unwrap(),
        "stdout-envelope" => {
            io::stdout()
                .write_all(&envelope(CORRELATION, "proved"))
                .unwrap();
        }
        "wrong-correlation" => {
            write_result(
                result,
                &envelope("51515151515151515151515151515151", "proved"),
            );
        }
        "malformed" => write_result(result, b"proved\n"),
        _ => process::exit(92),
    }
}

fn authenticated_execution(arguments: &[String]) {
    for (index, expected) in [
        (1, "--request"),
        (3, "--result"),
        (5, "--verifier"),
        (7, "--solver"),
        (9, "--timeout-seconds"),
        (11, "--auth-challenge"),
        (13, "--auth-invocation"),
        (15, "--auth-policy"),
        (17, "--auth-request"),
        (19, "--auth-verus"),
        (21, "--auth-solver"),
        (23, "--auth-recorder"),
    ] {
        if arguments[index] != expected {
            process::exit(91);
        }
    }
    if ![&arguments[2], &arguments[4], &arguments[6], &arguments[8]]
        .iter()
        .all(|path| path.starts_with("/proc/self/fd/"))
        || arguments[10].parse::<u32>().is_err()
    {
        process::exit(90);
    }
    for value in [
        &arguments[12],
        &arguments[14],
        &arguments[16],
        &arguments[18],
        &arguments[20],
        &arguments[22],
        &arguments[24],
    ] {
        if value.len() != 64
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            || value.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            process::exit(89);
        }
    }

    let request = fs::read(&arguments[2]).unwrap();
    if hex_digest(&request) != arguments[18] {
        process::exit(88);
    }
    let executable = fs::read("/proc/self/exe").unwrap();
    let executable_digest = hex_digest(&executable);
    if [&arguments[20], &arguments[22], &arguments[24]]
        .iter()
        .any(|digest| **digest != executable_digest)
    {
        process::exit(87);
    }

    io::stdout().write_all(b"authenticated stdout").unwrap();
    io::stderr().write_all(b"authenticated stderr").unwrap();
    let complete = request.get(10..26) == Some(COMPLETE_CORRELATION_BYTES.as_slice());
    let payload = if complete {
        result_envelope(COMPLETE_CORRELATION, "proved", COMPLETE_PROPERTIES)
    } else {
        envelope(CORRELATION, "proved")
    };
    let result = format!(
        "FE2O3-VERUS-AUTH-RESULT-V1\nchallenge={}\ninvocation={}\npolicy={}\nrequest={}\nverus={}\nsolver={}\nrecorder={}\nresult-bytes={}\n{}",
        arguments[12],
        arguments[14],
        arguments[16],
        arguments[18],
        arguments[20],
        arguments[22],
        arguments[24],
        payload.len(),
        String::from_utf8(payload).unwrap(),
    );
    write_result(Path::new(&arguments[4]), result.as_bytes());
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest.bytes().as_bytes() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

#[allow(clippy::zombie_processes)]
fn spawn_pipe_holder() {
    // Deliberately outlive the fixture parent to exercise inherited-pipe containment.
    process::Command::new(std::env::current_exe().unwrap())
        .arg("--pipe-holder")
        .spawn()
        .unwrap();
}

fn envelope(correlation: &str, outcome: &str) -> Vec<u8> {
    let properties = if outcome == "proved" {
        "bounds,race-freedom"
    } else {
        ""
    };
    result_envelope(correlation, outcome, properties)
}

fn result_envelope(correlation: &str, outcome: &str, properties: &str) -> Vec<u8> {
    format!(
        "FE2O3-VERIFIER-RESULT-V1\ncorrelation={correlation}\noutcome={outcome}\nproperties={properties}\ntrusted=\ndiagnostic-hex=\n"
    )
    .into_bytes()
}

fn write_result(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
}
