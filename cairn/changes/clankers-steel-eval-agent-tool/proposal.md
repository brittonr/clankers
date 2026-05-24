## Why

Clankers already has reviewed Steel runtime, orchestration, and turn-planning seams, but the accepted `steel-lisp-runtime` contract still treats an agent-visible `steel_eval` surface as optional. Giving Clankers a Steel tool is the smallest next dogfood slice: agents can ask the constrained embedded Steel runtime to evaluate bounded Scheme snippets without gaining ambient filesystem, process, network, provider, daemon, TUI, credential, or native-tool authority.

## What Changes

- Add a reviewed `steel-eval-agent-tool` capability for a built-in `steel_eval` tool that reuses the existing Clankers Steel runtime wrapper.
- Define the tool request/response/receipt contract, including source/profile inputs, bounded output, stable issue codes, and redaction.
- Require the tool to register consistently in standalone, daemon, attach, and disabled-tool flows without bypassing policy.
- Require deterministic positive/negative fixtures for pure eval, denied host functions, limit failures, redaction, and daemon tool-list parity.

## Impact

- **Files**: Steel runtime/tool host modules, built-in tool registry, daemon `ToolList`/disabled-tool rebuild paths, runtime receipts/tests, and lifecycle specs.
- **Testing**: Focused Rust tests for tool request validation, wrapper delegation, denial/limit/redaction receipts, registry/disabled-tool parity, plus Cairn validate/gate receipts before implementation.
- **Out of scope**: General-purpose OS sandbox claims, arbitrary host effects, Steel self-mutation expansion, network/process/filesystem tools, provider access, and making Steel the default planner for additional seams.
