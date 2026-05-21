## Decision
The response provider must preserve exact Codex request subcontracts: `text={"verbosity":"medium"}` by default with caller override behavior, marking the requested account active after successful login, entitlement probe retry and 401 refresh-retry probe headers, and raw SSE `response.function_call_arguments.delta` mapping into ordered tool-call delta stream boundaries.
