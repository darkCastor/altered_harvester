// Delta update system for incremental card database updates
// Provides efficient synchronization between different database versions

use std::collections::{HashMap, HashSet};
use std::fs::{File, metadata};
use std::io::{BufWriter, Write, BufReader, Read};
use std::path::Path;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

use crate::{OptimizedData, OptimizedCard};
use crate::optimizer_v2::{generate_numeric_id, OptimizedCard as OptimizedCardV2};

// Delta operation types
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DeltaOperation {
    Add(OptimizedCard),
    Modify(OptimizedCard),
    Remove(String), // Card reference
}

// Complete delta package
#[derive(Serialize, Deserialize, Debug)]
pub struct DeltaPackage {
    pub base_version: String,
    pub target_version: String,
    pub generated_at: DateTime<Utc>,
    pub operations: Vec<DeltaOperation>,
    pub checksum: String,
    pub statistics: DeltaStatistics,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeltaStatistics {
    pub total_operations: usize,
    pub cards_added: usize,
    pub cards_modified: usize,
    pub cards_removed: usize,
    pub size_bytes: usize,
    pub compression_ratio: f64,
}

// Database version metadata
#[derive(Serialize, Deserialize, Debug)]
pub struct DatabaseVersion {
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub card_count: usize,
    pub file_size: usize,
    pub checksum: String,
}

pub struct DeltaManager {
    versions: Vec<DatabaseVersion>,
    base_path: String,
}

impl DeltaManager {
    pub fn new(base_path: &str) -> Self {
        DeltaManager {
            versions: Vec::new(),
            base_path: base_path.to_string(),
        }
    }

    // Create delta between two database versions
    pub fn create_delta(
        &self,
        old_data: &OptimizedData,
        new_data: &OptimizedData,
        base_version: &str,
        target_version: &str,
    ) -> Result<DeltaPackage, Box<dyn std::error::Error>> {
        
        let mut operations = Vec::new();
        let mut stats = DeltaStatistics {
            total_operations: 0,
            cards_added: 0,
            cards_modified: 0,
            cards_removed: 0,
            size_bytes: 0,
            compression_ratio: 0.0,
        };

        // Create sets for efficient comparison
        let old_cards: HashMap<String, &OptimizedCard> = old_data.cards.iter()
            .map(|(k, v)| (k.clone(), v)).collect();
        let new_cards: HashMap<String, &OptimizedCard> = new_data.cards.iter()
            .map(|(k, v)| (k.clone(), v)).collect();
        
        let old_keys: HashSet<&String> = old_cards.keys().collect();
        let new_keys: HashSet<&String> = new_cards.keys().collect();

        // Find added cards
        for reference in new_keys.difference(&old_keys) {
            if let Some(card) = new_cards.get(*reference) {
                operations.push(DeltaOperation::Add((*card).clone()));
                stats.cards_added += 1;
            }
        }

        // Find removed cards
        for reference in old_keys.difference(&new_keys) {
            operations.push(DeltaOperation::Remove((*reference).clone()));
            stats.cards_removed += 1;
        }

        // Find modified cards
        for reference in old_keys.intersection(&new_keys) {
            if let (Some(old_card), Some(new_card)) = (old_cards.get(*reference), new_cards.get(*reference)) {
                if self.card_differs(old_card, new_card) {
                    operations.push(DeltaOperation::Modify((*new_card).clone()));
                    stats.cards_modified += 1;
                }
            }
        }

        stats.total_operations = operations.len();
        
        // Calculate checksum (simplified)
        let checksum = format!("{:x}", 
            operations.len() as u64 * 17 + stats.cards_added as u64 * 31 + stats.cards_modified as u64 * 37
        );

        let delta = DeltaPackage {
            base_version: base_version.to_string(),
            target_version: target_version.to_string(),
            generated_at: Utc::now(),
            operations,
            checksum,
            statistics: stats,
        };

        Ok(delta)
    }

    // Check if two cards differ significantly
    fn card_differs(&self, old_card: &OptimizedCard, new_card: &OptimizedCard) -> bool {
        old_card.name != new_card.name ||
        old_card.faction_ref != new_card.faction_ref ||
        old_card.rarity_ref != new_card.rarity_ref ||
        old_card.type_ref != new_card.type_ref ||
        old_card.main_cost != new_card.main_cost ||
        old_card.recall_cost != new_card.recall_cost ||
        old_card.is_suspended != new_card.is_suspended ||
        old_card.power.m != new_card.power.m ||
        old_card.power.o != new_card.power.o ||
        old_card.power.f != new_card.power.f ||
        old_card.image_path != new_card.image_path ||
        old_card.qr_url != new_card.qr_url
    }

    // Apply delta to existing database
    pub fn apply_delta(
        &self,
        base_data: &mut OptimizedData,
        delta: &DeltaPackage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        
        for operation in &delta.operations {
            match operation {
                DeltaOperation::Add(card) => {
                    base_data.cards.insert(self.get_card_reference(card), card.clone());
                }
                DeltaOperation::Modify(card) => {
                    base_data.cards.insert(self.get_card_reference(card), card.clone());
                }
                DeltaOperation::Remove(reference) => {
                    base_data.cards.remove(reference);
                }
            }
        }

        // Update metadata
        base_data.meta.total_cards = base_data.cards.len();
        base_data.meta.generated_at_utc = Utc::now();

        Ok(())
    }

    // Helper to extract card reference (would need to be implemented based on card structure)
    fn get_card_reference(&self, card: &OptimizedCard) -> String {
        // This is a placeholder - in a real implementation, you'd need to store
        // the reference in the OptimizedCard struct or calculate it consistently
        format!("CARD_{}", generate_numeric_id(&card.name))
    }

    // Save delta package to file
    pub fn save_delta(
        &self,
        delta: &DeltaPackage,
        filename: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(filename)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, delta)?;
        
        // Also save compressed version
        let compressed_filename = format!("{}.gz", filename);
        let compressed_data = flate2::write::GzEncoder::new(
            File::create(compressed_filename)?,
            flate2::Compression::best()
        );
        serde_json::to_writer(compressed_data, delta)?;
        
        Ok(())
    }

    // Load delta package from file
    pub fn load_delta(&self, filename: &str) -> Result<DeltaPackage, Box<dyn std::error::Error>> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let delta = serde_json::from_reader(reader)?;
        Ok(delta)
    }

    // Register a new database version
    pub fn register_version(
        &mut self,
        version: &str,
        file_path: &str,
        card_count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let metadata = metadata(file_path)?;
        let file_size = metadata.len() as usize;
        
        // Calculate simple checksum (in production, use proper hash)
        let checksum = format!("{:x}", file_size as u64 * 31 + card_count as u64 * 17);
        
        let db_version = DatabaseVersion {
            version: version.to_string(),
            timestamp: Utc::now(),
            card_count,
            file_size,
            checksum,
        };
        
        self.versions.push(db_version);
        Ok(())
    }

    // Get all registered versions
    pub fn get_versions(&self) -> &[DatabaseVersion] {
        &self.versions
    }

    // Create incremental update chain
    pub fn create_update_chain(
        &self,
        base_version: &str,
        target_version: &str,
    ) -> Vec<String> {
        // Simplified implementation - in practice, this would build an optimal
        // chain of delta updates to get from base to target version
        vec![format!("delta_{}_{}.json", base_version, target_version)]
    }
}

// Utility functions for delta operations
pub fn calculate_delta_size_reduction(
    full_database_size: usize,
    delta_size: usize,
) -> f64 {
    if full_database_size == 0 {
        0.0
    } else {
        1.0 - (delta_size as f64 / full_database_size as f64)
    }
}

// Create sample delta for demonstration
pub fn create_sample_delta() -> DeltaPackage {
    use crate::{OptimizedCard, LocalPowerStats};
    
    let sample_card = OptimizedCard {
        name: "Sample New Card".to_string(),
        type_ref: "HERO".to_string(),
        faction_ref: "AX".to_string(),
        rarity_ref: "COMMON".to_string(),
        image_path: "/path/to/image.jpg".to_string(),
        qr_url: "https://example.com/qr".to_string(),
        main_cost: 3,
        recall_cost: 1,
        is_suspended: false,
        power: LocalPowerStats { m: 2, o: 1, f: 3 },
    };

    DeltaPackage {
        base_version: "2.0.0".to_string(),
        target_version: "2.0.1".to_string(),
        generated_at: Utc::now(),
        operations: vec![DeltaOperation::Add(sample_card)],
        checksum: "sample_checksum".to_string(),
        statistics: DeltaStatistics {
            total_operations: 1,
            cards_added: 1,
            cards_modified: 0,
            cards_removed: 0,
            size_bytes: 1024,
            compression_ratio: 0.75,
        },
    }
}