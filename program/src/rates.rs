use crate::error::FxError;
use crate::liquidity::*;
use const_decoder::Decoder;
use num_traits::One;
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
        [USD_TO_EUR, EUR_TO_USD].contains(pubkey)
    }
}

// USD <-> EUR program @ Bksm888usoczFHiw2WqWhWhQ1YNST4KoBd3s3AybEkSt
pub const USD_TO_EUR: Pubkey = Pubkey::new_from_array(
    Decoder::Hex.decode(b"9FD23D498947B678DE43F4D143C239E64F92659CF9631638500ABA6CF21C3951"),
);

// EUR <-> USD program @ 6CUFp2TTBpF7RARbEVhgZhz2AA9L3ZGfpUUAr4g9croe
pub const EUR_TO_USD: Pubkey = Pubkey::new_from_array(
    Decoder::Hex.decode(b"4d3aa429d67459fbfda97c6478366d8056121fdb0551a00696dbae25b0fc560d"),
);

pub fn feed_for_token(from_mint: &Pubkey, to_mint: &Pubkey) -> Option<Pubkey> {
    match (*from_mint, *to_mint) {
        (USD_MINT, EUR_MINT) => Some(USD_TO_EUR),
        (EUR_MINT, USD_MINT) => Some(EUR_TO_USD),
        (_, _) => None,
    }
}

impl FxRates for StaticFx {
    fn rate<'info>(
        _fx_program: &AccountInfo<'info>,
        fx_feed: &AccountInfo<'info>,
    ) -> Result<Decimal, ProgramError> {
        let usd_to_eur_rate = Decimal::new(9, 1);
        match *fx_feed.key {
            USD_TO_EUR => Ok(usd_to_eur_rate), //
            EUR_TO_USD => Ok(Decimal::one() / usd_to_eur_rate),
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
