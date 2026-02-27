# DriveWipe Commit Guide

This project uses an automated versioning system based on **Conventional Commits** and **LOC (Lines of Code)** thresholds. To ensure versions are bumped correctly, please follow these guidelines.

## Commit Message Format

```text
type(scope): description

[optional body]

[optional footer(s)]
```

### 1. Types
The `type` determines the baseline version bump:

| Type | Version Bump | Description |
| :--- | :--- | :--- |
| `feat` | **Minor** | A new feature for the user. |
| `fix` | **Patch** | A bug fix. |
| `chore` | **Patch** | Maintenance, dependencies, or internal changes. |
| `refactor`| **Patch** | Code change that neither fixes a bug nor adds a feature. |
| `style` | **Patch** | Changes that do not affect the meaning of the code (white-space, formatting, etc). |
| `test` | **Patch** | Adding missing tests or correcting existing tests. |

### 2. Scopes
The `scope` tells the system **which crate** to version. Use the following:
- `core`: For `crates/drivewipe-core`
- `cli`: For `crates/drivewipe-cli`
- `tui`: For `crates/drivewipe-tui`
- `gui`: For `crates/drivewipe-gui`

**Example:** `feat(cli): add colorful progress bars`

### 3. Major Releases & Breaking Changes
Major version bumps are **manual**. To trigger a Major bump (e.g., `0.1.0` -> `1.0.0`), you must include one of these keywords in the message or footer:
- `Major-Release`
- `BREAKING CHANGE`

**Example:**
```text
feat(core): Major-Release redesign drive enumeration API

This completely changes how drives are detected and is not backwards compatible.
```

---

## The LOC Safety Trigger (The "2026 Standard")

Our automation includes a "Safety Trigger" that promotes a version bump if the code change is significantly large, regardless of the commit type:

- **> 250 Lines Changed**: Automatically promoted to a **Patch** (if it wasn't already).
- **> 1,000 Lines Changed**: Automatically promoted to a **Minor** (if it wasn't already).

*Note: Major bumps are never automated by LOC; they always require a manual keyword.*

## Examples

| Commit Message | Lines Changed | Resulting Bump |
| :--- | :--- | :--- |
| `fix(core): small typo` | 2 | `core` Patch |
| `fix(core): massive refactor` | 1,200 | `core` **Minor** (LOC Trigger) |
| `feat(tui): new dashboard` | 300 | `tui` Minor |
| `chore(cli): cleanup` | 300 | `cli` **Patch** (LOC Trigger) |
| `feat(gui): Major-Release` | 10 | `gui` **Major** (Manual Trigger) |
