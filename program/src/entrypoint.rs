use crate::error::FxError;
use crate::processor::FxSwap;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint;
use solana_program::entrypoint::ProgramResult;
use solana_program::msg;
use solana_program::program_error::PrintProgramError;
use solana_program::pubkey::Pubkey;

#[cfg(not(feature = "no-entrypoint"))]
entrypoint!(process_instruction);
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!(
        "process_instruction: {}: {} accounts, data={:?}",
        program_id,
        accounts.len(),
        instruction_data
    );
    FxSwap::process(program_id, accounts, instruction_data).map_err(|err| {
        err.print::<FxError>();
        err
    })
}
