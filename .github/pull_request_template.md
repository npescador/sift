## Description

<!-- What does this PR do? Why is it needed? Be specific. -->

## Type of Change

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that changes existing behavior)
- [ ] Documentation update
- [ ] Refactor (no functional changes)
- [ ] Test addition or fix
- [ ] CI / tooling change

## Related Issues

<!-- Link issues this PR resolves or relates to -->
<!-- Examples: Fixes #123 | Closes #456 | Related to #789 -->

## What Changed

<!-- A concise bullet list of the key changes. Reviewers should understand the scope at a glance. -->

- 

## Testing

<!-- How was this change tested? What test cases were added or verified? -->

- [ ] Unit tests added / updated
- [ ] Integration tests added / updated
- [ ] Manual validation performed (describe below)

**Manual validation:**
<!-- Describe the commands you ran and what you observed -->

## Exit Code Behavior

<!-- If this PR touches the executor, command routing, or passthrough logic: -->
- [ ] Exit code behavior is unchanged
- [ ] Exit code behavior change is intentional (explain above)
- [ ] Not applicable

## Pre-Submit Checklist

- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] No `unwrap()` added to production paths without documented justification
- [ ] `CHANGELOG.md` updated (if user-facing change)
- [ ] Documentation updated (if behavior or API changed)
