# LLM agent systems and semantic debugging

Vibe must be able to build LLM agents—including coding agents that create and
modify other agents—without reducing them to untyped prompt strings and logs.
Agent behavior is a typed, inspectable state machine connected to the same
semantic graph and transaction system used for ordinary programs.

## First-class objects

The semantic model adds `agent`, `role`, `prompt_template`, `model_endpoint`,
`message_schema`, `tool`, `capability`, `policy`, `memory`, `task`, `plan`,
`trace`, `evaluation`, `fixture`, and `incident`. Prompts are versioned templates
with typed inputs/outputs. A model call states provider capability requirements,
sampling configuration, context inputs, output schema, budget, retry policy, and
effects. Provider-specific names stay in deployment configuration.

Tools are typed functions with effect and capability declarations. Filesystem,
network, process, secret, external-write, and human-approval effects are explicit.
An agent receives a least-privilege capability set; it cannot acquire a capability
by generating text. Tool results are typed values, not trusted instructions.

An agent may propose semantic transactions that add or modify another agent's
prompt, policy, tools, evaluation suite, or implementation. The normal transaction
rules still apply: expected revisions, preserved contracts, mandatory evaluations,
semantic diff, atomic commit, and rollback. Self-modification is never a special
back door.

## Execution and debugging

Each run emits an append-only causal trace of externally meaningful events:

```text
input -> state transition -> model request metadata -> structured response
      -> policy decision -> tool request/result -> semantic transaction
      -> validation/evaluation -> output
```

The trace records object revisions, sanitized messages where policy permits,
tool arguments/results, effects, timings, token/resource usage, errors, retries,
state diffs, and random/provider request identifiers. It does **not** require or
store hidden chain-of-thought. Concise declared rationale and citations may be
stored as ordinary outputs.

Debugging commands operate on semantic events:

```text
agent show @agent.coder
trace show @run.2026_09_11
trace why @event.tool_call_17
trace dataflow @input.issue -> @output.patch
replay @run.2026_09_11 --stub-model --from @event.tool_call_12
compare @run.a @run.b
counterfactual @run.a --model deploy.model_b
eval @agent.coder --suite @eval.agent_regression
```

Deterministic replay substitutes recorded model/tool results and re-executes the
orchestrator. Live replay explicitly marks non-repeatable external state. Breakpoints
can target object identity, state transition, effect, policy denial, validation
failure, token budget, or repeated tool-call pattern. A debugger shows current
typed state, reachable capabilities, pending obligations, causal predecessors,
and the first semantic divergence between two runs.

## Reliability and evaluation

Agent correctness is distributional and adversarial, not established by one run.
Evaluation suites contain pinned tasks, fixtures, graders, invariants, budgets,
and expected effect ceilings. They measure task success, unsafe actions prevented,
invalid tool calls, semantic-edit validity, regressions introduced/caught, repair
iterations, context/tokens, latency, and cost. Coding-agent tests seed unit, shape,
precision, permission, prompt-injection, stale-context, and concurrency failures.

Production release gates compare candidate and baseline agents with confidence
intervals. Traces are privacy-classified, secrets are redacted at collection, and
retention is policy-controlled. No debugger feature may turn secret or private
model reasoning into telemetry.

