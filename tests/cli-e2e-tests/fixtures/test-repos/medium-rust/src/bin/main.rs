//! Main binary for test Rust project

use test_rust_project::{User, UserManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = UserManager::new();
    
    // Create some test users
    let id1 = manager.create_user("Alice".to_string(), "alice@example.com".to_string())?;
    let id2 = manager.create_user("Bob".to_string(), "bob@example.com".to_string())?;
    let id3 = manager.create_user("Charlie".to_string(), "charlie@example.com".to_string())?;
    
    println!("Created users with IDs: {}, {}, {}", id1, id2, id3);
    
    // List all users
    let users = manager.list_users();
    println!("Total users: {}", users.len());
    
    for user in users {
        println!("User {}: {} ({}) - Active: {}", user.id, user.name, user.email, user.active);
    }
    
    // Deactivate a user
    manager.deactivate_user(id2)?;
    println!("Deactivated user {}", id2);
    
    // Update a user
    manager.update_user(id1, Some("Alice Smith".to_string()), None)?;
    println!("Updated user {}", id1);
    
    // Try to create duplicate email (should fail)
    match manager.create_user("David".to_string(), "alice@example.com".to_string()) {
        Ok(_) => println!("This shouldn't happen!"),
        Err(e) => println!("Expected error: {}", e),
    }
    
    Ok(())
}
