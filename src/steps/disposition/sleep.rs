use crate::components::physical::Needs;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Sleep`
///
/// **Real-world effect** — every tick, raises `needs.energy` and
/// `needs.temperature` by the configured per-tick deltas. Advances
/// when `ticks >= duration`.
///
/// **Plan-level preconditions** — emitted under `ZoneIs(ResidenceZone)`
/// by `src/ai/planner/actions.rs::sleeping_actions`. Duration is
/// passed in by the caller (varies with energy deficit).
///
/// **Runtime preconditions** — none; the effect runs every tick
/// unconditionally while in progress.
///
/// **Witness** — `StepOutcome<()>`. Sleep's effect is
/// unconditional once the step is running; there's no failure
/// path that returns Advance without having slept.
///
/// **Feature emission** — none. Sleep is not currently tracked as
/// a Positive Feature (it's ubiquitous and not a diagnostic
/// signal on its own).
pub fn resolve_sleep(
    ticks: u64,
    duration: u64,
    needs: &mut Needs,
    d: &DispositionConstants,
) -> StepOutcome<()> {
    needs.energy = (needs.energy + d.sleep_energy_per_tick).min(1.0);
    needs.temperature = (needs.temperature + d.sleep_temperature_per_tick).min(1.0);
    if ticks >= duration {
        StepOutcome::bare(StepResult::Advance)
    } else {
        StepOutcome::bare(StepResult::Continue)
    }
}
