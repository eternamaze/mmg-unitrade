use crate::common::account_model::{OrderId, SubAccountId};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ExchangeError {
    #[error("Network connection failed: {0}")]
    Network(String),

    #[error("API rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Order not found: {id} in account {sub_account_id}")]
    OrderNotFound {
        sub_account_id: SubAccountId,
        id: OrderId,
    },

    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),

    #[error("Invalid instrument: {0}")]
    InvalidInstrument(String),

    #[error("Exchange internal error: {0}")]
    ExchangeInternal(String),

    #[error("Operation not supported: {0}")]
    NotSupported(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type ExchangeResult<T> = Result<T, ExchangeError>;
