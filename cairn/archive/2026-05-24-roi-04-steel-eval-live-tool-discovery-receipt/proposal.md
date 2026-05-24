# Change: Steel eval live tool discovery receipt

## Why

Unit tests prove registration, but a product-level receipt should prove an actual Clankers runtime/tool-list path sees `steel_eval` by default and hides it through opt-out or disabled-tool policy.

## What Changes

- Add or run a live/product-level discovery receipt for default `steel_eval` publication.
- Cover default-present and explicit-hidden/disabled-hidden paths.
- Keep the receipt safe: metadata only, no raw prompts/secrets, no host authority.

## Non-Goals

- No new Steel authority.
- No broad daemon protocol redesign.
- No reliance on network or external credentials for the core discovery proof.
