use clap::Parser;
use m10_fx_solana::instruction::FxEvent;
use m10_fx_solana::state::FxData;
use solana_client::rpc_client::RpcClient;
use solana_program::clock::UnixTimestamp;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::message::Message;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::system_instruction::create_account;
use solana_program::sysvar::SysvarId;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use spl_token::state::Account;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_RPC_URL: &str = "localhost:8899";

#[derive(Parser)]
#[clap(name = "command")]
#[clap(bin_name = "command")]
struct Command {
    #[clap(short, long)]
    key_path: PathBuf,
    #[clap(short, long)]
    url: Option<String>,
    #[clap(short, long)]
    program_id: Option<Pubkey>,
    #[clap(subcommand)]
    command: RPC,
}

#[derive(clap::Subcommand, Debug)]
enum RPC {
    CreateTestAccount(CreateTestAccount),
    Initiate(Initiate),
}

#[derive(clap::Args, Debug)]
#[clap(author, version, about, long_about = None)]
struct CreateTestAccount {
    output: PathBuf,
}

#[derive(clap::Args, Debug)]
#[clap(author, version, about, long_about = None)]
struct Initiate {
    #[clap(short, long, value_parser)]
    from: Pubkey,
    #[clap(short, long, value_parser)]
    to: Pubkey,
    #[clap(short, long, value_parser)]
    amount: u64,
    #[clap(long, value_parser)]
    min: u64,
    #[clap(long, value_parser)]
    max: u64,
    #[clap(short, long, value_parser)]
    valid_until: Option<u64>,
}

pub fn main() {
    let Command {
        url,
        key_path,
        program_id,
        command,
    } = Command::parse();

    let client = RpcClient::new(url.unwrap_or_else(|| DEFAULT_RPC_URL.to_string()));
    let signer =
        solana_sdk::signer::keypair::read_keypair_file(key_path).expect("Invalid key pair");
    let program_id = program_id.unwrap_or_else(m10_fx_solana::id);

    match command {
        RPC::Initiate(initiate) => {
            println!("{:?}", initiate);
            let mut instructions = vec![];

            let account = client
                .get_account(&initiate.from)
                .expect("Could not retrieve account");
            let account_data = Account::unpack(&account.data).expect("invalid account data");

            // Create an empty account
            let new_key = Keypair::new();
            let lamports = client
                .get_minimum_balance_for_rent_exemption(Account::LEN)
                .expect("Could not get rent-exempt balance");
            let create_account_ix = create_account(
                &initiate.from,
                &new_key.pubkey(),
                lamports,
                Account::LEN as u64,
                &initiate.from,
            );
            instructions.push(create_account_ix);

            // Initialize SPL token holding account
            let init_account_ix = spl_token::instruction::initialize_account(
                &spl_token::id(),
                &initiate.from,
                &account_data.mint,
                &initiate.from,
            )
            .expect("could not create init account instruction");
            instructions.push(init_account_ix);

            // Transfer some tokens to holding account
            let transfer_to_holding = spl_token::instruction::transfer(
                &spl_token::id(),
                &initiate.from,
                &new_key.pubkey(),
                &initiate.from,
                &[],
                initiate.amount,
            )
            .expect("Could not create transfer instruction");
            instructions.push(transfer_to_holding);

            // Create the FX account
            let lamports = client
                .get_minimum_balance_for_rent_exemption(FxData::LEN)
                .expect("Could not get rent-exempt balance");
            let create_account_ix = create_account(
                &initiate.from,
                &new_key.pubkey(),
                lamports,
                FxData::LEN as u64,
                &initiate.from,
            );
            instructions.push(create_account_ix);

            // Invoke the Initiate command
            let initiate_ix = Instruction::new_with_borsh(
                program_id,
                &FxEvent::Initiate {
                    amount: initiate.amount,
                    upper_limit: initiate.max,
                    lower_limit: initiate.min,
                    valid_until: initiate.valid_until.unwrap_or_else(|| {
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                    }) as UnixTimestamp,
                },
                vec![
                    AccountMeta::new_readonly(initiate.from, true),
                    AccountMeta::new(new_key.pubkey(), false),
                    AccountMeta::new_readonly(initiate.to, false),
                    AccountMeta::new(program_id, false),
                    AccountMeta::new_readonly(Rent::id(), false),
                    AccountMeta::new_readonly(spl_token::id(), false),
                ],
            );
            instructions.push(initiate_ix);

            // get a blockhash
            let recent_blockhash = client
                .get_latest_blockhash()
                .expect("error: unable to get recent blockhash");

            // Execute transactions
            let tx = Transaction::new(
                &[&signer],
                Message::new(&instructions, Some(&signer.pubkey())),
                recent_blockhash,
            );
            let _signature = client
                .send_and_confirm_transaction(&tx)
                .expect("Invalid transaction");
        }
        RPC::CreateTestAccount(create_test_account) => {
            let new_key = Keypair::new();
            solana_sdk::signer::keypair::write_keypair_file(&new_key, create_test_account.output)
                .expect("TODO: panic message");
        }
    }
}
