use std::fmt;

/// Custom error type for the Omni-Mixer
#[derive(Debug)]
pub enum OmniMixerError {
    /// Treasury has insufficient funds
    InsufficientTreasuryFunds(String),
    /// Invalid position data
    InvalidPosition(String),
    /// Mixing round not found
    RoundNotFound(String),
    /// Round is in invalid state for operation
    InvalidRoundState(String),
    /// Merkle root calculation failed
    MerkleRootCalculation(String),
    /// Serialization/deserialization error
    Serialization(String),
    /// Database operation failed
    Database(String),
    /// Network communication error
    Network(String),
    /// Configuration error
    Configuration(String),
    /// Internal system error
    Internal(String),
}

impl fmt::Display for OmniMixerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OmniMixerError::InsufficientTreasuryFunds(msg) => {
                write!(f, "Insufficient treasury funds: {}", msg)
            }
            OmniMixerError::InvalidPosition(msg) => {
                write!(f, "Invalid position: {}", msg)
            }
            OmniMixerError::RoundNotFound(msg) => {
                write!(f, "Round not found: {}", msg)
            }
            OmniMixerError::InvalidRoundState(msg) => {
                write!(f, "Invalid round state: {}", msg)
            }
            OmniMixerError::MerkleRootCalculation(msg) => {
                write!(f, "Merkle root calculation failed: {}", msg)
            }
            OmniMixerError::Serialization(msg) => {
                write!(f, "Serialization error: {}", msg)
            }
            OmniMixerError::Database(msg) => {
                write!(f, "Database error: {}", msg)
            }
            OmniMixerError::Network(msg) => {
                write!(f, "Network error: {}", msg)
            }
            OmniMixerError::Configuration(msg) => {
                write!(f, "Configuration error: {}", msg)
            }
            OmniMixerError::Internal(msg) => {
                write!(f, "Internal error: {}", msg)
            }
        }
    }
}

impl std::error::Error for OmniMixerError {}

/// Result type alias for Omni-Mixer operations
pub type Result<T> = std::result::Result<T, OmniMixerError>;

// Conversion implementations for common error types

impl From<serde_json::Error> for OmniMixerError {
    fn from(err: serde_json::Error) -> Self {
        OmniMixerError::Serialization(err.to_string())
    }
}

impl From<std::io::Error> for OmniMixerError {
    fn from(err: std::io::Error) -> Self {
        OmniMixerError::Internal(format!("IO error: {}", err))
    }
}

impl From<std::num::ParseIntError> for OmniMixerError {
    fn from(err: std::num::ParseIntError) -> Self {
        OmniMixerError::InvalidPosition(format!("Parse error: {}", err))
    }
}

impl From<uuid::Error> for OmniMixerError {
    fn from(err: uuid::Error) -> Self {
        OmniMixerError::Internal(format!("UUID error: {}", err))
    }
}

impl From<std::time::SystemTimeError> for OmniMixerError {
    fn from(err: std::time::SystemTimeError) -> Self {
        OmniMixerError::Internal(format!("System time error: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = OmniMixerError::InsufficientTreasuryFunds("Not enough funds".to_string());
        assert!(error.to_string().contains("Insufficient treasury funds"));

        let error = OmniMixerError::RoundNotFound("round_123".to_string());
        assert!(error.to_string().contains("Round not found"));
    }

    #[test]
    fn test_error_conversions() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let omni_err: OmniMixerError = json_err.into();
        match omni_err {
            OmniMixerError::Serialization(_) => {},
            _ => panic!("Expected Serialization error"),
        }
    }
}
