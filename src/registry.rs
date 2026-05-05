use crate::providers::LyricsProvider;
use std::collections::HashMap;

#[derive(Default)]
pub struct ProviderRegistry {
    providers: HashMap<&'static str, Box<dyn LyricsProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, provider: Box<dyn LyricsProvider>) {
        self.providers.insert(provider.id(), provider);
    }

    pub fn get(&self, id: &str) -> Option<&dyn LyricsProvider> {
        self.providers.get(id).map(|p| p.as_ref())
    }
}
