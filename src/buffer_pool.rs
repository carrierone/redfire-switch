/*
 * Buffer Pool for High-Performance Audio Processing
 *
 * Provides reusable buffers to reduce allocations in hot paths
 * like codec processing and TDM frame handling.
 */

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

/// Reusable buffer pool to reduce allocations
pub struct BufferPool<T: Clone + Default> {
    pool: Arc<Mutex<VecDeque<Vec<T>>>>,
    buffer_size: usize,
    max_pool_size: usize,
}

impl<T: Clone + Default> BufferPool<T> {
    /// Create a new buffer pool
    pub fn new(buffer_size: usize, max_pool_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(VecDeque::with_capacity(max_pool_size))),
            buffer_size,
            max_pool_size,
        }
    }

    /// Get a buffer from the pool or create a new one
    pub fn get(&self) -> PooledBuffer<T> {
        let mut pool = self.pool.lock();
        let buffer = pool
            .pop_front()
            .unwrap_or_else(|| vec![T::default(); self.buffer_size]);

        PooledBuffer {
            buffer,
            pool: Arc::clone(&self.pool),
            max_pool_size: self.max_pool_size,
        }
    }

    /// Pre-allocate buffers in the pool
    pub fn preallocate(&self, count: usize) {
        let mut pool = self.pool.lock();
        for _ in 0..count.min(self.max_pool_size) {
            if pool.len() >= self.max_pool_size {
                break;
            }
            pool.push_back(vec![T::default(); self.buffer_size]);
        }
    }
}

/// A buffer borrowed from the pool
pub struct PooledBuffer<T: Clone + Default> {
    buffer: Vec<T>,
    pool: Arc<Mutex<VecDeque<Vec<T>>>>,
    max_pool_size: usize,
}

impl<T: Clone + Default> PooledBuffer<T> {
    /// Get a mutable reference to the buffer
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.buffer
    }

    /// Get an immutable reference to the buffer
    pub fn as_slice(&self) -> &[T] {
        &self.buffer
    }

    /// Resize the buffer if needed
    pub fn resize(&mut self, new_len: usize, value: T) {
        self.buffer.resize(new_len, value);
    }
}

impl<T: Clone + Default> Drop for PooledBuffer<T> {
    fn drop(&mut self) {
        // Clear the buffer and return it to the pool
        self.buffer.clear();
        self.buffer.resize(self.buffer.capacity(), T::default());

        let mut pool = self.pool.lock();
        if pool.len() < self.max_pool_size {
            pool.push_back(std::mem::take(&mut self.buffer));
        }
    }
}

impl<T: Clone + Default> std::ops::Deref for PooledBuffer<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<T: Clone + Default> std::ops::DerefMut for PooledBuffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

/// Specialized buffer pool for audio samples
pub struct AudioBufferPool {
    f32_pool: BufferPool<f32>,
    u8_pool: BufferPool<u8>,
    i16_pool: BufferPool<i16>,
}

impl AudioBufferPool {
    /// Create a new audio buffer pool with common sizes
    pub fn new() -> Self {
        Self {
            f32_pool: BufferPool::new(160, 32), // 20ms at 8kHz
            u8_pool: BufferPool::new(32, 64),   // TDM frame size
            i16_pool: BufferPool::new(160, 32), // Linear PCM buffer
        }
    }

    /// Get an f32 buffer for audio processing
    pub fn get_f32_buffer(&self) -> PooledBuffer<f32> {
        self.f32_pool.get()
    }

    /// Get a u8 buffer for TDM frames
    pub fn get_u8_buffer(&self) -> PooledBuffer<u8> {
        self.u8_pool.get()
    }

    /// Get an i16 buffer for linear PCM
    pub fn get_i16_buffer(&self) -> PooledBuffer<i16> {
        self.i16_pool.get()
    }

    /// Preallocate buffers for better performance
    pub fn preallocate_all(&self) {
        self.f32_pool.preallocate(16);
        self.u8_pool.preallocate(32);
        self.i16_pool.preallocate(16);
    }
}

impl Default for AudioBufferPool {
    fn default() -> Self {
        Self::new()
    }
}

/// String cache for frequently used channel IDs
pub struct ChannelIdCache {
    cache: Arc<Mutex<HashMap<(u16, u8), String>>>,
}

use std::collections::HashMap;

impl ChannelIdCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::with_capacity(256))),
        }
    }

    /// Get or create a channel ID string
    pub fn get_or_create(&self, circuit_id: u16, channel_num: u8) -> String {
        let mut cache = self.cache.lock();
        cache
            .entry((circuit_id, channel_num))
            .or_insert_with(|| format!("C{}-{}", circuit_id, channel_num))
            .clone()
    }

    /// Preallocate common channel IDs
    pub fn preallocate(&self, circuit_id: u16, max_channels: u8) {
        let mut cache = self.cache.lock();
        for channel in 1..=max_channels {
            cache.insert(
                (circuit_id, channel),
                format!("C{}-{}", circuit_id, channel),
            );
        }
    }
}

impl Default for ChannelIdCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool() {
        let pool: BufferPool<f32> = BufferPool::new(100, 10);

        // Get a buffer
        let mut buffer1 = pool.get();
        assert_eq!(buffer1.len(), 100);

        // Modify the buffer
        buffer1[0] = 1.0;

        // Drop returns it to pool
        drop(buffer1);

        // Get another buffer - should reuse the previous one
        let buffer2 = pool.get();
        assert_eq!(buffer2.len(), 100);
    }

    #[test]
    fn test_audio_buffer_pool() {
        let pool = AudioBufferPool::new();
        pool.preallocate_all();

        let mut f32_buf = pool.get_f32_buffer();
        assert_eq!(f32_buf.len(), 160);

        f32_buf[0] = 0.5;
        drop(f32_buf);

        let u8_buf = pool.get_u8_buffer();
        assert_eq!(u8_buf.len(), 32);
    }

    #[test]
    fn test_channel_id_cache() {
        let cache = ChannelIdCache::new();

        let id1 = cache.get_or_create(1, 5);
        assert_eq!(id1, "C1-5");

        // Second call should return cached value
        let id2 = cache.get_or_create(1, 5);
        assert_eq!(id2, "C1-5");
    }
}
