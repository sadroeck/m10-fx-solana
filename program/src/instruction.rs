use crate::utils::pda_swap;
use borsh::{BorshDeserialize as Deserialize, BorshSerialize as Serialize};
use rust_decimal::Decimal;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::sysvar::SysvarId;
use std::time::Duration;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum FxEvent {
    /// Request a quoted FX swap between the [`from`] & [`to`] accounts based on the provided limits.
    /// if either the [`upper_limit`] or [`lower_limit`] is exceeded,
    /// the quote is settled at the current market rate.
    /// If the [`valid_until`] is exceeded, the quote is settled at the current market rate.
    /// Accounts:
    ///     0. [`initializer`] - The account of the person initializing the fx swap
    ///     1. [`from_account`] `[signer]` `[writable]` Temporary token account that should be created prior to this instruction and owned by the initializer
    ///     2. [`to_account`] The receiver's token account for the funds they will receive when the swap executes
    ///     3. [`fx_account`] `[signer]` `[writable]` The fx account, it will hold all necessary info about the swap.
    ///     4. [`rent`] The rent sysvar
    ///     5. [`token`] The SPL token program
    ///     6. [`fx_feed`] The program providing the FX feed
    ///     7. [`from_liquidity_account`] `[writable]` The liquidity provider for the [`from_account`]'s token
    ///     8. [`pda_account`] Program derived address for the [`from_account`] transfer
    Initiate {
        amount: u64,
        upper_limit: [u8; 16], // Decimal
        lower_limit: [u8; 16], // Decimal
        valid_for: u64,
    },
    /// Attempt to settle the FX swap based on the initiated conditions
    /// Accounts:
    ///     0. [`initializer`] - `[writable]` The account of the person initializing the fx swap
    ///     1. [`to_account`] `[writable]` The receiver's token account for the funds they will receive when the swap executes
    ///     2. [`to_liquidity`] `[signer]` `[writable]` The liquidity provider for the [`to_account`]'s token
    ///     3. [`fx_account`] `[writable]` The fx account, it will hold all necessary info about the swap.
    ///     4. [`token`] The SPL token program
    ///     5. [`fx_feed`] The program providing the FX feed
    ///     6. [`fx_program`] The Fx-swap program
    TryExecute,
}

impl FxEvent {
    #[inline]
    pub fn from_bytes(input: &[u8]) -> Result<Self, ProgramError> {
        Self::try_from_slice(input).map_err(|err| ProgramError::BorshIoError(err.to_string()))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn initiate(
    initializer: Pubkey,
    from: Pubkey,
    to: Pubkey,
    fx_account: Pubkey,
    fx_feed: Pubkey,
    from_liquidity: Pubkey,
    amount: u64,
    upper_limit: Decimal,
    lower_limit: Decimal,
    valid_for: Option<Duration>,
) -> Instruction {
    let (pda, _) = pda_swap();
    Instruction::new_with_borsh(
        crate::id(),
        &FxEvent::Initiate {
            amount,
            upper_limit: upper_limit.serialize(),
            lower_limit: lower_limit.serialize(),
            valid_for: valid_for
                .unwrap_or_else(|| Duration::from_secs(300))
                .as_secs(),
        },
        vec![
            AccountMeta::new_readonly(initializer, false),
            AccountMeta::new(from, true),
            AccountMeta::new_readonly(to, false),
            AccountMeta::new(fx_account, true),
            AccountMeta::new_readonly(Rent::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(fx_feed, false),
            AccountMeta::new(from_liquidity, false),
            AccountMeta::new_readonly(pda, false),
        ],
    )
}

pub fn execute(
    initializer: Pubkey,
    to: Pubkey,
    to_liquidity: Pubkey,
    fx_account: Pubkey,
    fx_feed: Pubkey,
) -> Instruction {
    Instruction::new_with_borsh(
        crate::id(),
        &FxEvent::TryExecute,
        vec![
            AccountMeta::new(initializer, false),
            AccountMeta::new(to, false),
            AccountMeta::new(to_liquidity, true),
            AccountMeta::new(fx_account, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(fx_feed, false),
            AccountMeta::new_readonly(crate::id(), false),
        ],
    )
}
