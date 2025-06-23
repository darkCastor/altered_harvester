use std::process::Command;

fn main() {
    // Compile FlatBuffer schema
    let output = Command::new("flatc")
        .args(&["--rust", "-o", "src/", "schema/cards.fbs"])
        .output()
        .expect("Failed to execute flatc. Make sure FlatBuffers compiler is installed.");

    if !output.status.success() {
        panic!("flatc failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    println!("cargo:rerun-if-changed=schema/cards.fbs");
}