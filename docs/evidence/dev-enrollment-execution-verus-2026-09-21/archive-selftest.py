"""Adverse evidence checks, limited to owned temporary copies of receipts."""

import argparse
import json
from pathlib import Path
import runpy
import shutil
import tempfile


def main():
    local = Path(__file__).resolve().parent
    api = runpy.run_path(str(local / 'archive.py'))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=api['ROOT'])
    parser.add_argument('--archive', type=Path, default=local)
    args = parser.parse_args()
    rejected = 0
    with tempfile.TemporaryDirectory(prefix='fe2o3-enrollment-adverse-') as owned:
        temporary = Path(owned)
        contents, module = api['signed_sources'](args.repo, temporary / 'source')
        campaign = temporary / 'campaign'
        shutil.copytree(args.archive / 'campaign', campaign)
        runs = 0

        def check_campaign():
            nonlocal runs
            runs += 1
            return api['validate_campaign'](campaign, module, contents, temporary / f'run-{runs}')

        def adverse(name, path, replacement, check):
            nonlocal rejected
            original = path.read_bytes()
            try:
                path.write_bytes(replacement(original))
                try:
                    check()
                except (ValueError, KeyError, TypeError):
                    rejected += 1
                else:
                    raise AssertionError(f'accepted adverse evidence: {name}')
            finally:
                path.write_bytes(original)

        def change(edit):
            def replace(data):
                value = json.loads(data)
                edit(value)
                return (json.dumps(value) + '\n').encode()
            return replace

        api['need'](len(check_campaign()) == 23, 'baseline complete campaign')
        adverse('floating obligation count', campaign / 'report.json',
                change(lambda d: d.update(positive_obligations=263.0)), check_campaign)
        adverse('numeric qualified flag', campaign / 'report.json', change(lambda d: d.update(qualified=1)), check_campaign)
        adverse('empty inputs', campaign / 'inputs-before.json', lambda _: b'{}', check_campaign)
        adverse('omitted input', campaign / 'inputs-after.json', change(lambda d: d.pop(next(iter(d)))), check_campaign)
        adverse('solver environment', campaign / 'environment.json', change(lambda d: d.update(HOME='/foreign')), check_campaign)
        case = campaign / 'positive_before'
        adverse('omitted generated input', case / 'sources.json', change(lambda d: d.pop(next(iter(d)))), check_campaign)
        adverse('changed candidate', case / 'context_version_journal_enrollment_v1.rs', lambda b: b + b'\n', check_campaign)
        record = case / 'solver/record.json'
        for name, edit in [
            ('partial verification', lambda d: d['command'].append('--verify-root')),
            ('wrong result status', lambda d: d.update(status=1)),
            ('retained group', lambda d: d.update(group_absent=False)),
            ('extra receipt field', lambda d: d.update(extra=True)),
        ]:
            adverse(name, record, change(edit), check_campaign)
        adverse('wrong solver summary', case / 'solver/stdout.log',
                change(lambda d: d['verification-results'].update(errors=1)), check_campaign)
        adverse('wrong closure command', campaign / 'closure-before/record.json',
                change(lambda d: d['command'].append('foreign')), check_campaign)
        extra = case / 'solver/unchecked'
        extra.write_bytes(b'extra')
        try:
            try:
                check_campaign()
            except ValueError:
                rejected += 1
            else:
                raise AssertionError('accepted extra solver file')
        finally:
            extra.unlink()
        link = temporary / 'linked-solver'
        link.symlink_to(case / 'solver', target_is_directory=True)
        try:
            module.archive_helpers.entries(link)
        except ValueError:
            rejected += 1
        else:
            raise AssertionError('accepted symlink directory')
        qualification = temporary / 'qualification-data'
        qualification.mkdir()
        for name in ('qualify.py', 'finished.json'):
            shutil.copyfile(args.archive / name, qualification / name)
        shutil.copytree(args.archive / 'qualification', qualification / 'qualification')

        def check_qualification():
            api['validate_qualification'](qualification, module)

        check_qualification()
        receipt = qualification / 'qualification/lint/receipt.json'
        for name, edit in [
            ('command', lambda d: d.update(command=['true'])), ('cwd', lambda d: d.update(cwd='/foreign')),
            ('environment', lambda d: d.update(environment={})), ('stdin', lambda d: d.update(stdin_sha256='0' * 64)),
            ('timeout', lambda d: d.update(timeout_seconds=1)), ('exit', lambda d: d.update(exit=True)),
            ('process group', lambda d: d.update(group_absent=False)), ('timestamp', lambda d: d.update(finished_ns=0)),
        ]:
            adverse(name, receipt, change(edit), check_qualification)
        adverse('raw stream checksum', qualification / 'qualification/lint/stdout', lambda b: b + b'\n', check_qualification)
        adverse('native claim', qualification / 'finished.json', change(lambda d: d.update(native_execution=True)), check_qualification)
        adverse('changed driver', qualification / 'qualify.py', lambda b: b + b'\n', check_qualification)
        check_qualification()
        api['need'](len(check_campaign()) == 23, 'restored campaign')
    print(json.dumps({'adverse_cases_rejected': rejected, 'source_commit': api['SOURCE'], 'passed': True}))


if __name__ == '__main__':
    main()
