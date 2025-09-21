//! High-performance string interning for common telecommunications identifiers
//! Eliminates repeated allocations for phone number prefixes, trunk names, etc.

use ahash::AHasher;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::hash::BuildHasherDefault;
use std::sync::RwLock;
use string_interner::{backend::StringBackend, DefaultSymbol, StringInterner};

type FastHasher = BuildHasherDefault<AHasher>;

/// Symbol type for interned strings
pub type Symbol = DefaultSymbol;

/// High-performance string interner for telecommunications data
#[derive(Debug)]
pub struct TelecomStringInterner {
    /// Core string interner
    interner: RwLock<StringInterner<StringBackend<DefaultSymbol>>>,
    /// Fast lookup cache for frequently accessed strings
    symbol_cache: DashMap<Symbol, String, FastHasher>,
    /// Reverse lookup cache for string-to-symbol conversion
    string_cache: DashMap<String, Symbol, FastHasher>,
}

impl TelecomStringInterner {
    /// Create a new string interner optimized for telecom data
    pub fn new() -> Self {
        Self {
            interner: RwLock::new(StringInterner::new()),
            symbol_cache: DashMap::with_hasher(FastHasher::default()),
            string_cache: DashMap::with_hasher(FastHasher::default()),
        }
    }

    /// Intern a string and return its symbol
    pub fn intern<T: AsRef<str>>(&self, string: T) -> Symbol {
        let string_ref = string.as_ref();

        // Fast path: check if already cached
        if let Some(symbol) = self.string_cache.get(string_ref) {
            return *symbol;
        }

        // Slow path: intern the string
        let symbol = {
            let mut interner = self.interner.write().unwrap();
            interner.get_or_intern(string_ref)
        };

        // Cache for future lookups
        self.string_cache.insert(string_ref.to_string(), symbol);
        self.symbol_cache.insert(symbol, string_ref.to_string());

        symbol
    }

    /// Resolve a symbol back to its string
    pub fn resolve(&self, symbol: Symbol) -> Option<String> {
        // Fast path: check cache first
        if let Some(string) = self.symbol_cache.get(&symbol) {
            return Some(string.clone());
        }

        // Slow path: resolve from interner
        let interner = self.interner.read().unwrap();
        if let Some(string) = interner.resolve(symbol) {
            let owned_string = string.to_string();
            // Cache for future lookups
            self.symbol_cache.insert(symbol, owned_string.clone());
            Some(owned_string)
        } else {
            None
        }
    }

    /// Get statistics about the interner
    pub fn stats(&self) -> InternerStats {
        let interner = self.interner.read().unwrap();
        InternerStats {
            interned_strings: interner.len(),
            symbol_cache_size: self.symbol_cache.len(),
            string_cache_size: self.string_cache.len(),
        }
    }

    /// Clear caches (keep interned strings for memory efficiency)
    pub fn clear_caches(&self) {
        self.symbol_cache.clear();
        self.string_cache.clear();
    }
}

impl Default for TelecomStringInterner {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about string interner usage
#[derive(Debug, Clone)]
pub struct InternerStats {
    pub interned_strings: usize,
    pub symbol_cache_size: usize,
    pub string_cache_size: usize,
}

/// Global string interner instances for different data types
pub struct GlobalInterners {
    /// Phone numbers and prefixes (1NXXNXXXXXX, +1, etc.)
    pub phone_numbers: TelecomStringInterner,
    /// Trunk names and identifiers
    pub trunk_ids: TelecomStringInterner,
    /// Customer identifiers and names
    pub customer_ids: TelecomStringInterner,
    /// SIP call-ids and dialog identifiers
    pub sip_identifiers: TelecomStringInterner,
    /// Codec names (G.711, G.729, etc.)
    pub codec_names: TelecomStringInterner,
    /// Error messages and status codes
    pub error_messages: TelecomStringInterner,
}

impl GlobalInterners {
    pub fn new() -> Self {
        Self {
            phone_numbers: TelecomStringInterner::new(),
            trunk_ids: TelecomStringInterner::new(),
            customer_ids: TelecomStringInterner::new(),
            sip_identifiers: TelecomStringInterner::new(),
            codec_names: TelecomStringInterner::new(),
            error_messages: TelecomStringInterner::new(),
        }
    }

    /// Pre-populate with common telecommunications strings
    pub fn preload_common_strings(&self) {
        // Common phone number prefixes
        let common_prefixes = [
            "1", "+1", "011", "00", "1800", "1888", "1877", "1866", "1855", "1844", "1833", "1822",
            "911", "411", "311", "211", "611", "711", "811",
        ];
        for prefix in &common_prefixes {
            self.phone_numbers.intern(prefix);
        }

        // Common codec names
        let common_codecs = [
            "G.711", "PCMU", "PCMA", "G.729", "G.722", "G.723", "GSM", "iLBC", "Speex", "Opus",
            "AMR", "AMR-WB", "SILK",
        ];
        for codec in &common_codecs {
            self.codec_names.intern(codec);
        }

        // Common SIP response codes and messages
        let common_sip_messages = [
            "100 Trying",
            "180 Ringing",
            "183 Progress",
            "200 OK",
            "300 Multiple Choices",
            "301 Moved Permanently",
            "302 Moved Temporarily",
            "400 Bad Request",
            "401 Unauthorized",
            "403 Forbidden",
            "404 Not Found",
            "480 Temporarily Unavailable",
            "486 Busy Here",
            "487 Request Terminated",
            "500 Internal Server Error",
            "503 Service Unavailable",
            "504 Server Time-out",
        ];
        for message in &common_sip_messages {
            self.error_messages.intern(message);
        }

        // Common trunk prefixes
        let common_trunk_prefixes = [
            "trunk-",
            "sip-",
            "pri-",
            "fxo-",
            "fxs-",
            "t1-",
            "e1-",
            "origination-",
            "termination-",
            "test-",
            "backup-",
        ];
        for prefix in &common_trunk_prefixes {
            self.trunk_ids.intern(prefix);
        }
    }

    /// Get overall statistics
    pub fn overall_stats(&self) -> OverallInternerStats {
        OverallInternerStats {
            phone_numbers: self.phone_numbers.stats(),
            trunk_ids: self.trunk_ids.stats(),
            customer_ids: self.customer_ids.stats(),
            sip_identifiers: self.sip_identifiers.stats(),
            codec_names: self.codec_names.stats(),
            error_messages: self.error_messages.stats(),
        }
    }
}

impl Default for GlobalInterners {
    fn default() -> Self {
        let interners = Self::new();
        interners.preload_common_strings();
        interners
    }
}

/// Overall statistics for all interners
#[derive(Debug, Clone)]
pub struct OverallInternerStats {
    pub phone_numbers: InternerStats,
    pub trunk_ids: InternerStats,
    pub customer_ids: InternerStats,
    pub sip_identifiers: InternerStats,
    pub codec_names: InternerStats,
    pub error_messages: InternerStats,
}

/// Global interner instance
pub static INTERNERS: Lazy<GlobalInterners> = Lazy::new(|| GlobalInterners::default());

/// Convenient access functions
pub fn intern_phone_number<T: AsRef<str>>(number: T) -> Symbol {
    INTERNERS.phone_numbers.intern(number)
}

pub fn intern_trunk_id<T: AsRef<str>>(trunk_id: T) -> Symbol {
    INTERNERS.trunk_ids.intern(trunk_id)
}

pub fn intern_customer_id<T: AsRef<str>>(customer_id: T) -> Symbol {
    INTERNERS.customer_ids.intern(customer_id)
}

pub fn intern_sip_id<T: AsRef<str>>(sip_id: T) -> Symbol {
    INTERNERS.sip_identifiers.intern(sip_id)
}

pub fn intern_codec_name<T: AsRef<str>>(codec: T) -> Symbol {
    INTERNERS.codec_names.intern(codec)
}

pub fn intern_error_message<T: AsRef<str>>(message: T) -> Symbol {
    INTERNERS.error_messages.intern(message)
}

/// Resolve functions
pub fn resolve_phone_number(symbol: Symbol) -> Option<String> {
    INTERNERS.phone_numbers.resolve(symbol)
}

pub fn resolve_trunk_id(symbol: Symbol) -> Option<String> {
    INTERNERS.trunk_ids.resolve(symbol)
}

pub fn resolve_customer_id(symbol: Symbol) -> Option<String> {
    INTERNERS.customer_ids.resolve(symbol)
}

pub fn resolve_sip_id(symbol: Symbol) -> Option<String> {
    INTERNERS.sip_identifiers.resolve(symbol)
}

pub fn resolve_codec_name(symbol: Symbol) -> Option<String> {
    INTERNERS.codec_names.resolve(symbol)
}

pub fn resolve_error_message(symbol: Symbol) -> Option<String> {
    INTERNERS.error_messages.resolve(symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_interning() {
        let interner = TelecomStringInterner::new();

        let symbol1 = interner.intern("1234567890");
        let symbol2 = interner.intern("1234567890");

        // Same string should return same symbol
        assert_eq!(symbol1, symbol2);

        // Should be able to resolve back
        assert_eq!(interner.resolve(symbol1), Some("1234567890".to_string()));
    }

    #[test]
    fn test_global_interners() {
        let symbol = intern_phone_number("18001234567");
        assert_eq!(
            resolve_phone_number(symbol),
            Some("18001234567".to_string())
        );

        let codec_symbol = intern_codec_name("G.711");
        assert_eq!(resolve_codec_name(codec_symbol), Some("G.711".to_string()));
    }

    #[test]
    fn test_preloaded_strings() {
        // Common strings should already be interned
        let symbol1 = intern_codec_name("G.711");
        let symbol2 = intern_codec_name("G.711");
        assert_eq!(symbol1, symbol2);
    }
}
