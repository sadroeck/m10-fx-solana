use crate::error::FxError;
use crate::instruction::FxEvent;
use crate::liquidity::{DemoLiquidity, LiquidityProvider};
use crate::rates::{DemoFx, FxRates};
use crate::state::FxData;
use crate::utils::{pda_swap, PDA_SEED};
use rust_decimal::Decimal;
use solana_program::account_info::{next_account_info, AccountInfo};
use solana_program::clock::Clock;
use solana_program::entrypoint::ProgramResult;
use solana_program::msg;
use solana_program::program::{invoke, invoke_signed};
use solana_program::program_error::ProgramError;
use solana_program::program_pack::{IsInitialized, Pack};
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;
use spl_token::state::Account;
use std::ops::Range;
use std::time::Duration;

pub struct FxSwap;

impl FxSwap {
    pub fn process(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        match FxEvent::from_bytes(instruction_data)? {
            FxEvent::Initiate {
                amount,
                upper_limit,
                lower_limit,
                valid_for,
            } => {
                // Validate parameters
                if lower_limit > upper_limit {
                    return Err(FxError::InvalidRequest)?;
                }

                let limits = lower_limit..upper_limit;
                msg!(
                    "Initiate amount={} limit={:?} valid_until={:?}",
                    amount,
                    limits,
                    valid_for,
                );
                Self::initiate(accounts, amount, limits, Duration::from_secs(valid_for))
            }
            FxEvent::TryExecute => {
                msg!("Trying to execute");
                Self::try_execute(accounts)
            }
        }
    }

    fn initiate(
        accounts: &[AccountInfo],
        amount: u64,
        limits: Range<Decimal>,
        valid_for: Duration,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        // Extract accounts
        let initializer = next_account_info(account_info_iter)?;
        let from_account = next_account_info(account_info_iter)?;
        let to_account = next_account_info(account_info_iter)?;
        let fx_account = next_account_info(account_info_iter)?;
        let rent = next_account_info(account_info_iter)?;
        let token = next_account_info(account_info_iter)?;
        let fx_feed = next_account_info(account_info_iter)?;
        let from_liquidity_account = next_account_info(account_info_iter)?;
        let pda_account = next_account_info(account_info_iter)?;

        // Generate PDA
        let (pda, bump_seed) = pda_swap();

        // The from & to holding accounts need to be part of a swappable token
        spl_token::check_program_account(from_account.owner)
            .map_err(|_| FxError::InvalidTokenId)?;
        spl_token::check_program_account(to_account.owner).map_err(|_| FxError::InvalidTokenId)?;

        // Validate the account is rent-exempt
        let rent = &Rent::from_account_info(rent).map_err(|_| FxError::NotRentExempt)?;
        if !rent.is_exempt(fx_account.lamports(), fx_account.data_len()) {
            return Err(FxError::NotRentExempt)?;
        }

        // Check ephemeral `from` account balance
        if Account::unpack(&from_account.try_borrow_data()?)?.amount != amount {
            return Err(FxError::InvalidAmount)?;
        }

        // Retrieve the liquidity providers
        let from_liquidity =
            DemoLiquidity::liquidity_account(&Account::unpack(&from_account.try_borrow_data()?)?)
                .ok_or(FxError::NoLiquidity)?;
        if from_liquidity != *from_liquidity_account.key {
            return Err(FxError::InvalidRequest)?;
        }
        let to_liquidity =
            DemoLiquidity::liquidity_account(&Account::unpack(&to_account.try_borrow_data()?)?)
                .ok_or(FxError::NoLiquidity)?;

        // Initialize the FX data in the account
        let valid_until = Clock::get()?.unix_timestamp + valid_for.as_secs() as i64;
        let mut fx_data = FxData::unpack_unchecked(&fx_account.try_borrow_data()?)?;
        if fx_data.is_initialized() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        fx_data = FxData {
            is_initialized: true,
            initializer: *initializer.key,
            from_holding: *from_account.key,
            to_holding: *to_account.key,
            from_liquidity,
            to_liquidity,
            amount,
            limits,
            valid_until,
            fx_feed: *fx_feed.key,
        };
        FxData::pack(fx_data, &mut fx_account.try_borrow_mut_data()?)?;

        // Transfer the funds from `from` -> `liquidity`
        let transfer_funds_ix = spl_token::instruction::transfer(
            token.key,
            from_account.key,
            from_liquidity_account.key,
            &pda,
            &[&pda],
            amount,
        )?;
        invoke_signed(
            &transfer_funds_ix,
            &[
                from_account.clone(),
                from_liquidity_account.clone(),
                pda_account.clone(),
            ],
            &[&[PDA_SEED, &[bump_seed]]],
        )?;

        // Close the `from` account
        let close_account_ix = spl_token::instruction::close_account(
            token.key,
            from_account.key,
            initializer.key,
            &pda,
            &[&pda],
        )?;
        invoke_signed(
            &close_account_ix,
            &[
                from_account.clone(),
                initializer.clone(),
                pda_account.clone(),
            ],
            &[&[PDA_SEED, &[bump_seed]]],
        )?;

        Ok(())
    }

    fn try_execute(accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let initializer = next_account_info(account_info_iter)?;
        let to_account = next_account_info(account_info_iter)?;
        let to_liquidity = next_account_info(account_info_iter)?;
        let fx_account = next_account_info(account_info_iter)?;
        let token = next_account_info(account_info_iter)?;
        let fx_feed = next_account_info(account_info_iter)?;
        let fx_program = next_account_info(account_info_iter)?;

        let fx_data = FxData::unpack_unchecked(&fx_account.try_borrow_data()?)?;
        // We're trying to execute an uninitialized FX swap
        if !fx_data.is_initialized() {
            return Err(FxError::InvalidRequest)?;
        }
        // Check if we're executing the swap between the correct accounts
        if *to_account.key != fx_data.to_holding || *to_liquidity.key != fx_data.to_liquidity {
            return Err(FxError::InvalidRequest)?;
        }

        // The execute is scheduled with a different FX feed
        if fx_data.fx_feed != *fx_feed.key {
            return Err(FxError::InvalidFxFeed)?;
        }

        // Check if the initiater matches
        if fx_data.initializer != *initializer.key {
            return Err(FxError::InvalidTokenId)?;
        }

        // Fetch the current time estimate
        let now = Clock::get()?.unix_timestamp;

        // Fetch the current exchange rate
        let rate = DemoFx::rate(fx_program, fx_feed)?;

        // Calculate the swap value
        let dec = Decimal::new(fx_data.amount as i64, 0);
        let fx_amount: u64 = (dec * rate)
            .try_into()
            .map_err(|_| FxError::InvalidAmount)?;

        let in_time = fx_data.valid_until > now;
        let within_limits = fx_data.limits.contains(&rate);
        if in_time && within_limits {
            return Err(FxError::SwapConditionsNotMet)?;
        }

        // Transfer [`to_liquidity`] -> [`to_account`]
        let to_swap = spl_token::instruction::transfer(
            token.key,
            to_liquidity.key,
            to_account.key,
            to_liquidity.key,
            &[to_liquidity.key],
            fx_amount as u64,
        )?;
        invoke(
            &to_swap,
            &[
                to_liquidity.clone(),
                to_account.clone(),
                to_liquidity.clone(),
            ],
        )?;

        // Close the FX account
        **initializer.lamports.borrow_mut() = initializer
            .lamports()
            .checked_add(fx_account.lamports())
            .ok_or(FxError::InvalidAmount)?;
        **fx_account.lamports.borrow_mut() = 0;
        *fx_account.try_borrow_mut_data()? = &mut [];

        Ok(())
    }
}
