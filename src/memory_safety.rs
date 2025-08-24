/*
 * Memory Safety and Concurrency Utilities
 *
 * Provides safe wrappers and utilities to eliminate common memory safety
 * and concurrency issues in the redfire-switch telephony application.
 *
 * Key safety patterns:
 * - Safe Arc cloning without try_unwrap
 * - Proper error handling instead of unwrap/panic
 * - Deadlock-free lock ordering
 * - Race condition prevention
 * - Memory leak detection helpers
 */

use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

/// Safe wrapper for Arc operations that prevents common memory safety issues
pub struct SafeArc<T> {
    inner: Arc<T>,
    creation_time: Instant,
}

impl<T> SafeArc<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(value),
            creation_time: Instant::now(),
        }
    }

    /// Safe clone that tracks reference count
    pub fn safe_clone(&self) -> Self {
        debug!(
            "Cloning Arc - current strong count: {}",
            Arc::strong_count(&self.inner)
        );
        Self {
            inner: Arc::clone(&self.inner),
            creation_time: self.creation_time,
        }
    }

    /// Get strong reference count
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// Get weak reference count  
    pub fn weak_count(&self) -> usize {
        Arc::weak_count(&self.inner)
    }

    /// Get age of this Arc
    pub fn age(&self) -> Duration {
        self.creation_time.elapsed()
    }

    /// Create a weak reference
    pub fn downgrade(&self) -> Weak<T> {
        Arc::downgrade(&self.inner)
    }

    /// Get reference to inner value
    pub fn as_ref(&self) -> &T {
        &self.inner
    }

    /// Try to get mutable reference if this is the only reference
    pub fn try_get_mut(&mut self) -> Option<&mut T> {
        Arc::get_mut(&mut self.inner)
    }
}

impl<T> Clone for SafeArc<T> {
    fn clone(&self) -> Self {
        self.safe_clone()
    }
}

impl<T> std::ops::Deref for SafeArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Safe mutex wrapper that prevents deadlocks and panics
pub struct SafeMutex<T> {
    inner: Mutex<T>,
    name: String,
    creation_time: Instant,
}

impl<T> SafeMutex<T> {
    pub fn new(value: T, name: impl Into<String>) -> Self {
        Self {
            inner: Mutex::new(value),
            name: name.into(),
            creation_time: Instant::now(),
        }
    }

    /// Safe lock with timeout and error handling
    pub fn safe_lock(&self, timeout: Duration) -> Result<MutexGuard<T>> {
        let start = Instant::now();

        // Try to acquire lock with spinning timeout
        loop {
            match self.inner.try_lock() {
                Ok(guard) => {
                    debug!("Acquired lock '{}' after {:?}", self.name, start.elapsed());
                    return Ok(guard);
                }
                Err(_) if start.elapsed() < timeout => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(_) => {
                    error!("Lock timeout after {:?} for mutex '{}'", timeout, self.name);
                    return Err(anyhow!("Mutex lock timeout: {}", self.name));
                }
            }
        }
    }

    /// Non-blocking try lock
    pub fn try_lock(&self) -> Result<MutexGuard<T>> {
        self.inner
            .try_lock()
            .map_err(|_| anyhow!("Mutex '{}' is locked", self.name))
    }

    /// Get mutex name for debugging
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Safe RwLock wrapper with proper error handling
pub struct SafeRwLock<T> {
    inner: RwLock<T>,
    name: String,
    creation_time: Instant,
}

impl<T> SafeRwLock<T> {
    pub fn new(value: T, name: impl Into<String>) -> Self {
        Self {
            inner: RwLock::new(value),
            name: name.into(),
            creation_time: Instant::now(),
        }
    }

    /// Safe read lock with timeout
    pub fn safe_read(&self, timeout: Duration) -> Result<RwLockReadGuard<T>> {
        let start = Instant::now();

        loop {
            match self.inner.try_read() {
                Ok(guard) => {
                    debug!(
                        "Acquired read lock '{}' after {:?}",
                        self.name,
                        start.elapsed()
                    );
                    return Ok(guard);
                }
                Err(_) if start.elapsed() < timeout => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(_) => {
                    error!(
                        "Read lock timeout after {:?} for RwLock '{}'",
                        timeout, self.name
                    );
                    return Err(anyhow!("RwLock read timeout: {}", self.name));
                }
            }
        }
    }

    /// Safe write lock with timeout
    pub fn safe_write(&self, timeout: Duration) -> Result<RwLockWriteGuard<T>> {
        let start = Instant::now();

        loop {
            match self.inner.try_write() {
                Ok(guard) => {
                    debug!(
                        "Acquired write lock '{}' after {:?}",
                        self.name,
                        start.elapsed()
                    );
                    return Ok(guard);
                }
                Err(_) if start.elapsed() < timeout => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(_) => {
                    error!(
                        "Write lock timeout after {:?} for RwLock '{}'",
                        timeout, self.name
                    );
                    return Err(anyhow!("RwLock write timeout: {}", self.name));
                }
            }
        }
    }

    /// Try non-blocking read
    pub fn try_read(&self) -> Result<RwLockReadGuard<T>> {
        self.inner
            .try_read()
            .map_err(|_| anyhow!("RwLock '{}' read lock failed", self.name))
    }

    /// Try non-blocking write
    pub fn try_write(&self) -> Result<RwLockWriteGuard<T>> {
        self.inner
            .try_write()
            .map_err(|_| anyhow!("RwLock '{}' write lock failed", self.name))
    }
}

/// Safe channel wrapper that handles disconnections gracefully
pub struct SafeChannel<T> {
    sender: mpsc::UnboundedSender<T>,
    name: String,
}

impl<T> SafeChannel<T> {
    pub fn new(name: impl Into<String>) -> (Self, mpsc::UnboundedReceiver<T>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let channel = Self {
            sender: tx,
            name: name.into(),
        };
        (channel, rx)
    }

    /// Safe send that handles receiver disconnection
    pub fn safe_send(&self, value: T) -> Result<()> {
        self.sender
            .send(value)
            .map_err(|_| anyhow!("Channel '{}' receiver disconnected", self.name))
    }

    /// Check if receiver is still connected
    pub fn is_connected(&self) -> bool {
        !self.sender.is_closed()
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<T> Clone for SafeChannel<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            name: self.name.clone(),
        }
    }
}

/// Memory leak detection utility
pub struct MemoryTracker {
    allocations: SafeRwLock<std::collections::HashMap<String, (usize, Instant)>>,
}

impl MemoryTracker {
    pub fn new() -> Self {
        Self {
            allocations: SafeRwLock::new(std::collections::HashMap::new(), "memory_tracker"),
        }
    }

    /// Track allocation
    pub fn track_allocation(&self, name: impl Into<String>, size: usize) -> Result<()> {
        let mut allocs = self.allocations.safe_write(Duration::from_millis(100))?;
        allocs.insert(name.into(), (size, Instant::now()));
        Ok(())
    }

    /// Track deallocation
    pub fn track_deallocation(&self, name: &str) -> Result<()> {
        let mut allocs = self.allocations.safe_write(Duration::from_millis(100))?;
        if allocs.remove(name).is_none() {
            warn!("Attempted to deallocate unknown allocation: {}", name);
        }
        Ok(())
    }

    /// Get current allocations
    pub fn get_allocations(&self) -> Result<Vec<(String, usize, Duration)>> {
        let allocs = self.allocations.safe_read(Duration::from_millis(100))?;
        let now = Instant::now();

        Ok(allocs
            .iter()
            .map(|(name, (size, time))| (name.clone(), *size, now.duration_since(*time)))
            .collect())
    }

    /// Check for potential leaks (allocations older than threshold)
    pub fn check_leaks(&self, age_threshold: Duration) -> Result<Vec<String>> {
        let allocs = self.get_allocations()?;
        Ok(allocs
            .into_iter()
            .filter(|(_, _, age)| *age > age_threshold)
            .map(|(name, _, _)| name)
            .collect())
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Safe parsing utilities that avoid unwrap/expect
pub mod safe_parsing {
    use super::*;

    /// Safe string to IP address parsing
    pub fn parse_socket_addr(addr_str: &str) -> Result<std::net::SocketAddr> {
        addr_str
            .parse()
            .map_err(|e| anyhow!("Invalid socket address '{}': {}", addr_str, e))
    }

    /// Safe string to number parsing
    pub fn parse_number<T>(num_str: &str) -> Result<T>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        num_str
            .parse()
            .map_err(|e| anyhow!("Invalid number '{}': {}", num_str, e))
    }

    /// Safe duration parsing from seconds
    pub fn parse_duration_secs(secs_str: &str) -> Result<Duration> {
        let secs: u64 = parse_number(secs_str)?;
        Ok(Duration::from_secs(secs))
    }

    /// Safe duration parsing from milliseconds
    pub fn parse_duration_millis(millis_str: &str) -> Result<Duration> {
        let millis: u64 = parse_number(millis_str)?;
        Ok(Duration::from_millis(millis))
    }
}

/// Concurrency utilities
pub mod concurrency {
    use super::*;
    use std::collections::HashMap;

    /// Lock ordering manager to prevent deadlocks
    pub struct LockOrderManager {
        lock_ids: SafeRwLock<HashMap<String, usize>>,
        next_id: std::sync::atomic::AtomicUsize,
    }

    impl LockOrderManager {
        pub fn new() -> Self {
            Self {
                lock_ids: SafeRwLock::new(HashMap::new(), "lock_order_manager"),
                next_id: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        /// Register a lock and get its ordering ID
        pub fn register_lock(&self, name: &str) -> Result<usize> {
            let mut locks = self.lock_ids.safe_write(Duration::from_millis(100))?;

            if let Some(&id) = locks.get(name) {
                return Ok(id);
            }

            let id = self
                .next_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            locks.insert(name.to_string(), id);
            Ok(id)
        }

        /// Validate lock acquisition order to prevent deadlocks
        pub fn validate_order(&self, current_locks: &[&str], new_lock: &str) -> Result<bool> {
            let locks = self.lock_ids.safe_read(Duration::from_millis(100))?;

            let new_lock_id = locks
                .get(new_lock)
                .copied()
                .ok_or_else(|| anyhow!("Unregistered lock: {}", new_lock))?;

            // Check that all current locks have lower IDs than the new lock
            for &current_lock in current_locks {
                let current_id = locks
                    .get(current_lock)
                    .copied()
                    .ok_or_else(|| anyhow!("Unregistered lock: {}", current_lock))?;

                if current_id >= new_lock_id {
                    warn!("Potential deadlock: trying to acquire lock '{}' (id: {}) while holding '{}' (id: {})",
                          new_lock, new_lock_id, current_lock, current_id);
                    return Ok(false);
                }
            }

            Ok(true)
        }
    }

    impl Default for LockOrderManager {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_safe_arc_operations() {
        let arc = SafeArc::new(42);
        assert_eq!(*arc, 42);
        assert_eq!(arc.strong_count(), 1);

        let arc2 = arc.safe_clone();
        assert_eq!(arc.strong_count(), 2);
        assert_eq!(arc2.strong_count(), 2);

        drop(arc2);
        assert_eq!(arc.strong_count(), 1);
    }

    #[tokio::test]
    async fn test_safe_mutex_timeout() {
        let mutex = SafeMutex::new(42, "test_mutex");

        let _guard1 = mutex.try_lock().unwrap();

        // This should timeout
        let result = mutex.safe_lock(Duration::from_millis(10));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_safe_channel() {
        let (channel, mut rx) = SafeChannel::new("test_channel");

        assert!(channel.is_connected());
        channel.safe_send(42).unwrap();

        let received = timeout(Duration::from_millis(100), rx.recv())
            .await
            .unwrap();
        assert_eq!(received, Some(42));
    }

    #[test]
    fn test_memory_tracker() {
        let tracker = MemoryTracker::new();

        tracker.track_allocation("test_alloc", 1024).unwrap();
        let allocs = tracker.get_allocations().unwrap();
        assert_eq!(allocs.len(), 1);

        tracker.track_deallocation("test_alloc").unwrap();
        let allocs = tracker.get_allocations().unwrap();
        assert_eq!(allocs.len(), 0);
    }

    #[test]
    fn test_safe_parsing() {
        use safe_parsing::*;

        assert!(parse_socket_addr("127.0.0.1:8080").is_ok());
        assert!(parse_socket_addr("invalid").is_err());

        assert_eq!(parse_number::<i32>("42").unwrap(), 42);
        assert!(parse_number::<i32>("invalid").is_err());

        assert_eq!(parse_duration_secs("5").unwrap(), Duration::from_secs(5));
        assert_eq!(
            parse_duration_millis("500").unwrap(),
            Duration::from_millis(500)
        );
    }
}
