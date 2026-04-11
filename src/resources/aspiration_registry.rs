use std::path::Path;

use bevy_ecs::prelude::*;

use crate::components::aspirations::{AspirationChain, AspirationDomain};

// ---------------------------------------------------------------------------
// AspirationRegistry
// ---------------------------------------------------------------------------

/// All aspiration chains available in the simulation, loaded from RON files.
#[derive(Resource, Debug)]
pub struct AspirationRegistry {
    chains: Vec<AspirationChain>,
}

impl AspirationRegistry {
    /// Load all `.ron` files from a directory and parse them as aspiration chain lists.
    pub fn load_from_dir(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut chains = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "ron"))
            .collect();
        // Sort by filename for deterministic load order.
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let contents = std::fs::read_to_string(entry.path())?;
            let file_chains: Vec<AspirationChain> = ron::from_str(&contents)?;
            chains.extend(file_chains);
        }
        Ok(Self { chains })
    }

    /// All chains in a given domain.
    pub fn chains_for_domain(&self, domain: AspirationDomain) -> Vec<&AspirationChain> {
        self.chains.iter().filter(|c| c.domain == domain).collect()
    }

    /// Look up a chain by its unique name.
    pub fn chain_by_name(&self, name: &str) -> Option<&AspirationChain> {
        self.chains.iter().find(|c| c.name == name)
    }

    /// All loaded chains.
    pub fn all_chains(&self) -> &[AspirationChain] {
        &self.chains
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_registry() -> AspirationRegistry {
        AspirationRegistry::load_from_dir(Path::new("assets/narrative/aspirations"))
            .expect("aspiration RON files should parse")
    }

    #[test]
    fn all_ron_files_parse() {
        let registry = load_test_registry();
        assert!(!registry.chains.is_empty(), "no aspiration chains loaded",);
    }

    #[test]
    fn every_domain_has_chains() {
        let registry = load_test_registry();
        let domains = [
            AspirationDomain::Hunting,
            AspirationDomain::Combat,
            AspirationDomain::Social,
            AspirationDomain::Herbcraft,
            AspirationDomain::Exploration,
            AspirationDomain::Building,
            AspirationDomain::Leadership,
        ];
        for domain in domains {
            assert!(
                !registry.chains_for_domain(domain).is_empty(),
                "no chains for domain {domain:?}",
            );
        }
    }

    #[test]
    fn chain_by_name_lookup() {
        let registry = load_test_registry();
        let chain = registry.chain_by_name("Master of the Hunt");
        assert!(chain.is_some(), "Master of the Hunt chain not found");
        let chain = chain.unwrap();
        assert_eq!(chain.domain, AspirationDomain::Hunting);
        assert!(!chain.milestones.is_empty());
    }

    #[test]
    fn every_chain_has_milestones() {
        let registry = load_test_registry();
        for chain in &registry.chains {
            assert!(
                !chain.milestones.is_empty(),
                "chain '{}' has no milestones",
                chain.name,
            );
        }
    }

    #[test]
    fn every_milestone_has_narrative() {
        let registry = load_test_registry();
        for chain in &registry.chains {
            for milestone in &chain.milestones {
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
