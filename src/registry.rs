use crate::config::ProviderEntry;
use crate::providers::LyricsProvider;
use std::collections::HashMap;

type ProviderFactory = fn(Option<&str>) -> Box<dyn LyricsProvider>;

#[derive(Default)]
pub struct ProviderRegistry {
    factories: HashMap<String, ProviderFactory>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: &str, factory: ProviderFactory) {
        self.factories.insert(name.to_string(), factory);
    }

    pub fn create(&self, entry: &ProviderEntry) -> Option<Box<dyn LyricsProvider>> {
        let factory = self.factories.get(&entry.name)?;
        Some(factory(entry.param.as_deref()))
    }
}
