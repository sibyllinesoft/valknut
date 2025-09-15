use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Test that Rust tree-sitter parsing works
    let rust_code = r#"
fn main() {
    println!("Hello, world!");
}

struct User {
    name: String,
    age: u32,
}

impl User {
    fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }
    
    fn greet(&self) -> String {
        format!("Hello, I'm {}", self.name)
    }
}
"#;
    
    let mut adapter = valknut_rs::lang::rust_lang::RustAdapter::new()?;
    let entities = adapter.extract_code_entities(rust_code, "test.rs")?;
    
    println!("Successfully parsed {} Rust entities:", entities.len());
    for entity in entities {
        println!("  - {} ({})", entity.name, entity.entity_type);
    }
    
    Ok(())
}
