# Change: Formalize Compatibility API Boundaries

## Why

Several Clankers surfaces are intentionally compatibility APIs: transcript records, provider/router compatibility adapters, legacy imports, and desktop session DTOs. They should stay available when explicitly opted in, but they must not become default green SDK contracts by accident.

## What Changes

- Add a general compatibility-boundary policy for optional support, compatibility aliases, legacy imports, and desktop/session adapter APIs.
- Require each compatibility API to name its owner, feature/import boundary, migration path, and serialization/behavior fixture.
- Extend inventories so default SDK exports and examples stay free of compatibility-only records unless explicitly documented.

## Impact

- **Files**: SDK inventory policy, message contract boundary rail, provider/router adapter parity rails, session transcript docs, and embedded examples.
- **Testing**: message contract boundary rail, provider/router parity rails, embedded SDK inventory/budget rails, and aggregate embedded SDK acceptance.
