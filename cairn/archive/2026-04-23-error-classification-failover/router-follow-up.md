# Router follow-up note

This change intentionally stops at current-repo provider/agent wiring.

## Deferred external work
- Thread `ClassifiedError` through the external `clanker-router` crate's public error surfaces.
- Revisit router-side routing/fallback policy once current-repo classification semantics stabilize.

## Reason
- `clanker-router` is an external crate and was scoped out of this change in `proposal.md` and `design.md`.
- Current repo now exports `ClassifiedError` and uses it internally, which is the prerequisite for a follow-up external-router change.
