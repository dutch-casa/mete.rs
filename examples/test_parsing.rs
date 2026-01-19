use mete::{analyze_file, Language};
use std::io::Write;
use tempfile::NamedTempFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let code = r#"
fn main() {
    println!("Hello");
}

fn foo(x: i32) -> i32 {
    if x > 0 {
        x + 1
    } else {
        x - 1
    }
}

fn bar(a: i32, b: i32) -> i32 {
    a + b
}
"#;

    // Write to temp file for analysis
    let mut temp_file = NamedTempFile::with_suffix(".rs")?;
    temp_file.write_all(code.as_bytes())?;
    let path = temp_file.path().to_path_buf();

    let result = analyze_file(path, Language::Rust)?;

    println!("=== Analysis Response ===");
    println!("functions_count: {}", result.function_count);
    println!("loc: {}", result.loc);
    println!("cc_max: {}", result.cc_max);
    println!("mi: {}", result.mi);

    println!("\n=== Functions Detected ({}) ===", result.functions.len());
    for func in &result.functions {
        println!(
            "  - {:?}: loc={}, cc={}, cognitive={}",
            func.name, func.loc, func.cc, func.cognitive
        );
    }

    Ok(())
}
