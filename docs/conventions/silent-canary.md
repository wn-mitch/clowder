# Silent-canary surfaces are forbidden

Classifier functions whose *purpose* is detecting silent failures must not use catch-all arms. The catch-all IS the failure mode the classifier exists to catch — a new enum variant silently inherits the default and bypasses the canary.

## The rule

Functions like `expected_to_fire_per_soak`, `category`, `feature_name`, and the parallel shape for any future enum-keyed canary **must use exhaustive `match`** so adding a variant is a compile error until explicitly classified.

Examples of forbidden patterns:

```rust
// ❌ catch-all bypasses the canary
match feature {
    Feature::Specific => true,
    _ => true,  // new variants silently inherit
}

// ❌ same problem with default branches
fn category(f: Feature) -> Category {
    if f == Feature::A { return Category::X; }
    Category::Neutral  // catch-all
}
```

Use:

```rust
// ✅ adding a variant is a compile error
match feature {
    Feature::A => true,
    Feature::B => false,
    Feature::C => true,
    // no _ => arm
}
```

## Parallel-iteration arrays

Hand-maintained iteration arrays parallel to an enum (e.g., `Feature::ALL: &[Feature]` at `src/resources/system_activation.rs`) silently undercount when a variant is added. Guard with:

1. A coverage test asserting the array's length against a hand-maintained sentinel.
2. A uniqueness check via `std::mem::discriminant`.

Precedent test: `feature_all_is_exhaustive_and_unique` in `system_activation.rs`.

## When `linkme::distributed_slice` is the right tool

When an enum's declarations naturally distribute across many files, prefer `linkme::distributed_slice` outright. Each declaration site registers itself into the global slice; there's no parallel hand-maintained list to drift.

- **Fits when:** each declaration lives in its own file (precedent: `CAT_DSE_REGISTRY` per ticket 438).
- **Doesn't fit when:** the enum + all its variants live in a single file.

## Precedents

- **367 pre-Commit-4 amend** — `Feature::ALL` omission produced a false-negative never-fired canary.
- **368** — retired the `_ => true` catch-all in `expected_to_fire_per_soak` + added the `ALL` coverage test.
- **438** — the `CAT_DSE_REGISTRY` distributed-slice migration.
