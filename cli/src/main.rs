use clap::Parser;
use m10_fx_solana::liquidity::{DemoLiquidity, LiquidityProvider};
use m10_fx_solana::rates::{feed_for_token, DemoFx, FxRates};
use m10_fx_solana::state::FxData;
use m10_fx_solana::utils::pda_swap;
use rust_decimal::prelude::One;
use rust_decimal::Decimal;
use solana_client::client_error::ClientError;
use solana_client::rpc_client::RpcClient;
use solana_program::account_info::AccountInfo;
use solana_program::instruction::InstructionError;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_program::system_instruction::create_account;
use solana_sdk::account::ReadableAccount;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::{read_keypair_file, Keypair};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::{Transaction, TransactionError};
use spl_token::state::{Account, Mint};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;

const DEFAULT_RPC_URL: &str = "http://127.0.0.1:8899";
const EXECUTE_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Parser)]
#[clap(name = "command")]
#[clap(bin_name = "command")]
struct Command {
    #[clap(short, long)]
    url: Option<String>,
    #[clap(subcommand)]
    command: RPC,
}

#[derive(clap::Subcommand, Debug)]
enum RPC {
    Initiate(Initiate),
    Execute(Execute),
}

#[derive(clap::Args, Debug)]
#[clap(author, version, about, long_about = None)]
struct Initiate {
    #[clap(short, long)]
    signer: PathBuf,
    #[clap(short, long, value_parser)]
    payer: PathBuf,
    #[clap(short, long, value_parser)]
    from: Pubkey,
    #[clap(short, long, value_parser)]
    to: Pubkey,
    #[clap(short, long, value_parser)]
    amount: u64,
    #[clap(
        long,
        value_parser,
        help = "Percentage margin on the current exchange rate"
    )]
    margin: Decimal,
    #[clap(short, long, value_parser, help = "Duration in seconds")]
    valid_for: Option<u64>,
}

#[derive(clap::Args, Debug)]
#[clap(author, version, about, long_about = None)]
struct Execute {
    #[clap(short, long, value_parser)]
    fx_account: Pubkey,
    #[clap(short, long, value_parser)]
    liquidity: PathBuf,
    #[clap(short, long, value_parser)]
    payer: PathBuf,
}

pub fn main() {
    let Command { url, command } = Command::parse();

    let client = RpcClient::new(url.unwrap_or_else(|| DEFAULT_RPC_URL.to_string()));

    match command {
        RPC::Initiate(initiate) => {
            println!("{:?}", initiate);

            let signer = read_keypair_file(&initiate.signer).expect("Invalid key pair");
            let payer = read_keypair_file(&initiate.payer).expect("Could not read payer key");
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

            // Define limits
            if initiate.margin.is_sign_negative() || initiate.margin > Decimal::one() {
                panic!("Margin should be between 0.0 & 1.0: {}", initiate.margin);
            }
            let mut fake_1 = FakeAccounts::default();
            let mut fake_2 = FakeAccounts::default();
            let rate = DemoFx::rate(&fake_1.info(&from_liquidity), &fake_2.info(&fx_feed))
                .expect("Could not get current FX rate");
            println!("Current exchange rate {}", rate);
            let min = rate * (Decimal::one() - initiate.margin);
            let max = rate * (Decimal::one() + initiate.margin);
            println!(
                "Set margin to {}. Limits: [{}, {}]",
                initiate.margin, min, max
            );

            // Invoke the Initiate command
            let initiate_ix = m10_fx_solana::instruction::initiate(
                initiate.from,
                new_key.pubkey(),
                initiate.to,
                fx_key.pubkey(),
                fx_feed,
                from_liquidity,
                initiate.amount,
                max,
                min,
                initiate.valid_for.map(Duration::from_secs),
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
            if let Err(err) = client.send_and_confirm_transaction_with_spinner(&tx) {
                panic!("{:#?}", err);
            }
            println!(
                "Created account {} with {} funds",
                new_key.pubkey(),
                spl_token::amount_to_ui_amount(initiate.amount, mint_data.decimals)
            );
            println!("Created FX account {}", fx_key.pubkey());
        }
        RPC::Execute(execute) => {
            println!("{:?}", execute);
            let payer = read_keypair_file(&execute.payer).expect("Could not find payer key");
            loop {
                match try_execute(&client, &payer, &execute) {
                    Ok(_) => {
                        println!("Successfully executed FX swap");
                        return;
                    }
                    Err(err) => {
                        if let Some(TransactionError::InstructionError(
                            _,
                            InstructionError::Custom(7),
                        )) = err.get_transaction_error()
                        {
                            // Swap conditions not met
                            println!("Swap conditions not met. Sleeping {:?}", EXECUTE_INTERVAL);
                            sleep(EXECUTE_INTERVAL);
                        } else {
                            panic!("{:#?}", err);
                        }
                    }
                }
            }
        }
    }

    fn try_execute(
        client: &RpcClient,
        payer: &Keypair,
        execute: &Execute,
    ) -> Result<(), ClientError> {
        let fx_account = client
            .get_account(&execute.fx_account)
            .expect("Could not retrieve FX account");
        let fx_data = FxData::unpack(fx_account.data()).expect("invalid FX data");
        if !fx_data.is_initialized {
            panic!("Fx data has not yet been initialized");
        }

        let liquidity_key =
            read_keypair_file(&execute.liquidity).expect("Could not read liquidity key");
        if fx_data.to_liquidity != liquidity_key.pubkey() {
            panic!(
                "Mismatched liquidity provider, expected {}",
                fx_data.to_liquidity
            );
        }

        let execute_ix = m10_fx_solana::instruction::execute(
            fx_data.initializer,
            fx_data.to_holding,
            fx_data.to_liquidity,
            execute.fx_account,
            fx_data.fx_feed,
        );

        // get a blockhash
        let recent_blockhash = client
            .get_latest_blockhash()
            .expect("error: unable to get recent blockhash");

        // Execute transactions
        let tx = Transaction::new_signed_with_payer(
            &[execute_ix],
            Some(&payer.pubkey()),
            &[&liquidity_key, &payer],
            recent_blockhash,
        );
        client.send_and_confirm_transaction_with_spinner_and_commitment(
            &tx,
            CommitmentConfig::processed(),
        )?;
        Ok(())
    }
}

#[derive(Default)]
struct FakeAccounts {
    data: [u8; 0],
    lamports: u64,
}

impl FakeAccounts {
    fn info<'a>(&'a mut self, public_key: &'a Pubkey) -> AccountInfo<'a> {
        AccountInfo {
            key: &public_key,
            is_signer: false,
            is_writable: false,
            lamports: Rc::new(RefCell::new(&mut self.lamports)),
            data: Rc::new(RefCell::new(&mut self.data)),
            owner: &public_key,
            executable: false,
            rent_epoch: 0,
        }
    }
}
