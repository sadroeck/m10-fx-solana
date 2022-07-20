use const_decoder::Decoder;
use solana_program::pubkey::Pubkey;
use spl_token::state::Account;

pub trait LiquidityProvider {
    fn liquidity_account(token_account: &Account) -> Option<Pubkey>;
}

/// A demo liquidity provider with pre-allocated funds for certain tokens
pub struct DemoLiquidity;

// 9TpPPxkhRr43JEoFx1CBq3PG2Z9RYPLCc2ih2bErhNB7
pub const SAR_MINT: Pubkey = Pubkey::new_from_array(
    Decoder::Hex.decode(b"7dbc2d164c1c47b4182805e85a513be63513dba6cbf1420b6b7594257c0b22ae"),
);
// 3SiXzpU2XxCLaPKMWuaNjXDMrWjqGrcYoD6HN9A633W9
pub const SAR_LIQUIDITY: Pubkey = Pubkey::new_from_array(
    Decoder::Hex.decode(b"244ddb71fa469b36a86698c396d043f78f22e5446ed00df525eed4c19c4669a2"),
);

// EfBRenoHB4hZYSxpDPvWzavSDaoFLTUHi1johJSAg5LU
pub const IDR_MINT: Pubkey = Pubkey::new_from_array(
    Decoder::Hex.decode(b"caefc9d4c14b87cc74a2b3797e004238b7739ba908678053f091ce53f8750f81"),
);
// 4xF16x82rckft4oiQob6wob8KTeVtVsMusU4wVnQsXQ
pub const IDR_LIQUIDITY: Pubkey = Pubkey::new_from_array(
    Decoder::Hex.decode(b"0103343f2399232dc0bb9c2f4cb8a036d3e07bcb1f2b5899c6cf0a6ed1f90ad3"),
);

impl LiquidityProvider for DemoLiquidity {
    fn liquidity_account(token_account: &Account) -> Option<Pubkey> {
        match token_account.mint {
            IDR_MINT => Some(IDR_LIQUIDITY),
            SAR_MINT => Some(SAR_LIQUIDITY),
            _ => None,
        }
    }
}
