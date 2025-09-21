use lasso::{Capacity, Rodeo, Spur, ThreadedRodeo};
use std::sync::Arc;

/// A lightweight key representing an interned string
pub type InternedString = Spur;

/// Thread-safe string interner for the entire valknut analysis pipeline
#[derive(Clone)]
pub struct StringInterner {
    inner: Arc<ThreadedRodeo>,
}

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
            inner: Arc::new(ThreadedRodeo::with_capacity(Capacity::for_strings(capacity))),
        }
    }

    /// Intern a string and return its key, or return existing key if already interned
    pub fn get_or_intern<S: AsRef<str>>(&self, string: S) -> InternedString {
        self.inner.get_or_intern(string.as_ref())
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
        self.inner.strings().map(|s| s.len() + std::mem::size_of::<String>()).sum::<usize>()
            + (self.inner.len() * std::mem::size_of::<InternedString>())
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for StringInterner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StringInterner")
            .field("len", &self.len())
            .field("memory_usage", &self.memory_usage())
            .finish()
    }
}

/// Global string interner instance for the entire valknut analysis
static GLOBAL_INTERNER: once_cell::sync::Lazy<StringInterner> = 
    once_cell::sync::Lazy::new(|| StringInterner::with_capacity(100_000));

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
        
        let handle = thread::spawn(move || {
            interner_clone.get_or_intern("thread_test")
        });
        
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