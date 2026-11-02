# SPDX-License-Identifier: GPL-3.0-or-later
import json
from pathlib import Path
import tempfile
import unittest
from web4_report import summarize


class ReportTests(unittest.TestCase):
    def test_process_tree_samples_and_bom_metrics_retain_outliers(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'dawj.metrics.json').write_text(json.dumps({'passed': True, 'command_ms': {'worst': 2222}}), encoding='utf-8-sig')
            (root / 'dawj.memory.csv').write_text('seconds,private_bytes,processes\n600,104857600,1\n660,106954752,1\n720,109051904,1\n', encoding='utf-8-sig')
            (root / 'dawj.stderr.log').write_text('Error = 1412\n', encoding='utf-8')
            result, = summarize(root)
            self.assertEqual(result['metrics']['command_ms']['worst'], 2222)
            self.assertEqual(result['memory']['peak_private_mib'], 104)
            self.assertEqual(result['memory']['slope_after_ten_minutes_mib_per_minute'], 2)
            self.assertEqual(result['teardown_1412_count'], 1)


if __name__ == '__main__':
    unittest.main()
