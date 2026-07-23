# Benchmark Protocol

## Objective

Measure whether CodeSpace reduces context volume, tool calls, latency, and task cost without reducing solution correctness.

## Required controls

- Pin repository and benchmark harness commits.
- Use identical model, model version, temperature, max output, system prompt, and tool permissions.
- Clear agent conversation state between trials.
- Run at least 20 trials per task/configuration.
- Publish raw prompts, tool traces, token accounting, wall-clock timings, exit status, and correctness judgments.
- Separate cold-index cost from warm-query cost.

## Test arms

1. Baseline agent with file search/read tools only.
2. Agent with CodeSpace MCP tools available but no mandatory instruction.
3. Agent instructed to call `cse_context` before repository exploration.

## Metrics

### Critical

- Task pass rate against deterministic tests.
- Input tokens and cached input tokens.
- Total tool calls and file-read calls.
- Secret leakage count.

### Important

- Median and p95 wall-clock latency.
- Median context bytes returned by CodeSpace.
- Index build time and incremental update time.
- Impact recall against a manually labeled change set.

### Optional

- Developer-rated usefulness.
- False-positive edge rate by language.
- Cost per successfully completed task.

## Claim rules

A claim such as “70% fewer tokens” is valid only for the published benchmark scope. Do not generalize a result from one repository, task, model, or language to the entire product.

## Built-in microbenchmark

```bash
cse benchmark --query "authentication context" --iterations 100
```

This measures warm local query execution only. It does not measure model quality or end-to-end development speed.
