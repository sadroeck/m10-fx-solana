use const_decoder::Decoder;
use solana_program::pubkey::Pubkey;
use spl_token::state::Account;

pub trait LiquidityProvider {
    fn liquidity_account(token_account: &Account) -> Option<Pubkey>;
}

/// A demo liquidity provider with pre-allocated funds for certain tokens
pub struct DemoLiquidity;

// Bksm888usoczFHiw2WqWhWhQ1YNST4KoBd3s3AybEkSt
pub const SAR_MINT: Pubkey = Pubkey::new_from_array(
    Decoder::Hex.decode(b"9fd23d498947b678de43f4d143c239e64f92659cf9631638500aba6cf21c3951"),
);
// 9kzb7SoQ6zBjVFpUyokXpkwTaSqKDcvuVWRAQopZ3Z8Q
pub const SAR_LIQUIDITY: Pubkey = Pubkey::new_from_array(
    Decoder::Hex.decode(b"822295d627a82daa76efdb02cd83f3a6f862773c47c9c01b4e1e4fc2535372dd"),
);

// GmJBGLGQYxWuPS8VMtEgAMcNEzEPT4nQSX8EcJmMuyCQ
pub const IDR_MINT: Pubkey = Pubkey::new_from_array(
    Decoder::Hex.decode(b"ea38481432a51693ad14dd810301ecc21244b33e61c0ebe6735440a4bbd93c15"),
);
// 7uRDndX5VQpHMP1XPeqpimfmSVYiSLydxQq8GYrsedKM
pub const IDR_LIQUIDITY: Pubkey = Pubkey::new_from_array(
    Decoder::Hex.decode(b"6693b5b4f0c5d3e0a46b9fe5b502fd84df6c044366f1745bfbad085f483fd000"),
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
