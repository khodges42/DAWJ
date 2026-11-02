# SPDX-License-Identifier: GPL-3.0-or-later
"""Guard the default launch target and persisted Tauri identity across cutover."""
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parent.parent


class AlphaLayout(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.metadata = json.loads(subprocess.check_output(
            ['cargo', 'metadata', '--format-version', '1', '--no-deps', '--locked', '--offline'],
            cwd=ROOT, encoding='utf-8'))
        cls.packages = {p['name']: p for p in cls.metadata['packages']}

    def test_default_is_tauri_and_fallback_remains_a_separate_binary(self):
        primary = self.packages['dawj']
        fallback = self.packages['dawj-egui']
        self.assertEqual(self.metadata['workspace_default_members'], [primary['id']])
        self.assertEqual(Path(primary['manifest_path']).parent.name, 'tauri-app')
        self.assertEqual(Path(fallback['manifest_path']).parent.name, 'performance-app')
        for package in (primary, fallback):
            self.assertTrue(any(t['name'] == package['name'] and 'bin' in t['kind'] for t in package['targets']))
        self.assertTrue(any(t['name'] == 'synthetic_midi' for t in fallback['targets']))

    def test_persistence_identity_and_native_recovery_are_retained(self):
        config = json.loads((ROOT / 'crates/tauri-app/tauri.conf.json').read_text(encoding='utf-8'))
        self.assertEqual(config['identifier'], 'org.dawj.performance')
        self.assertFalse(config['bundle']['active'])
        self.assertNotIn('unsafe-inline', config['app']['security']['csp'])
        source = (ROOT / 'crates/tauri-app/src/main.rs').read_text(encoding='utf-8')
        for recovery in ['Ctrl+Alt+Shift+R', '--safe-mode', '--reset-skin']:
            self.assertIn(recovery, source)


if __name__ == '__main__':
    unittest.main()
