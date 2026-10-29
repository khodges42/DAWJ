#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""M0 inventory gate: any external package requires an explicit review update."""
import json
import subprocess

metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--locked", "--offline"],
    text=True,
))
members = set(metadata["workspace_members"])
failures = []
for package in metadata["packages"]:
    if package["id"] not in members or package["source"] is not None:
        failures.append(f"Unreviewed external package: {package['name']}")
    if package["license"] != "GPL-3.0-or-later":
        failures.append(f"Unexpected license: {package['name']}: {package['license']}")
    print(f"{package['name']} {package['version']}: {package['license']}")
if failures:
    raise SystemExit("\n".join(failures))
print("M0 dependency audit passed: workspace packages only; no external crates.")

