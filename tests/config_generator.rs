// Helper script to generate a proper configuration for testing
use valknut_rs::core::config::ValknutConfig;

fn main() {
    let config = ValknutConfig::default();
    let yaml = serde_yaml::to_string(&config).expect("Failed to serialize config");
    println!("{}", yaml);
}
