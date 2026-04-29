//! Disposition-switch helper. The single inline call site
//! (`evaluate_dispositions` at `src/systems/disposition.rs`)
//! constructs `Disposition::new(chosen, 0, personality)` today —
//! `disposition_started_tick` is initialized to 0 by `Disposition::new`,
//! and this helper writes the actual switch tick after construction.

use crate::components::disposition::{Disposition, DispositionKind};

/// Record a disposition switch by writing the current tick into
/// `state.disposition_started_tick`. 072 introduces both the field
/// and this writer; 075 (`CommitmentTenure` Modifier) is the first
/// reader.
///
/// **Real-world effect** — writes `state.disposition_started_tick = tick`.
/// The `_new` parameter is reserved for 075's per-disposition tenure
/// curve (which keys off `kind` to look up a per-disposition target
/// commitment); 072 doesn't read it.
pub fn record_disposition_switch(state: &mut Disposition, _new: DispositionKind, tick: u64) {
    state.disposition_started_tick = tick;
}
