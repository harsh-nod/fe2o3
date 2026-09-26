# Maintenance debugger rebuild and static qualification

Status: private build and CPU/static evidence only. The public physical-v3
package remains disabled; physical-v2 and public PROFILE=None are unchanged.
Accepted broad exits remain M1/V1/V2/U1/U2/U3 (6/18).

The private maintenance successor was rebuilt from its exact retained R10
source donor, with four existing-file changes and no predecessor mutation.
The changes maintain strong process/target ownership and epoch/inflight checks
around the existing commit effect. References may remain until destruction
after exceptions and can delay target close; unchanged close timing is not
claimed.

Preparation, configuration, bootstrap and full GDB build all passed separately
with unchanged source/input/tool observations. The resulting debugger is
199,543,392 bytes; its adapter object is 8,773,904 bytes. Neither executable
was launched by these build gates.

The separately qualified static checks passed 87 controls, including 29 new
maintenance controls and 58 inherited controls. Actual object layout measured
eight types from bounded readelf output. The adapter is 8,728 bytes; its logical
reservation is 14,488 bytes, including the separate 256-byte maintenance
scratch, under the unchanged 65,536-byte cap. The source projection and readelf
alias identities were unchanged before and after the check.

| Retained evidence | Bytes | SHA-256 |
| --- | --- | --- |
| Full GDB build | 302,767 | `f50dcace7bf61d28a97b7630115d8c2be4aa2a3f97c11ba4c5c4bf586d996acb` |
| Actual object/source checks | 427,872 | `021835baf750603ddacf5d4680f8f720f8bf1d8a0657d68242139390d7ad9a55` |
| 55 decoder/derivation controls | 248,129 | `90f0fd887f8294ef5bdb00ffcf572f6313587f5b5d8f79c9256571df5688150f` |
| Six startup draft controls | 87,052 | `f46191ce82aa07eb8badb756cf59f6cef5626ed0ec09a4dbb453c93233da8044` |
| Actual two-pass derivation | 278,016 | `91a2d05acfe3eb5aba5e5d4b43fb13c0ee8f5a1bf0ec6cbe9a66b83653f8af0c` |
| Measured artifact proposal | 334,377 | `23d3f04ce80a0973d4902a1f6726607baabfdd853662cc7b0eb98b5d63f2738d` |

The closed build-evidence decoder passed 24 new controls alongside 31 unchanged
derivation controls. Six source-only startup-routing controls also passed.
The fresh read-only derivation then measured the exact files twice: 290 read
keys, 790,136,768 reserved bytes under the unchanged 1 GiB cumulative cap,
78 data files, nine directories, 54 generated/source Python pairs and 80
static candidates. Only the debugger candidate changed content; 54 Python
candidates changed paths only and 25 candidates were unchanged. The generated
data Makefile changed; all 16 library aliases and the interpreter target were
unchanged. These are static measurements, not a loaded-process observation.

New startup and loaded-closure observations, controller/family qualification
and a fresh coordinated native attempt are still required. Historical startup leases and binary identities
cannot authorize the rebuilt executable.

No startup, GPU dispatch, stopped-wave capture, register observation or native
qualification is claimed here. The earlier failed publication and cleanup
receipts remain retained; successful cleanup is not capture success.
