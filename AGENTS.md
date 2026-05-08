# Agent Guidelines

## Karpathy Guidelines

These guidelines reduce common LLM coding mistakes. They apply when writing,
reviewing, debugging, or refactoring code.

Tradeoff: they bias toward caution over speed. For trivial tasks, use judgment.

### Think Before Coding

Do not assume, hide confusion, or silently pick an interpretation.

- State assumptions explicitly before implementing.
- If multiple interpretations exist, present them instead of choosing silently.
- If a simpler approach is sufficient, say so and use it.
- If something is unclear, stop, name the confusion, and ask.

### Simplicity First

Write the minimum code that solves the problem. Do not add speculative behavior.

- Do not add features beyond what was requested.
- Do not create abstractions for single-use code.
- Do not add flexibility or configurability that was not requested.
- Do not add error handling for impossible scenarios.
- If the solution is much longer than necessary, simplify it.

### Surgical Changes

Touch only what is required. Clean up only changes introduced by the current
task.

- Do not improve adjacent code, comments, or formatting unless required.
- Do not refactor unrelated code.
- Match the existing style, even if another style would be preferred.
- If unrelated dead code is noticed, mention it instead of deleting it.
- Remove imports, variables, or functions made unused by the current change.
- Do not remove pre-existing dead code unless explicitly asked.

Every changed line should trace directly to the user's request.

### Goal-Driven Execution

Turn tasks into verifiable goals and loop until verified.

- For bugs, reproduce the issue before fixing it when practical.
- For validation work, cover invalid inputs and confirm the expected behavior.
- For refactors, preserve behavior and run relevant checks before and after when
  practical.
- For multi-step tasks, state a brief plan with verification for each step.

Example plan format:

```text
1. Identify relevant code -> verify: confirm call path and current behavior
2. Add focused coverage -> verify: test fails for the target issue
3. Implement the minimal fix -> verify: focused test passes
4. Run relevant checks -> verify: no regression
```
