#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
import json
from pathlib import Path
import tempfile
import unittest
from sample_manifests import migrate, validate, extract


class ProvenanceTests(unittest.TestCase):
    def test_unknown_rights_and_distinct_licenses_are_not_relicensed(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp); doc=root/'SOURCE.md'
            doc.write_text('# Different sounds\n\n**Creator:** Someone\n**License:** CC-BY-4.0\n**Attribution:** Credit Someone\n',encoding='utf-8')
            before=doc.read_bytes(); result=extract(doc,root,'pack')
            self.assertEqual(result['license'],'CC-BY-4.0');self.assertEqual(result['attribution'],'Credit Someone')
            self.assertEqual(doc.read_bytes(),before)
            doc.write_text('# Mystery\n',encoding='utf-8');self.assertIsNone(extract(doc,root,'pack')['license'])

    def test_manual_corrections_overrides_and_stable_identity_survive_reorganization(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp); audio=root/'Kick.wav';audio.write_bytes(b'original unchanged source')
            (root/'SOURCE.md').write_text('# Pack\n\n**License:** CC0 1.0\n',encoding='utf-8')
            m=migrate(root); original_id=m['samples']['Kick.wav']['id']
            m['samples']['Kick.wav']['license']='CC-BY-4.0';m['samples']['Kick.wav']['attribution']='Specific credit'
            m['sources']['SOURCE.md']['creator']='Manually verified creator'
            (root/'pack.json').write_text(json.dumps(m),encoding='utf-8')
            audio.rename(root/'Renamed.wav');m=migrate(root)
            self.assertEqual(m['samples']['Renamed.wav']['id'],original_id)
            self.assertEqual(m['samples']['Renamed.wav']['original_path'],'Kick.wav')
            self.assertEqual(m['samples']['Renamed.wav']['license'],'CC-BY-4.0')
            self.assertEqual(m['sources']['SOURCE.md']['creator'],'Manually verified creator')
            self.assertEqual(validate(root,m),[])

    def test_csv_original_names_resolve_from_parent_or_collection_root(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp); drums=root/'drums';drums.mkdir();(drums/'BD01.wav').write_bytes(b'kick')
            (drums/'sample-map.csv').write_text('original_path,new_path\nbd8/BD0000.WAV,drums/BD01.wav\n',encoding='utf-8')
            m=migrate(root);self.assertEqual(m['samples']['drums/BD01.wav']['original_path'],'bd8/BD0000.WAV')
            self.assertIsNone(m['samples']['drums/BD01.wav']['source'])

    def test_document_change_requires_review_instead_of_silent_fact_replacement(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);(root/'a.wav').write_bytes(b'a');doc=root/'SOURCE.md'
            doc.write_text('# Source\n\n**License:** CC0 1.0\n',encoding='utf-8');m=migrate(root)
            (root/'pack.json').write_text(json.dumps(m),encoding='utf-8');doc.write_text('# Source\n\n**License:** Other\n',encoding='utf-8')
            next_m=migrate(root);self.assertEqual(next_m['sources']['SOURCE.md']['license'],'CC0-1.0')
            self.assertTrue(any('documentation changed' in e for e in validate(root,next_m)))


if __name__=='__main__':unittest.main()
