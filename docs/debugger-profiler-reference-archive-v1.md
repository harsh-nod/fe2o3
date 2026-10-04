# Debugger and profiler reference evidence archive V1

The `fe2o3-reference-evidence-archive-v1` envelope is the portable input to
the deterministic reference debugger/profiler workflow. It retains the
existing canonical KIR V7, simulation request, Profiler Bundle V4, Variant V1
treatment manifest, raw profiler source, schedule, artifact, and optional ISA,
counter, and PC-sample bytes. It does not define a new diagnosis, profiler, or
agent protocol.

## Admission

The archive has a fixed magic value and a bounded ordered table of fixed-role
members. Each table record contains the role, byte count, SHA-256, and exact
member bytes. Roles are unique and sorted in the one canonical order. Decoder
admission requires:

- at most 192 MiB of archive input and 22 members;
- all required simulator and baseline/candidate treatment roles exactly once;
- no unknown, duplicate, missing, reordered, trailing, or truncated content;
- each declared member byte count within its role and treatment aggregate
  limit before any member-content allocation;
- every member SHA-256 to match its bytes; and
- the whole archive SHA-256 to match a canonical lowercase caller pin.

The fixed role vocabulary contains no caller-defined paths. A spelling such as
`../artifact` is an unknown role and fails admission. Members remain in memory
and are never extracted, so they cannot be symlinks or hardlinks. The archive
file itself uses the reference client's regular-file capture boundary, which
rejects symlinks, multiple links, size changes, inode replacement, and metadata
changes around the bounded read.

## Authority

The archive is inert evidence. It grants no execution, compiler, artifact,
proof, load, dispatch, attach, pause, scheduling, KFD, or collection authority.
SHA-256 authenticates the caller-selected content identity only. It is not a
signature and does not identify the archive producer.

The reference client separately admits the exact installed debugger and
profiler service executable bytes, copies each into a mode-0500 sealed memfd,
reopens the image read-only, and verifies its exact byte identity and immutable
seals. Archive children execute only those `/proc/self/fd/N` images with an
empty environment. No inherited loader, locale, search-path, temporary-path,
ROCm, sanitizer, or project variable participates. This isolation is specific
to archive mode; the frozen legacy workflow retains its descriptor and
environment behavior. The client then reuses the read-only JSONL protocols to
open and page captures, validate diagnosis V2 citations, compare Variant V1
treatments, and plan the minimum next capture. The archive report is
deterministic and contains the whole-archive identity, every ordered member
identity, both exact executable byte identities, and the unchanged
`fe2o3-agent-reference-report-v1` result. Simulator KIR and request members are
copied into distinct mode-0400 sealed memfds and admitted by the debugger's
paired `--kir-v7-fd`/`--request-fd` options. Only those descriptors lose
close-on-exec in the intended debugger child; JSONL remains on stdin/stdout.
No raw descriptor is durable evidence.

## Checkout-free acceptance

The production integration acceptance generates compact generic evidence in
the test producer, stages exactly three built production binaries plus the
archive into an isolated directory, supplies hostile inherited environment
variables, changes the
working directory to the staged directory, and invokes only:

```text
./fe2o3-agent-reference-client \
  --archive evidence.fe2archive \
  --archive-sha256 EXPECTED_SHA256 \
  --debugger fe2o3-debug \
  --profiler-service fe2o3-agent-profiler-service
```

No checkout path or loose member path is supplied to the fresh process. Two
runs must produce byte-identical reports with exact evidence citations for the
memory out-of-bounds and barrier-divergence diagnoses, the conservative
schedule/resource comparison, and the minimum next-capture plan.
