#!/bin/bash
# CLI E2E Test Repository Creator
# Creates various test repositories for comprehensive CLI testing

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Clean up existing test repos
rm -rf "${SCRIPT_DIR}/test-repos" 2>/dev/null || true
mkdir -p "${SCRIPT_DIR}/test-repos"

# Create Small Python Project
create_small_python_project() {
    local repo_dir="${SCRIPT_DIR}/test-repos/small-python"
    mkdir -p "${repo_dir}/src" "${repo_dir}/tests"
    
    cat > "${repo_dir}/src/calculator.py" << 'EOF'
"""Simple calculator module for testing."""

class Calculator:
    """A basic calculator class."""
    
    def add(self, a, b):
        """Add two numbers."""
        return a + b
    
    def subtract(self, a, b):
        """Subtract two numbers."""
        return a - b
    
    def multiply(self, a, b):
        """Multiply two numbers."""
        if a == 0 or b == 0:
            return 0
        return a * b
    
    def divide(self, a, b):
        """Divide two numbers."""
        if b == 0:
            raise ValueError("Cannot divide by zero")
        return a / b

def fibonacci(n):
    """Calculate nth Fibonacci number."""
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)

def is_prime(n):
    """Check if a number is prime."""
    if n < 2:
        return False
    for i in range(2, int(n**0.5) + 1):
        if n % i == 0:
            return False
    return True
EOF

    cat > "${repo_dir}/tests/test_calculator.py" << 'EOF'
"""Tests for calculator module."""
import unittest
from src.calculator import Calculator, fibonacci, is_prime

class TestCalculator(unittest.TestCase):
    def setUp(self):
        self.calc = Calculator()
    
    def test_add(self):
        self.assertEqual(self.calc.add(2, 3), 5)
    
    def test_subtract(self):
        self.assertEqual(self.calc.subtract(5, 3), 2)
    
    def test_multiply(self):
        self.assertEqual(self.calc.multiply(3, 4), 12)
    
    def test_divide(self):
        self.assertEqual(self.calc.divide(8, 2), 4)
        with self.assertRaises(ValueError):
            self.calc.divide(5, 0)

class TestFunctions(unittest.TestCase):
    def test_fibonacci(self):
        self.assertEqual(fibonacci(0), 0)
        self.assertEqual(fibonacci(1), 1)
        self.assertEqual(fibonacci(5), 5)
    
    def test_is_prime(self):
        self.assertTrue(is_prime(7))
        self.assertFalse(is_prime(8))
        self.assertFalse(is_prime(1))
EOF

    cat > "${repo_dir}/README.md" << 'EOF'
# Small Python Project

A simple calculator for testing Valknut CLI functionality.

## Usage

```python
from src.calculator import Calculator
calc = Calculator()
result = calc.add(2, 3)
```
EOF
}

# Create Medium Rust Project
create_medium_rust_project() {
    local repo_dir="${SCRIPT_DIR}/test-repos/medium-rust"
    mkdir -p "${repo_dir}/src/bin" "${repo_dir}/src/lib" "${repo_dir}/tests"
    
    cat > "${repo_dir}/Cargo.toml" << 'EOF'
[package]
name = "test-rust-project"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }

[[bin]]
name = "main"
path = "src/bin/main.rs"
EOF

    cat > "${repo_dir}/src/lib.rs" << 'EOF'
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
EOF

    cat > "${repo_dir}/src/bin/main.rs" << 'EOF'
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
EOF

    cat > "${repo_dir}/tests/integration_tests.rs" << 'EOF'
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
EOF
}

# Create Large Mixed Language Project
create_large_mixed_project() {
    local repo_dir="${SCRIPT_DIR}/test-repos/large-mixed"
    mkdir -p "${repo_dir}/backend/src" "${repo_dir}/frontend/src/components" "${repo_dir}/frontend/src/utils" \
             "${repo_dir}/scripts" "${repo_dir}/docs" "${repo_dir}/tests"
    
    # Backend (Python)
    cat > "${repo_dir}/backend/src/main.py" << 'EOF'
"""Main backend application."""
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from typing import List, Optional
import asyncio
import logging

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

app = FastAPI(title="Test API", version="1.0.0")

class Item(BaseModel):
    id: Optional[int] = None
    name: str
    description: Optional[str] = None
    price: float
    category: str

class ItemCreate(BaseModel):
    name: str
    description: Optional[str] = None
    price: float
    category: str

class ItemUpdate(BaseModel):
    name: Optional[str] = None
    description: Optional[str] = None
    price: Optional[float] = None
    category: Optional[str] = None

# In-memory storage (don't do this in production!)
items_db: List[Item] = []
next_id = 1

@app.get("/")
async def root():
    return {"message": "Test API is running"}

@app.get("/items", response_model=List[Item])
async def get_items(category: Optional[str] = None, limit: int = 100):
    """Get all items, optionally filtered by category."""
    filtered_items = items_db
    if category:
        filtered_items = [item for item in items_db if item.category.lower() == category.lower()]
    return filtered_items[:limit]

@app.get("/items/{item_id}", response_model=Item)
async def get_item(item_id: int):
    """Get a specific item by ID."""
    for item in items_db:
        if item.id == item_id:
            return item
    raise HTTPException(status_code=404, detail="Item not found")

@app.post("/items", response_model=Item)
async def create_item(item: ItemCreate):
    """Create a new item."""
    global next_id
    
    # Validate data
    if item.price <= 0:
        raise HTTPException(status_code=400, detail="Price must be positive")
    
    if not item.name.strip():
        raise HTTPException(status_code=400, detail="Name cannot be empty")
    
    # Check for duplicate names
    for existing_item in items_db:
        if existing_item.name.lower() == item.name.lower():
            raise HTTPException(status_code=400, detail="Item with this name already exists")
    
    new_item = Item(
        id=next_id,
        name=item.name.strip(),
        description=item.description.strip() if item.description else None,
        price=item.price,
        category=item.category.strip()
    )
    
    items_db.append(new_item)
    next_id += 1
    
    logger.info(f"Created item: {new_item.name} (ID: {new_item.id})")
    return new_item

@app.put("/items/{item_id}", response_model=Item)
async def update_item(item_id: int, item_update: ItemUpdate):
    """Update an existing item."""
    for i, existing_item in enumerate(items_db):
        if existing_item.id == item_id:
            # Create updated item
            updated_data = existing_item.dict()
            update_data = item_update.dict(exclude_unset=True)
            
            # Validate updates
            if "price" in update_data and update_data["price"] <= 0:
                raise HTTPException(status_code=400, detail="Price must be positive")
            
            if "name" in update_data and not update_data["name"].strip():
                raise HTTPException(status_code=400, detail="Name cannot be empty")
            
            # Check for duplicate names (excluding current item)
            if "name" in update_data:
                for other_item in items_db:
                    if (other_item.id != item_id and 
                        other_item.name.lower() == update_data["name"].lower()):
                        raise HTTPException(status_code=400, detail="Item with this name already exists")
            
            updated_data.update(update_data)
            updated_item = Item(**updated_data)
            items_db[i] = updated_item
            
            logger.info(f"Updated item: {updated_item.name} (ID: {updated_item.id})")
            return updated_item
    
    raise HTTPException(status_code=404, detail="Item not found")

@app.delete("/items/{item_id}")
async def delete_item(item_id: int):
    """Delete an item."""
    for i, item in enumerate(items_db):
        if item.id == item_id:
            deleted_item = items_db.pop(i)
            logger.info(f"Deleted item: {deleted_item.name} (ID: {deleted_item.id})")
            return {"message": f"Item {item_id} deleted successfully"}
    
    raise HTTPException(status_code=404, detail="Item not found")

# Health check endpoint
@app.get("/health")
async def health_check():
    """Health check endpoint."""
    return {
        "status": "healthy",
        "items_count": len(items_db),
        "timestamp": asyncio.get_event_loop().time()
    }

if __name__ == "__main__":
    import uvicorn
    
    # Add some sample data
    sample_items = [
        ItemCreate(name="Laptop", description="High-performance laptop", price=999.99, category="electronics"),
        ItemCreate(name="Coffee Mug", description="Ceramic coffee mug", price=12.99, category="kitchen"),
        ItemCreate(name="Book", description="Programming book", price=39.99, category="books"),
    ]
    
    for sample_item in sample_items:
        asyncio.run(create_item(sample_item))
    
    uvicorn.run(app, host="0.0.0.0", port=8000)
EOF

    # Frontend (JavaScript/TypeScript)
    cat > "${repo_dir}/frontend/src/components/ItemList.js" << 'EOF'
/**
 * ItemList component for displaying a list of items
 */
import React, { useState, useEffect, useCallback } from 'react';
import { fetchItems, deleteItem } from '../utils/api.js';
import ItemCard from './ItemCard.js';
import LoadingSpinner from './LoadingSpinner.js';
import ErrorMessage from './ErrorMessage.js';

const ItemList = ({ category = null, onItemSelect = null }) => {
    const [items, setItems] = useState([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);
    const [filter, setFilter] = useState('');
    const [sortBy, setSortBy] = useState('name');
    const [sortOrder, setSortOrder] = useState('asc');

    const loadItems = useCallback(async () => {
        try {
            setLoading(true);
            setError(null);
            
            const params = {};
            if (category) {
                params.category = category;
            }
            
            const data = await fetchItems(params);
            setItems(data);
        } catch (err) {
            console.error('Failed to load items:', err);
            setError('Failed to load items. Please try again.');
        } finally {
            setLoading(false);
        }
    }, [category]);

    useEffect(() => {
        loadItems();
    }, [loadItems]);

    const handleDeleteItem = async (itemId) => {
        if (!window.confirm('Are you sure you want to delete this item?')) {
            return;
        }

        try {
            await deleteItem(itemId);
            setItems(prevItems => prevItems.filter(item => item.id !== itemId));
        } catch (err) {
            console.error('Failed to delete item:', err);
            setError('Failed to delete item. Please try again.');
        }
    };

    const filteredAndSortedItems = items
        .filter(item => {
            if (!filter) return true;
            return (
                item.name.toLowerCase().includes(filter.toLowerCase()) ||
                item.description?.toLowerCase().includes(filter.toLowerCase()) ||
                item.category.toLowerCase().includes(filter.toLowerCase())
            );
        })
        .sort((a, b) => {
            let aValue = a[sortBy];
            let bValue = b[sortBy];

            // Handle different data types
            if (typeof aValue === 'string') {
                aValue = aValue.toLowerCase();
                bValue = bValue.toLowerCase();
            }

            if (sortOrder === 'asc') {
                return aValue < bValue ? -1 : aValue > bValue ? 1 : 0;
            } else {
                return aValue > bValue ? -1 : aValue < bValue ? 1 : 0;
            }
        });

    if (loading) {
        return <LoadingSpinner message="Loading items..." />;
    }

    if (error) {
        return (
            <ErrorMessage 
                message={error} 
                onRetry={loadItems}
            />
        );
    }

    return (
        <div className="item-list">
            <div className="item-list-header">
                <h2>
                    {category ? `${category} Items` : 'All Items'} 
                    <span className="item-count">({filteredAndSortedItems.length})</span>
                </h2>
                
                <div className="item-list-controls">
                    <input
                        type="text"
                        placeholder="Filter items..."
                        value={filter}
                        onChange={(e) => setFilter(e.target.value)}
                        className="filter-input"
                    />
                    
                    <select
                        value={sortBy}
                        onChange={(e) => setSortBy(e.target.value)}
                        className="sort-select"
                    >
                        <option value="name">Sort by Name</option>
                        <option value="price">Sort by Price</option>
                        <option value="category">Sort by Category</option>
                    </select>
                    
                    <button
                        onClick={() => setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc')}
                        className="sort-order-btn"
                    >
                        {sortOrder === 'asc' ? '↑' : '↓'}
                    </button>
                    
                    <button
                        onClick={loadItems}
                        className="refresh-btn"
                    >
                        Refresh
                    </button>
                </div>
            </div>
            
            {filteredAndSortedItems.length === 0 ? (
                <div className="no-items">
                    {filter ? 'No items match your filter.' : 'No items found.'}
                </div>
            ) : (
                <div className="items-grid">
                    {filteredAndSortedItems.map(item => (
                        <ItemCard
                            key={item.id}
                            item={item}
                            onSelect={onItemSelect}
                            onDelete={handleDeleteItem}
                        />
                    ))}
                </div>
            )}
        </div>
    );
};

export default ItemList;
EOF

    cat > "${repo_dir}/frontend/src/utils/api.js" << 'EOF'
/**
 * API utility functions for interacting with the backend
 */

const API_BASE_URL = process.env.REACT_APP_API_URL || 'http://localhost:8000';

class ApiError extends Error {
    constructor(message, status, data = null) {
        super(message);
        this.name = 'ApiError';
        this.status = status;
        this.data = data;
    }
}

/**
 * Generic fetch wrapper with error handling
 */
async function apiRequest(endpoint, options = {}) {
    const url = `${API_BASE_URL}${endpoint}`;
    
    const defaultOptions = {
        headers: {
            'Content-Type': 'application/json',
            ...options.headers,
        },
        ...options,
    };

    try {
        const response = await fetch(url, defaultOptions);
        
        let data = null;
        const contentType = response.headers.get('content-type');
        
        if (contentType && contentType.includes('application/json')) {
            data = await response.json();
        } else {
            data = await response.text();
        }

        if (!response.ok) {
            throw new ApiError(
                data?.detail || data?.message || `HTTP ${response.status}`,
                response.status,
                data
            );
        }

        return data;
    } catch (error) {
        if (error instanceof ApiError) {
            throw error;
        }
        
        // Network or other errors
        throw new ApiError(
            'Network error. Please check your connection.',
            0,
            { originalError: error }
        );
    }
}

/**
 * Fetch all items with optional filtering
 */
export async function fetchItems(params = {}) {
    const queryParams = new URLSearchParams();
    
    Object.entries(params).forEach(([key, value]) => {
        if (value !== null && value !== undefined) {
            queryParams.append(key, value.toString());
        }
    });
    
    const queryString = queryParams.toString();
    const endpoint = `/items${queryString ? `?${queryString}` : ''}`;
    
    return apiRequest(endpoint);
}

/**
 * Fetch a single item by ID
 */
export async function fetchItem(itemId) {
    return apiRequest(`/items/${itemId}`);
}

/**
 * Create a new item
 */
export async function createItem(itemData) {
    return apiRequest('/items', {
        method: 'POST',
        body: JSON.stringify(itemData),
    });
}

/**
 * Update an existing item
 */
export async function updateItem(itemId, updateData) {
    return apiRequest(`/items/${itemId}`, {
        method: 'PUT',
        body: JSON.stringify(updateData),
    });
}

/**
 * Delete an item
 */
export async function deleteItem(itemId) {
    return apiRequest(`/items/${itemId}`, {
        method: 'DELETE',
    });
}

/**
 * Health check
 */
export async function healthCheck() {
    return apiRequest('/health');
}

/**
 * Utility function to validate item data
 */
export function validateItemData(itemData) {
    const errors = {};

    if (!itemData.name || itemData.name.trim().length === 0) {
        errors.name = 'Name is required';
    }

    if (itemData.price === undefined || itemData.price === null || itemData.price <= 0) {
        errors.price = 'Price must be a positive number';
    }

    if (!itemData.category || itemData.category.trim().length === 0) {
        errors.category = 'Category is required';
    }

    return {
        isValid: Object.keys(errors).length === 0,
        errors,
    };
}

/**
 * Utility function to format price
 */
export function formatPrice(price) {
    return new Intl.NumberFormat('en-US', {
        style: 'currency',
        currency: 'USD',
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
    }).format(price);
}

/**
 * Utility function to format date
 */
export function formatDate(dateString) {
    const date = new Date(dateString);
    return new Intl.DateTimeFormat('en-US', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    }).format(date);
}

/**
 * Debounce utility for search inputs
 */
export function debounce(func, wait) {
    let timeout;
    return function executedFunction(...args) {
        const later = () => {
            clearTimeout(timeout);
            func(...args);
        };
        clearTimeout(timeout);
        timeout = setTimeout(later, wait);
    };
}
EOF

    cat > "${repo_dir}/README.md" << 'EOF'
# Large Mixed Language Project

This is a comprehensive test project for Valknut CLI analysis, featuring:

- **Backend**: Python FastAPI application with REST endpoints
- **Frontend**: React/JavaScript components and utilities  
- **Scripts**: Build and deployment automation
- **Documentation**: API docs and user guides

## Structure

```
├── backend/          # Python FastAPI backend
├── frontend/         # React frontend components
├── scripts/          # Build and deployment scripts
├── docs/            # Documentation
└── tests/           # Cross-integration tests
```

## Features

- User management system
- Item inventory with CRUD operations
- Real-time data synchronization
- Comprehensive error handling
- Performance optimizations
- Security best practices

## Development

```bash
# Backend
cd backend && pip install -r requirements.txt
python src/main.py

# Frontend  
cd frontend && npm install
npm start
```
EOF
}

# Create Performance Test Repository
create_performance_test_repo() {
    local repo_dir="${SCRIPT_DIR}/test-repos/performance-test"
    mkdir -p "${repo_dir}/src"
    
    cat > "${repo_dir}/src/complex_algorithms.py" << 'EOF'
"""Complex algorithms for performance testing."""
import time
import random
from typing import List, Dict, Any
from collections import defaultdict

class PerformanceTestSuite:
    """Collection of performance-intensive algorithms."""
    
    def __init__(self):
        self.cache = {}
        self.stats = defaultdict(int)
    
    def fibonacci_recursive(self, n: int) -> int:
        """Inefficient recursive Fibonacci for complexity testing."""
        if n in self.cache:
            return self.cache[n]
        
        if n <= 1:
            result = n
        else:
            result = self.fibonacci_recursive(n-1) + self.fibonacci_recursive(n-2)
        
        self.cache[n] = result
        return result
    
    def bubble_sort(self, arr: List[int]) -> List[int]:
        """Bubble sort implementation for complexity analysis."""
        arr = arr.copy()
        n = len(arr)
        
        for i in range(n):
            swapped = False
            for j in range(0, n - i - 1):
                if arr[j] > arr[j + 1]:
                    arr[j], arr[j + 1] = arr[j + 1], arr[j]
                    swapped = True
                    self.stats['swaps'] += 1
            
            if not swapped:
                break
            
            self.stats['iterations'] += 1
        
        return arr
    
    def nested_loops_complexity(self, n: int) -> Dict[str, Any]:
        """Multiple nested loops for complexity testing."""
        results = {
            'single_loop': [],
            'double_loop': [],
            'triple_loop': []
        }
        
        # O(n) complexity
        for i in range(n):
            results['single_loop'].append(i * 2)
        
        # O(n²) complexity
        for i in range(n):
            for j in range(n):
                if i != j:
                    results['double_loop'].append((i, j))
        
        # O(n³) complexity  
        for i in range(min(n, 10)):  # Limit to prevent timeout
            for j in range(min(n, 10)):
                for k in range(min(n, 10)):
                    if i + j + k == n:
                        results['triple_loop'].append((i, j, k))
        
        return results
    
    def recursive_tree_traversal(self, depth: int, branching_factor: int = 3) -> int:
        """Recursive tree traversal for complexity analysis."""
        if depth <= 0:
            return 1
        
        total = 0
        for _ in range(branching_factor):
            total += self.recursive_tree_traversal(depth - 1, branching_factor)
        
        return total
    
    def memory_intensive_operation(self, size: int) -> List[List[int]]:
        """Memory-intensive operation for resource testing."""
        matrix = []
        for i in range(size):
            row = []
            for j in range(size):
                # Complex calculation to stress both CPU and memory
                value = (i * j) % 1000
                if value % 2 == 0:
                    value = value ** 2
                else:
                    value = value * 3 + 1
                row.append(value)
            matrix.append(row)
        
        return matrix
    
    def string_operations_complexity(self, text: str, operations: int) -> Dict[str, Any]:
        """String operation complexity testing."""
        results = {
            'concatenations': '',
            'replacements': text,
            'searches': []
        }
        
        # String concatenation in loop (inefficient)
        for i in range(operations):
            results['concatenations'] += f"operation_{i}_"
        
        # Multiple string replacements
        for i in range(operations):
            old_char = chr(ord('a') + (i % 26))
            new_char = chr(ord('A') + (i % 26))
            results['replacements'] = results['replacements'].replace(old_char, new_char)
        
        # String searching
        for i in range(operations):
            search_term = f"search_{i}"
            if search_term in text:
                results['searches'].append(i)
        
        return results
    
    def graph_traversal_complexity(self, nodes: int) -> Dict[str, Any]:
        """Graph traversal algorithms for complexity testing."""
        # Create a random graph
        graph = defaultdict(list)
        for i in range(nodes):
            # Each node connects to random other nodes
            connections = random.randint(1, min(nodes - 1, 5))
            for _ in range(connections):
                target = random.randint(0, nodes - 1)
                if target != i:
                    graph[i].append(target)
        
        # Depth-First Search
        visited_dfs = set()
        def dfs(node):
            if node in visited_dfs:
                return 0
            visited_dfs.add(node)
            count = 1
            for neighbor in graph[node]:
                count += dfs(neighbor)
            return count
        
        dfs_nodes = dfs(0) if nodes > 0 else 0
        
        # Breadth-First Search
        visited_bfs = set()
        queue = [0] if nodes > 0 else []
        bfs_nodes = 0
        
        while queue:
            node = queue.pop(0)
            if node not in visited_bfs:
                visited_bfs.add(node)
                bfs_nodes += 1
                for neighbor in graph[node]:
                    if neighbor not in visited_bfs:
                        queue.append(neighbor)
        
        return {
            'graph_size': nodes,
            'dfs_visited': dfs_nodes,
            'bfs_visited': bfs_nodes,
            'graph_density': sum(len(neighbors) for neighbors in graph.values()) / nodes if nodes > 0 else 0
        }

def run_performance_benchmark():
    """Run a comprehensive performance benchmark."""
    suite = PerformanceTestSuite()
    
    print("Running performance benchmark...")
    start_time = time.time()
    
    # Test different complexity scenarios
    results = {}
    
    # Fibonacci test
    results['fibonacci'] = suite.fibonacci_recursive(25)
    
    # Sorting test
    test_array = [random.randint(1, 1000) for _ in range(100)]
    results['sorted_array'] = suite.bubble_sort(test_array)
    
    # Nested loops test
    results['nested_loops'] = suite.nested_loops_complexity(20)
    
    # Recursive tree test
    results['tree_traversal'] = suite.recursive_tree_traversal(8)
    
    # Memory test
    results['memory_matrix'] = len(suite.memory_intensive_operation(50))
    
    # String operations test
    test_text = "The quick brown fox jumps over the lazy dog. " * 100
    results['string_ops'] = suite.string_operations_complexity(test_text, 50)
    
    # Graph traversal test
    results['graph_traversal'] = suite.graph_traversal_complexity(100)
    
    end_time = time.time()
    
    print(f"Benchmark completed in {end_time - start_time:.2f} seconds")
    print(f"Cache hits: {len(suite.cache)}")
    print(f"Operations stats: {dict(suite.stats)}")
    
    return results

if __name__ == "__main__":
    run_performance_benchmark()
EOF

    # Create a Go file for mixed language complexity
    cat > "${repo_dir}/src/concurrent_algorithms.go" << 'EOF'
package main

import (
	"fmt"
	"math/rand"
	"runtime"
	"sync"
	"time"
)

// ComplexStruct represents a complex data structure for testing
type ComplexStruct struct {
	ID       int
	Name     string
	Values   []float64
	Children map[string]*ComplexStruct
	mutex    sync.RWMutex
}

// NewComplexStruct creates a new complex struct with random data
func NewComplexStruct(id int, depth int) *ComplexStruct {
	cs := &ComplexStruct{
		ID:       id,
		Name:     fmt.Sprintf("struct_%d", id),
		Values:   make([]float64, rand.Intn(100)+10),
		Children: make(map[string]*ComplexStruct),
	}

	// Fill values with random data
	for i := range cs.Values {
		cs.Values[i] = rand.Float64() * 1000
	}

	// Create children recursively (limited depth to prevent stack overflow)
	if depth > 0 {
		numChildren := rand.Intn(5) + 1
		for i := 0; i < numChildren; i++ {
			childID := id*10 + i
			childKey := fmt.Sprintf("child_%d", i)
			cs.Children[childKey] = NewComplexStruct(childID, depth-1)
		}
	}

	return cs
}

// ProcessValues performs complex calculations on values
func (cs *ComplexStruct) ProcessValues() float64 {
	cs.mutex.RLock()
	defer cs.mutex.RUnlock()

	var result float64
	for i, value := range cs.Values {
		// Complex mathematical operations
		if i%2 == 0 {
			result += value * value
		} else {
			result += value / (float64(i) + 1)
		}

		// Simulate some processing time
		for j := 0; j < 1000; j++ {
			result += float64(j) * 0.001
		}
	}

	return result
}

// ProcessChildrenConcurrently processes all children concurrently
func (cs *ComplexStruct) ProcessChildrenConcurrently() map[string]float64 {
	cs.mutex.RLock()
	children := make(map[string]*ComplexStruct)
	for k, v := range cs.Children {
		children[k] = v
	}
	cs.mutex.RUnlock()

	results := make(map[string]float64)
	var wg sync.WaitGroup
	var mutex sync.Mutex

	for key, child := range children {
		wg.Add(1)
		go func(k string, c *ComplexStruct) {
			defer wg.Done()
			result := c.ProcessValues()
			
			mutex.Lock()
			results[k] = result
			mutex.Unlock()
		}(key, child)
	}

	wg.Wait()
	return results
}

// DeepTraversal performs a deep traversal of the structure
func (cs *ComplexStruct) DeepTraversal(visitor func(*ComplexStruct)) {
	cs.mutex.RLock()
	defer cs.mutex.RUnlock()

	visitor(cs)

	for _, child := range cs.Children {
		child.DeepTraversal(visitor)
	}
}

// ConcurrentAlgorithms demonstrates various concurrent patterns
type ConcurrentAlgorithms struct {
	data     []*ComplexStruct
	results  map[int]float64
	mutex    sync.RWMutex
	workerWG sync.WaitGroup
}

// NewConcurrentAlgorithms creates a new instance
func NewConcurrentAlgorithms(size int) *ConcurrentAlgorithms {
	data := make([]*ComplexStruct, size)
	for i := 0; i < size; i++ {
		data[i] = NewComplexStruct(i, 3) // depth of 3
	}

	return &ConcurrentAlgorithms{
		data:    data,
		results: make(map[int]float64),
	}
}

// ParallelProcessing processes data using multiple goroutines
func (ca *ConcurrentAlgorithms) ParallelProcessing(numWorkers int) {
	jobs := make(chan *ComplexStruct, len(ca.data))
	
	// Start workers
	for w := 0; w < numWorkers; w++ {
		ca.workerWG.Add(1)
		go ca.worker(jobs)
	}

	// Send jobs
	for _, item := range ca.data {
		jobs <- item
	}
	close(jobs)

	ca.workerWG.Wait()
}

// worker processes items from the jobs channel
func (ca *ConcurrentAlgorithms) worker(jobs <-chan *ComplexStruct) {
	defer ca.workerWG.Done()
	
	for item := range jobs {
		// Simulate complex processing
		result := item.ProcessValues()
		
		// Process children concurrently
		childResults := item.ProcessChildrenConcurrently()
		for _, childResult := range childResults {
			result += childResult * 0.1
		}

		// Store result safely
		ca.mutex.Lock()
		ca.results[item.ID] = result
		ca.mutex.Unlock()
		
		// Simulate some additional processing time
		time.Sleep(time.Millisecond * time.Duration(rand.Intn(10)+1))
	}
}

// PipelineProcessing demonstrates pipeline pattern
func (ca *ConcurrentAlgorithms) PipelineProcessing() <-chan float64 {
	// Stage 1: Generate data
	stage1 := make(chan *ComplexStruct)
	go func() {
		defer close(stage1)
		for _, item := range ca.data {
			stage1 <- item
		}
	}()

	// Stage 2: Process values
	stage2 := make(chan float64)
	go func() {
		defer close(stage2)
		for item := range stage1 {
			result := item.ProcessValues()
			stage2 <- result
		}
	}()

	// Stage 3: Apply transformations
	stage3 := make(chan float64)
	go func() {
		defer close(stage3)
		for value := range stage2 {
			// Apply complex transformation
			transformed := value * 1.5
			if transformed > 10000 {
				transformed = transformed / 2
			}
			stage3 <- transformed
		}
	}()

	return stage3
}

// MemoryIntensiveOperation creates and processes large amounts of data
func (ca *ConcurrentAlgorithms) MemoryIntensiveOperation() {
	// Create large slices
	largeSlice := make([][]float64, 1000)
	for i := range largeSlice {
		largeSlice[i] = make([]float64, 1000)
		for j := range largeSlice[i] {
			largeSlice[i][j] = rand.Float64() * 100
		}
	}

	// Process in parallel
	var wg sync.WaitGroup
	numGoroutines := runtime.NumCPU()
	chunkSize := len(largeSlice) / numGoroutines

	for i := 0; i < numGoroutines; i++ {
		start := i * chunkSize
		end := start + chunkSize
		if i == numGoroutines-1 {
			end = len(largeSlice)
		}

		wg.Add(1)
		go func(start, end int) {
			defer wg.Done()
			for i := start; i < end; i++ {
				for j := range largeSlice[i] {
					// Complex calculation
					largeSlice[i][j] = largeSlice[i][j]*largeSlice[i][j] + float64(i*j)
				}
			}
		}(start, end)
	}

	wg.Wait()
}

func main() {
	fmt.Println("Starting concurrent algorithms performance test...")
	
	// Set up random seed
	rand.Seed(time.Now().UnixNano())
	
	start := time.Now()
	
	// Create algorithm suite
	ca := NewConcurrentAlgorithms(100)
	
	// Test parallel processing
	fmt.Println("Running parallel processing...")
	ca.ParallelProcessing(runtime.NumCPU())
	
	// Test pipeline processing
	fmt.Println("Running pipeline processing...")
	results := ca.PipelineProcessing()
	count := 0
	total := 0.0
	for result := range results {
		total += result
		count++
	}
	
	fmt.Printf("Pipeline processed %d items, average: %.2f\n", count, total/float64(count))
	
	// Test memory intensive operation
	fmt.Println("Running memory intensive operation...")
	ca.MemoryIntensiveOperation()
	
	elapsed := time.Since(start)
	fmt.Printf("Total execution time: %v\n", elapsed)
	fmt.Printf("Final results count: %d\n", len(ca.results))
	
	// Print memory stats
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	fmt.Printf("Memory allocated: %d KB\n", m.Alloc/1024)
	fmt.Printf("Total allocations: %d\n", m.TotalAlloc/1024)
	fmt.Printf("Number of GC runs: %d\n", m.NumGC)
}
EOF
}

# Create Configuration Test Repository
create_config_test_repo() {
    local repo_dir="${SCRIPT_DIR}/test-repos/config-test"
    mkdir -p "${repo_dir}/configs"
    
    # Minimal configuration
    cat > "${repo_dir}/configs/minimal.yml" << 'EOF'
analysis:
  enable_scoring: true
  enable_graph_analysis: false
  enable_lsh_analysis: false
  enable_refactoring_analysis: false
  enable_coverage_analysis: false
  enable_structure_analysis: false
  enable_names_analysis: false
  confidence_threshold: 0.8
  max_files: 10
  include_patterns: ["**/*.py"]
  exclude_patterns: []
scoring:
  normalization_scheme: z_score
  use_bayesian_fallbacks: false
  confidence_reporting: false
  weights:
    complexity: 1.0
    graph: 0.0
    structure: 0.0
    style: 0.0
    coverage: 0.0
  statistical_params:
    confidence_level: 0.95
    min_sample_size: 5
    outlier_threshold: 2.0
EOF

    # Maximum configuration
    cat > "${repo_dir}/configs/maximum.yml" << 'EOF'
analysis:
  enable_scoring: true
  enable_graph_analysis: true
  enable_lsh_analysis: true
  enable_refactoring_analysis: true
  enable_coverage_analysis: true
  enable_structure_analysis: true
  enable_names_analysis: true
  confidence_threshold: 0.6
  max_files: 1000
  include_patterns: ["**/*.py", "**/*.rs", "**/*.js", "**/*.ts", "**/*.go"]
  exclude_patterns: ["*/node_modules/*", "*/target/*", "*/__pycache__/*"]
scoring:
  normalization_scheme: z_score
  use_bayesian_fallbacks: true
  confidence_reporting: true
  weights:
    complexity: 1.0
    graph: 0.9
    structure: 0.8
    style: 0.7
    coverage: 0.6
  statistical_params:
    confidence_level: 0.99
    min_sample_size: 20
    outlier_threshold: 3.0
graph:
  enable_betweenness: true
  enable_closeness: true
  enable_cycle_detection: true
  max_exact_size: 50000
  use_approximation: true
  approximation_sample_rate: 0.2
lsh:
  num_hashes: 256
  num_bands: 32
  shingle_size: 5
  similarity_threshold: 0.8
  max_candidates: 500
  use_semantic_similarity: true
languages:
  python:
    enabled: true
    file_extensions: [".py", ".pyi"]
    tree_sitter_language: "python"
    max_file_size_mb: 20.0
    complexity_threshold: 15.0
    additional_settings: {}
  rust:
    enabled: true
    file_extensions: [".rs"]
    tree_sitter_language: "rust"
    max_file_size_mb: 20.0
    complexity_threshold: 20.0
    additional_settings: {}
  javascript:
    enabled: true
    file_extensions: [".js", ".jsx"]
    tree_sitter_language: "javascript"
    max_file_size_mb: 15.0
    complexity_threshold: 12.0
    additional_settings: {}
  typescript:
    enabled: true
    file_extensions: [".ts", ".tsx"]
    tree_sitter_language: "typescript"
    max_file_size_mb: 15.0
    complexity_threshold: 12.0
    additional_settings: {}
io:
  cache_dir: "/tmp/valknut-cache"
  enable_caching: true
  cache_ttl_seconds: 7200
  report_dir: "./reports"
  report_format: json
performance:
  max_threads: 8
  memory_limit_mb: 4096
  file_timeout_seconds: 120
  total_timeout_seconds: 3600
  enable_simd: true
  batch_size: 50
structure:
  min_branch_recommendation_gain: 0.2
  min_files_for_split: 10
  target_loc_per_subdir: 2000
coverage:
  auto_discover: true
  search_paths: ["./coverage/", "./target/coverage/", "./htmlcov/"]
  file_patterns: ["coverage.xml", "lcov.info", "coverage.json", ".coverage"]
  max_age_days: 14
  coverage_file: null
EOF

    # Invalid configuration
    cat > "${repo_dir}/configs/invalid.yml" << 'EOF'
analysis:
  enable_scoring: not_a_boolean
  confidence_threshold: "invalid_number"
  max_files: -1
  include_patterns: "should_be_array"
scoring:
  weights:
    complexity: "not_a_number"
    invalid_weight: 1.0
performance:
  max_threads: 0
  memory_limit_mb: -100
EOF

    # Sample source file
    cat > "${repo_dir}/sample.py" << 'EOF'
def simple_function():
    return "Hello, World!"

class TestClass:
    def __init__(self):
        self.value = 42
    
    def get_value(self):
        return self.value
EOF
}

# Main execution
echo "Creating test repositories..."

create_small_python_project
echo "✓ Created small Python project"

create_medium_rust_project  
echo "✓ Created medium Rust project"

create_large_mixed_project
echo "✓ Created large mixed language project"

create_performance_test_repo
echo "✓ Created performance test repository"

create_config_test_repo
echo "✓ Created configuration test repository"

echo "
Test repositories created in: ${SCRIPT_DIR}/test-repos/

Available repositories:
- small-python/     : Simple Python calculator project
- medium-rust/      : Medium-sized Rust user management system
- large-mixed/      : Large project with Python backend and JS frontend
- performance-test/ : Complex algorithms for performance testing
- config-test/      : Repository with various configuration files

Use these repositories to test different CLI scenarios and configurations.
"