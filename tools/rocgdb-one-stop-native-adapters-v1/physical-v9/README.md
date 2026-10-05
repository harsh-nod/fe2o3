# Disabled paired-completion resume source (physical-v9)

This optional GPL source layer follows physical-v8 at the same pinned ROCgdb commit. It corrects the explicitly admitted all-stop-on-non-stop, host-first one-host/one-wave completion transaction. It requires both actual resume returns and final commit; duplicate, wrong-order/scope, stale state and partial failures refuse. It does not enable activation, capture or publication. CPU/source checks are not native qualification.

Apply the predecessor layers through physical-v8, then patches/0001-disabled-paired-completion-resume.patch. Keep every predecessor package unchanged. Run the bounded verifier with the explicit canonical full source root and stage physical-paired-completion-resume-disabled-v9. The 63 selected rows total 2264529 bytes under the unchanged 2293760-byte bound.

CPU controls: node --test tests/*-tests.mjs; compile tests/completion-sequence.cc using -std=c++17 -Wall -Wextra -Werror -pedantic -I src, and again with undefined-behavior sanitizer. Run node tests/completion-placement.mjs /canonical/source/gdb against the composed full source. The placement test reads source only and does not start GDB.

Private experimental product paths, leases and runtime observations are intentionally absent. A successful private attempt does not make this disabled public layer active. Native scratch is charged once as sizeof(completion_resume_scratch); the two-byte sequence and resume-site member are already included in sizeof(native_adapter). The unchanged native logical ceiling is 65536 bytes; compiler-specific actual layout must still be measured for each product.
