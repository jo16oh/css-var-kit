# Instructions

- In this project, we do not use mod.rs for declaring Rust modules. Instead, we place module_name.rs alongside the module_name directory at the same level. This is the recommended practice in Rust. Under no circumstances should you write a mod.rs file.

- As a general rule, write code in a functional style utilizing method chaining. An imperative style using mut is only permitted when justified by specific performance or logical reasons.

- For the sake of clarity, annotate any lifetimes originating from the loaded CSS files with the name 'src.

- Leverage the lightningcss or cssparser crates, and avoid custom implementations for logic as much as possible.

- Avoid redundant comments; instead, convey your intent through symbol naming. Write comments only when the logic becomes complex.
