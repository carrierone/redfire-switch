# Redfire Cross-Project Contracts

Single source of truth for the interface contracts that tie the three sibling
projects together:

- **redfire-pbx** (this repo) — Class 5 PBX control plane.
- **redfire-switch** — Class 4 switch (Carrier One). Signs STIR/SHAKEN, terminates the tenant tech-prefix trunk.
- **redfire-boss** — B/OSS. Owns accounts, DIDs, billing, and rates the PBX usage feed.

The three form a triangle. Every edge is a contract, and every contract has a
schema, golden fixtures, and a CI check in each repo that touches it. This
folder is the authority. redfire-switch and redfire-boss vendor a copy (git
subtree, submodule, or CI fetch) and verify it against `MANIFEST.sha256`.

## The contracts (triangle edges)

| Edge | Direction | Contract | Enforced in |
| --- | --- | --- | --- |
| BOSS -> PBX | provision/update/suspend tenant | `schemas/boss_to_pbx.provision_tenant.schema.json` | boss (producer), pbx (consumer) |
| BOSS -> PBX | assign/unassign DID | `schemas/boss_to_pbx.assign_did.schema.json` | boss (producer), pbx (consumer) |
| PBX -> BOSS | billable usage record (revenue) | `schemas/pbx_to_boss.usage_record.schema.json` | pbx (producer), boss (consumer) |
| PBX <-> Switch | trunk auth / tenant identification | `schemas/pbx_switch.trunk_auth.schema.json` | pbx (producer), switch (consumer) |
| PBX <-> Switch | tech-prefix wire format | `techprefix/cases.json` | pbx (encoder), switch (decoder) |

Identifier spine that must stay consistent across all three:

```
boss accounts.id  ==  pbx tenant.boss_account_id
boss voice_accounts.switchAccountId  ==  switch IpAuthConfig.customer_id  ==  pbx TenantCarrierBinding.boss_switch_account_id
tenant tech_prefix  (assigned by pbx/boss)  ==  switch IpAuthConfig.required_tech_prefix
boss dids.id  ==  pbx pbx_did_assignment.boss_did_id
```

## Running the checks

```bash
# validate schemas, fixtures, and tech-prefix round-trip (no dependencies)
python3 contracts/tools/contract_check.py

# regenerate the drift manifest after intentional changes
python3 contracts/tools/manifest.py --write

# verify a vendored copy has not drifted (used by switch/boss CI)
python3 contracts/tools/manifest.py --check
```

`contract_check.py` is intentionally dependency-free (Python stdlib only) so it
runs in any repo's CI, including the Rust and Node repos.

## How each repo uses this

- **redfire-pbx**: source of truth. CI runs `contract_check.py` and asserts the
  manifest is current. Producer-side tests assert generated payloads validate.
- **redfire-switch**: vendors `contracts/`, runs `manifest.py --check` for
  drift, and its Rust test suite feeds `techprefix/cases.json` through the real
  `extract_tech_prefix` parser so encode/decode round-trips with the PBX.
- **redfire-boss**: vendors `contracts/`, runs `manifest.py --check`, and a jest
  test validates the adapter's outbound provision/DID payloads and asserts the
  PBX usage records it ingests match `pbx_to_boss.usage_record`.

## Changing a contract

1. Edit the schema/fixtures/cases here.
2. `contract_check.py` must pass and both a valid and invalid fixture must exist.
3. `manifest.py --write` to refresh checksums.
4. Bump the contract `version`, and open coordinated PRs in the consuming repos.
   Their `manifest.py --check` will fail until they re-sync, which is the point.
