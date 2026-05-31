Evidence-ID: initial-cairn-gates
Task-ID: V1
Artifact-Type: command-log
Covers: remaining-coupling-drain.hotspot-inventory
Status: pass

# Initial Cairn Gates

Commands run from `/home/brittonr/git/clankers`:

```text
nix run .#cairn -- gate proposal drain-remaining-coupling-hotspots --root .
status: 0
verdict: PASS
receipt_hash: 9ced47ee94634ed04f47ccab80ebf5977656859afcfb40f1e439dd1d36d634b4

nix run .#cairn -- gate design drain-remaining-coupling-hotspots --root .
status: 0
verdict: PASS
receipt_hash: 388ecc7816c78e8eafa5d1668c97aff1a8c1b53da717045d52921b265a834f4f

nix run .#cairn -- gate tasks drain-remaining-coupling-hotspots --root .
status: 0
verdict: PASS
receipt_hash: eaedaac5aaee61943e91ccb0f6d7836058bc6b52ea57f9cac597aeaffee56a50

nix run .#cairn -- gate tasks drain-remaining-coupling-hotspots --root .
status: 0
verdict: PASS after checking I9/V1/V2
receipt_hash: b1d977ed21d4d6dc67b24e72585b69cbe39fbb26ac2685bc011ea5776f6774d0

nix run .#cairn -- validate --root .
status: 0
valid: true
changes: 1
specs_validated: 51
```
