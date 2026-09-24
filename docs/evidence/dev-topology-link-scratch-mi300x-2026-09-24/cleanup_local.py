#!/usr/bin/env python3
"""Remove only this completed campaign's redundant local builds and checkouts."""
import importlib.util
from pathlib import Path
import signal

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('scratch_cleanup', HERE / 'campaign.py')
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)
for number in C.B.MANAGED:
    signal.signal(number, C.B.interrupted)
packet = C.REPO / 'docs/evidence/dev-topology-link-scratch-mi300x-2026-09-24'
index = C.H.parse_json((packet / 'archive-index.json').read_bytes())
for run in ('native1', 'native2'):
    C.H.need(C.H.sha(packet / (run + '.tar.gz')) == index[run]['archive_sha256'], 'retained campaign archive')
collection = C.H.parse_json((HERE / 'native2/collection.json').read_bytes())
C.H.need(collection['failures'] == [] and collection['owned_cleanup'] is True, 'native campaign completed and cleaned')
rec = C.B.Recorder(HERE / 'cleanup-local', C.REPO)
for cohort in ('baseline', 'candidate'):
    target = HERE / 'native2' / ('target-' + cohort)
    C.H.need(target.resolve() == target and target.is_dir(), 'exact owned target')
    rec.run('clean-' + cohort, ['/home/harsh/.cargo/bin/cargo', 'clean', '--manifest-path',
            str(HERE / 'native2' / ('source-' + cohort) / 'Cargo.toml'), '--target-dir', str(target)], 120,
            env={'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin', 'RUSTUP_TOOLCHAIN': 'nightly-2026-04-03', 'LC_ALL': 'C'})
    C.H.need(not target.exists(), 'owned target absent')
for run in ('native1', 'native2'):
    for cohort in ('baseline', 'candidate'):
        checkout = HERE / run / ('source-' + cohort)
        expected = C.H.parse_json((HERE / run / (cohort + '-source.json')).read_bytes())
        C.H.need(checkout.resolve() == checkout and C.B.inventory(checkout) == expected, 'unchanged redundant checkout')
        rec.run(run + '-' + cohort, ['/usr/bin/python3', '-I', '-B', '-c',
                'from pathlib import Path; import shutil,sys; p=Path(sys.argv[1]); assert p.is_dir() and not p.is_symlink() and p.resolve()==p; shutil.rmtree(p); assert not p.exists()', str(checkout)], 120,
                env={'HOME': '/home/harsh', 'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'})
C.B.write_json(rec.output / 'absence.json', {str(path): not path.exists() for path in
    [HERE / 'target', HERE / 'native1/target', HERE / 'native2/target-baseline', HERE / 'native2/target-candidate',
     *[HERE / run / ('source-' + cohort) for run in ('native1', 'native2') for cohort in ('baseline', 'candidate')]]})
