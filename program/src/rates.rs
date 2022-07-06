use crate::error::FxError;
use const_decoder::Decoder;
use rust_decimal::Decimal;
use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

pub trait FxRates {
    fn rate<'info>(
        fx_program: &AccountInfo<'info>,
        fx_feed: &AccountInfo<'info>,
    ) -> Result<Decimal, ProgramError>;
}

/// FX rates based on the ChainLink FX oracle
pub struct ChainLinkFx;

impl FxRates for ChainLinkFx {
    fn rate<'info>(
        fx_program: &AccountInfo<'info>,
        fx_feed: &AccountInfo<'info>,
    ) -> Result<Decimal, ProgramError> {
        let rate = chainlink_solana::latest_round_data(fx_program.clone(), fx_feed.clone())?.answer;
        let decimals = chainlink_solana::decimals(fx_program.clone(), fx_feed.clone())?;
        let rate = Decimal::try_from_i128_with_scale(rate, decimals as u32)
            .map_err(|_| FxError::InvalidAmount)?;
        Ok(rate)
    }
}

/// FX rates based on static amounts.
/// Intended for testing/demo-purposes.
pub struct StaticFx {}

impl StaticFx {
    fn is_demo(pubkey: &Pubkey) -> bool {
        [SAR_TO_IDR].contains(pubkey)
    }
}

// SAR <-> IDR program @ Bksm888usoczFHiw2WqWhWhQ1YNST4KoBd3s3AybEkSt
pub const SAR_TO_IDR: Pubkey = Pubkey::new_from_array(
    Decoder::Hex.decode(b"9FD23D498947B678DE43F4D143C239E64F92659CF9631638500ABA6CF21C3951"),
);

impl FxRates for StaticFx {
    fn rate<'info>(
        _fx_program: &AccountInfo<'info>,
        fx_feed: &AccountInfo<'info>,
    ) -> Result<Decimal, ProgramError> {
        match *fx_feed.key {
            SAR_TO_IDR => Ok(Decimal::new(397044, 2)), //
            _ => Err(FxError::InvalidFxFeed)?,
        }
    }
}

/// Combination of [`StaticFx`] & [`ChainLinkFx`] for demo purposes
pub struct DemoFx;

impl FxRates for DemoFx {
    fn rate<'info>(
        fx_program: &AccountInfo<'info>,
        fx_feed: &AccountInfo<'info>,
    ) -> Result<Decimal, ProgramError> {
        if StaticFx::is_demo(fx_feed.key) {
            StaticFx::rate(fx_program, fx_feed)
        } else {
            ChainLinkFx::rate(fx_program, fx_feed)
        }
    }
}
