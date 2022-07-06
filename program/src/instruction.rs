use borsh::{BorshDeserialize as Deserialize, BorshSerialize as Serialize};
use solana_program::clock::UnixTimestamp;
use solana_program::program_error::ProgramError;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum FxEvent {
    /// Request a quoted FX swap between the [`from`] & [`to`] accounts based on the provided limits.
    /// if either the [`upper_limit`] or [`lower_limit`] is exceeded,
    /// the quote is settled at the current market rate.
    /// If the [`valid_until`] is exceeded, the quote is settled at the current market rate.
    /// Accounts:
    ///     0. [`initializer`] - `[signer]` The account of the person initializing the fx swap
    ///     1. [`from_account`] `[writable]` Temporary token account that should be created prior to this instruction and owned by the initializer
    ///     2. [`to_account`] The receiver's token account for the token they will receive when the swap executes
    ///     3. [`fx_account`] `[writable]` The fx account, it will hold all necessary info about the swap.
    ///     4. [`rent`] The rent sysvar
    ///     5. [`token`] The token program
    ///     6. [`fx_feed`] The program providing the FX feed
    Initiate {
        amount: u64,
        upper_limit: u64,
        lower_limit: u64,
        valid_until: UnixTimestamp,
    },
    /// Attempt to settle the FX swap based on the initiated conditions
    /// Accounts:
    ///     0. [`fx_account`] `[writable]` The fx account, it will hold all necessary info about the swap.
    ///     1. [`fx_program`] The program providing FX information
    ///     2. [`fx_feed`] The program providing the FX feed
    ///     3. [`token`] The token program
    TryExecute,
}

impl FxEvent {
    #[inline]
    pub fn from_bytes(input: &[u8]) -> Result<Self, ProgramError> {
        Self::try_from_slice(input).map_err(|err| ProgramError::BorshIoError(err.to_string()))
    }
}
