use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash};
use std::sync::Mutex;

pub struct ShardedMap<K, V> {
    shards: Vec<Mutex<HashMap<K, V>>>,
    hasher: RandomState,
}

impl<K: Hash + Eq, V> ShardedMap<K, V> {
    pub fn new(shards: usize) -> Self {
        assert!(shards >= 1, "shards must be at least 1");
        Self {
            shards: (0..shards).map(|_| Mutex::new(HashMap::new())).collect(),
            hasher: RandomState::new(),
        }
    }

    pub fn insert(&self, key: K, value: V) -> Option<V> {
        self.shard(&key).lock().unwrap().insert(key, value)
    }

    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        self.shard(key).lock().unwrap().get(key).cloned()
    }

    pub fn update<R>(&self, key: K, default: impl FnOnce() -> V, f: impl FnOnce(&mut V) -> R) -> R {
        let mut shard = self.shard(&key).lock().unwrap();
        f(shard.entry(key).or_insert_with(default))
    }

    fn shard(&self, key: &K) -> &Mutex<HashMap<K, V>> {
        // Reduce in u64 so the choice does not depend on the pointer width.
        let i = self.hasher.hash_one(key) % self.shards.len() as u64;
        &self.shards[i as usize]
    }
}
