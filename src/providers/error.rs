use nd_pdk::lyrics::Error as LyricsError;
use thiserror::Error;

const DEFAULT_RETRY_AFTER_SECS: i64 = 60;
const MIN_RETRY_AFTER_SECS: i64 = 1;
const MAX_RETRY_AFTER_SECS: i64 = 24 * 3600;

pub type ProviderResult<T> = Result<T, ProviderError>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderError {
    #[error("rate limited, retry in {retry_after_secs}s")]
    RateLimited { retry_after_secs: i64 },

    #[error("{0}")]
    Other(String),
}

impl ProviderError {
    pub fn other(message: impl Into<String>) -> Self {
        ProviderError::Other(message.into())
    }

    pub fn rate_limited(retry_after_secs: Option<i64>) -> Self {
        ProviderError::RateLimited {
            retry_after_secs: retry_after_secs
                .unwrap_or(DEFAULT_RETRY_AFTER_SECS)
                .clamp(MIN_RETRY_AFTER_SECS, MAX_RETRY_AFTER_SECS),
        }
    }
}

impl From<ProviderError> for LyricsError {
    fn from(error: ProviderError) -> Self {
        LyricsError::new(error.to_string())
    }
}
