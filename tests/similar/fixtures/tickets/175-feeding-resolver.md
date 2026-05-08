---
id: 175
title: feeding resolver double-deducts food on multi-kitten meals
status: done
cluster: null
added: 2026-03-15
landed-at: def456
landed-on: 2026-03-20
---

## Why

The feed-kitten resolver iterates kittens and decrements the colony food stockpile for each kitten in the meal, but the witnessed feature emission already accounts for the meal once. This caused food bookkeeping to drift negative under heavy kitten cohorts.

## Approach

Move the stockpile decrement out of the per-kitten loop and into the witness path so it fires exactly once per meal regardless of kitten count.

## Verification

Soak with a litter of 6 kittens — pre-fix shows negative food; post-fix shows monotone-decreasing food matching the per-meal cost.
