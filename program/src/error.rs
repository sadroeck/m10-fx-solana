use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use solana_program::decode_error::DecodeError;
use solana_program::msg;
use solana_program::program_error::{PrintProgramError, ProgramError};
use std::error::Error;

#[derive(thiserror::Error, Debug, FromPrimitive)]
pub enum FxError {
    #[error("Invalid request")]
    InvalidRequest,
    #[error("Missing signature")]
    MissingSignature,
    #[error("Invalid token ID")]
    InvalidTokenId,
    #[error("Not rent exempt")]
    NotRentExempt,
    #[error("Invalid amount")]
    InvalidAmount,
    #[error("Invalid FX feed")]
    InvalidFxFeed,
    #[error("No available liquidity provider")]
    NoLiquidity,
    #[error("Swap conditions not met")]
    SwapConditionsNotMet,
}

pub type FxResult<T> = Result<T, FxError>;

impl From<FxError> for ProgramError {
    fn from(err: FxError) -> Self {
        ProgramError::Custom(err as u32)
    }
}

impl<T> DecodeError<T> for FxError {
    fn type_of() -> &'static str {
        "FxError"
    }
}

impl PrintProgramError for FxError {
    fn print<E>(&self)
    where
        E: 'static + Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        msg!("{:?}", self)
    }
}
