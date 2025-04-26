extern crate alloc;
use alloc::{vec::Vec, vec};
use arceos_api::misc::ax_random;

/// Simple HashMap
pub struct HashMap<K, V> {
    buckets: Vec<Option<Vec<(K, V)>>>,
    size: usize,
    secret: u128,
}

impl<K: core::hash::Hash + Eq + core::clone::Clone, V: core::clone::Clone> HashMap<K, V> {
    /// Create a new empty HashMap
    pub fn new() -> Self {
        const INITIAL_BUCKETS: usize = 64;

        Self {
            buckets: vec![None; INITIAL_BUCKETS],
            size: 0,
            secret: ax_random(),
        }
    }

    /// Insert a key-value pair
    pub fn insert(&mut self, k: K, v: V) {
        let idx = self.hash(&k) % self.buckets.len();

        match &mut self.buckets[idx] {
            Some(bucket) => {
                for &mut (ref existing_key, ref mut existing_value) in bucket.iter_mut() {
                    if existing_key == &k {
                        *existing_value = v;
                        return;
                    }
                }
                bucket.push((k, v));
            }
            None => {
                self.buckets[idx] = Some(vec![(k, v)]);
            }
        }

        self.size += 1;
    }

    /// Simple hash function
    fn hash(&self, k: &K) -> usize {
        use core::hash::{Hash, Hasher};

        let mut hasher = SimpleHasher(self.secret);
        k.hash(&mut hasher);
        hasher.finish() as usize
    }
    /// iter()
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            buckets: self.buckets.iter(),
            current_bucket: None,
        }
    }
}

/// A simple hasher using secret
struct SimpleHasher(u128);

impl core::hash::Hasher for SimpleHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = self.0.wrapping_mul(31).wrapping_add(b as u128);
        }
    }

    fn finish(&self) -> u64 {
        (self.0 >> 64) as u64 ^ (self.0 as u64)
    }
}

pub struct Iter<'a, K, V> {
    buckets: core::slice::Iter<'a, Option<Vec<(K, V)>>>,
    current_bucket: Option<core::slice::Iter<'a, (K, V)>>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(bucket_iter) = &mut self.current_bucket {
                if let Some((k, v)) = bucket_iter.next() {
                    return Some((k, v));
                }
            }
            match self.buckets.next() {
                Some(Some(bucket)) => {
                    self.current_bucket = Some(bucket.iter());
                }
                Some(None) => {
                    continue;
                }
                None => {
                    return None;
                }
            }
        }
    }
}

