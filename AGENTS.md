# PartyLine repository instructions

## Agent parity

Agent parity is part of the definition of done.

- Every new or changed human-facing action and meaningful app state must have an equivalent through PartyLine's agent tools.
- Human controls and agent tools must use the same underlying logic, validation, persistence, and events.
- Update the tool schemas, native adapter, permission rules, parity inventory, and tests in the same change as the feature.
- Expose progress, completion, errors, and recovery actions—including operations that finish asynchronously.
- Keep agent actions visible in the human UI. Preserve human-controlled access permissions. Never return or log API-key values.
- Maintain [docs/partyline-agent-control.md](docs/partyline-agent-control.md) as the UI/native-command-to-tool inventory. Record any unsupported behavior explicitly.
- Test behavior, not merely command-name coverage: inputs, results, state changes, errors, and permissions must agree.
- Maintain the CI inventory-drift check and run the agent-control tests whenever relevant UI or native behavior changes.
- A feature is incomplete until its agent equivalent and tests work. Report unresolved parity gaps in the handoff and PR description.

The checks and their limits are documented in the inventory's **Maintenance and CI** section. Use the isolated tests while a recording is active. Do not launch, restart, repackage over, or change the running app to validate a source change during a recording.
