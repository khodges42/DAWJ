#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check tracked Markdown's local links and JSON syntax; no network or ignored notes."""
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit

root = Path(__file__).resolve().parent.parent
if (root / '.git').exists():
    names = subprocess.check_output(['git', '-c', f'safe.directory={root.as_posix()}', 'ls-files', '-z'], cwd=root).decode().split('\0')
    paths = [root / n for n in names if n and (root / n).is_file()]
else:
    # A checkout-index/archive validation tree has no .git, and no ignored material.
    paths = [p for p in root.rglob('*') if p.is_file() and 'target' not in p.relative_to(root).parts]
errors = []
for path in paths:
    if path.suffix.lower() == '.md':
        text = path.read_text(encoding='utf-8-sig')
        text = re.sub(r'```[^\n]*\n.*?```', '', text, flags=re.S)
        for match in re.finditer(r'!?\[[^\]]*\]\((<[^>]+>|[^\s)]+)(?:\s+"[^"]*")?\)', text):
            href = match[1].strip('<>')
            parts = urlsplit(href)
            if parts.scheme or parts.netloc or not parts.path:
                continue
            target = (path.parent / unquote(parts.path)).resolve()
            if not target.exists():
                errors.append(f'{path.relative_to(root)}: missing local target {href}')
    elif path.suffix == '.json' and 'docs' in path.relative_to(root).parts:
        try:
            json.loads(path.read_text(encoding='utf-8-sig'))
        except ValueError as exc:
            errors.append(f'{path.relative_to(root)}: {exc}')
for error in errors:
    print(error)
if errors:
    sys.exit(1)
print(f'Local Markdown links and documentation JSON passed across {len(paths)} repository files.')
