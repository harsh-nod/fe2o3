# License and separation

SPDX-License-Identifier: GPL-3.0-or-later

This tools/rocgdb-stopped-wave-observation-v1 subtree, including the producer
additions, patch, test adaptations, source checker and documentation, is
supplied under the GNU General Public License, version 3 or (at your option)
any later version. Repository MIT/Apache defaults do not relicense it.

COPYING is the exact upstream COPYING3 from ROCm/ROCgdb commit
48b1d324e389d2ed5e19822d377ff9050770233d. Existing upstream notices remain in the
patched source. New files carry GPL-3.0-or-later notices; no contributor
copyright assignment to the Free Software Foundation is implied.

The existing runtime observation GPL tool is a separate unchanged prerequisite.
The installed AMD dbgapi header is an external build input under its own terms,
not copied or relicensed by this package. No debugger executable or full
upstream source tree is distributed here.

This opt-in tooling is not linked into fe2o3 Rust crates. Do not copy it into a
dual-licensed core library. A distributor of a resulting debugger remains
responsible for GPL terms and corresponding source. There is no warranty;
see COPYING.
