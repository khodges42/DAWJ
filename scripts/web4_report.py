#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Summarize opt-in Windows release runs; no device access or dependencies."""
import argparse
import csv
from datetime import datetime
import json
from pathlib import Path
from statistics import median


def summarize(root):
    results = []
    for path in sorted(root.glob('*.metrics.json')):
        name = path.name.removesuffix('.metrics.json')
        metrics = json.loads(path.read_text(encoding='utf-8-sig'))
        with (root / (name + '.memory.csv')).open(encoding='utf-8-sig') as stream:
            rows = list(csv.DictReader(stream))
        samples = [(float(r['seconds']), int(r['private_bytes']) / 1048576) for r in rows]
        tail = [(t, m) for t, m in samples if t >= 600]
        slope = None
        if len(tail) >= 3:
            mean_t = sum(t for t, _ in tail) / len(tail)
            mean_m = sum(m for _, m in tail) / len(tail)
            slope = 60 * sum((t - mean_t) * (m - mean_m) for t, m in tail) / sum((t - mean_t)**2 for t, _ in tail)
        memory = {'samples': len(samples), 'requested_sample_interval_seconds': 5,
                  'median_actual_sample_interval_seconds': median(samples[i][0] - samples[i-1][0] for i in range(1, len(samples))) if len(samples) > 1 else None,
                  'first_private_mib': samples[0][1] if samples else None,
                  'peak_private_mib': max((m for _, m in samples), default=None),
                  'last_five_minutes_median_private_mib': median(m for t, m in samples if t >= samples[-1][0] - 300) if samples else None,
                  'slope_after_ten_minutes_mib_per_minute': slope,
                  'process_counts': sorted({int(r['processes']) for r in rows})}
        log = (root / (name + '.stderr.log')).read_text(encoding='utf-8', errors='replace')
        host_path = root / (name + '.host.csv')
        host = None
        if host_path.exists():
            with host_path.open(encoding='utf-8-sig') as stream:
                observed = list(csv.DictReader(stream))
            cpu = [float(r['cpu_percent']) for r in observed if r['cpu_percent']]
            gpu = [float(r['gpu_busiest_engine_percent']) for r in observed if r['gpu_busiest_engine_percent']]
            correlated = []
            for event in metrics.get('outliers', []):
                if observed:
                    nearest = min(observed, key=lambda r: abs(datetime.fromisoformat(r['utc'].replace('Z', '+00:00')).timestamp()*1000-event['utc_ms']))
                    delta = abs(datetime.fromisoformat(nearest['utc'].replace('Z', '+00:00')).timestamp()*1000-event['utc_ms'])
                    correlated.append({'event': event, 'nearest_host_sample': nearest, 'distance_ms': delta})
            host = {'samples': len(observed), 'cpu_median_percent': median(cpu) if cpu else None,
                    'cpu_max_percent': max(cpu, default=None), 'gpu_busiest_engine_max_percent': max(gpu, default=None),
                    'detected_games': sorted({r['detected_games'] for r in observed if r['detected_games']}),
                    'outlier_correlations': correlated}
        results.append({'host_load': host, 'run': root.name, 'metrics': metrics, 'memory': memory,
                        'teardown_1412_count': log.count('Error = 1412')})
    return results


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('runs', nargs='+', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--preflight', type=Path)
    args = parser.parse_args()
    result = {'timing_resolution_ms': 0.1, 'quantiles_are_bin_upper_bounds': True,
              'histogram_overflow_ms': 1000, 'runs': [r for p in args.runs for r in summarize(p)]}
    if args.preflight:
        result['preflight'] = json.loads(args.preflight.read_text(encoding='utf-8-sig'))
    if not result['runs']:
        parser.error('No completed metric files found')
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(f"Wrote {len(result['runs'])} measured frontend runs to {args.output}")
