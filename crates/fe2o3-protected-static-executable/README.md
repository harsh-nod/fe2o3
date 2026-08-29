# Protected static executable custody

`fe2o3-protected-static-executable` is the canonical host-runtime boundary for executable images
used by protected fe2o3 services. It admits a bounded read-only source against an exact SHA-256 and
length, validates the loader-independent static ELF profile, copies the bytes into an anonymous
mode-0555 executable memfd owned by the requested service identity, and seals that object against
content, size, mode, ownership, and seal changes.

The resulting value is move-only and does not implement `AsFd`. It can revalidate its retained
object or produce one close-on-exec clone for a controlled `execveat` transition. Running services
use the same contract to admit `/proc/self/exe`, so provisioning and execution cannot silently
drift onto different image rules.

This crate grants no compiler, signing, publication, loading, launch, process, or GPU authority.
