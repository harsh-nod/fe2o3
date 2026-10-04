#![forbid(unsafe_code)]

mod protocol;
mod reference_attention;
mod reference_basic;
mod reference_gpt;
mod reference_low_precision;
mod reference_systems;
mod references;

use protocol::{Request, Response};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

fn execute(input: Vec<u8>) -> Result<Vec<u8>, String> {
    if input.is_empty() || input.len() > protocol::MAX_REQUEST {
        return Err("request byte bound".to_owned());
    }
    let request: Request = serde_json::from_slice(&input).map_err(|e| e.to_string())?;
    request.validate()?;
    request.corpus_shape(&references::generate(&request.kernel)?)?;
    let outputs = references::evaluate(&request)?;
    let response = Response::new(&request, Sha256::digest(&input).into(), outputs)?;
    serde_json::to_vec(&response).map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let bytes = match args.as_slice() {
        [mode] if mode == "list" => serde_json::to_vec(&serde_json::json!({
            "schema": "fe2o3-tutorial-reference-corpus-v92", "kernels": references::kernels(),
            "authority": false,
        }))
        .map_err(|e| e.to_string())?,
        [mode, kernel] if mode == "generate" => {
            let request = references::generate(kernel)?;
            request.validate()?;
            serde_json::to_vec(&request).map_err(|e| e.to_string())?
        }
        [mode] if mode == "execute" => {
            let mut input = Vec::new();
            std::io::stdin()
                .take(protocol::MAX_REQUEST as u64 + 1)
                .read_to_end(&mut input)
                .map_err(|e| e.to_string())?;
            execute(input)?
        }
        [mode, path] if mode == "execute" => {
            use rustix::fs::{Mode, OFlags};
            use std::os::unix::fs::MetadataExt;
            let file = std::fs::File::from(
                rustix::fs::openat(
                    rustix::fs::CWD,
                    path,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|e| e.to_string())?,
            );
            let before = file.metadata().map_err(|e| e.to_string())?;
            if !before.is_file() || before.len() > protocol::MAX_REQUEST as u64 {
                return Err("bounded regular request required".to_owned());
            }
            let mut input = Vec::new();
            (&file)
                .take(before.len() + 1)
                .read_to_end(&mut input)
                .map_err(|e| e.to_string())?;
            let identity = |m: std::fs::Metadata| {
                (
                    m.dev(),
                    m.ino(),
                    m.mode(),
                    m.nlink(),
                    m.uid(),
                    m.gid(),
                    m.len(),
                    m.mtime(),
                    m.mtime_nsec(),
                    m.ctime(),
                    m.ctime_nsec(),
                )
            };
            if input.len() as u64 != before.len()
                || identity(before) != identity(file.metadata().map_err(|e| e.to_string())?)
                || identity(file.metadata().map_err(|e| e.to_string())?)
                    != identity(std::fs::symlink_metadata(path).map_err(|e| e.to_string())?)
            {
                return Err("request file changed".to_owned());
            }
            execute(input)?
        }
        _ => return Err("expected list, generate KERNEL or execute [REQUEST]".to_owned()),
    };
    if bytes.len() > protocol::MAX_RESPONSE {
        return Err("response byte bound".to_owned());
    }
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&bytes)
        .and_then(|()| stdout.write_all(b"\n"))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_current_corpus_request_executes_independent_reference_and_covers_all_outputs() {
        let names = references::kernels();
        assert_eq!(names.len(), 45);
        assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
        for name in names {
            let request = references::generate(name).unwrap();
            request.validate().unwrap();
            let bytes = serde_json::to_vec(&request).unwrap();
            let response = execute(bytes.clone()).unwrap_or_else(|error| panic!("{name}: {error}"));
            let json: serde_json::Value = serde_json::from_slice(&response).unwrap();
            assert_eq!(json["kernel"], name);
            assert_eq!(json["authority"], false);
            assert_eq!(
                json["request_sha256"],
                serde_json::json!(<[u8; 32]>::from(Sha256::digest(&bytes)))
            );
            assert!(!json["outputs"].as_array().unwrap().is_empty());
        }
        for name in [
            "aggregate_enum",
            "aggregate_pointer",
            "aggregate_drop",
            "unknown",
        ] {
            assert!(references::generate(name).is_err());
        }
    }

    #[test]
    fn actual_request_values_are_consumed_and_malformed_shapes_refused() {
        let request = references::generate("vecadd").unwrap();
        let original = serde_json::to_vec(&request).unwrap();
        let baseline = execute(original.clone()).unwrap();
        let mut changed: serde_json::Value = serde_json::from_slice(&original).unwrap();
        let input = changed["arguments"][0]["bytes"]
            .as_str()
            .unwrap()
            .to_owned();
        changed["arguments"][0]["bytes"] = format!("0x0000803f{}", &input[10..]).into();
        assert_ne!(
            execute(serde_json::to_vec(&changed).unwrap()).unwrap(),
            baseline
        );
        for field in ["kernel", "grid", "workgroup", "arguments"] {
            let mut invalid: serde_json::Value = serde_json::from_slice(&original).unwrap();
            invalid[field] = serde_json::Value::Null;
            assert!(execute(serde_json::to_vec(&invalid).unwrap()).is_err());
        }
        changed["unexpected"] = true.into();
        assert!(execute(serde_json::to_vec(&changed).unwrap()).is_err());
    }
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "reference-v92 refused: {}",
                error.chars().take(1024).collect::<String>()
            );
            std::process::ExitCode::FAILURE
        }
    }
}
