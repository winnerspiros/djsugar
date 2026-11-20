# Clippy Pedantic Triage

This document records which clippy pedantic and nursery lints are candidates for
enabling in the workspace lint table (`Cargo.toml` `[workspace.lints.clippy]`).
Only lints the current code already satisfies are set to `deny` or `warn`; the
rest are listed here for future evaluation.

## Currently Enabled

- `dbg_macro = "deny"` — no `dbg!` macros in the codebase
- `todo = "deny"` — no `todo!` macros in the codebase

## Pedantic Lints Worth Enabling Later

| Lint | Reason | Blockers |
|------|--------|----------|
| `missing_docs_in_private_items` | Encourages documentation of internal APIs | Many private items lack docs; would need sweeping pass |
| `multiple_crate_versions` | Detects duplicate crate versions in deps | Some duplicates exist (e.g., `getrandom` 0.3 and 0.4); resolve via Cargo.toml alignment |
| `wildcard_enum_match_arm` | Catches non-exhaustive matches when new variants added | Several matches use `_` catch-all; convert to explicit variants |
| `unwrap_used` / `expect_used` | Forces proper error handling in non-test code | Many `unwrap`/`expect` in non-test code; replace with `?` or error types |
| `similar_names` | Finds confusingly similar identifiers | Some local variables differ only by case/underscore; rename |
| `module_name_repetitions` | Avoids stuttering in public APIs (e.g., `foo::Foo`) | Several public types repeat module name; consider re-exports or renames |
| `cast_possible_truncation` / `cast_precision_loss` / `cast_sign_loss` | Catches lossy numeric casts | DSP code has many `as f32`/`as i32` casts; audit each for correctness |
| `items_after_statements` | Enforces items before statements in blocks | A few blocks mix items and statements; reorder |
| `match_same_arms` | Detects duplicate match arms | Some matches have identical arms; deduplicate |
| `needless_pass_by_value` | Suggests passing by reference where appropriate | Several functions take owned values they don't consume; change to `&`/`&mut` |

## Nursery Lints Worth Enabling Later

| Lint | Reason | Blockers |
|------|--------|----------|
| `missing_docs` | Requires docs on all public items | Many public items undocumented; incremental pass needed |
| `unused_crate_dependencies` | Finds crates in Cargo.toml not used in code | Overlaps with cargo-machete; enable after machete clean |

## Lints Intentionally Not Enabled

| Lint | Reason |
|------|--------|
| `print_stderr` | Codebase uses `eprintln!` extensively for logging/debugging; not a style issue |
| `print_stdout` | Codebase uses `println!` in CLI/tools; acceptable there |
| `cognitive_complexity` | Many DSP/engine functions exceed threshold; refactoring is large scope |
| `too_many_lines` | Several functions exceed 100 lines; splitting harms readability in hot paths |

## Process

When enabling a new lint:
1. Add it to `[workspace.lints.clippy]` in root `Cargo.toml` at `warn` level
2. Run `cargo clippy --workspace --all-targets` locally
3. Fix all new warnings
4. Promote to `deny` once clean