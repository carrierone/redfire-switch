#!/usr/bin/env node
/*
 * Dependency-free contract validator for the redfire triple
 * (redfire-ucaas <-> redfire-switch <-> redfire-boss), Node.js edition.
 *
 * CommonJS so it can be both run as a CLI (`node contract_check.cjs`) and
 * required directly by ts-jest/CommonJS test suites in redfire-boss without
 * an ESM/CJS interop dance. This is a faithful port of contract_check.py so
 * that Node repos can validate the vendored contracts natively in CI/jest
 * without adding a JSON Schema dependency. The Python and Node validators
 * MUST agree; both are covered by the same fixtures and golden cases, and
 * both are hashed into MANIFEST.sha256 so neither can drift.
 *
 * Implements the subset of JSON Schema Draft-07 used by our contracts:
 * type, required, properties, additionalProperties, enum, pattern,
 * minLength, minimum, minItems, and array items. No third-party deps.
 *
 * Usage:
 *   node contract_check.cjs [--contracts DIR]
 * Exit non-zero on any failure. Prints a summary.
 */
"use strict";
const fs = require("node:fs");
const path = require("node:path");

const HERE = __dirname;
const DEFAULT_CONTRACTS = path.normalize(path.join(HERE, ".."));

function typeOk(value, t) {
  switch (t) {
    case "object":
      return value !== null && typeof value === "object" && !Array.isArray(value);
    case "array":
      return Array.isArray(value);
    case "string":
      return typeof value === "string";
    case "integer":
      return typeof value === "number" && Number.isInteger(value);
    case "number":
      return typeof value === "number";
    case "boolean":
      return typeof value === "boolean";
    case "null":
      return value === null;
    default:
      return true;
  }
}

function typeName(value) {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}

function deepEqual(a, b) {
  return JSON.stringify(a) === JSON.stringify(b);
}

// Return an array of error strings (empty means valid).
function validate(value, schema, p = "$") {
  const errors = [];

  const t = schema.type;
  if (t !== undefined) {
    const types = Array.isArray(t) ? t : [t];
    if (!types.some((one) => typeOk(value, one))) {
      errors.push(`${p}: expected type ${JSON.stringify(types)}, got ${typeName(value)}`);
      return errors; // further checks are unsafe if the type is wrong
    }
  }

  if ("enum" in schema && !schema.enum.some((e) => deepEqual(e, value))) {
    errors.push(`${p}: ${JSON.stringify(value)} not in enum ${JSON.stringify(schema.enum)}`);
  }

  if (typeof value === "string") {
    if ("minLength" in schema && value.length < schema.minLength) {
      errors.push(`${p}: string shorter than minLength ${schema.minLength}`);
    }
    if ("pattern" in schema && new RegExp(schema.pattern).test(value) === false) {
      errors.push(`${p}: ${JSON.stringify(value)} does not match pattern ${schema.pattern}`);
    }
  }

  if (typeof value === "number" && typeof value !== "boolean") {
    if ("minimum" in schema && value < schema.minimum) {
      errors.push(`${p}: ${value} below minimum ${schema.minimum}`);
    }
  }

  if (Array.isArray(value)) {
    if ("minItems" in schema && value.length < schema.minItems) {
      errors.push(`${p}: fewer than minItems ${schema.minItems}`);
    }
    if (schema.items) {
      value.forEach((item, i) => {
        errors.push(...validate(item, schema.items, `${p}[${i}]`));
      });
    }
  }

  if (value !== null && typeof value === "object" && !Array.isArray(value)) {
    const props = schema.properties || {};
    for (const req of schema.required || []) {
      if (!(req in value)) errors.push(`${p}: missing required property '${req}'`);
    }
    const addl = "additionalProperties" in schema ? schema.additionalProperties : true;
    for (const [k, v] of Object.entries(value)) {
      if (k.startsWith("_")) continue; // allow _invalid_reason / _comment annotations
      if (k in props) {
        errors.push(...validate(v, props[k], `${p}.${k}`));
      } else if (addl === false) {
        errors.push(`${p}: additional property '${k}' not allowed`);
      }
    }
  }

  return errors;
}

function loadJson(p) {
  return JSON.parse(fs.readFileSync(p, "utf8"));
}

/** Load a named contract schema (e.g. "boss_to_ucaas.provision_tenant"). */
function loadSchema(name, contracts = DEFAULT_CONTRACTS) {
  return loadJson(path.join(contracts, "schemas", name + ".schema.json"));
}

function schemaForFixture(contracts, fixtureName) {
  // fixtures are <contract>.valid.json / <contract>.invalid.json
  const parts = fixtureName.split(".");
  const base = parts.slice(0, parts.length - 2).join(".");
  return path.join(contracts, "schemas", base + ".schema.json");
}

// ---- reference tech-prefix codec (canonical PREFIX*E164 behaviour) ----

function encodeRuriUser(techPrefix, dialedE164) {
  if (techPrefix === null || techPrefix === undefined) return dialedE164;
  return `${techPrefix}*${dialedE164}`;
}

function decodeRuriUser(ruriUser) {
  if (!ruriUser.includes("*")) return [null, ruriUser];
  const idx = ruriUser.startsWith("*") ? ruriUser.lastIndexOf("*") : ruriUser.indexOf("*");
  return [ruriUser.slice(0, idx), ruriUser.slice(idx + 1)];
}

function techPrefixChecks(tp) {
  const failures = [];
  let checked = 0;
  for (const c of tp.encode_cases || []) {
    const got = encodeRuriUser(c.tech_prefix, c.dialed_e164);
    checked += 1;
    if (got !== c.ruri_user) {
      failures.push(`techprefix encode ${c.name}: got ${JSON.stringify(got)} expected ${JSON.stringify(c.ruri_user)}`);
    }
  }
  for (const c of tp.decode_cases || []) {
    const [pfx, dialed] = decodeRuriUser(c.ruri_user);
    checked += 1;
    if (pfx !== c.expect_tech_prefix || dialed !== c.expect_dialed) {
      failures.push(
        `techprefix decode ${c.name}: got (${JSON.stringify(pfx)},${JSON.stringify(dialed)}) ` +
          `expected (${JSON.stringify(c.expect_tech_prefix)},${JSON.stringify(c.expect_dialed)})`
      );
    }
  }
  return { checked, failures };
}

function runChecks(contracts = DEFAULT_CONTRACTS) {
  const schemasDir = path.join(contracts, "schemas");
  const fixturesDir = path.join(contracts, "fixtures");
  const failures = [];
  let checked = 0;

  // 1. Every schema must itself be valid JSON.
  for (const fn of fs.readdirSync(schemasDir).sort()) {
    if (!fn.endsWith(".schema.json")) continue;
    try {
      loadJson(path.join(schemasDir, fn));
    } catch (e) {
      failures.push(`schema ${fn} is not valid JSON: ${e.message}`);
    }
  }

  // 2. Every fixture validates (valid must pass, invalid must fail).
  for (const fn of fs.readdirSync(fixturesDir).sort()) {
    if (!fn.endsWith(".json")) continue;
    const schemaPath = schemaForFixture(contracts, fn);
    if (!fs.existsSync(schemaPath)) {
      failures.push(`fixture ${fn}: no schema at ${path.basename(schemaPath)}`);
      continue;
    }
    const schema = loadJson(schemaPath);
    const data = loadJson(path.join(fixturesDir, fn));
    const errs = validate(data, schema);
    checked += 1;
    const isInvalid = fn.endsWith(".invalid.json");
    if (isInvalid && errs.length === 0) {
      failures.push(`fixture ${fn}: expected to FAIL validation but passed`);
    }
    if (!isInvalid && errs.length > 0) {
      failures.push(`fixture ${fn}: expected to PASS but got errors: ${JSON.stringify(errs)}`);
    }
  }

  // 3. Tech-prefix golden cases: reference encode/decode round-trip.
  const tp = loadJson(path.join(contracts, "techprefix", "cases.json"));
  const fromTp = techPrefixChecks(tp);
  checked += fromTp.checked;
  failures.push(...fromTp.failures);

  return { checked, contracts, failures };
}

function main(argv) {
  let contracts = DEFAULT_CONTRACTS;
  const i = argv.indexOf("--contracts");
  if (i !== -1 && argv[i + 1]) contracts = argv[i + 1];

  const { checked, failures } = runChecks(contracts);
  console.log(`contract_check(node): ${checked} checks run against ${contracts}`);
  if (failures.length) {
    console.log(`FAILED (${failures.length}):`);
    for (const f of failures) console.log(`  - ${f}`);
    return 1;
  }
  console.log("OK: all contract checks passed");
  return 0;
}

module.exports = {
  validate,
  loadSchema,
  loadJson,
  runChecks,
  encodeRuriUser,
  decodeRuriUser,
  DEFAULT_CONTRACTS,
};

// Run as a CLI when invoked directly (not when required by jest).
if (require.main === module) {
  process.exit(main(process.argv.slice(2)));
}
