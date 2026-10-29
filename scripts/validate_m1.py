#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Reproducible M1 gate. Hardware tests use silent samples, never audible tones."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

parser = argparse.ArgumentParser()
parser.add_argument('--hardware', action='store_true', help='probe default device and run 900-second silent stream plus fresh-stream restart')
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
commands = [
    ['cargo', 'fmt', '--all', '--check'],
    ['cargo', 'check', '--workspace', '--all-targets', '--locked', '--offline'],
    ['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--offline', '--', '-D', 'warnings'],
    ['cargo', 'test', '--workspace', '--all-targets', '--locked', '--offline'],
    ['cargo', 'test', '--workspace', '--doc', '--locked', '--offline'],
    [sys.executable, 'scripts/audio_dependencies.py'],
]
if args.hardware:
    commands += [
        ['cargo', 'run', '--release', '-p', 'audio-backend', '--example', 'soak', '--locked', '--offline', '--', '--probe'],
        ['cargo', 'run', '--release', '-p', 'audio-backend', '--example', 'soak', '--locked', '--offline', '--', '--run', '900'],
    ]
results = []
for index, command in enumerate(commands):
    print('Running:', ' '.join(command), flush=True)
    start = time.monotonic()
    run = subprocess.run(command, cwd=root, capture_output=True, text=True)
    log = root / 'target' / f'm1-validation-{index}.log'
    log.parent.mkdir(exist_ok=True)
    log.write_text(run.stdout + run.stderr, encoding='utf-8')
    results.append(dict(command=command, exit_code=run.returncode, elapsed_seconds=time.monotonic()-start, log=str(log)))
    print(run.stdout + run.stderr, flush=True)
    if run.returncode:
        break
(root / 'target/m1-validation.json').write_text(json.dumps(results, indent=2), encoding='utf-8')
raise SystemExit(any(item['exit_code'] for item in results))
