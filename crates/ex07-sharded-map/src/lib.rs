use std::hash::Hash;
use std::marker::PhantomData;

pub struct ShardedMap<K, V> {
    // Replace this placeholder with your own fields.
    _placeholder: PhantomData<(K, V)>,
}

impl<K: Hash + Eq, V> ShardedMap<K, V> {
    pub fn new(shards: usize) -> Self {
        let _ = shards;
        todo!()
    }

    pub fn insert(&self, key: K, value: V) -> Option<V> {
        let _ = (key, value);
        todo!()
    }

    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        let _ = key;
        todo!()
    }

    pub fn update<R>(&self, key: K, default: impl FnOnce() -> V, f: impl FnOnce(&mut V) -> R) -> R {
        let _ = (key, default, f);
        todo!()
    }
}
