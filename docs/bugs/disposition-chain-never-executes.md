# Bug: resolve_disposition_chains matches zero cats — hunting/foraging movement never executes

## Symptom

Cats are 98% stationary per-tick. The `resolve_disposition_chains` system's query matches **zero entities every tick**, confirmed by logging. All the scent-based hunting, active foraging, and prey AI code we wrote never runs.

## What we know

### Logging confirmed:
- `evaluate_dispositions` sees 8 cats (correct) and calls `commands.entity(entity).insert(Disposition::new(...))` every tick
- `disposition_to_chain` query `(With<Disposition>, Without<TaskChain>)` matches **0 cats** every tick
- `resolve_disposition_chains` query `(&mut TaskChain, ...)` matches **0 cats** every tick
- Cats permanently show 8 without disposition in `evaluate_dispositions`

### Root cause hypothesis: deferred commands never flush between disposition systems

Bevy commands are deferred. The disposition pipeline is:

1. `evaluate_dispositions` — inserts `Disposition` via `commands` (deferred)
2. `disposition_to_chain` — queries `With<Disposition>` (never sees it)
3. `resolve_disposition_chains` — queries `&mut TaskChain` (never sees it)

These are registered with `.after()` ordering constraints, NOT `.chain()`. Bevy's `.chain()` inserts `ApplyDeferred` between systems automatically. `.after()` does NOT.

We tried adding an explicit `ApplyDeferred` system between `disposition_to_chain` and `resolve_disposition_chains`, but we also need one between `evaluate_dispositions` and `disposition_to_chain`. Without it, the `Disposition` component inserted in step 1 is invisible to step 2.

But even with `ApplyDeferred`, the Disposition is being inserted and apparently never sticking — `evaluate_dispositions` sees 8 cats WITHOUT disposition on every subsequent tick too, meaning the insert from the previous tick didn't persist.

### Possible explanations (not yet confirmed):
1. **`evaluate_actions` (old system in ai.rs) is competing** — it also queries `Without<Disposition>` and sets `current.action`. Both run as standalone systems on the same cats. The old system might be handling cats before `evaluate_dispositions` gets to them, or overwriting state.
2. **`check_anxiety_interrupts` strips Dispositions** — it runs BEFORE `evaluate_dispositions` and removes Disposition+TaskChain for cats with critical needs or nearby threats. If starvation/exhaustion triggers are too sensitive, it might strip every disposition immediately.
3. **Commands from `evaluate_dispositions` are being overwritten** — if `evaluate_actions` also inserts commands on the same entity in the same tick, the last-writer-wins behavior in Bevy could drop the Disposition insert.
4. **`ApplyDeferred` placement is wrong** — we added it between d2c and resolve, but we need it between eval and d2c too.

## Fix plan

### Step 0: Add `tracing` / `eprintln` logging to EVERY disposition system

Before debugging anything else, instrument all four systems so we can see the full pipeline in one run. Use `eprintln!` gated on a tick counter (first 200 ticks). Log:

**`check_anxiety_interrupts`:**
- How many cats matched (With<Disposition>)
- For each: position, disposition kind, whether interrupt fires, interrupt reason

**`evaluate_dispositions`:**  
- How many cats matched (Without<Disposition>)
- For each: position, current action, ticks_remaining, chosen disposition
- Whether `current.ticks_remaining != 0` causes early skip (the system skips cats whose action hasn't finished)

**`disposition_to_chain`:**
- How many cats matched (With<Disposition>, Without<TaskChain>)  
- For each: position, disposition kind, whether chain was built, chain step count

**`resolve_disposition_chains`:**
- How many cats matched (has TaskChain)
- For each: position, current step kind, phase (search/stalk/pounce), movement result

**`evaluate_actions` (ai.rs — the OLD system):**
- How many cats matched
- Whether it's setting actions that compete with the disposition system

After running with logs, delete the log file:
```bash
cargo run -- --headless --duration 5 --seed 42 --event-log /dev/null --log /dev/null 2> /tmp/disp_debug.log
head -200 /tmp/disp_debug.log
rm /tmp/disp_debug.log
```

### Step 1: Add ApplyDeferred between evaluate_dispositions and disposition_to_chain

Both in `src/plugins/simulation.rs` and `src/main.rs` headless schedule:

```rust
// After evaluate_dispositions, flush commands so Disposition is visible to disposition_to_chain
app.add_systems(
    FixedUpdate,
    ApplyDeferred
        .after(systems::disposition::evaluate_dispositions)
        .before(systems::disposition::disposition_to_chain),
);
```

We already added one between `disposition_to_chain` and `resolve_disposition_chains`. We need BOTH.

### Step 2: Check if evaluate_dispositions has an early-return gate

Read `evaluate_dispositions` carefully. The system likely has:
```rust
if current.ticks_remaining != 0 {
    continue; // Skip cats whose action is still running
}
```

If `evaluate_actions` (the old system) sets `ticks_remaining` to a non-zero value, `evaluate_dispositions` will skip that cat forever. Check whether `evaluate_actions` is the one actually running cat behavior, and `evaluate_dispositions` never gets a chance because ticks_remaining is always non-zero.

### Step 3: Determine if evaluate_actions should be removed

`evaluate_actions` in `ai.rs` is the OLD action system. `evaluate_dispositions` was supposed to replace it. But both are registered:
- `simulation.rs:137` — `systems::ai::evaluate_actions`  
- `simulation.rs:107` — `systems::disposition::evaluate_dispositions`

They both query `Without<Disposition>` cats. If `evaluate_actions` is handling all cats and keeping them busy (setting ticks_remaining > 0), then `evaluate_dispositions` never fires for those cats.

**Likely fix:** `evaluate_actions` needs to be gated or removed for cats that should use the disposition system. Either:
- Add a component marker to distinguish disposition-driven cats from legacy cats
- Or remove `evaluate_actions` entirely if all cats should use dispositions

### Step 4: Verify with traces

After fixing, run:
```bash
cargo run -- --headless --duration 10 --seed 42 --trace-positions 1 \
  --event-log /tmp/trace.jsonl --log /dev/null

cat /tmp/trace.jsonl | python3 -c "
import json, sys, collections
trails = collections.defaultdict(list)
for line in sys.stdin:
    e = json.loads(line)
    if e.get('type') == 'PositionTrace':
        trails[e['cat']].append((e['tick'], tuple(e['position']), e['action']))
for cat in sorted(trails):
    t = trails[cat]
    stationary = sum(1 for i in range(1, len(t)) if t[i][1] == t[i-1][1])
    n = len(t) - 1
    print(f'{cat}: {100*stationary/max(n,1):.0f}% stationary')
"
rm /tmp/trace.jsonl
```

Target: < 30% stationary.

### Step 5: Clean up debug logging

Remove all `eprintln!` debug statements added in Step 0.

## Files involved

| File | Role |
|------|------|
| `src/systems/disposition.rs` | All four disposition systems: check_anxiety_interrupts, evaluate_dispositions, disposition_to_chain, resolve_disposition_chains |
| `src/systems/ai.rs:291` | `evaluate_actions` — the OLD action system, likely competing |
| `src/plugins/simulation.rs` | System registration + ApplyDeferred placement |
| `src/main.rs` | Headless schedule — must mirror simulation.rs exactly |

## Current state of debug logging

These `eprintln!` lines are currently in the code (added during this session, should be cleaned up as part of the fix):

- `disposition.rs` — `[EVAL]` in evaluate_dispositions (cat count + food)
- `disposition.rs` — `[EVAL-INSERT]` at Disposition insert point
- `disposition.rs` — `[D2C]` in disposition_to_chain (cat count)
- `disposition.rs` — `[DISP-CHAINS]` in resolve_disposition_chains (cat count)
- `disposition.rs` — `[DISP-SKIP]` when step is not a disposition step
- `disposition.rs` — `[HUNT]` at HuntPrey handler entry
- `disposition.rs` — `[HUNT-SEARCH]` in Search phase movement
- `disposition.rs` — `[ANXIETY-STRIP]` when anxiety interrupt removes disposition

## Quick reproduction

```bash
cargo run -- --headless --duration 2 --seed 42 --event-log /dev/null --log /dev/null 2>&1 | grep -E "\[EVAL\]|\[D2C\]|\[DISP-CHAINS\]" | head -10
```

Expected output showing the bug:
```
[EVAL] cats_without_disposition=8 ...
[D2C] cats_with_disposition_no_chain=0
[DISP-CHAINS] tick=100010 matched_cats=0
```

The 8→0→0 pipeline is the bug. Dispositions are inserted but never visible to downstream systems.
