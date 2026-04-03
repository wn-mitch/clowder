pub mod identity;
pub mod personality;
pub mod physical;
pub mod mental;
pub mod skills;

pub use identity::{Age, Appearance, Gender, LifeStage, Name, Orientation, Species};
pub use personality::Personality;
pub use physical::{Health, Injury, InjuryKind, Needs, Position};
pub use mental::{Memory, MemoryEntry, MemoryType, Mood, MoodModifier};
pub use skills::{Corruption, MagicAffinity, Skills, Training};
