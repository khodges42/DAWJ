#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Additive factory provenance migration. Never writes SOURCE.md or audio.

python scripts/sample_manifests.py --update
python scripts/sample_manifests.py --check
Existing facts/IDs and per-sample overrides win over extraction. Correct manifests
manually when documentation is ambiguous; --update never guesses legal permission.
"""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import re
import uuid

ROOT = Path(__file__).resolve().parent.parent
NAMESPACE = uuid.UUID('c59fb7fb-392a-4485-bcd9-f57f981ae778')


def identity(kind, name):
    return kind + '-' + str(uuid.uuid5(NAMESPACE, name))


def read(path):
    return path.read_text(encoding='utf-8-sig')


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def extract(path, base, pack_id):
    text = read(path)
    fields = {}
    for key, value in re.findall(r'^\*\*([^*]+):\*\*\s*(.*?)\s*$', text, re.M):
        fields[key.lower()] = value.strip()
    def section(name):
        match = re.search(r'^## ' + re.escape(name) + r'\s*\n(.*?)(?=^## |\Z)', text, re.M | re.S)
        return match[1].strip() if match else None
    title = re.search(r'^# (.+)', text, re.M)
    license_text = fields.get('license')
    license_id = 'CC0-1.0' if license_text and re.search(r'\bCC0\b', license_text) else license_text
    return {
        'id': identity('source', pack_id + '/' + path.relative_to(base).as_posix()),
        'title': title[1] if title else path.parent.name,
        'document': path.relative_to(base).as_posix(),
        'document_sha256': digest(path),
        'description': section('About'),
        'creator': fields.get('creator'),
        'contributors': re.findall(r'^Author:\s*(.+)', text, re.M),
        'source_url': fields.get('original website') or fields.get('website'),
        'license': license_id,
        'license_url': fields.get('license url'),
        'copyright': fields.get('copyright'),
        'attribution': fields.get('attribution') or fields.get('attribution requirements'),
        'redistribution': section('Redistribution'),
        'modifications': next(iter(re.findall(r'^- Local modifications:\s*(.+)', text, re.M)), None),
        'preservation': section('Source and preservation'),
    }


def merge_missing(existing, extracted):
    # Explicit null also wins: a human may have marked an extracted claim unknown.
    for key, value in extracted.items():
        existing.setdefault(key, value)
    return existing


def migrate(base):
    path = base / 'pack.json'
    old = json.loads(read(path)) if path.exists() else {}
    if old.get('version', 1) not in (1, 2):
        raise ValueError(f'{path}: unsupported manifest version')
    result = merge_missing(old, {'id': base.name, 'name': base.name, 'license': None,
                                'creator': None, 'provenance': None, 'extra': {}})
    result['version'] = 2
    sources = result.setdefault('sources', {})
    documents = sorted(base.rglob('SOURCE.md'))
    for doc in documents:
        if doc.is_symlink():
            continue
        key = doc.relative_to(base).as_posix()
        sources[key] = merge_missing(sources.get(key, {}), extract(doc, base, result['id']))
    original = {}
    for mapping in sorted(base.rglob('sample-map.csv')):
        with mapping.open(encoding='utf-8-sig', newline='') as stream:
            for row in csv.DictReader(stream):
                if row.get('new_path') and row.get('original_path'):
                    candidates = [mapping.parent / row['new_path'], base / row['new_path']]
                    target = next((p for p in candidates if p.is_file()), candidates[0])
                    local = target.relative_to(base).as_posix()
                    original[local] = row
    samples = result.setdefault('samples', {})
    wavs = sorted(p for p in base.rglob('*') if p.is_file() and not p.is_symlink() and p.suffix.lower() == '.wav')
    present = {p.relative_to(base).as_posix() for p in wavs}
    for audio in wavs:
        key = audio.relative_to(base).as_posix()
        checksum = digest(audio)
        if key not in samples:
            # A uniquely identifiable file relocation keeps its immutable source ID.
            moved = [k for k, v in samples.items() if k not in present and v.get('sha256') == checksum]
            if len(moved) == 1:
                samples[key] = samples.pop(moved[0])
        ancestors = [doc for doc in documents if doc.parent == audio.parent or doc.parent in audio.parents]
        doc = max(ancestors, key=lambda d: len(d.parts), default=None)
        row = original.get(key, {})
        record = {'id': identity('sample', result['id'] + '/' + key),
                  'source': doc.relative_to(base).as_posix() if doc else None,
                  'original_path': row.get('original_path', key),
                  'original_category': str(Path(row.get('original_path', key)).parent).replace('\\', '/'),
                  'sha256': checksum, 'license': row.get('license') or None,
                  'creator': row.get('creator') or None, 'attribution': row.get('attribution') or None,
                  'metadata': {**{k: v for k, v in row.items() if k not in ('new_path', 'original_path', 'sha256')},
                               'path_basis': 'source_map' if row else 'recorded_local'}}
        samples[key] = merge_missing(samples.get(key, {}), record)
        merge_missing(samples[key]['metadata'], record['metadata'])
    return result


def validate(base, manifest):
    errors = []
    if set(manifest)-{'version','id','name','license','creator','provenance','extra','sources','samples'}:
        errors.append(f'{base.name}: unsupported manifest fields; preserve additional facts under extra')
    if len(manifest.get('sources', {})) > 256 or len(manifest.get('samples', {})) > 4096:
        errors.append(f'{base.name}: runtime manifest budget exceeded')
    ids = set()
    for key, sample in manifest.get('samples', {}).items():
        path = (base / key).resolve()
        if not path.is_relative_to(base.resolve()) or not path.is_file():
            errors.append(f'{base.name}/{key}: missing or unsafe audio path')
            continue
        if sample.get('id') in ids or not sample.get('id'):
            errors.append(f'{base.name}/{key}: missing/duplicate sample ID')
        ids.add(sample.get('id'))
        if digest(path) != sample.get('sha256'):
            errors.append(f'{base.name}/{key}: source content changed; review its original identity/hash')
        if sample.get('source') is not None and sample['source'] not in manifest.get('sources', {}):
            errors.append(f'{base.name}/{key}: unknown provenance source')
    for key, source in manifest.get('sources', {}).items():
        doc = (base / source['document']).resolve()
        if not doc.is_relative_to(base.resolve()) or not doc.is_file() or digest(doc) != source['document_sha256']:
            errors.append(f'{base.name}/{key}: documentation changed/missing; review extracted facts')
    actual = {p.relative_to(base).as_posix() for p in base.rglob('*') if p.is_file() and p.suffix.lower() == '.wav'}
    if actual != set(manifest.get('samples', {})):
        errors.append(f'{base.name}: manifest/sample coverage differs; run --update')
    return errors


def notice(packs):
    lines = ['# Factory sample attributions', '',
             'Generated from pack.json manifests. Original SOURCE.md, README, licenses,',
             'renaming maps and per-sample overrides remain authoritative. Unknown means',
             'not documented here, not permission granted or attribution waived.', '']
    for base, pack in packs:
        lines += [f'## {pack["name"]}', '', f'Pack ID: `{pack["id"]}`', '']
        if not pack.get('sources'):
            lines += [f'Creator: {pack.get("creator") or "Unknown"}', '',
                      f'License: {pack.get("license") or "Unknown"}', '', pack.get('provenance') or 'Provenance: Unknown', '']
        for source in pack.get('sources', {}).values():
            lines += [f'### {source["title"]}', '', f'Creator: {source.get("creator") or "Unknown"}', '',
                      f'License: {source.get("license") or "Unknown"}', '',
                      f'Attribution requirement: {source.get("attribution") or "Unknown; consult original documentation"}', '',
                      f'Original documentation: `{base.name}/{source["document"]}`', '',
                      source.get('redistribution') or 'Redistribution conditions: Unknown', '']
        for key, sample in pack.get('samples', {}).items():
            if any(sample.get(k) for k in ('license', 'creator', 'attribution')):
                lines += [f'- `{key}` overrides: ' + json.dumps({k:sample[k] for k in ('license','creator','attribution') if sample.get(k)}, ensure_ascii=False)]
        lines.append('')
    return '\n'.join(lines).rstrip() + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT / 'resources/samples')
    parser.add_argument('--update', action='store_true')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    packs = []; errors = []
    for base in sorted(p for p in args.root.iterdir() if p.is_dir() and not p.is_symlink()):
        manifest = migrate(base)
        path = base / 'pack.json'
        if args.update:
            path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        elif not path.exists() or json.loads(read(path)) != manifest:
            errors.append(f'{base.name}: missing metadata; run --update')
        errors += validate(base, manifest)
        packs.append((base, manifest))
    output = args.root / 'ATTRIBUTIONS.md'
    text = notice(packs)
    if args.update:
        output.write_text(text, encoding='utf-8')
    elif not output.exists() or read(output) != text:
        errors.append('Aggregated attribution notice differs; run --update')
    unknown = sum(not s.get('attribution') for _,p in packs for s in p.get('sources',{}).values())
    print(f'{len(packs)} collections, {sum(len(p["samples"]) for _,p in packs)} samples, '
          f'{sum(len(p["sources"]) for _,p in packs)} SOURCE records; {unknown} attribution requirements undocumented.')
    for error in errors:
        print(error)
    raise SystemExit(bool(errors))


if __name__ == '__main__':
    main()
