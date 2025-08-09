use thiserror::Error;

#[derive(Error, Debug)]
pub enum AiError {
    #[error("Provider error: {source}")]
    Provider {
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    #[error("Authentication failed: {message}")]
    Authentication { message: String },

    #[error("Rate limited: {message}")]
    RateLimit { message: String },

    #[error("Model not found: {model_id}")]
    ModelNotFound { model_id: String },

    #[error("JSON parsing error: {source}")]
    Json { source: serde_json::Error },

    #[error("Network error: {source}")]
    Network {
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Stream ended unexpectedly")]
    StreamEnded,

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl AiError {
    pub fn provider<E>(source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Provider {
            source: Box::new(source),
        }
    }

    pub fn network<E>(source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Network {
            source: Box::new(source),
        }
    }

    pub fn invalid_request<S: Into<String>>(message: S) -> Self {
        Self::InvalidRequest {
            message: message.into(),
        }
    }

    pub fn internal<S: Into<String>>(message: S) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

impl From<serde_json::Error> for AiError {
    fn from(source: serde_json::Error) -> Self {
        Self::Json { source }
    }
}
