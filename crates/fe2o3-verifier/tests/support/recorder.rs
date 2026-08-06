use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;
use std::time::Duration;

const CORRELATION: &str = "32323232323232323232323232323232";

fn main() {
    let arguments: Vec<_> = std::env::args().collect();
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
    format!(
        "FE2O3-VERIFIER-RESULT-V1\ncorrelation={correlation}\noutcome={outcome}\nproperties={properties}\ntrusted=\ndiagnostic-hex=\n"
    )
    .into_bytes()
}

fn write_result(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
}
