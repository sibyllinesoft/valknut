//! String interning infrastructure for memory-efficient entity handling.
//!
//! This module provides thread-safe string interning using the `lasso` crate,
//! enabling zero-copy string comparisons and significant memory savings when
//! processing large codebases with many duplicate strings (file paths, entity
//! names, AST node types).
//!
//! # Key Components
//!
//! - [`StringInterner`]: Thread-safe interner for storing unique strings
//! - [`InternedString`]: Lightweight key type for interned strings
//! - [`global_interner`]: Singleton instance pre-populated with common AST types
//!
//! # Usage
//!
//! ```ignore
//! use valknut::core::interning::{intern, resolve};
//!
//! // Intern a string and get a lightweight key
//! let key = intern("function_definition");
//!
//! // Resolve the key back to the string (zero-cost lookup)
//! let name = resolve(key);
//! assert_eq!(name, "function_definition");
//! ```

use lasso::{Capacity, Rodeo, Spur, ThreadedRodeo};
use std::sync::Arc;

/// A lightweight key representing an interned string
pub type InternedString = Spur;

/// Thread-safe string interner for the entire valknut analysis pipeline
#[derive(Clone)]
pub struct StringInterner {
    inner: Arc<ThreadedRodeo>,
}

/// Factory, interning, and lookup methods for [`StringInterner`].
impl StringInterner {
    /// Create a new string interner with default capacity
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ThreadedRodeo::default()),
        }
    }

    /// Create a new string interner with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(ThreadedRodeo::with_capacity(Capacity::for_strings(
                capacity,
            ))),
        }
    }

    /// Intern a string and return its key, or return existing key if already interned
    pub fn get_or_intern<S: AsRef<str>>(&self, string: S) -> InternedString {
        self.inner.get_or_intern(string.as_ref())
    }

    /// Batch intern multiple strings for optimal performance during parsing
    /// Returns a vector of interned keys in the same order as input
    pub fn batch_intern<S: AsRef<str>>(&self, strings: &[S]) -> Vec<InternedString> {
        // For ThreadedRodeo, batch operations are already optimized internally
        // But we can still provide a convenience method for cleaner code
        strings
            .iter()
            .map(|s| self.inner.get_or_intern(s.as_ref()))
            .collect()
    }

    /// Get the key for an already-interned string, returns None if not found
    pub fn get<S: AsRef<str>>(&self, string: S) -> Option<InternedString> {
        self.inner.get(string.as_ref())
    }

    /// Resolve an interned string key back to the original string
    pub fn resolve(&self, key: InternedString) -> &str {
        self.inner.resolve(&key)
    }

    /// Check if a string is already interned
    pub fn contains<S: AsRef<str>>(&self, string: S) -> bool {
        self.inner.contains(string.as_ref())
    }

    /// Get the number of interned strings
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the interner is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get memory usage statistics
    pub fn memory_usage(&self) -> usize {
        // Approximate memory usage calculation
        // Each string has overhead + the string data itself
        self.inner
            .strings()
            .map(|s| s.len() + std::mem::size_of::<String>())
            .sum::<usize>()
            + (self.inner.len() * std::mem::size_of::<InternedString>())
    }
}

/// Default implementation for [`StringInterner`].
impl Default for StringInterner {
    /// Returns a new string interner with default capacity.
    fn default() -> Self {
        Self::new()
    }
}

/// [`Debug`] implementation for [`StringInterner`].
impl std::fmt::Debug for StringInterner {
    /// Formats the interner showing length and memory usage.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StringInterner")
            .field("len", &self.len())
            .field("memory_usage", &self.memory_usage())
            .finish()
    }
}

/// Pre-populate common AST node types and keywords to eliminate string comparisons
fn create_prepopulated_interner() -> StringInterner {
    let interner = StringInterner::with_capacity(100_000);

    // Pre-intern common AST node types to eliminate string matching during parsing
    let common_node_types = [
        // Common across languages
        "identifier",
        "function_definition",
        "class_definition",
        "method_definition",
        "call_expression",
        "assignment",
        "import_statement",
        "import_from_statement",
        "if_statement",
        "for_statement",
        "while_statement",
        "try_statement",
        "expression_statement",
        "return_statement",
        "comment",
        "string",
        "number",
        // Python specific
        "module",
        "decorated_definition",
        "async_function_definition",
        "lambda",
        "list_comprehension",
        "dictionary_comprehension",
        "set_comprehension",
        // JavaScript/TypeScript specific
        "program",
        "function_declaration",
        "arrow_function",
        "method_definition",
        "class_declaration",
        "interface_declaration",
        "type_alias_declaration",
        // Rust specific
        "source_file",
        "function_item",
        "struct_item",
        "impl_item",
        "trait_item",
        "mod_item",
        "use_declaration",
        "macro_invocation",
        // Go specific
        "source_file",
        "function_declaration",
        "method_declaration",
        "type_declaration",
        "interface_type",
        "struct_type",
        "package_clause",
        "import_declaration",
    ];

    // Batch intern all common types
    interner.batch_intern(&common_node_types);

    interner
}

/// Global string interner instance for the entire valknut analysis
static GLOBAL_INTERNER: once_cell::sync::Lazy<StringInterner> =
    once_cell::sync::Lazy::new(create_prepopulated_interner);

/// Get a reference to the global string interner
pub fn global_interner() -> &'static StringInterner {
    &GLOBAL_INTERNER
}

/// Convenience function to intern a string using the global interner
pub fn intern<S: AsRef<str>>(string: S) -> InternedString {
    global_interner().get_or_intern(string)
}

/// Convenience function to resolve an interned string using the global interner
pub fn resolve(key: InternedString) -> &'static str {
    global_interner().resolve(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_interning() {
        let interner = StringInterner::new();

        let key1 = interner.get_or_intern("hello");
        let key2 = interner.get_or_intern("world");
        let key3 = interner.get_or_intern("hello"); // Duplicate

        assert_eq!(key1, key3); // Same string should get same key
        assert_ne!(key1, key2); // Different strings should get different keys

        assert_eq!(interner.resolve(key1), "hello");
        assert_eq!(interner.resolve(key2), "world");
        assert_eq!(interner.len(), 2); // Only 2 unique strings
    }

    #[test]
    fn test_global_interner() {
        let key1 = intern("global_test");
        let key2 = intern("global_test");

        assert_eq!(key1, key2);
        assert_eq!(resolve(key1), "global_test");
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let interner = StringInterner::new();
        let interner_clone = interner.clone();

        let handle = thread::spawn(move || interner_clone.get_or_intern("thread_test"));

        let key1 = interner.get_or_intern("thread_test");
        let key2 = handle.join().unwrap();

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_memory_usage_tracking() {
        let interner = StringInterner::new();
        let initial_usage = interner.memory_usage();

        interner.get_or_intern("test_memory_usage");
        let after_usage = interner.memory_usage();

        assert!(after_usage > initial_usage);
    }
}
