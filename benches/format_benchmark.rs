use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::fs;
use std::time::Instant;
use serde_json;
use flatbuffers::root;

mod cards_generated;
use cards_generated::altered_cards::*;

fn benchmark_json_parsing(c: &mut Criterion) {
    let json_data = fs::read_to_string("altered_optimized.json")
        .expect("Failed to read JSON file");
    
    c.bench_function("json_parse", |b| {
        b.iter(|| {
            let parsed: serde_json::Value = serde_json::from_str(black_box(&json_data))
                .expect("Failed to parse JSON");
            black_box(parsed);
        })
    });
}

fn benchmark_flatbuffer_access(c: &mut Criterion) {
    let fb_data = fs::read("altered_cards.fb")
        .expect("Failed to read FlatBuffer file");
    
    c.bench_function("flatbuffer_access", |b| {
        b.iter(|| {
            let card_db = root::<CardDatabase>(black_box(&fb_data))
                .expect("Failed to get root");
            
            // Access some data to ensure it's not optimized away
            let cards = card_db.cards().unwrap();
            let first_card = cards.get(0);
            let name = first_card.name().unwrap_or("");
            black_box(name);
        })
    });
}

fn benchmark_card_lookup(c: &mut Criterion) {
    let json_data = fs::read_to_string("altered_optimized.json")
        .expect("Failed to read JSON file");
    let fb_data = fs::read("altered_cards.fb")
        .expect("Failed to read FlatBuffer file");
    
    c.bench_function("json_card_lookup", |b| {
        b.iter(|| {
            let parsed: serde_json::Value = serde_json::from_str(&json_data).unwrap();
            let cards = parsed["cards"].as_object().unwrap();
            let mut count = 0;
            for (_, card) in cards {
                if card["name"].as_str().unwrap_or("").contains("Sierra") {
                    count += 1;
                }
            }
            black_box(count);
        })
    });
    
    c.bench_function("flatbuffer_card_lookup", |b| {
        b.iter(|| {
            let card_db = root::<CardDatabase>(&fb_data).unwrap();
            let cards = card_db.cards().unwrap();
            let mut count = 0;
            for i in 0..cards.len() {
                let card = cards.get(i);
                if card.name().unwrap_or("").contains("Sierra") {
                    count += 1;
                }
            }
            black_box(count);
        })
    });
}

criterion_group!(benches, benchmark_json_parsing, benchmark_flatbuffer_access, benchmark_card_lookup);
criterion_main!(benches);