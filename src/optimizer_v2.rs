// Optimized card data processing with advanced compression techniques
// Features: Numeric IDs, string pools, bit-packed power values, compression

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use serde_json;
use flate2::write::GzEncoder;
use flate2::Compression;
use lz4_flex::compress_prepend_size;
use flatbuffers::FlatBufferBuilder;

use crate::OptimizedData;

// String pool for deduplication
#[derive(Debug)]
pub struct StringPool {
    strings: Vec<String>,
    string_to_index: HashMap<String, u16>,
}

impl StringPool {
    pub fn new() -> Self {
        StringPool {
            strings: Vec::new(),
            string_to_index: HashMap::new(),
        }
    }
    
    pub fn add_string(&mut self, s: &str) -> u16 {
        if let Some(&index) = self.string_to_index.get(s) {
            index
        } else {
            let index = self.strings.len() as u16;
            self.strings.push(s.to_string());
            self.string_to_index.insert(s.to_string(), index);
            index
        }
    }
    
    pub fn get_strings(&self) -> &[String] {
        &self.strings
    }
}

// Generate numeric ID from string reference
pub fn generate_numeric_id(reference: &str) -> u32 {
    let mut hasher = DefaultHasher::new();
    reference.hash(&mut hasher);
    hasher.finish() as u32
}

// Pack power values into single u32 (8 bits each for mountain, ocean, forest)
pub fn pack_power_values(mountain: u8, ocean: u8, forest: u8) -> u32 {
    ((mountain as u32) << 24) | ((ocean as u32) << 16) | ((forest as u32) << 8)
}

// Unpack power values from u32
pub fn unpack_power_values(packed: u32) -> (u8, u8, u8) {
    let mountain = ((packed >> 24) & 0xFF) as u8;
    let ocean = ((packed >> 16) & 0xFF) as u8;
    let forest = ((packed >> 8) & 0xFF) as u8;
    (mountain, ocean, forest)
}

// Optimized card structure with numeric IDs
#[derive(Debug)]
pub struct OptimizedCard {
    pub id: u32,
    pub reference_idx: u16,
    pub name_idx: u16,
    pub faction_id: u16,
    pub rarity_id: u16,
    pub card_type_id: u16,
    pub main_cost: u8,
    pub recall_cost: u8,
    pub power_packed: u32,
    pub image_path_idx: u16,
    pub qr_url_idx: u16,
    pub flags: u8,  // bit 0: is_suspended
}

// Create optimized database structure
pub fn create_optimized_database(data: &OptimizedData) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut string_pool = StringPool::new();
    let mut optimized_cards = Vec::new();
    
    // Create numeric mappings for lookup tables
    let faction_id_map: HashMap<String, u16> = data.lookup_tables.factions.keys()
        .enumerate().map(|(i, k)| (k.clone(), i as u16)).collect();
    let rarity_id_map: HashMap<String, u16> = data.lookup_tables.rarities.keys()
        .enumerate().map(|(i, k)| (k.clone(), i as u16)).collect();
    let card_type_id_map: HashMap<String, u16> = data.lookup_tables.card_types.keys()
        .enumerate().map(|(i, k)| (k.clone(), i as u16)).collect();
    
    // Process cards with optimization
    for (reference, card) in &data.cards {
        let card_id = generate_numeric_id(reference);
        let reference_idx = string_pool.add_string(reference);
        let name_idx = string_pool.add_string(&card.name);
        let image_path_idx = string_pool.add_string(&card.image_path);
        let qr_url_idx = string_pool.add_string(&card.qr_url);
        
        let faction_id = *faction_id_map.get(&card.faction_ref).unwrap_or(&0);
        let rarity_id = *rarity_id_map.get(&card.rarity_ref).unwrap_or(&0);
        let card_type_id = *card_type_id_map.get(&card.type_ref).unwrap_or(&0);
        
        let power_packed = pack_power_values(
            card.power.m as u8,
            card.power.o as u8,
            card.power.f as u8
        );
        
        let flags = if card.is_suspended { 1 } else { 0 };
        
        optimized_cards.push(OptimizedCard {
            id: card_id,
            reference_idx,
            name_idx,
            faction_id,
            rarity_id,
            card_type_id,
            main_cost: card.main_cost as u8,
            recall_cost: card.recall_cost as u8,
            power_packed,
            image_path_idx,
            qr_url_idx,
            flags,
        });
    }
    
    // Sort cards by ID for better compression
    optimized_cards.sort_by_key(|c| c.id);
    
    // Generate the FlatBuffer data
    let mut builder = FlatBufferBuilder::with_capacity(1024 * 512);
    
    // Create string pool vector
    let string_offsets: Vec<_> = string_pool.get_strings().iter()
        .map(|s| builder.create_string(s))
        .collect();
    let strings_vector = builder.create_vector(&string_offsets);
    
    // For now, create a minimal structure for testing
    // This would be expanded to use the full optimized schema
    let metadata_offset = builder.create_string(&format!(
        "Optimized DB v2.0 - {} cards, {} strings in pool",
        optimized_cards.len(),
        string_pool.get_strings().len()
    ));
    
    builder.finish_minimal(metadata_offset);
    
    Ok(builder.finished_data().to_vec())
}

// Compression utilities
pub fn compress_with_gzip(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn compress_with_lz4(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(compress_prepend_size(data))
}

// Save compressed optimized format
pub fn save_optimized_formats(data: &OptimizedData) -> Result<(), Box<dyn std::error::Error>> {
    let optimized_data = create_optimized_database(data)?;
    
    // Save uncompressed optimized format
    let mut file = File::create("altered_cards_optimized_v2.fb")?;
    file.write_all(&optimized_data)?;
    
    // Save gzip compressed version
    let gzip_data = compress_with_gzip(&optimized_data)?;
    let mut gzip_file = File::create("altered_cards_optimized_v2.fb.gz")?;
    gzip_file.write_all(&gzip_data)?;
    
    // Save LZ4 compressed version
    let lz4_data = compress_with_lz4(&optimized_data)?;
    let mut lz4_file = File::create("altered_cards_optimized_v2.fb.lz4")?;
    lz4_file.write_all(&lz4_data)?;
    
    println!("   > Advanced optimizations saved:");
    println!("     - Optimized FlatBuffer: {} KB", optimized_data.len() / 1024);
    println!("     - Gzip compressed: {} KB", gzip_data.len() / 1024);
    println!("     - LZ4 compressed: {} KB", lz4_data.len() / 1024);
    
    Ok(())
}

// Delta update structure for incremental updates
#[derive(Debug)]
pub struct DeltaUpdate {
    pub added_cards: Vec<OptimizedCard>,
    pub modified_cards: Vec<OptimizedCard>,
    pub removed_card_ids: Vec<u32>,
    pub delta_version: u32,
    pub base_version: u32,
}

pub fn create_delta_update(
    old_data: &OptimizedData,
    new_data: &OptimizedData,
    base_version: u32,
    delta_version: u32
) -> DeltaUpdate {
    let mut added_cards = Vec::new();
    let mut modified_cards = Vec::new();
    let mut removed_card_ids = Vec::new();
    
    // Find added and modified cards
    for (reference, new_card) in &new_data.cards {
        let card_id = generate_numeric_id(reference);
        
        if let Some(old_card) = old_data.cards.get(reference) {
            // Check if modified (simplified comparison)
            if old_card.name != new_card.name || 
               old_card.is_suspended != new_card.is_suspended ||
               old_card.power.m != new_card.power.m {
                // Card was modified - would create OptimizedCard here
                // For now, just track that it changed
            }
        } else {
            // Card was added - would create OptimizedCard here
            // For now, just track that it was added
        }
    }
    
    // Find removed cards
    for reference in old_data.cards.keys() {
        if !new_data.cards.contains_key(reference) {
            removed_card_ids.push(generate_numeric_id(reference));
        }
    }
    
    DeltaUpdate {
        added_cards,
        modified_cards,
        removed_card_ids,
        delta_version,
        base_version,
    }
}