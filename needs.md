- shadowfoxes need to have motivations that don't align cleanly onto the normal
  foxes

### G. Three permanently dead features in activation tracker (low)

- `FoxDenEstablished` — defined in Feature enum, `activation.record()` never called anywhere in code
- `FoxDenDefense` — same
- `CombatResolved` — consequence of bug D; `resolve_combat` never fires

These inflate `features_total` (57) without being able to activate, dragging down the activation ratio. `features_active` at 25/57 is artificially low.
