#!/usr/bin/env python3
"""
Dependency-free contract validator for the redfire triple
(redfire-ucaas <-> redfire-switch <-> redfire-boss).

Single source of truth lives in redfire-ucaas/contracts/. Each repo vendors or
submodules this folder and runs this script in CI so all three stay in lock-step.

Implements the subset of JSON Schema Draft-07 used by our contracts:
type, required, properties, additionalProperties, enum, pattern, minLength,
minimum, minItems, and array items. No third-party deps so it runs anywhere
Python 3 exists.

Usage:
  contract_check.py [--contracts DIR]

Exit non-zero on any failure. Prints a summary.
"""
import json
import os
import re
import sys
import hashlib
import argparse

HERE = os.path.dirname(os.path.abspath(__file__))
CONTRACTS = os.path.normpath(os.path.join(HERE, ".."))


def _type_ok(value, t):
    if t == "object":
        return isinstance(value, dict)
    if t == "array":
        return isinstance(value, list)
    if t == "string":
        return isinstance(value, str)
    if t == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if t == "number":
        return isinstance(value, (int, float)) and not isinstance(value, bool)
    if t == "boolean":
        return isinstance(value, bool)
    if t == "null":
        return value is None
    return True


def validate(value, schema, path="$"):
    """Return list of error strings (empty means valid)."""
    errors = []

    t = schema.get("type")
    if t is not None:
        types = t if isinstance(t, list) else [t]
        if not any(_type_ok(value, one) for one in types):
            errors.append(f"{path}: expected type {types}, got {type(value).__name__}")
            return errors  # further checks are unsafe if type is wrong

    if "enum" in schema and value not in schema["enum"]:
        errors.append(f"{path}: {value!r} not in enum {schema['enum']}")

    if isinstance(value, str):
        if "minLength" in schema and len(value) < schema["minLength"]:
            errors.append(f"{path}: string shorter than minLength {schema['minLength']}")
        if "pattern" in schema and not re.search(schema["pattern"], value):
            errors.append(f"{path}: {value!r} does not match pattern {schema['pattern']}")

    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in schema and value < schema["minimum"]:
            errors.append(f"{path}: {value} below minimum {schema['minimum']}")

    if isinstance(value, list):
        if "minItems" in schema and len(value) < schema["minItems"]:
            errors.append(f"{path}: fewer than minItems {schema['minItems']}")
        item_schema = schema.get("items")
        if item_schema:
            for i, item in enumerate(value):
                errors += validate(item, item_schema, f"{path}[{i}]")

    if isinstance(value, dict):
        props = schema.get("properties", {})
        for req in schema.get("required", []):
            if req not in value:
                errors.append(f"{path}: missing required property '{req}'")
        addl = schema.get("additionalProperties", True)
        for k, v in value.items():
            if k.startswith("_"):
                continue  # allow _invalid_reason / _comment annotations
            if k in props:
                errors += validate(v, props[k], f"{path}.{k}")
            elif addl is False:
                errors.append(f"{path}: additional property '{k}' not allowed")
    return errors


def load_json(p):
    with open(p) as f:
        return json.load(f)


def schema_for_fixture(fixture_name):
    # fixtures are <contract>.valid.json / <contract>.invalid.json
    base = fixture_name.rsplit(".", 2)[0]
    return os.path.join(CONTRACTS, "schemas", base + ".schema.json")


def run():
    ap = argparse.ArgumentParser()
    ap.add_argument("--contracts", default=CONTRACTS)
    args = ap.parse_args()
    contracts = args.contracts

    schemas_dir = os.path.join(contracts, "schemas")
    fixtures_dir = os.path.join(contracts, "fixtures")

    failures = []
    checked = 0

    # 1. Every schema must itself be valid JSON.
    for fn in sorted(os.listdir(schemas_dir)):
        if not fn.endswith(".schema.json"):
            continue
        try:
            load_json(os.path.join(schemas_dir, fn))
        except Exception as e:
            failures.append(f"schema {fn} is not valid JSON: {e}")

    # 2. Every fixture validates (valid must pass, invalid must fail) against its schema.
    for fn in sorted(os.listdir(fixtures_dir)):
        if not fn.endswith(".json"):
            continue
        schema_path = schema_for_fixture(fn)
        if not os.path.exists(schema_path):
            failures.append(f"fixture {fn}: no schema at {os.path.basename(schema_path)}")
            continue
        schema = load_json(schema_path)
        data = load_json(os.path.join(fixtures_dir, fn))
        errs = validate(data, schema)
        checked += 1
        is_invalid_fixture = fn.endswith(".invalid.json")
        if is_invalid_fixture and not errs:
            failures.append(f"fixture {fn}: expected to FAIL validation but passed")
        if not is_invalid_fixture and errs:
            failures.append(f"fixture {fn}: expected to PASS but got errors: {errs}")

    # 3. Tech-prefix golden cases: reference encode/decode round-trip.
    tp = load_json(os.path.join(contracts, "techprefix", "cases.json"))
    from_tp = tech_prefix_checks(tp)
    checked += from_tp["checked"]
    failures += from_tp["failures"]

    print(f"contract_check: {checked} checks run against {contracts}")
    if failures:
        print(f"FAILED ({len(failures)}):")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("OK: all contract checks passed")
    return 0


# ---- reference tech-prefix codec (the canonical PREFIX*E164 behaviour) ----

def encode_ruri_user(tech_prefix, dialed_e164):
    if tech_prefix is None:
        return dialed_e164
    return f"{tech_prefix}*{dialed_e164}"


def decode_ruri_user(ruri_user):
    """Return (tech_prefix, dialed) using the canonical '*'-separated form.
    A leading '*' that is part of a star-led prefix is handled: the FIRST '*'
    after at least 3 chars is the separator only if a separator exists later.
    We follow the documented cases: split on the LAST '*' when a star-led
    prefix is present, otherwise the single '*'.
    """
    if "*" not in ruri_user:
        return (None, ruri_user)
    # star-led prefix like *1001*1555...: two '*'. Separator is the last one.
    if ruri_user.startswith("*"):
        idx = ruri_user.rfind("*")
        return (ruri_user[:idx], ruri_user[idx + 1:])
    idx = ruri_user.find("*")
    return (ruri_user[:idx], ruri_user[idx + 1:])


def tech_prefix_checks(tp):
    failures = []
    checked = 0
    for c in tp.get("encode_cases", []):
        got = encode_ruri_user(c["tech_prefix"], c["dialed_e164"])
        checked += 1
        if got != c["ruri_user"]:
            failures.append(f"techprefix encode {c['name']}: got {got!r} expected {c['ruri_user']!r}")
    for c in tp.get("decode_cases", []):
        pfx, dialed = decode_ruri_user(c["ruri_user"])
        checked += 1
        if pfx != c["expect_tech_prefix"] or dialed != c["expect_dialed"]:
            failures.append(
                f"techprefix decode {c['name']}: got ({pfx!r},{dialed!r}) "
                f"expected ({c['expect_tech_prefix']!r},{c['expect_dialed']!r})"
            )
    return {"checked": checked, "failures": failures}


if __name__ == "__main__":
    sys.exit(run())
