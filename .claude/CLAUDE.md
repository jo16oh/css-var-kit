# Instructions

- In this project, we do not use mod.rs for declaring Rust modules. Instead, we place module_name.rs alongside the module_name directory at the same level. This is the recommended practice in Rust. Under no circumstances should you write a mod.rs file.

- As a general rule, write code in a functional style utilizing method chaining. An imperative style using mut is only permitted when justified by specific performance or logical reasons.

- For the sake of clarity, annotate any lifetimes originating from the loaded CSS files with the name 'src.

- Leverage the lightningcss or cssparser crates, and avoid custom implementations for logic as much as possible.

- Avoid redundant comments; instead, convey your intent through symbol naming. Write comments only when the logic becomes complex.

# Releasing

1. Write the release notes for users, not reviewers, in a file outside the repo.
2. Run `just bump-version <level> notes.md --yes`, where `<level>` is the release type I give, such
   as `patch`. It pushes a release branch and opens its PR with the file as the body. Arguments after
   the file go to bumpp.
3. I merge the PR. The release workflow tags the merge commit, publishes the packages, and creates
   the GitHub release. The notes start with the PR body, followed by the PRs merged since the last
   tag.

Step 2 pushes and opens a PR. Run it only on my explicit go-ahead.
