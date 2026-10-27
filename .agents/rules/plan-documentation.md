# Rule: Plan Documentation in docs/plans

Whenever formulating, drafting, or updating any task or architectural plan:

1. **Mandatory Repository Sync**: Do not keep implementation plans solely in transient brain artifacts or chat outputs. Every plan MUST be recorded as a Markdown file in the repository at:
   ```
   docs/plans/YYYYMMDDSeq-<artifact-name>.md
   ```
2. **Naming Convention**:
   - `YYYYMMDD`: 8-digit date representation using local date (e.g., `20261027`).
   - `Seq`: 2-digit zero-padded incremental sequence per day (e.g. `01`, `02`, ..., `17`). The first file of the day is `01` (e.g., `docs/plans/2026102701-...md`), the 17th is `17` (e.g., `docs/plans/2026102717-...md`). Inspect existing files in `docs/plans/` for the current date to select the next sequential number.
   - `<artifact-name>`: Hyphen-separated (kebab-case) short descriptive title of the initiative (e.g. `collection-runner-phase6`, `graphql-support`, `scope-settings-modularization`).
3. **Plan Content Checklist**:
   - **Goal & Context**: Clear background reference to specs (`docs/specs/`) or debt items (`docs/technical-debt.md`).
   - **Architectural Decisions**: Impacted modules, state management entity changes, event flows, and persistence targets.
   - **Step-by-Step Implementation Tasks**: Concrete, incremental phases with verification milestones.
   - **Risk Mitigation & Verification Steps**: Edge cases handled and exact `cargo test` / `cargo clippy` commands to validate completion.
