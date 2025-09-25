use parking_lot::RwLock;
use std::collections::HashMap;

use crate::storage::types::KvEntry;

#[derive(Debug)]
pub struct Shard {
    pub map: RwLock<HashMap<String, KvEntry>>,
}

impl Shard {
    pub fn new() -> Self {
        Self {
            map: RwLock::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &str) -> Option<KvEntry> {
        let map = self.map.read();
        map.get(key).cloned()
    }

    pub fn set(&self, key: String, entry: KvEntry) -> Option<KvEntry> {
        let mut map = self.map.write();
        map.insert(key, entry)
    }

    pub fn del(&self, key: &str) -> Option<KvEntry> {
        let mut map = self.map.write();
        map.remove(key)
    }

    pub fn exists(&self, key: &str) -> bool {
        let map = self.map.read();
        map.contains_key(key)
    }

    pub fn len(&self) -> usize {
        let map = self.map.read();
        map.len()
    }

    // For snapshotting — returns clone of entire shard
    pub fn snapshot(&self) -> HashMap<String, KvEntry> {
        let map = self.map.read();
        map.clone()
    }
}
