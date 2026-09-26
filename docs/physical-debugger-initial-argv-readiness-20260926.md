# Bounded debugger command-line readiness — 2026-09-26

This #281 checkpoint fixes a CPU-reproduced startup race in the initial
owned-debugger command-line check. The public controller remains disabled
(`PROFILE = None`, unbound runtime inputs). It does not qualify physical
capture, GPU execution, or a new broad milestone exit; accepted exits stay
M1/V1/V2/U1/U2/U3 (6/18).

## Observation and scope

The preceding failure-capture native attempt refused an empty command-line
observation before sending any debugger command. Its family cleanup passed,
but capture did not. That failed attempt and its consumed coordination are
retained unchanged.

A separate CPU-only probe spawned 32 fixed harmless children. Two observations
changed from empty to the exact 154-byte command line on the same owned child,
within 32,390 and 29,600 ns. All 32 children were reaped; an independent audit
confirmed the observer, children and process group absent afterward. The probe
used no debugger, target, device or GPU. This reproduces a host mechanism; it
does not prove the cause of the earlier debugger failure.

Probe receipt: 102,191 bytes, SHA-256
`f33a71b7ca79042ef9d803facd0140a95f8ef7d5af6ffb3dc27dac3e07bd68ce`.
Independent cleanup audit: 2,482 bytes, SHA-256
`5fe68f326e1322e2757f9d51c25374a0e4857a6e178a3b45c905b456602cf24b`.
A preceding request was refused during preparation because its executable path
was outside the runner's allowed output namespaces; it never executed. The
successor used a fresh permitted namespace without widening that allowlist.

## Implemented behavior

Only initial setup may read again after an empty result: at most 16 reads,
each capped at 2,048 bytes plus one overflow byte, with at most 15 scheduler
yields. There is no relaunch, new deadline, seventeenth read, or final yield.
Before and after each successful read, the original clock and exact owned
child, pidfd, process stamp, executable and scope are checked. Full command-line
equality is required; a nonempty mismatch, read error, exit, identity change,
overflow or expired original deadline refuses immediately.

The complete maximum source-callgraph accounting is 432 bounded content-read
requests and 2,523,568 requested bytes, 222 explicit clock checks, 32 child
status checks, 64 pidfd polls and at most 15 yields. These are source-level
request bounds, not proof that an individual filesystem syscall or EINTR loop
cannot stall. Sixteen empty reads still refuse safely.

Later command-line checks and the existing failure capture/teardown are
unchanged. Fixed-size diagnostic counters report attempts, empties, identity
guards and yields. They carry no execution or admission authority.

## Qualification

The formatted public package passed 31 Node controls, 58 library Rust tests
and 71 controller Rust tests (129 total), all-target strict Clippy, build and
whitespace checks. This includes 17 new pure readiness tests and three new
source-surface controls. No controller executable was invoked.

The source verifier matched all 27 selected leaves / 257,370 bytes, including
23 Rust files. It verifies selected bytes, not whole-tree authentication or
runtime qualification.

Gate: `compiler-public-initial-argv-readiness-r1`.
Receipt: 62,024 bytes, SHA-256
`fea0d81807f32c3c7699bf928564cff1fa8a8755511a75b3b339dfbb7f5d9a8d`.
Source census: 8,395 files / 119,938,294 bytes, SHA-256
`24f0d74d7638b36b3c2b8ef98f244edc1205a573db4466d1de684de2d619056a`.
Root independently rehashed the request, all 25 selected inputs, tools and
retained streams, and checked equal before/after source observations.

A fresh private configuration, build, family/input binding, read-only replay,
coordination and reviewed one-attempt native qualification remain necessary.
Old consumed native coordination is not reusable.
