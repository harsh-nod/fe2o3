use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) struct TestDirectory(PathBuf);

impl TestDirectory {
    pub(crate) fn new() -> Self {
        Self::allocate_with_counter(&NEXT_DIRECTORY_ID)
    }

    fn allocate_with_counter(counter: &AtomicU64) -> Self {
        loop {
            let id = counter
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    current.checked_add(1)
                })
                .expect("pinned executable test directory counter exhausted");
            let path = std::env::temp_dir().join(format!(
                "cargo-fe2o3-pinned-executable-{}-{id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!(
                    "create pinned executable test directory {}: {error}",
                    path.display()
                ),
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        match fs::remove_dir_all(&self.0) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) if std::thread::panicking() => {
                eprintln!(
                    "failed to remove pinned executable test directory {}: {error}",
                    self.0.display()
                );
            }
            Err(error) => panic!(
                "remove pinned executable test directory {}: {error}",
                self.0.display()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Arc, Barrier};

    #[test]
    fn duplicate_module_counters_allocate_unique_directories_under_concurrency() {
        const THREADS: usize = 32;
        const DIRECTORIES_PER_THREAD: usize = 32;

        let duplicate_counters = [AtomicU64::new(0), AtomicU64::new(0)];
        let start = Arc::new(Barrier::new(THREADS));
        let directories = std::thread::scope(|scope| {
            let handles = (0..THREADS)
                .map(|thread_id| {
                    let start = Arc::clone(&start);
                    let counter = &duplicate_counters[thread_id % duplicate_counters.len()];
                    scope.spawn(move || {
                        start.wait();
                        (0..DIRECTORIES_PER_THREAD)
                            .map(|_| TestDirectory::allocate_with_counter(counter))
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>();

            handles
                .into_iter()
                .flat_map(|handle| handle.join().expect("directory allocator thread panicked"))
                .collect::<Vec<_>>()
        });

        let paths = directories
            .iter()
            .map(|directory| directory.path().to_path_buf())
            .collect::<Vec<_>>();
        let unique = paths.iter().collect::<HashSet<_>>();
        assert_eq!(unique.len(), THREADS * DIRECTORIES_PER_THREAD);
        assert!(paths.iter().all(|path| path.is_dir()));

        drop(directories);
        assert!(paths.iter().all(|path| !path.exists()));
    }
}
