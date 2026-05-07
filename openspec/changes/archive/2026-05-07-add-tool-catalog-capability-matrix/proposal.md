## Why

Tool catalog decoupling introduced capability packs, disabled-tool filtering, custom host tools, and explicit extension publication. Current checks prove important slices, but not the combined policy matrix that embedders will actually exercise.

## What Changes

- Add a matrix for tool catalog pack combinations, disabled filters, custom tool collisions, side-effect classes, and absent/present extension services.
- Verify dangerous packs remain opt-in and independent when combined.
- Keep metadata-only catalog queries free of runtime startup side effects.

## Capabilities

### Modified Capabilities
- `tool-host-embedding`: catalog construction gets combined feature-matrix acceptance.

## Impact

- **Files**: catalog builder tests, matrix fixtures/checker, embedded SDK acceptance script.
- **APIs**: no public runtime API changes intended.
- **Testing**: focused catalog tests plus embedded SDK acceptance.
