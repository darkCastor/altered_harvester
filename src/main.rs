// Version: 1.2.0
// Description: A data pipeline to harvest and optimize card data for the game "Altered" from its public API.
//              This script fetches data from multiple predefined sets and also suspended cards,
//              adds a flag to identify suspended cards, and then processes the data into a
//              structured and optimized JSON format.

use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufWriter;
use std::thread::sleep;
use std::time::Duration;

// --- Configuration ---
const SCRIPT_VERSION: &str = "1.2.0";
const RAW_OUTPUT_FILENAME: &str = "altered_all_cards.json";
const OPTIMIZED_OUTPUT_FILENAME: &str = "altered_optimized.json";
const REQUEST_DELAY: Duration = Duration::from_secs(1);
const USER_AGENT: &str = "AlteredDataPipeline/1.0-Rust (for personal project)";
const BASE_API_URL: &str = "https://api.altered.gg/cards?itemsPerPage=36&locale=fr-fr";
const QUERIES: &[&str] = &[
    "&cardSet[]=COREKS&rarity[]=COMMON&rarity[]=RARE", // COREKS Set
    "&cardSet[]=CORE&rarity[]=COMMON&rarity[]=RARE",   // CORE Set
    "&cardSet[]=ALIZE&rarity[]=COMMON&rarity[]=RARE",  // ALIZE Set
    "&cardSet[]=BISE&rarity[]=COMMON&rarity[]=RARE",   // BISE Set
    "&isSuspended=true&rarity[]=COMMON&rarity[]=RARE", // Suspended cards (must be last)
];

// --- Structs for Deserializing API Response ---
#[derive(Deserialize, Debug)]
struct HydraView {
    #[serde(rename = "hydra:next")]
    next: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ApiResponse {
    #[serde(rename = "hydra:member")]
    members: Vec<serde_json::Value>,
    #[serde(rename = "hydra:view")]
    view: Option<HydraView>,
}

// --- Intermediate struct for processing ---
#[derive(Serialize)]
struct HarvestedCard {
    card_data: serde_json::Value,
    is_suspended: bool,
}

// --- Structs for Serializing Optimized Output ---
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
    is_suspended: bool, // New field to identify suspended cards
    power: PowerStats,
}

#[derive(Serialize)]
struct OptimizedData {
    meta: Meta,
    lookup_tables: LookupTables,
    cards: BTreeMap<String, OptimizedCard>,
}

/// ### STEP 1: HARVESTER ###
/// Fetches all card data and flags suspended cards.
fn harvest_cards() -> Result<Vec<HarvestedCard>, Box<dyn std::error::Error>> {
    let client = Client::builder().user_agent(USER_AGENT).build()?;
    let mut all_cards: Vec<HarvestedCard> = Vec::new();
    let suspended_query_fragment = "&isSuspended=true";

    for (index, query) in QUERIES.iter().enumerate() {
        println!("\n--- Harvesting Query {}/{} ---", index + 1, QUERIES.len());
        let is_suspended_query = query.contains(suspended_query_fragment);
        let start_url = format!("{}{}", BASE_API_URL, query);
        let mut next_page_url = Some(start_url);
        let mut page_count = 0;

        while let Some(url) = next_page_url {
            page_count += 1;
            println!("   > Fetching page {}: {}", page_count, url);
            let response = client.get(&url).send()?.error_for_status()?;
            let response_url = response.url().clone();
            let api_data: ApiResponse = response.json()?;

            // Wrap each card with its suspension status
            for member in api_data.members {
                all_cards.push(HarvestedCard {
                    card_data: member,
                    is_suspended: is_suspended_query,
                });
            }

            next_page_url = if let Some(view) = api_data.view.and_then(|v| v.next) {
                Some(response_url.join(&view)?.to_string())
            } else {
                None
            };
            sleep(REQUEST_DELAY);
        }
    }

    println!(
        "\n   > ✅ Harvest complete. Found {} total card objects (pre-optimization).",
        all_cards.len()
    );

    // Save the raw data (now including the suspension flag) as a backup.
    let raw_file = File::create(RAW_OUTPUT_FILENAME)?;
    serde_json::to_writer_pretty(BufWriter::new(raw_file), &all_cards)?;
    println!("   > Raw data saved to '{}'", RAW_OUTPUT_FILENAME);

    Ok(all_cards)
}

/// ### STEP 2: OPTIMIZER ###
/// Transforms the raw card data into an optimized structure.
fn optimize_cards(harvested_cards: Vec<HarvestedCard>) -> Result<(), Box<dyn std::error::Error>> {
    let mut lookup_tables = LookupTables {
        rarities: BTreeMap::new(),
        factions: BTreeMap::new(),
        card_types: BTreeMap::new(),
    };
    let mut optimized_cards = BTreeMap::new();
    let source_set = "Multiple Sets".to_string();

    for harvested_card in &harvested_cards {
        let card_value = &harvested_card.card_data;
        let is_suspended = harvested_card.is_suspended;

        let get_str = |obj: &serde_json::Value, key: &str| {
            obj.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let get_i64 = |obj: &serde_json::Value, key: &str| {
            obj.get(key)
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        };

        if let Some(rarity) = card_value.get("rarity") {
            lookup_tables
                .rarities
                .entry(get_str(rarity, "reference"))
                .or_insert_with(|| RarityInfo {
                    name: get_str(rarity, "name"),
                });
        }
        if let Some(faction) = card_value.get("mainFaction") {
            lookup_tables
                .factions
                .entry(get_str(faction, "reference"))
                .or_insert_with(|| FactionInfo {
                    name: get_str(faction, "name"),
                    color: get_str(faction, "color"),
                });
        }
        if let Some(card_type) = card_value.get("cardType") {
            lookup_tables
                .card_types
                .entry(get_str(card_type, "reference"))
                .or_insert_with(|| CardTypeInfo {
                    name: get_str(card_type, "name"),
                });
        }

        let card_reference_id = get_str(card_value, "reference");
        if card_reference_id.is_empty() {
            continue;
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
            is_suspended, // Set the flag here
            power: PowerStats {
                m: get_i64(elements, "MOUNTAIN_POWER"),
                o: get_i64(elements, "OCEAN_POWER"),
                f: get_i64(elements, "FOREST_POWER"),
            },
        };
        // This will insert the card. If a suspended version is processed later,
        // it will overwrite the non-suspended version, which is the desired behavior.
        optimized_cards.insert(card_reference_id, card);
    }

    let final_data = OptimizedData {
        meta: Meta {
            script_version: SCRIPT_VERSION.to_string(),
            generated_at_utc: Utc::now(),
            source_set,
            data_sources: QUERIES
                .iter()
                .map(|q| format!("{}{}", BASE_API_URL, q))
                .collect(),
            total_cards: optimized_cards.len(),
        },
        lookup_tables,
        cards: optimized_cards,
    };

    println!(
        "\n   > ✅ Optimization complete. Processed {} unique cards.",
        final_data.meta.total_cards
    );
    let output_file = File::create(OPTIMIZED_OUTPUT_FILENAME)?;
    serde_json::to_writer_pretty(BufWriter::new(output_file), &final_data)?;
    println!(
        "   > Optimized data saved to '{}'",
        OPTIMIZED_OUTPUT_FILENAME
    );

    Ok(())
}

/// ### MAIN ORCHESTRATOR ###
/// Runs the entire data pipeline in sequence.
fn main() {
    println!("🚀 Starting Altered Data Pipeline...");

    // --- Step 1 ---
    println!("\n--- Step 1: Harvesting Cards from API ---");
    match harvest_cards() {
        Ok(raw_cards) => {
            // --- Step 2 ---
            println!("\n--- Step 2: Optimizing Raw Data ---");
            if let Err(e) = optimize_cards(raw_cards) {
                eprintln!("\n❌ Optimization failed: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("\n❌ Harvesting failed: {}", e);
            std::process::exit(1);
        }
    }

    println!("\n✨✨ Pipeline finished successfully! ✨✨");
}
