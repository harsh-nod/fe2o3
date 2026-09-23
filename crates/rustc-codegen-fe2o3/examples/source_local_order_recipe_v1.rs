//! Explicit Linux headless author example. No generic CLI/pass selection.
//! Outputs are create-new and non-transactional: an earlier output may remain if
//! a later write fails. The library publishes no recipe/LLVM files or child
//! processes; caller-supplied rustc arguments retain ordinary rustc effects.
#![feature(rustc_private)]
#![forbid(unsafe_code)]
#[cfg(target_os = "linux")]
mod linux {
    use rustc_codegen_fe2o3::{
        SourceLocalOrderOrderV1 as Order, SourceLocalOrderRecipeRequestV1 as Request,
        SourceLocalOrderRelationV1 as Relation, SourceLocalOrderSourceBindingModeV1 as Binding,
        SourceLocalOrderStrengthV1 as Strength, run_source_local_order_recipe_driver_v1,
    };
    use sha2::{Digest, Sha256};
    use std::{
        fs::{self, File, Metadata, OpenOptions},
        io::{Read, Seek, SeekFrom, Write},
        os::unix::fs::{MetadataExt, OpenOptionsExt},
        path::{Path, PathBuf},
    };
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
    fn fail(message: &str) -> Box<dyn std::error::Error> {
        std::io::Error::other(message).into()
    }
    fn hash(text: &str) -> Result<[u8; 32]> {
        if text.len() != 64
            || !text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(fail("expected exact lowercase SHA-256"));
        }
        let mut result = [0; 32];
        for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
            let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
            result[index] = digit(pair[0]) * 16 + digit(pair[1]);
        }
        Ok(result)
    }
    fn absolute(path: &str) -> Result<PathBuf> {
        let path = PathBuf::from(path);
        if !path.is_absolute() || path.as_os_str().len() > 4096 {
            return Err(fail(
                "example recipe/output paths must be bounded absolute paths",
            ));
        }
        let parent = path.parent().ok_or_else(|| fail("output parent absent"))?;
        if fs::canonicalize(parent)? != parent || path.file_name().is_none() {
            return Err(fail("example requires an existing canonical parent"));
        }
        Ok(path)
    }
    fn same(a: &Metadata, b: &Metadata) -> bool {
        a.is_file()
            && b.is_file()
            && a.nlink() == 1
            && b.nlink() == 1
            && a.dev() == b.dev()
            && a.ino() == b.ino()
            && a.mode() == b.mode()
            && a.len() == b.len()
            && a.mtime() == b.mtime()
            && a.mtime_nsec() == b.mtime_nsec()
            && a.ctime() == b.ctime()
            && a.ctime_nsec() == b.ctime_nsec()
    }
    fn read(file: &mut File) -> Result<Vec<u8>> {
        file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(8193)?;
        let mut chunk = [0; 1024];
        loop {
            let remaining = 8193_usize
                .checked_sub(bytes.len())
                .ok_or_else(|| fail("recipe read bound"))?;
            let count = file.read(&mut chunk[..remaining.min(1024)])?;
            if count == 0 {
                break;
            }
            if bytes.len().checked_add(count).is_none_or(|n| n > 8192) {
                return Err(fail("recipe exceeds 8192 bytes"));
            }
            bytes.extend_from_slice(&chunk[..count]);
        }
        if bytes.is_empty() {
            return Err(fail("empty recipe"));
        }
        Ok(bytes)
    }
    struct RetainedRecipe {
        path: PathBuf,
        file: File,
        snapshot: Metadata,
        bytes: Vec<u8>,
    }
    impl RetainedRecipe {
        fn open(path: &str, expected: [u8; 32]) -> Result<Self> {
            let path = absolute(path)?;
            let mut file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open(&path)?;
            let snapshot = file.metadata()?;
            if !same(&snapshot, &fs::symlink_metadata(&path)?)
                || snapshot.len() == 0
                || snapshot.len() > 8192
            {
                return Err(fail("recipe is not one bounded regular retained file"));
            }
            let bytes = read(&mut file)?;
            if <[u8; 32]>::from(Sha256::digest(&bytes)) != expected {
                return Err(fail("recipe expected bytes differ"));
            }
            let mut result = Self {
                path,
                file,
                snapshot,
                bytes,
            };
            result.recheck()?;
            Ok(result)
        }
        fn recheck(&mut self) -> Result<()> {
            if !same(&self.snapshot, &self.file.metadata()?)
                || !same(&self.snapshot, &fs::symlink_metadata(&self.path)?)
            {
                return Err(fail("recipe file changed"));
            }
            let bytes = read(&mut self.file)?;
            if bytes != self.bytes
                || !same(&self.snapshot, &self.file.metadata()?)
                || !same(&self.snapshot, &fs::symlink_metadata(&self.path)?)
            {
                return Err(fail("recipe file changed"));
            }
            Ok(())
        }
    }
    fn publish(path: &Path, bytes: &[u8]) -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    }
    pub fn run() -> Result<()> {
        let mut args = Vec::new();
        let mut size = 0usize;
        for argument in std::env::args_os().skip(1) {
            let argument = argument
                .into_string()
                .map_err(|_| fail("arguments must be UTF-8"))?;
            size = size
                .checked_add(argument.len())
                .ok_or_else(|| fail("argument bytes overflow"))?;
            if args.len() >= 4096 || size > 1024 * 1024 {
                return Err(fail("bounded example arguments exceeded"));
            }
            args.push(argument);
        }
        let split = args
            .iter()
            .position(|s| s == "--")
            .ok_or_else(|| fail("missing -- before complete rustc argv"))?;
        let (options, rest) = args.split_at(split);
        let rustc = &rest[1..];
        let mut retained = None;
        let (request, recipe_output, llvm_output) = match options.first().map(String::as_str) {
            Some("create") if options.len() == 9 => {
                let preference = match options[3].as_str() {
                    "source-order" => Order::SourceOrder,
                    "reverse-ready" => Order::ReverseReady,
                    _ => return Err(fail("closed schedule choice")),
                };
                let relation = match options[4].as_str() {
                    "xor-before-or" => Relation::XorBeforeOr,
                    "or-before-xor" => Relation::OrBeforeXor,
                    _ => return Err(fail("closed canonical relation")),
                };
                let strength = match options[5].as_str() {
                    "exact" => Strength::Exact,
                    "advisory" => Strength::Advisory,
                    _ => return Err(fail("closed constraint strength")),
                };
                let binding = match options[6].as_str() {
                    "exact-revision" => Binding::ExactRevision,
                    "rebind-current" => Binding::RebindCurrent,
                    _ => return Err(fail("closed source-binding mode")),
                };
                (
                    Request::create(
                        &options[1],
                        hash(&options[2])?,
                        preference,
                        relation,
                        strength,
                        binding,
                    )?,
                    Some(absolute(&options[7])?),
                    absolute(&options[8])?,
                )
            }
            Some("replay") if options.len() == 6 => {
                let current = RetainedRecipe::open(&options[3], hash(&options[4])?)?;
                let request = Request::replay(&options[1], hash(&options[2])?, &current.bytes)?;
                retained = Some(current);
                (request, None, absolute(&options[5])?)
            }
            _ => {
                return Err(fail(
                    "usage: create SOURCE_REL SHA ORDER RELATION STRENGTH BINDING RECIPE_OUT LLVM_OUT -- RUSTC_ARGV... | replay SOURCE_REL SHA RECIPE_IN RECIPE_SHA LLVM_OUT -- RUSTC_ARGV...",
                ));
            }
        };
        if recipe_output.as_ref() == Some(&llvm_output) {
            return Err(fail("distinct new outputs required"));
        }
        for output in recipe_output.iter().chain(std::iter::once(&llvm_output)) {
            match fs::symlink_metadata(output) {
                Ok(_) => return Err(fail("output already exists")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let attempt = run_source_local_order_recipe_driver_v1(rustc, request);
        if let Some(file) = retained.as_mut() {
            file.recheck()?;
        }
        let output = attempt.result().map_err(|error| fail(error.diagnostic()))?;
        if let Some(path) = recipe_output {
            publish(
                &path,
                output
                    .created_recipe_bytes()
                    .ok_or_else(|| fail("Create did not return a recipe"))?,
            )?;
        } else if output.created_recipe_bytes().is_some() {
            return Err(fail("Replay unexpectedly regenerated a recipe"));
        }
        publish(&llvm_output, output.llvm_ir().as_bytes())?;
        println!(
            "checked canonical {:?}; constraint {:?}; LLVM from actual current L; no native/proof/launch authority",
            output.evidence().actual_relation(),
            output.evidence().constraint_outcome()
        );
        Ok(())
    }
}
fn main() -> std::process::ExitCode {
    #[cfg(target_os = "linux")]
    match linux::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "source local-order recipe: {error}; outputs are non-transactional and earlier new files may remain"
            );
            std::process::ExitCode::FAILURE
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("source local-order recipe example requires Linux");
        std::process::ExitCode::FAILURE
    }
}
