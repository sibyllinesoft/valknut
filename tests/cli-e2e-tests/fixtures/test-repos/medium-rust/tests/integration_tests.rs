use test_rust_project::{User, UserManager};

#[test]
fn test_user_creation() {
    let mut manager = UserManager::new();
    let id = manager.create_user("Test User".to_string(), "test@example.com".to_string()).unwrap();
    
    let user = manager.get_user(id).unwrap();
    assert_eq!(user.name, "Test User");
    assert_eq!(user.email, "test@example.com");
    assert!(user.active);
}

#[test]
fn test_duplicate_email() {
    let mut manager = UserManager::new();
    manager.create_user("User 1".to_string(), "same@example.com".to_string()).unwrap();
    
    let result = manager.create_user("User 2".to_string(), "same@example.com".to_string());
    assert!(result.is_err());
}

#[test]
fn test_user_operations() {
    let mut manager = UserManager::new();
    let id = manager.create_user("Test".to_string(), "test@example.com".to_string()).unwrap();
    
    // Test update
    manager.update_user(id, Some("Updated Name".to_string()), None).unwrap();
    let user = manager.get_user(id).unwrap();
    assert_eq!(user.name, "Updated Name");
    
    // Test deactivate
    manager.deactivate_user(id).unwrap();
    let user = manager.get_user(id).unwrap();
    assert!(!user.active);
    
    // Test activate
    manager.activate_user(id).unwrap();
    let user = manager.get_user(id).unwrap();
    assert!(user.active);
    
    // Test delete
    let deleted_user = manager.delete_user(id).unwrap();
    assert_eq!(deleted_user.name, "Updated Name");
    assert!(manager.get_user(id).is_none());
}
