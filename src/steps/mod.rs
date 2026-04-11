pub mod building;
pub mod disposition;
pub mod magic;

/// Outcome of a step handler. The dispatcher applies the result to the chain.
#[derive(Debug)]
pub enum StepResult {
    /// Step is still in progress, do nothing.
    Continue,
    /// Step succeeded, advance the chain.
    Advance,
    /// Step failed, fail the chain with a reason.
    Fail(String),
}
