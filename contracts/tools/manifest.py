#!/usr/bin/env python3
"""
Generate/verify a checksum manifest of the contracts folder.

Because redfire-switch and redfire-boss each vendor a copy of contracts/,
this manifest lets each repo's CI assert its copy is byte-identical to the
source of truth in redfire-pbx (or a pinned version), catching silent drift.

Usage:
  manifest.py            # print manifest to stdout
  manifest.py --write    # write contracts/MANIFEST.sha256
  manifest.py --check    # verify contracts/MANIFEST.sha256 matches current files
"""
import hashlib
import os
import sys
import argparse

HERE = os.path.dirname(os.path.abspath(__file__))
CONTRACTS = os.path.normpath(os.path.join(HERE, ".."))
MANIFEST = os.path.join(CONTRACTS, "MANIFEST.sha256")

# Files that make up the contract surface. Tools are included so the validator
# itself cannot drift between repos.
INCLUDE_DIRS = ["schemas", "fixtures", "techprefix", "tools"]


def iter_files():
    for d in INCLUDE_DIRS:
        base = os.path.join(CONTRACTS, d)
        for root, _, files in os.walk(base):
            for fn in sorted(files):
                if fn.endswith(".pyc"):
                    continue
                p = os.path.join(root, fn)
                rel = os.path.relpath(p, CONTRACTS)
                yield rel, p


def build():
    lines = []
    for rel, p in sorted(iter_files()):
        with open(p, "rb") as f:
            h = hashlib.sha256(f.read()).hexdigest()
        lines.append(f"{h}  {rel}")
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--write", action="store_true")
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    current = build()

    if args.write:
        with open(MANIFEST, "w") as f:
            f.write(current)
        print(f"wrote {MANIFEST}")
        return 0

    if args.check:
        if not os.path.exists(MANIFEST):
            print("MANIFEST.sha256 missing; run manifest.py --write")
            return 1
        with open(MANIFEST) as f:
            saved = f.read()
        if saved != current:
            print("CONTRACT DRIFT: contracts/ does not match MANIFEST.sha256")
            print("Re-sync the vendored contracts folder from redfire-pbx, or")
            print("update the manifest with manifest.py --write if this is the source repo.")
            return 1
        print("OK: contracts match manifest")
        return 0

    sys.stdout.write(current)
    return 0


if __name__ == "__main__":
    sys.exit(main())
