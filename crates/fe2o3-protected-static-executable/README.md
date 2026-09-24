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

## Native Resource Accounting

`ProtectedStaticExecutableV2` freshly admits the same image contract using the
caller's work/storage ledger. It does not convert an admitted V1 owner. Shared
private validation retains V1 retry behavior; native I/O uses single attempts and
rejects short transfers or interruption. Read-only reopening uses a fixed stack
descriptor path and checks the retained inode.

The shared ELF parser now uses fixed prefix arrays rather than temporary heap
vectors: at most 1024 program/load headers and 32 unique non-NULL dynamic tags
(23 currently allowed). Arbitrarily long zero NULL padding is still accepted
within the image bounds. Identity framing and validation precedence are unchanged.

`quota` reports full work and additional logical scratch for admission,
revalidation, or transfer. Inputs stay separately prepaid. Each descriptor pays
for the full logical image even when a clone shares an inode. Consuming admission
returns only owner growth; transfer returns the full new File charge. Scopes
restore entry storage without refunding work; callers reserve returned charges
before retaining results and retire full charges after drop.

Work includes image-length-dependent parser/table visits, all read/hash passes,
and a finite descriptor-call allowance. Scratch includes two read buffers, each
admitted only with capacity at most twice its requested payload, new backing,
fixed parser workspace, and ownership/control frames. Overcapacity is rejected
after allocation but before initialization or processing. These are logical
admission bounds, not allocator, RSS, generated-stack, instruction or latency
guarantees. Fixed parser arrays also apply to V1 callers.
