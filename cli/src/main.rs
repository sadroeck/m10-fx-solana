use clap::Parser;
use m10_fx_solana::instruction::FxEvent;
use m10_fx_solana::liquidity::{DemoLiquidity, LiquidityProvider};
use m10_fx_solana::rates::feed_for_token;
use m10_fx_solana::state::FxData;
use m10_fx_solana::utils::pda_swap;
use solana_client::rpc_client::RpcClient;
use solana_program::clock::UnixTimestamp;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::system_instruction::create_account;
use solana_program::sysvar::SysvarId;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::{read_keypair_file, Keypair};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use spl_token::state::{Account, Mint};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_RPC_URL: &str = "http://127.0.0.1:8899";

#[derive(Parser)]
#[clap(name = "command")]
#[clap(bin_name = "command")]
struct Command {
    #[clap(short, long)]
    key_path: PathBuf,
    #[clap(short, long)]
    url: Option<String>,
    #[clap(subcommand)]
    command: RPC,
}

#[derive(clap::Subcommand, Debug)]
enum RPC {
    Initiate(Initiate),
}

#[derive(clap::Args, Debug)]
#[clap(author, version, about, long_about = None)]
struct Initiate {
    #[clap(short, long, value_parser)]
    payer: PathBuf,
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
        command,
    } = Command::parse();

    let client = RpcClient::new(url.unwrap_or_else(|| DEFAULT_RPC_URL.to_string()));
    let signer = read_keypair_file(&key_path).expect("Invalid key pair");
    let program_id = m10_fx_solana::id();

    match command {
        RPC::Initiate(initiate) => {
            println!("{:?}", initiate);

            let payer = read_keypair_file(initiate.payer).expect("Could not read payer key");
            let mut instructions = vec![];

            let account = client
                .get_account(&initiate.from)
                .expect("Could not retrieve account");
            let account_data = Account::unpack(&account.data).expect("invalid account data");
            let mint_account = client
                .get_account(&account_data.mint)
                .expect("Could not find mint");
            let mint_data = Mint::unpack(&mint_account.data).expect("invalid mint account data");
            let to_account = client
                .get_account(&initiate.to)
                .expect("could not retrieve account");
            let to_account_data = Account::unpack(&to_account.data).expect("invalid account data");
            let from_liquidity =
                DemoLiquidity::liquidity_account(&account_data).expect("No liquidity provider");
            let fx_feed =
                feed_for_token(&account_data.mint, &to_account_data.mint).expect("unknown fx feed");

            // Keys
            let new_key = Keypair::new();
            let fx_key = Keypair::new();
            // Generate PDA
            let (pda, _bump_seed) = pda_swap();

            // Create an empty account
            let lamports = client
                .get_minimum_balance_for_rent_exemption(Account::LEN)
                .expect("Could not get rent-exempt balance");
            let create_account_ix = create_account(
                &payer.pubkey(),
                &new_key.pubkey(),
                lamports,
                Account::LEN as u64,
                &spl_token::id(),
            );
            instructions.push(create_account_ix);

            // Initialize SPL token holding account
            let init_account_ix = spl_token::instruction::initialize_account(
                &spl_token::id(),
                &new_key.pubkey(),
                &account_data.mint,
                &pda,
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
                &payer.pubkey(),
                &fx_key.pubkey(),
                lamports,
                FxData::LEN as u64,
                &m10_fx_solana::id(),
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
                            .checked_add(Duration::from_secs(300))
                            .unwrap()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                    }) as UnixTimestamp,
                },
                vec![
                    AccountMeta::new_readonly(initiate.from, false),
                    AccountMeta::new(new_key.pubkey(), true),
                    AccountMeta::new_readonly(initiate.to, false),
                    AccountMeta::new(fx_key.pubkey(), true),
                    AccountMeta::new_readonly(Rent::id(), false),
                    AccountMeta::new_readonly(spl_token::id(), false),
                    AccountMeta::new_readonly(fx_feed, false),
                    AccountMeta::new(from_liquidity, false),
                    AccountMeta::new_readonly(pda, false),
                ],
            );
            instructions.push(initiate_ix);

            // get a blockhash
            let recent_blockhash = client
                .get_latest_blockhash()
                .expect("error: unable to get recent blockhash");

            // Execute transactions
            let tx = Transaction::new_signed_with_payer(
                &instructions,
                Some(&payer.pubkey()),
                &[&payer, &new_key, &signer, &fx_key],
                recent_blockhash,
            );
            if let Err(err) = client.send_and_confirm_transaction_with_spinner_and_commitment(
                &tx,
                CommitmentConfig::processed(),
            ) {
                panic!("{:#?}", err);
            }
            println!(
                "Created account {} with {} funds",
                new_key.pubkey(),
                spl_token::amount_to_ui_amount(initiate.amount, mint_data.decimals)
            );
            println!("Created FX account {}", fx_key.pubkey());
        }
    }
}
