use reqwest::blocking::Client;
use serde::Deserialize;
use std::fs::File;
use std::io::BufWriter;
use std::thread::sleep;
use std::time::Duration;

// --- Configuration ---

// Constants are used for configuration, similar to the Python script.
const API_START_URL: &str = "https://api.altered.gg/cards?itemsPerPage=36&locale=fr-fr";
const OUTPUT_FILENAME: &str = "altered_all_cards.json";
const REQUEST_DELAY: Duration = Duration::from_secs(1);
const USER_AGENT: &str = "AlteredCardHarvester/1.0-Rust (for personal collection)";

// --- Data Structures ---

// In Rust, we define structs to represent the expected JSON structure.
// `serde` uses these to automatically parse the JSON.
// The `#[derive(Deserialize)]` macro generates the parsing code.
#[derive(Deserialize, Debug)]
struct HydraView {
    // The `#[serde(rename = "...")]` attribute tells serde how to map JSON keys
    // that are not valid Rust identifiers (like "hydra:next").
    #[serde(rename = "hydra:next")]
    next: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ApiResponse {
    #[serde(rename = "hydra:member")]
    members: Vec<serde_json::Value>, // Using serde_json::Value to accept any card structure.
    #[serde(rename = "hydra:view")]
    view: Option<HydraView>,
}

// The `main` function returns a `Result` to allow for easy error handling
// using the `?` operator.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting the harvest of Altered cards (Rust version)...");

    // Initialize the HTTP client.
    let client = Client::builder().user_agent(USER_AGENT).build()?;

    // This vector will store all the collected card data.
    let mut all_cards: Vec<serde_json::Value> = Vec::new();

    // The 'next_page_url' is wrapped in an `Option` to handle the end of pagination.
    let mut next_page_url = Some(API_START_URL.to_string());
    let mut page_count = 0;

    // A `while let` loop is an idiomatic way to loop as long as the Option contains a value.
    while let Some(url) = next_page_url {
        page_count += 1;
        println!("\n📄 Fetching page {}: {}", page_count, url);

        // Make the HTTP request and handle potential errors with `?`.
        let response = client.get(&url).send()?;

        // Check for HTTP errors (4xx or 5xx status codes).
        let response = response.error_for_status()?;

        // Get the final URL of the request (it might have followed redirects).
        let response_url = response.url().clone();

        // Parse the JSON response into our `ApiResponse` struct.
        let api_data: ApiResponse = response.json()?;

        // Add the found cards to our master list.
        let cards_on_page = api_data.members;
        println!(
            "  ✅ Found {} cards. Total harvested: {}",
            cards_on_page.len(),
            all_cards.len() + cards_on_page.len()
        );
        all_cards.extend(cards_on_page);

        // Determine the next URL to fetch.
        if let Some(view) = api_data.view {
            if let Some(next_path) = view.next {
                // Safely join the base URL with the relative path for the next page.
                let next_full_url = response_url.join(&next_path)?;
                next_page_url = Some(next_full_url.to_string());
            } else {
                // No 'next' path means we're on the last page.
                next_page_url = None;
            }
        } else {
            next_page_url = None;
        }

        if next_page_url.is_none() {
            println!("\n🏁 This was the last page. Harvest complete.");
        }

        // Be polite! Wait before the next request.
        sleep(REQUEST_DELAY);
    }

    // After the loop, save the collected data to a file.
    if !all_cards.is_empty() {
        println!(
            "\n💾 Saving {} cards to '{}'...",
            all_cards.len(),
            OUTPUT_FILENAME
        );

        // Create the output file.
        let file = File::create(OUTPUT_FILENAME)?;
        // Use a BufWriter for better performance on large writes.
        let writer = BufWriter::new(file);

        // `serde_json::to_writer_pretty` writes the data in a nicely formatted way.
        serde_json::to_writer_pretty(writer, &all_cards)?;

        println!("✨ Save successful!");
    } else {
        println!("\n⚠️ No cards were harvested. Nothing to save.");
    }

    // Return `Ok(())` to indicate success.
    Ok(())
}
