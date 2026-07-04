# Redfire Cross-Project Contracts

Single source of truth for the interface contracts that tie the three sibling
projects together:

- **redfire-ucaas** (this repo) — multitenant UCaaS control plane (Class 5 application layer: SIP registrar/edge, media, voicemail, apps, self-service portal).
- **redfire-switch** — Class 4 switch (Carrier One). Signs STIR/SHAKEN, terminates the tenant tech-prefix trunk.
- **redfire-boss** — B/OSS. Owns accounts, DIDs, billing, and rates the UCaaS usage feed.

The three form a triangle. Every edge is a contract, and every contract has a
schema, golden fixtures, and a CI check in each repo that touches it. This
folder is the authority. redfire-switch and redfire-boss vendor a copy (git
subtree, submodule, or CI fetch) and verify it against `MANIFEST.sha256`.

## The contracts (triangle edges)

| Edge | Direction | Contract | Enforced in |
| --- | --- | --- | --- |
| BOSS -> UCaaS | provision/update/suspend tenant | `schemas/boss_to_ucaas.provision_tenant.schema.json` | boss (producer), ucaas (consumer) |
| BOSS -> UCaaS | assign/unassign DID | `schemas/boss_to_ucaas.assign_did.schema.json` | boss (producer), ucaas (consumer) |
| UCaaS -> BOSS | billable usage record (revenue) | `schemas/ucaas_to_boss.usage_record.schema.json` | ucaas (producer), boss (consumer) |
| UCaaS <-> Switch | trunk auth / tenant identification | `schemas/ucaas_switch.trunk_auth.schema.json` | ucaas (producer), switch (consumer) |
| UCaaS <-> Switch | tech-prefix wire format | `techprefix/cases.json` | ucaas (encoder), switch (decoder) |

Identifier spine that must stay consistent across all three:

```
boss accounts.id  ==  ucaas tenant.boss_account_id
boss voice_accounts.switchAccountId  ==  switch IpAuthConfig.customer_id  ==  ucaas TenantCarrierBinding.boss_switch_account_id
tenant tech_prefix  (assigned by ucaas/boss)  ==  switch IpAuthConfig.required_tech_prefix
boss dids.id  ==  ucaas did_assignment.boss_did_id
```

## Running the checks

```bash
# validate schemas, fixtures, and tech-prefix round-trip (no dependencies)
python3 contracts/tools/contract_check.py     # Python reference validator
node    contracts/tools/contract_check.cjs    # Node reference validator (must agree)

# regenerate the drift manifest after intentional changes
python3 contracts/tools/manifest.py --write

# verify a vendored copy has not drifted (used by switch/boss CI)
python3 contracts/tools/manifest.py --check
```

Both validators are intentionally dependency-free (Python stdlib / Node stdlib
only) so they run in any repo's CI, including the Rust and Node repos. They
implement the same Draft-07 subset and are covered by the same fixtures, so a
divergence between them is itself a test failure.

## How each repo uses this

- **redfire-ucaas**: source of truth. CI runs both validators and asserts the
  manifest is current. Producer-side tests assert generated payloads validate.
- **redfire-switch**: vendors `contracts/`, runs `manifest.py --check` for
  drift, and its Rust test suite feeds `techprefix/cases.json` through the real
  `extract_tech_prefix` parser so encode/decode round-trips with the UCaaS side.
- **redfire-boss**: vendors `contracts/`, runs `manifest.py --check`, and a jest
  test validates the adapter's outbound provision/DID payloads and asserts the
  usage records it ingests match `ucaas_to_boss.usage_record`.

## Changing a contract

1. Edit the schema/fixtures/cases here.
2. Both validators must pass and both a valid and invalid fixture must exist.
3. `manifest.py --write` to refresh checksums.
4. Bump the contract `version`, and open coordinated PRs in the consuming repos.
   Their `manifest.py --check` will fail until they re-sync, which is the point.
