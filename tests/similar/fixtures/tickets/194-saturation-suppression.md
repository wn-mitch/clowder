---
id: 194
title: saturation suppression elevates Patrol over Hunt under predator pressure
status: ready
cluster: ai-substrate
initiative: [colony-safety]
added: 2026-04-25
---

## Why

When Hunt and Forage scores are suppressed by need-saturation, the freed L3 bandwidth flows to Patrol rather than higher-tier dispositions like Mate or Mentor. Patrol exposes cats to ShadowFox ambush, which thins the labour pool and triggers downstream starvation 24k ticks later.

## Approach

Substrate axes need to price the predator-exposure cost of what they elevate, not just the cost of what they suppress. Add a predator-exposure consideration field to Patrol's L2 score.

## Verification

Focal-cat trace post-fix: Patrol score drops in cells with non-zero ShadowFox influence; Mate / Mentor scores rise in their place.
