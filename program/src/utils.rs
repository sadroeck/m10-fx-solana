use solana_program::pubkey::Pubkey;

#[inline]
pub fn pda_swap() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[PDA_SEED], &crate::id())
}

pub const PDA_SEED: &[u8] = b"m10fxswap";
