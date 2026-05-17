use bevy_ecs::prelude::*;

use crate::ai::aspirations::ALL_CHAINS;
use crate::components::aspirations::{AspirationChain, AspirationDomain};

// ---------------------------------------------------------------------------
// AspirationRegistry
// ---------------------------------------------------------------------------

/// All aspiration chains available in the simulation. Ticket 321
/// retired RON loading in favor of [`ALL_CHAINS`] (code-defined const
/// data); the registry is a thin walkable wrapper around that table.
#[derive(Resource, Debug)]
pub struct AspirationRegistry {
    chains: &'static [&'static AspirationChain],
}

impl AspirationRegistry {
    /// Build the registry from the const [`ALL_CHAINS`] table. Called
    /// at app build by `SimulationPlugin::build`; parallel to
    /// `populate_method_registry` and `populate_dse_registry`.
    pub fn build_static() -> Self {
        Self { chains: ALL_CHAINS }
    }

    /// All chains in a given domain.
    pub fn chains_for_domain(&self, domain: AspirationDomain) -> Vec<&'static AspirationChain> {
        self.chains
            .iter()
            .copied()
            .filter(|c| c.domain == domain)
            .collect()
    }

    /// Look up a chain by its unique name.
    pub fn chain_by_name(&self, name: &str) -> Option<&'static AspirationChain> {
        self.chains.iter().copied().find(|c| c.name == name)
    }

    /// All registered chains, in declaration order.
    pub fn all_chains(&self) -> impl Iterator<Item = &'static AspirationChain> + '_ {
        self.chains.iter().copied()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> AspirationRegistry {
        AspirationRegistry::build_static()
    }

    #[test]
    fn all_chains_present() {
        let r = registry();
        assert!(r.all_chains().count() > 0, "no aspiration chains loaded");
    }

    #[test]
    fn every_domain_has_chains() {
        let r = registry();
        let domains = [
            AspirationDomain::Hunting,
            AspirationDomain::Combat,
            AspirationDomain::Social,
            AspirationDomain::Herbcraft,
            AspirationDomain::Exploration,
            AspirationDomain::Building,
            AspirationDomain::Leadership,
            // 398: Kinship — single chain `RAISE_OFFSPRING_ASPIRATION`.
            AspirationDomain::Kinship,
        ];
        for domain in domains {
            assert!(
                !r.chains_for_domain(domain).is_empty(),
                "no chains for domain {domain:?}",
            );
        }
    }

    #[test]
    fn chain_by_name_lookup() {
        let r = registry();
        let chain = r.chain_by_name("Master of the Hunt");
        assert!(chain.is_some(), "Master of the Hunt chain not found");
        let chain = chain.unwrap();
        assert_eq!(chain.domain, AspirationDomain::Hunting);
        assert!(!chain.milestones.is_empty());
    }

    #[test]
    fn every_chain_has_milestones() {
        let r = registry();
        for chain in r.all_chains() {
            assert!(
                !chain.milestones.is_empty(),
                "chain '{}' has no milestones",
                chain.name,
            );
        }
    }

    #[test]
    fn every_milestone_has_narrative() {
        let r = registry();
        for chain in r.all_chains() {
            for milestone in chain.milestones {
                assert!(
                    !milestone.narrative_on_complete.is_empty(),
                    "milestone '{}' in chain '{}' has empty narrative",
                    milestone.name,
                    chain.name,
                );
            }
        }
    }
}
