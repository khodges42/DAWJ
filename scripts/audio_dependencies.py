#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Exact inventory gate for M1.1 Windows x64; other targets need review."""
import json
from pathlib import Path
import subprocess
root = Path(__file__).resolve().parent.parent
inventory = json.loads((root / 'docs/windows-dependencies.json').read_text())
fields = ('name', 'version', 'source', 'license')
expected = {tuple(p[k] for k in fields) for p in inventory}
metadata = json.loads(subprocess.check_output([
    'cargo', 'metadata', '--format-version', '1', '--locked', '--offline',
    '--filter-platform', 'x86_64-pc-windows-msvc',
], cwd=root, text=True))
actual = set()
for package in metadata['packages']:
    if package['source'] is None:
        if package['license'] != 'GPL-3.0-or-later':
            raise SystemExit('Unexpected workspace license: ' + package['name'])
    else:
        actual.add(tuple(package[k] for k in fields))
if actual != expected:
    raise SystemExit(f'Dependency review required. Added: {actual - expected}; removed: {expected - actual}')
print(f'Windows x64 inventory passed: {len(actual)} external packages; Apache-2.0 selected.')
