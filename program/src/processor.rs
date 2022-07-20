use crate::error::FxError;
use crate::instruction::FxEvent;
use crate::liquidity::{DemoLiquidity, LiquidityProvider};
use crate::rates::{DemoFx, FxRates};
use crate::state::FxData;
use crate::utils::pda_swap;
use rust_decimal::Decimal;
use solana_program::account_info::{next_account_info, AccountInfo};
use solana_program::clock::{Clock, UnixTimestamp};
use solana_program::entrypoint::ProgramResult;
use solana_program::msg;
use solana_program::program::invoke_signed;
use solana_program::program_error::ProgramError;
use solana_program::program_pack::{IsInitialized, Pack};
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;
use spl_token::state::Account;
use std::ops::Range;

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
                valid_until,
            } => {
                // Validate parameters
                if lower_limit > upper_limit {
                    return Err(FxError::InvalidRequest)?;
                }
                msg!(
                    "Initiate amount={} limit=[{} , {}] valid_until={:?}",
                    amount,
                    lower_limit,
                    upper_limit,
                    valid_until,
                );
                Self::initiate(accounts, amount, lower_limit..upper_limit, valid_until)
            }
            FxEvent::TryExecute => {
                msg!("Trying to execute");
                Self::try_execute(&accounts)
            }
        }
    }

    fn initiate(
        accounts: &[AccountInfo],
        amount: u64,
        limits: Range<u64>,
        valid_until: UnixTimestamp,
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
        let mut fx_data = FxData::unpack_unchecked(&fx_account.try_borrow_data()?)?;
        if fx_data.is_initialized() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        fx_data = FxData {
            is_initialized: true,
            initializer_public_key: *initializer.key,
            from_holding: *from_account.key,
            to_holding: *to_account.key,
            from_liquidity,
            to_liquidity,
            amount,
            limits,
            valid_until,
            fx_feed_owner: fx_feed.owner.clone(),
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
            &[&[&b"m10fxswap"[..], &[bump_seed]]],
        )?;

        // Close the `from` account
        let close_account_ix = spl_token::instruction::close_account(
            &token.key,
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
            &[&[&b"m10fxswap"[..], &[bump_seed]]],
        )?;

        Ok(())
    }

    fn try_execute(accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let program_account = next_account_info(account_info_iter)?;
        let requester = next_account_info(account_info_iter)?;
        let from_account = next_account_info(account_info_iter)?;
        let to_account = next_account_info(account_info_iter)?;
        let from_liquidity = next_account_info(account_info_iter)?;
        let to_liquidity = next_account_info(account_info_iter)?;
        let fx_account = next_account_info(account_info_iter)?;
        let fx_program = next_account_info(account_info_iter)?;
        let fx_feed = next_account_info(account_info_iter)?;
        let token = next_account_info(account_info_iter)?;

        let fx_data = FxData::unpack_unchecked(&fx_account.try_borrow_data()?)?;
        // We're trying to execute an uninitialized FX swap
        if !fx_data.is_initialized() {
            return Err(FxError::InvalidRequest)?;
        }
        // Check if we're executing the swap between the correct accounts
        if *from_account.key != fx_data.from_holding
            || *to_account.key != fx_data.to_holding
            || *from_liquidity.key != fx_data.from_liquidity
            || *to_liquidity.key != fx_data.to_liquidity
        {
            return Err(FxError::InvalidRequest)?;
        }

        if Account::unpack(&from_account.try_borrow_data()?)?.mint
            != Account::unpack(&from_liquidity.try_borrow_data()?)?.mint
            || Account::unpack(&to_liquidity.try_borrow_data()?)?.mint
                != Account::unpack(&to_liquidity.try_borrow_data()?)?.mint
        {
            return Err(FxError::InvalidRequest)?;
        }

        // The execute is scheduled with a different FX feed
        if fx_data.fx_feed_owner != *fx_feed.owner {
            return Err(FxError::InvalidFxFeed)?;
        }

        // Fetch the current time estimate
        let now = Clock::get()?.unix_timestamp;

        // Fetch the current exchange rate
        let rate = DemoFx::rate(fx_program, fx_feed)?;

        // Calculate the swap value
        let fx_amount = (Decimal::new(fx_data.amount as i64, 0) * rate)
            .try_into()
            .map_err(|_| FxError::InvalidAmount)?;

        if now < fx_data.valid_until && fx_data.limits.contains(&fx_amount) {
            return Ok(());
        }

        // Transfer [`from_account`] -> [`from_liquidity`]
        let (pda, bump_seed) = pda_swap();
        let from_swap = spl_token::instruction::transfer(
            token.key,
            &fx_data.from_holding,
            &fx_data.from_liquidity,
            &pda,
            &[&pda],
            fx_data.amount,
        )?;
        invoke_signed(
            &from_swap,
            &[
                program_account.clone(),
                from_account.clone(),
                from_liquidity.clone(),
                token.clone(),
            ],
            &[&[&b"m10fxswap"[..], &[bump_seed]]],
        )?;
        // Transfer [`to_liquidity`] -> [`to_account`]
        let to_swap = spl_token::instruction::transfer(
            token.key,
            to_liquidity.key,
            to_account.key,
            &pda,
            &[&pda],
            fx_amount as u64,
        )?;
        invoke_signed(
            &to_swap,
            &[
                to_liquidity.clone(),
                to_account.clone(),
                fx_program.clone(),
                token.clone(),
            ],
            &[&[&b"m10fxswap"[..], &[bump_seed]]],
        )?;

        // Close the FX account
        let close_account = spl_token::instruction::close_account(
            token.key,
            fx_account.key,
            &fx_data.initializer_public_key,
            &pda,
            &[&pda],
        )?;
        invoke_signed(
            &close_account,
            &[
                program_account.clone(),
                requester.clone(),
                fx_account.clone(),
                token.clone(),
            ],
            &[&[&b"m10fxswap"[..], &[bump_seed]]],
        )?;

        //
        // msg!("Closing the escrow account...");
        // **initializers_main_account.lamports.borrow_mut() = initializers_main_account
        //     .lamports()
        //     .checked_add(escrow_account.lamports())
        //     .ok_or(EscrowError::AmountOverflow)?;
        // **escrow_account.lamports.borrow_mut() = 0;
        // *escrow_account.try_borrow_mut_data()? = &mut [];

        Ok(())
    }
}
