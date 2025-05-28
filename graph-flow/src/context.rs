use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;

/// Context for sharing data between tasks in a graph execution
#[derive(Clone, Debug)]
pub struct Context {
    data: Arc<DashMap<String, Value>>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
        }
    }

    pub async fn set(&self, key: impl Into<String>, value: impl serde::Serialize) {
        let value = serde_json::to_value(value).expect("Failed to serialize value");
        self.data.insert(key.into(), value);
    }

    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.data
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    pub async fn remove(&self, key: &str) -> Option<Value> {
        self.data.remove(key).map(|(_, v)| v)
    }

    pub async fn clear(&self) {
        self.data.clear();
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
