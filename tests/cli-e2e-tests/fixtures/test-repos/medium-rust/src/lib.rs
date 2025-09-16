//! Test Rust library for Valknut CLI analysis

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub active: bool,
}

#[derive(Debug)]
pub struct UserManager {
    users: HashMap<u64, User>,
    next_id: u64,
}

impl UserManager {
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
            next_id: 1,
        }
    }
    
    pub fn create_user(&mut self, name: String, email: String) -> Result<u64, String> {
        if name.is_empty() {
            return Err("Name cannot be empty".to_string());
        }
        
        if email.is_empty() {
            return Err("Email cannot be empty".to_string());
        }
        
        // Check for duplicate email
        for user in self.users.values() {
            if user.email == email {
                return Err("Email already exists".to_string());
            }
        }
        
        let id = self.next_id;
        let user = User {
            id,
            name,
            email,
            active: true,
        };
        
        self.users.insert(id, user);
        self.next_id += 1;
        
        Ok(id)
    }
    
    pub fn get_user(&self, id: u64) -> Option<&User> {
        self.users.get(&id)
    }
    
    pub fn update_user(&mut self, id: u64, name: Option<String>, email: Option<String>) -> Result<(), String> {
        let user = self.users.get_mut(&id).ok_or("User not found")?;
        
        if let Some(name) = name {
            if name.is_empty() {
                return Err("Name cannot be empty".to_string());
            }
            user.name = name;
        }
        
        if let Some(email) = email {
            if email.is_empty() {
                return Err("Email cannot be empty".to_string());
            }
            
            // Check for duplicate email
            for (other_id, other_user) in &self.users {
                if *other_id != id && other_user.email == email {
                    return Err("Email already exists".to_string());
                }
            }
            
            user.email = email;
        }
        
        Ok(())
    }
    
    pub fn delete_user(&mut self, id: u64) -> Result<User, String> {
        self.users.remove(&id).ok_or("User not found".to_string())
    }
    
    pub fn list_users(&self) -> Vec<&User> {
        self.users.values().collect()
    }
    
    pub fn activate_user(&mut self, id: u64) -> Result<(), String> {
        let user = self.users.get_mut(&id).ok_or("User not found")?;
        user.active = true;
        Ok(())
    }
    
    pub fn deactivate_user(&mut self, id: u64) -> Result<(), String> {
        let user = self.users.get_mut(&id).ok_or("User not found")?;
        user.active = false;
        Ok(())
    }
}

impl Default for UserManager {
    fn default() -> Self {
        Self::new()
    }
}
