use crate::{config::LyricsProviderId, providers::LyricsProvider};
use std::collections::HashMap;

pub struct ProviderRegistry {
    providers: HashMap<LyricsProviderId, Box<dyn LyricsProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn LyricsProvider>) {
        self.providers.insert(provider.id(), provider);
    }

    pub fn get(&self, id: &LyricsProviderId) -> Option<&Box<dyn LyricsProvider>> {
        self.providers.get(id)
    }
}
