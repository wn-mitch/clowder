//! `RecentTargetFailures` — placeholder.
//!
//! Ticket 072 introduces this module so the `plan_substrate` API surface
//! can accept `Option<&mut RecentTargetFailures>` arguments today. The
//! struct body and behavior land in ticket 073 (`RecentTargetFailures` +
//! `target_recent_failure` Consideration on all 6 target DSEs); 072
//! callers pass `None` for these arguments and the placeholder body is
//! never read.
//!
//! Do not add fields here without first updating the ticket-073 design
//! — the shape is load-bearing for the per-cat target-failure memory
//! that will be exposed as an IAUS `Consideration`.

use bevy_ecs::prelude::*;

/// Per-cat memory of recently-failed target entities. Body lands in 073.
#[derive(Component, Debug, Clone, Default)]
pub struct RecentTargetFailures;
