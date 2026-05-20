## Tasks

- [ ] [serial] V1: Run fixture `fixtures/provider/request_shape.json` with helper `assert_request_shape` for request shape [covers=provider-transport.request-shape]
- [ ] [serial] V2: Run command `cargo nextest run provider_stream_boundaries` for stream event boundary fixtures [covers=provider-transport.stream-boundary]
- [ ] [serial] V3: Run fixture `retry_policy_attempts.golden` with retry policy helper [covers=provider-transport.retry-policy]
- [ ] [serial] V4: Run scripts/check-redaction-receipts.rs fixture for redaction policy and receipt fields [covers=provider-transport.redaction,provider-transport.receipts]
- [ ] [serial] V5: Run fixture `catalog_visibility.golden` for discovery visibility [covers=provider-transport.discovery]
