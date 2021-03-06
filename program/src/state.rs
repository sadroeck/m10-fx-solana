use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use rust_decimal::Decimal;
use solana_program::clock::UnixTimestamp;
use solana_program::program_error::ProgramError;
use solana_program::program_pack::{IsInitialized, Pack, Sealed};
use solana_program::pubkey::Pubkey;
use std::mem::size_of;
use std::ops::Range;

#[derive(Debug)]
pub struct FxData {
    pub is_initialized: bool,
    // Contract initiator
    pub initializer: Pubkey,
    // Sender & Receiver
    pub from_holding: Pubkey,
    pub to_holding: Pubkey,

    // Liquidity accounts
    pub from_liquidity: Pubkey,
    pub to_liquidity: Pubkey,

    // Swap parameters
    pub amount: u64,
    pub limits: Range<Decimal>,
    pub valid_until: UnixTimestamp,

    // FX feed
    pub fx_feed: Pubkey,
}

impl Sealed for FxData {}

impl Pack for FxData {
    const LEN: usize = size_of::<bool>()
        + 5 * size_of::<Pubkey>()
        + size_of::<u64>()
        + 2 * size_of::<Decimal>()
        + size_of::<UnixTimestamp>()
        + size_of::<Pubkey>();

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, FxData::LEN];
        let (
            is_initialized,
            initializer_public_key,
            from_holding_account_public_key,
            to_holding_account_public_key,
            from_liquidity,
            to_liquidity,
            amount,
            upper_limit,
            lower_limit,
            valid_until,
            fx_feed,
        ) = mut_array_refs![
            dst,
            size_of::<bool>(),
            size_of::<Pubkey>(),
            size_of::<Pubkey>(),
            size_of::<Pubkey>(),
            size_of::<Pubkey>(),
            size_of::<Pubkey>(),
            size_of::<u64>(),
            size_of::<Decimal>(),
            size_of::<Decimal>(),
            size_of::<UnixTimestamp>(),
            size_of::<Pubkey>()
        ];

        is_initialized[0] = self.is_initialized as u8;
        initializer_public_key.copy_from_slice(self.initializer.as_ref());
        from_holding_account_public_key.copy_from_slice(self.from_holding.as_ref());
        to_holding_account_public_key.copy_from_slice(self.to_holding.as_ref());
        from_liquidity.copy_from_slice(self.from_liquidity.as_ref());
        to_liquidity.copy_from_slice(self.to_liquidity.as_ref());
        *amount = self.amount.to_be_bytes();
        upper_limit.copy_from_slice(&self.limits.end.serialize());
        lower_limit.copy_from_slice(&self.limits.start.serialize());
        *valid_until = self.valid_until.to_be_bytes();
        fx_feed.copy_from_slice(self.fx_feed.as_ref());
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, FxData::LEN];
        let (
            is_initialized,
            initializer,
            from_holding_account_public_key,
            to_holding_account_public_key,
            from_liquidity,
            to_liquidity,
            amount,
            upper_limit,
            lower_limit,
            valid_until,
            fx_feed_owner,
        ) = array_refs![
            src,
            size_of::<bool>(),
            size_of::<Pubkey>(),
            size_of::<Pubkey>(),
            size_of::<Pubkey>(),
            size_of::<Pubkey>(),
            size_of::<Pubkey>(),
            size_of::<u64>(),
            size_of::<Decimal>(),
            size_of::<Decimal>(),
            size_of::<UnixTimestamp>(),
            size_of::<Pubkey>()
        ];
        let is_initialized = match is_initialized {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        Ok(Self {
            is_initialized,
            initializer: Pubkey::from(*initializer),
            from_holding: Pubkey::from(*from_holding_account_public_key),
            to_holding: Pubkey::from(*to_holding_account_public_key),
            from_liquidity: Pubkey::from(*from_liquidity),
            to_liquidity: Pubkey::from(*to_liquidity),
            amount: u64::from_be_bytes(*amount),
            limits: Decimal::deserialize(*lower_limit)..Decimal::deserialize(*upper_limit),
            valid_until: UnixTimestamp::from_be_bytes(*valid_until),
            fx_feed: Pubkey::from(*fx_feed_owner),
        })
    }
}

impl IsInitialized for FxData {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}
