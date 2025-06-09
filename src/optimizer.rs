// Version: 1.2.0
// Description: A standalone script to optimize raw card data for the game "Altered".
//              It reads a raw JSON file harvested by the main script, processes it
//              into a structured format, adds lookup tables, and includes metadata.
//              This version is updated to handle the `is_suspended` flag.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};

// --- Configuration ---
const SCRIPT_VERSION: &str = "1.2.0"; // Version of this optimizer script
const SOURCE_FILE: &str = "altered_all_cards.json";
const OUTPUT_FILE: &str = "altered_optimized.json";

// --- Input Data Structures ---
// This struct must match the output of the harvester function in main.rs
#[derive(Deserialize)]
struct HarvestedCard {
    card_data: serde_json::Value,
    is_suspended: bool,
}

// --- Output Data Structures ---
#[derive(Serialize)]
struct Meta {
    script_version: String,
    generated_at_utc: DateTime<Utc>,
    source_set: String,
    data_sources: Vec<String>,
    total_cards: usize,
}

#[derive(Serialize)]
struct RarityInfo {
    name: String,
}

#[derive(Serialize)]
struct FactionInfo {
    name: String,
    color: String,
}

#[derive(Serialize)]
struct CardTypeInfo {
    name: String,
}

#[derive(Serialize)]
struct LookupTables {
    rarities: BTreeMap<String, RarityInfo>,
    factions: BTreeMap<String, FactionInfo>,
    card_types: BTreeMap<String, CardTypeInfo>,
}

#[derive(Serialize)]
struct PowerStats {
    m: i64,
    o: i64,
    f: i64,
}

#[derive(Serialize)]
struct OptimizedCard {
    name: String,
    type_ref: String,
    faction_ref: String,
    rarity_ref: String,
    image_path: String,
    qr_url: String,
    main_cost: i64,
    recall_cost: i64,
    is_suspended: bool, // New field
    power: PowerStats,
}

#[derive(Serialize)]
struct OptimizedData {
    meta: Meta,
    lookup_tables: LookupTables,
    cards: BTreeMap<String, OptimizedCard>,
}

// --- Main Logic ---
// This function is not called by main.rs but can be used if you want to run optimization separately.
pub fn run_optimizer() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚙️  Starting JSON optimization (Standalone Rust version)...");

    // --- 1. Load and Parse Source File ---
    println!("   > Loading raw data from '{}'...", SOURCE_FILE);
    let source_file = match File::open(SOURCE_FILE) {
        Ok(file) => file,
        Err(_) => {
            eprintln!(
                "   ❌ Error: Source file '{}' not found. Please run the harvester first.",
                SOURCE_FILE
            );
            return Ok(()); // Exit gracefully
        }
    };
    let reader = BufReader::new(source_file);

    // Parse into the HarvestedCard structure
    let raw_harvest: Vec<HarvestedCard> = serde_json::from_reader(reader)?;
    println!("   > Loaded {} raw card objects.", raw_harvest.len());

    // --- 2. Initialize Structures for Optimized Data ---
    let mut lookup_tables = LookupTables {
        rarities: BTreeMap::new(),
        factions: BTreeMap::new(),
        card_types: BTreeMap::new(),
    };
    let mut optimized_cards = BTreeMap::new();

    // Since we don't know the exact source set from the raw file, we make a reasonable guess or use a generic value.
    let source_set = raw_harvest
        .get(0)
        .and_then(|h| h.card_data.get("cardSet"))
        .and_then(|s| s.get("reference"))
        .and_then(|r| r.as_str())
        .unwrap_or("Multiple Sets") // Default to generic value
        .to_string();

    // --- 3. Process Each Card ---
    for harvested_card in &raw_harvest {
        let card_value = &harvested_card.card_data;
        let is_suspended = harvested_card.is_suspended;

        // Use helper closures for clean, safe data extraction
        let get_str = |obj: &serde_json::Value, key: &str| -> String {
            obj.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let get_i64 = |obj: &serde_json::Value, key: &str| -> i64 {
            obj.get(key)
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        };

        // Populate lookup tables
        if let Some(rarity) = card_value.get("rarity") {
            let rarity_ref = get_str(rarity, "reference");
            if !rarity_ref.is_empty() {
                lookup_tables
                    .rarities
                    .entry(rarity_ref)
                    .or_insert_with(|| RarityInfo {
                        name: get_str(rarity, "name"),
                    });
            }
        }
        if let Some(faction) = card_value.get("mainFaction") {
            let faction_ref = get_str(faction, "reference");
            if !faction_ref.is_empty() {
                lookup_tables
                    .factions
                    .entry(faction_ref)
                    .or_insert_with(|| FactionInfo {
                        name: get_str(faction, "name"),
                        color: get_str(faction, "color"),
                    });
            }
        }
        if let Some(card_type) = card_value.get("cardType") {
            let type_ref = get_str(card_type, "reference");
            if !type_ref.is_empty() {
                lookup_tables
                    .card_types
                    .entry(type_ref)
                    .or_insert_with(|| CardTypeInfo {
                        name: get_str(card_type, "name"),
                    });
            }
        }

        // Create the simplified, optimized card object
        let card_reference_id = get_str(card_value, "reference");
        if card_reference_id.is_empty() {
            continue; // Skip cards without a reference ID
        }

        let elements = card_value
            .get("elements")
            .unwrap_or(&serde_json::Value::Null);

        let card = OptimizedCard {
            name: get_str(card_value, "name"),
            type_ref: get_str(
                card_value
                    .get("cardType")
                    .unwrap_or(&serde_json::Value::Null),
                "reference",
            ),
            faction_ref: get_str(
                card_value
                    .get("mainFaction")
                    .unwrap_or(&serde_json::Value::Null),
                "reference",
            ),
            rarity_ref: get_str(
                card_value.get("rarity").unwrap_or(&serde_json::Value::Null),
                "reference",
            ),
            image_path: get_str(card_value, "imagePath"),
            qr_url: get_str(card_value, "qrUrlDetail"),
            main_cost: get_i64(elements, "MAIN_COST"),
            recall_cost: get_i64(elements, "RECALL_COST"),
            is_suspended, // Set the flag
            power: PowerStats {
                m: get_i64(elements, "MOUNTAIN_POWER"),
                o: get_i64(elements, "OCEAN_POWER"),
                f: get_i64(elements, "FOREST_POWER"),
            },
        };
        optimized_cards.insert(card_reference_id, card);
    }

    println!(
        "   > Optimization complete. Processed {} unique cards.",
        optimized_cards.len()
    );

    // --- 4. Assemble and Save the Final Optimized Data ---
    let final_data = OptimizedData {
        meta: Meta {
            script_version: SCRIPT_VERSION.to_string(),
            generated_at_utc: Utc::now(),
            source_set,
            // Since this script only reads the file, we can't know the original queries.
            data_sources: vec![format!("Loaded from '{}'", SOURCE_FILE)],
            total_cards: optimized_cards.len(),
        },
        lookup_tables,
        cards: optimized_cards,
    };

    println!("   > 💾 Saving optimized data to '{}'...", OUTPUT_FILE);
    let output_file = File::create(OUTPUT_FILE)?;
    let writer = BufWriter::new(output_file);
    serde_json::to_writer_pretty(writer, &final_data)?;
    println!("   > ✨ Save successful!");

    Ok(())
}
