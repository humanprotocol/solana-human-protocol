use chrono::prelude::*;
use clap::{
    crate_description, crate_name, crate_version, value_t, value_t_or_exit, App, AppSettings, Arg,
    SubCommand,
};
use hmt_escrow::state::DataHash;
use hmt_escrow::state::DataUrl;
use hmt_escrow::{
    self, 
    instruction::{
        initialize as initialize_escrow, payout, setup as setup_escrow, store_results,
        cancel as cancel_escrow, complete as complete_escrow,
    },
    processor::Processor as EscrowProcessor, state::Escrow,
};
use solana_clap_utils::{
    input_parsers::{pubkey_of, value_of},
    input_validators::{is_amount, is_keypair, is_parsable, is_pubkey, is_url},
    keypair::signer_from_path,
};
use solana_client::rpc_client::RpcClient;
use solana_program::{
    instruction::Instruction, program_option::COption, program_pack::Pack, pubkey::Pubkey,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    native_token::*,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_token::{
    self, instruction::initialize_account, state::Account as TokenAccount, state::Mint as TokenMint,
};
use std::fs::File;
use std::io::BufReader;
use std::{fmt::Display, process::exit, str, str::FromStr};

struct Config {
    rpc_client: RpcClient,
    verbose: bool,
    owner: Box<dyn Signer>,
    fee_payer: Box<dyn Signer>,
    commitment_config: CommitmentConfig,
}

type Error = Box<dyn std::error::Error>;
type CommandResult = Result<Option<Transaction>, Error>;

macro_rules! unique_signers {
    ($vec:ident) => {
        $vec.sort_by_key(|l| l.pubkey());
        $vec.dedup();
    };
}

fn check_fee_payer_balance(config: &Config, required_balance: u64) -> Result<(), Error> {
    let balance = config.rpc_client.get_balance(&config.fee_payer.pubkey())?;
    if balance < required_balance {
        Err(format!(
            "Fee payer, {}, has insufficient balance: {} required, {} available",
            config.fee_payer.pubkey(),
            lamports_to_sol(required_balance),
            lamports_to_sol(balance)
        )
        .into())
    } else {
        Ok(())
    }
}

fn command_create(
    config: &Config,
    mint: &Pubkey,
    launcher: &Option<Pubkey>,
    canceler: &Option<Pubkey>,
    canceler_token: &Option<Pubkey>,
    duration: u64,
) -> CommandResult {
    let escrow_token_account = Keypair::new();
    println!(
        "Creating escrow token account {}",
        escrow_token_account.pubkey()
    );

    let escrow_account = Keypair::new();

    let token_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)?;
    let escrow_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Escrow::LEN)?;
    let mut total_rent_free_balances = token_account_balance + escrow_account_balance;

    // Calculate withdraw authority used for minting pool tokens
    let (authority, _) =
        EscrowProcessor::find_authority_bump_seed(&hmt_escrow::id(), &escrow_account.pubkey());

    if config.verbose {
        println!("Escrow authority {}", authority);
    }

    let mut instructions: Vec<Instruction> = vec![
        // Account for the escrow tokens
        system_instruction::create_account(
            &config.fee_payer.pubkey(),
            &escrow_token_account.pubkey(),
            token_account_balance,
            TokenAccount::LEN as u64,
            &spl_token::id(),
        ),
        // Account for the escrow
        system_instruction::create_account(
            &config.fee_payer.pubkey(),
            &escrow_account.pubkey(),
            escrow_account_balance,
            Escrow::LEN as u64,
            &hmt_escrow::id(),
        ),
        // Initialize escrow token account
        initialize_account(
            &spl_token::id(),
            &escrow_token_account.pubkey(),
            mint,
            &authority,
        )?,
    ];

    let mut signers = vec![
        config.fee_payer.as_ref(),
        &escrow_token_account,
        &escrow_account,
    ];

    // Unwrap optionals
    let launcher: Pubkey = launcher.unwrap_or(config.owner.pubkey());
    let canceler: Pubkey = canceler.unwrap_or(config.owner.pubkey());

    let canceler_token_account = Keypair::new();
    let canceler_token: Pubkey = match canceler_token {
        Some(value) => *value,
        None => {
            println!(
                "Creating canceler token account {}",
                canceler_token_account.pubkey()
            );

            instructions.extend(vec![
                // Account for the canceler tokens
                system_instruction::create_account(
                    &config.fee_payer.pubkey(),
                    &canceler_token_account.pubkey(),
                    token_account_balance,
                    TokenAccount::LEN as u64,
                    &spl_token::id(),
                ),
                // Initialize canceler token account
                initialize_account(
                    &spl_token::id(),
                    &canceler_token_account.pubkey(),
                    mint,
                    &canceler,
                )?,
            ]);

            signers.push(&canceler_token_account);

            total_rent_free_balances += token_account_balance;

            canceler_token_account.pubkey()
        }
    };

    println!("Creating escrow {}", escrow_account.pubkey());
    instructions.extend(vec![
        // Initialize escrow account
        initialize_escrow(
            &hmt_escrow::id(),
            &escrow_account.pubkey(),
            mint,
            &escrow_token_account.pubkey(),
            &launcher,
            &canceler,
            &canceler_token,
            duration,
        )?,
    ]);

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        total_rent_free_balances + fee_calculator.calculate_fee(&transaction.message()),
    )?;
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn format_coption_key<'a>(optional: &'a COption<Pubkey>) -> Box<dyn std::fmt::Display + 'a> {
    match optional {
        COption::Some(key) => Box::new(key),
        COption::None => Box::new("None"),
    }
}

fn command_info(config: &Config, escrow: &Pubkey) -> CommandResult {
    let account_data = config.rpc_client.get_account_data(escrow)?;
    let escrow: Escrow = Escrow::unpack_from_slice(account_data.as_slice())?;

    // Check token mint to convert amount to float
    let account_data = config
        .rpc_client
        .get_account_data(&escrow.token_mint)
        .or(Err("Cannot read escrow mint data"))?;
    let mint_info: TokenMint = TokenMint::unpack_from_slice(account_data.as_slice())
        .map_err(|_| format!("{} is not a valid mint address", escrow.token_mint))?;

    println!("Escrow information");
    println!("==================");
    println!("State: {:?}", escrow.state);
    println!(
        "Expires: {}",
        NaiveDateTime::from_timestamp(escrow.expires, 0)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    );
    println!("Token mint: {}", escrow.token_mint);
    println!("Token account: {}", escrow.token_account);
    println!("Launcher: {}", escrow.launcher);
    println!("Canceler: {}", escrow.canceler);
    println!("Canceler token account: {}", escrow.canceler_token_account);
    println!();
    println!("Reputation oracle");
    println!("=================");
    println!("Account: {}", format_coption_key(&escrow.reputation_oracle));
    println!(
        "Token account: {}",
        format_coption_key(&escrow.reputation_oracle_token_account)
    );
    println!("Fee: {}%", escrow.reputation_oracle_stake);
    println!();
    println!("Recording oracle");
    println!("================");
    println!("Account: {}", format_coption_key(&escrow.recording_oracle));
    println!(
        "Token account: {}",
        format_coption_key(&escrow.recording_oracle_token_account)
    );
    println!("Fee: {}%", escrow.recording_oracle_stake);
    println!();
    println!("Data");
    println!("====");
    println!(
        "Job manifest URL: {}",
        str::from_utf8(escrow.manifest_url.as_ref()).unwrap_or("")
    );
    println!(
        "Job manifest hash: {}",
        hex::encode(escrow.manifest_hash.as_ref())
    );
    println!(
        "Final results URL: {}",
        str::from_utf8(escrow.final_results_url.as_ref()).unwrap_or("")
    );
    println!(
        "Final results hash: {}",
        hex::encode(escrow.final_results_hash.as_ref())
    );
    println!();
    println!("Amounts and recipients");
    println!("======================");
    println!(
        "Amount: {} ({} sent)",
        spl_token::amount_to_ui_amount(escrow.total_amount, mint_info.decimals),
        spl_token::amount_to_ui_amount(escrow.sent_amount, mint_info.decimals),
    );
    println!(
        "Recipients: {} ({} sent)",
        escrow.total_recipients, escrow.sent_recipients,
    );

    Ok(None)
}

/// Issues setup command
#[allow(clippy::too_many_arguments)]
fn command_setup(
    config: &Config,
    escrow: &Pubkey,
    reputation_oracle: &Option<Pubkey>,
    reputation_oracle_token: &Option<Pubkey>,
    reputation_oracle_stake: u8,
    recording_oracle: &Option<Pubkey>,
    recording_oracle_token: &Option<Pubkey>,
    recording_oracle_stake: u8,
    manifest_url: &str,
    manifest_hash: &Option<String>,
) -> CommandResult {
    // Validate parameters
    if reputation_oracle_stake > 100
        || recording_oracle_stake > 100
        || reputation_oracle_stake.saturating_add(recording_oracle_stake) > 100
    {
        return Err("Invalid stake values".into());
    }

    let manifest_url: DataUrl = DataUrl::from_str(manifest_url).or(Err("URL too long"))?;
    let manifest_hash: DataHash = match manifest_hash {
        None => Default::default(),
        Some(value) => {
            let bytes = hex::decode(value).or(Err("Hash decoding error"))?;
            DataHash::new_from_slice(&bytes).or(Err("Wrong hash size"))?
        }
    };

    let mut instructions: Vec<Instruction> = vec![];
    let token_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)?;
    let mut total_rent_free_balances = 0;

    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];

    // Read escrow state
    let account_data = config.rpc_client.get_account_data(escrow)?;
    let escrow_info: Escrow = Escrow::unpack_from_slice(account_data.as_slice())?;

    // Unwrap optionals
    let reputation_oracle: Pubkey = reputation_oracle.unwrap_or(config.owner.pubkey());
    let recording_oracle: Pubkey = recording_oracle.unwrap_or(config.owner.pubkey());
    let reputation_oracle_token_account = Keypair::new();
    let reputation_oracle_token: Pubkey = match reputation_oracle_token {
        Some(value) => *value,
        None => {
            println!(
                "Creating reputation oracle token account {}",
                reputation_oracle_token_account.pubkey()
            );

            instructions.extend(vec![
                // Account for the reputation oracle tokens
                system_instruction::create_account(
                    &config.fee_payer.pubkey(),
                    &reputation_oracle_token_account.pubkey(),
                    token_account_balance,
                    TokenAccount::LEN as u64,
                    &spl_token::id(),
                ),
                // Initialize reputation oracle token account
                initialize_account(
                    &spl_token::id(),
                    &reputation_oracle_token_account.pubkey(),
                    &escrow_info.token_mint,
                    &reputation_oracle,
                )?,
            ]);

            signers.push(&reputation_oracle_token_account);

            total_rent_free_balances += token_account_balance;

            reputation_oracle_token_account.pubkey()
        }
    };
    let recording_oracle_token_account = Keypair::new();
    let recording_oracle_token: Pubkey = match recording_oracle_token {
        Some(value) => *value,
        None => {
            println!(
                "Creating recording oracle token account {}",
                recording_oracle_token_account.pubkey()
            );

            instructions.extend(vec![
                // Account for the reputation oracle tokens
                system_instruction::create_account(
                    &config.fee_payer.pubkey(),
                    &recording_oracle_token_account.pubkey(),
                    token_account_balance,
                    TokenAccount::LEN as u64,
                    &spl_token::id(),
                ),
                // Initialize reputation oracle token account
                initialize_account(
                    &spl_token::id(),
                    &recording_oracle_token_account.pubkey(),
                    &escrow_info.token_mint,
                    &recording_oracle,
                )?,
            ]);

            signers.push(&recording_oracle_token_account);

            total_rent_free_balances += token_account_balance;

            recording_oracle_token_account.pubkey()
        }
    };

    instructions.extend(vec![
        // Add escrow setup instruction
        setup_escrow(
            &hmt_escrow::id(),
            &escrow,
            &config.owner.pubkey(),
            &reputation_oracle,
            &reputation_oracle_token,
            reputation_oracle_stake,
            &recording_oracle,
            &recording_oracle_token,
            recording_oracle_stake,
            &manifest_url,
            &manifest_hash,
        )?,
    ]);

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        total_rent_free_balances + fee_calculator.calculate_fee(&transaction.message()),
    )?;
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

/// Issues store results command
fn command_store_results(
    config: &Config,
    escrow: &Pubkey,
    amount: f64,
    recipients: u64,
    results_url: &str,
    results_hash: &Option<String>,
) -> CommandResult {
    // Validate parameters
    let results_url: DataUrl = DataUrl::from_str(results_url).or(Err("URL too long"))?;
    let results_hash: DataHash = match results_hash {
        None => Default::default(),
        Some(value) => {
            let bytes = hex::decode(value).or(Err("Hash decoding error"))?;
            DataHash::new_from_slice(&bytes).or(Err("Wrong hash size"))?
        }
    };

    // Read escrow state
    let account_data = config
        .rpc_client
        .get_account_data(escrow)
        .or(Err("Cannot read escrow data"))?;
    let escrow_info: Escrow = Escrow::unpack_from_slice(account_data.as_slice())
        .map_err(|_| format!("{} is not a valid escrow address", escrow))?;

    // Check token mint to convert amount to u64
    let account_data = config
        .rpc_client
        .get_account_data(&escrow_info.token_mint)
        .or(Err("Cannot read escrow mint data"))?;
    let mint_info: TokenMint = TokenMint::unpack_from_slice(account_data.as_slice())
        .map_err(|_| format!("{} is not a valid mint address", escrow_info.token_mint))?;

    let amount = spl_token::ui_amount_to_amount(amount, mint_info.decimals);

    let mut transaction = Transaction::new_with_payer(
        &[
            // Store results instruction
            store_results(
                &hmt_escrow::id(),
                &escrow,
                &config.owner.pubkey(),
                amount,
                recipients,
                &results_url,
                &results_hash,
            )?,
        ],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

#[derive(Debug)]
struct PayoutRecord {
    recipient: Pubkey,
    amount: f64,
}

/// Creates transaction for payout from the escrow account
fn command_payout(config: &Config, escrow: &Pubkey, file_name: &str) -> CommandResult {
    // Read CSV file and validate its contents
    let file = File::open(file_name).map_err(|_| format!("Cannot find file {}", file_name))?;
    let file_reader = BufReader::new(file);
    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(file_reader);

    let recipients: Vec<PayoutRecord> = csv_reader
        .records()
        .filter_map(|record| {
            record.ok().and_then(|record| {
                let recipient: Option<Pubkey> =
                    Pubkey::from_str(record.get(0).unwrap_or_default()).ok();
                let amount: Option<f64> = record.get(1).unwrap_or_default().parse::<f64>().ok();
                match (recipient, amount) {
                    (Some(recipient), Some(amount)) => Some(PayoutRecord { recipient, amount }),
                    _ => None
                }
            })
        })
        .collect();
    if recipients.is_empty() {
        return Err("Cannot find anyone to sent tokens to".into());
    }
    let total_amount: f64 = recipients.iter().map(|x| x.amount).sum();

    // Read escrow state
    let account_data = config
        .rpc_client
        .get_account_data(escrow)
        .or(Err("Cannot read escrow data"))?;
    let escrow_info: Escrow = Escrow::unpack_from_slice(account_data.as_slice())
        .map_err(|_| format!("{} is not a valid escrow address", escrow))?;

    // Check oracle accounts
    let reputation_oracle_token_account = escrow_info
        .reputation_oracle_token_account
        .ok_or::<Error>("Reputation oracle token account not defined".into())?;
    let recording_oracle_token_account = escrow_info
        .recording_oracle_token_account
        .ok_or::<Error>("Recording oracle token account not defined".into())?;

    // Check token mint to convert amount to u64
    let account_data = config
        .rpc_client
        .get_account_data(&escrow_info.token_mint)
        .or(Err("Cannot read escrow mint data"))?;
    let mint_info: TokenMint = TokenMint::unpack_from_slice(account_data.as_slice())
        .map_err(|_| format!("{} is not a valid mint address", escrow_info.token_mint))?;
    let total_amount = spl_token::ui_amount_to_amount(total_amount, mint_info.decimals);

    // Check escrow token account balance
    let account_data = config
        .rpc_client
        .get_account_data(&escrow_info.token_account)
        .or(Err("Cannot read escrow token account data"))?;
    let token_account_info: TokenAccount = TokenAccount::unpack_from_slice(account_data.as_slice())
        .map_err(|_| {
            format!(
                "{} is not a valid token account address",
                escrow_info.token_account
            )
        })?;

    if total_amount > token_account_info.amount {
        return Err(format!(
            "{} tokens needed on escrow account, only {} found",
            spl_token::amount_to_ui_amount(total_amount, mint_info.decimals),
            spl_token::amount_to_ui_amount(token_account_info.amount, mint_info.decimals)
        )
        .into());
    }

    let authority =
        EscrowProcessor::authority_id(&hmt_escrow::id(), &escrow, escrow_info.bump_seed)?;
    let mut instructions_ui_amount: f64 = 0.0;
    let instructions: Vec<Instruction> = recipients
        .iter()
        .filter_map(|record| {
            let instruction = payout(
                &hmt_escrow::id(),
                &escrow,
                &config.owner.pubkey(),
                &escrow_info.token_account,
                &authority,
                &record.recipient,
                &reputation_oracle_token_account,
                &recording_oracle_token_account,
                &spl_token::id(),
                spl_token::ui_amount_to_amount(record.amount, mint_info.decimals),
            )
            .ok();

            if instruction != None {
                println!("{}: {}", record.recipient, record.amount);
                instructions_ui_amount += record.amount;
            }

            instruction
        })
        .collect();

    if !instructions.is_empty() {
        let total_fees = escrow_info.reputation_oracle_stake + escrow_info.recording_oracle_stake;
        if total_fees != 0 {
            println!("Sending {} to {} recipients", instructions_ui_amount, instructions.len());
            println!("{}% ({}) will be used to pay oracle fees", total_fees, total_fees as f64 * instructions_ui_amount / 100.0);
        }

        let mut transaction =
            Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

        let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
        check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
        let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
        unique_signers!(signers);
        transaction.sign(&signers, recent_blockhash);
        Ok(Some(transaction))
    } else {
        Err("No payouts created".into())
    }
}

fn command_cancel(config: &Config, escrow: &Pubkey) -> CommandResult {
    let account_data = config.rpc_client.get_account_data(escrow)?;
    let escrow_info: Escrow = Escrow::unpack_from_slice(account_data.as_slice())?;

    let authority =
        EscrowProcessor::authority_id(&hmt_escrow::id(), &escrow, escrow_info.bump_seed)?;

    let mut transaction = Transaction::new_with_payer(
        &[
            cancel_escrow(
                &hmt_escrow::id(),
                &escrow,
                &config.owner.pubkey(),
                &escrow_info.token_account,
                &authority,
                &escrow_info.canceler_token_account,
                &spl_token::id(),
            )?,
        ],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_complete(config: &Config, escrow: &Pubkey) -> CommandResult {
    let mut transaction = Transaction::new_with_payer(
        &[
            complete_escrow(
                &hmt_escrow::id(),
                &escrow,
                &config.owner.pubkey(),
            )?,
        ],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

/// Return an error if a hex cannot be parsed.
pub fn is_hex<T>(string: T) -> Result<(), String>
where
    T: AsRef<str> + Display,
{
    match hex::decode(string.as_ref()) {
        Ok(_) => Ok(()),
        Err(err) => Err(format!("{}", err)),
    }
}

fn main() {
    let matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg({
            let arg = Arg::with_name("config_file")
                .short("C")
                .long("config")
                .value_name("PATH")
                .takes_value(true)
                .global(true)
                .help("Configuration file to use");
            if let Some(ref config_file) = *solana_cli_config::CONFIG_FILE {
                arg.default_value(&config_file)
            } else {
                arg
            }
        })
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .takes_value(false)
                .global(true)
                .help("Show additional information"),
        )
        .arg(
            Arg::with_name("json_rpc_url")
                .long("url")
                .value_name("URL")
                .takes_value(true)
                .validator(is_url)
                .help("JSON RPC URL for the cluster.  Default from the configuration file."),
        )
        .arg(
            Arg::with_name("owner")
                .long("owner")
                .value_name("KEYPAIR")
                .validator(is_keypair)
                .takes_value(true)
                .help(
                    "Specify the stake pool or stake account owner. \
                     This may be a keypair file, the ASK keyword. \
                     Defaults to the client keypair.",
                ),
        )
        .arg(
            Arg::with_name("fee_payer")
                .long("fee-payer")
                .value_name("KEYPAIR")
                .validator(is_keypair)
                .takes_value(true)
                .help(
                    "Specify the fee-payer account. \
                     This may be a keypair file, the ASK keyword. \
                     Defaults to the client keypair.",
                ),
        )
        .subcommand(SubCommand::with_name("create").about("Create a new escrow")
            .arg(
                Arg::with_name("mint")
                    .long("mint")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Mint address for the token managed by this escrow"),
            )
            .arg(
                Arg::with_name("launcher")
                    .long("launcher")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Account which can manage the escrow [default: --owner]"),
            )
            .arg(
                Arg::with_name("canceler")
                    .long("canceler")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Account which is able to cancel this escrow [default: --owner]"),
            )
            .arg(
                Arg::with_name("canceler_token")
                    .long("canceler-receiver")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Token account which can receive tokens specified by the --mint parameter [default: new token account owned by the --canceler]"),
            )
            .arg(
                Arg::with_name("duration")
                    .long("duration")
                    .short("d")
                    .validator(is_parsable::<u64>)
                    .value_name("SECONDS")
                    .takes_value(true)
                    .required(true)
                    .help("Escrow duration in seconds, once this time passes escrow contract is no longer operational"),
            )
        )
        .subcommand(SubCommand::with_name("info").about("Shows information about the escrow account")
            .arg(
                Arg::with_name("escrow")
                    .validator(is_pubkey)
                    .index(1)
                    .value_name("ESCROW_ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Escrow address"),
            )
        )
        .subcommand(SubCommand::with_name("setup").about("Configures and launches escrow")
            .arg(
                Arg::with_name("escrow")
                    .validator(is_pubkey)
                    .index(1)
                    .value_name("ESCROW_ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Escrow address"),
            )
            .arg(
                Arg::with_name("reputation_oracle")
                    .long("reputation-oracle")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Escrow reputation oracle address [default: --owner]"),
            )
            .arg(
                Arg::with_name("reputation_oracle_token")
                    .long("reputation-oracle-token")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Reputation oracle token address [default: new token account owned by the --reputation-oracle]"),
            )
            .arg(
                Arg::with_name("reputation_oracle_stake")
                    .long("reputation-oracle-stake")
                    .validator(is_parsable::<u8>)
                    .value_name("PERCENT")
                    .takes_value(true)
                    .required(true)
                    .help("Reputation oracle fee in payouts, from 0 to 100 percent"),
            )
            .arg(
                Arg::with_name("recording_oracle")
                    .long("recording-oracle")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Escrow recording oracle address [default: --owner]"),
            )
            .arg(
                Arg::with_name("recording_oracle_token")
                    .long("recording-oracle-token")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Recording oracle token address [default: new token account owned by the --recording-oracle]"),
            )
            .arg(
                Arg::with_name("recording_oracle_stake")
                    .long("recording-oracle-stake")
                    .validator(is_parsable::<u8>)
                    .value_name("PERCENT")
                    .takes_value(true)
                    .required(true)
                    .help("Recording oracle fee in payouts, from 0 to 100 percent"),
            )
            .arg(
                Arg::with_name("manifest_url")
                    .long("manifest-url")
                    .validator(is_url)
                    .value_name("URL")
                    .takes_value(true)
                    .help("Job manifest URL [default: empty string]"),
            )
            .arg(
                Arg::with_name("manifest_hash")
                    .long("manifest-hash")
                    .validator(is_hex)
                    .value_name("HEX")
                    .takes_value(true)
                    .help("20-byte manifest SHA1 hash in hex format [default: 0-byte hash]"),
            )
        )
        .subcommand(SubCommand::with_name("store-results").about("Stores results in the escrow")
            .arg(
                Arg::with_name("escrow")
                    .validator(is_pubkey)
                    .index(1)
                    .value_name("ESCROW_ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Escrow address"),
            )
            .arg(
                Arg::with_name("amount")
                    .long("amount")
                    .validator(is_amount)
                    .value_name("AMOUNT")
                    .takes_value(true)
                    .required(true)
                    .help("Total amount to be sent out from the escrow."),
            )
            .arg(
                Arg::with_name("recipients")
                    .long("recipients")
                    .validator(is_parsable::<u64>)
                    .value_name("COUNT")
                    .takes_value(true)
                    .required(true)
                    .help("Number of recipients to receive tokens from the escrow."),
            )
            .arg(
                Arg::with_name("results_url")
                    .long("results-url")
                    .validator(is_url)
                    .value_name("URL")
                    .takes_value(true)
                    .help("Final results URL [default: empty string]"),
            )
            .arg(
                Arg::with_name("results_hash")
                    .long("results-hash")
                    .validator(is_hex)
                    .value_name("HEX")
                    .takes_value(true)
                    .help("20-byte results SHA1 hash in hex format [default: 0-byte hash]"),
            )
        )
        .subcommand(SubCommand::with_name("payout").about("Pays tokens from the escrow account")
            .arg(
                Arg::with_name("escrow")
                    .validator(is_pubkey)
                    .index(1)
                    .value_name("ESCROW_ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Escrow address"),
            )
            .arg(
                Arg::with_name("file_name")
                    .validator(is_parsable::<String>)
                    .index(2)
                    .value_name("FILE")
                    .takes_value(true)
                    .required(true)
                    .help("CSV file with recipients and amounts, <address>,<amount> on each line"),
            )
        )
        .subcommand(SubCommand::with_name("cancel").about("Cancels escrow, all remaining funds are returned to the canceler's token account")
            .arg(
                Arg::with_name("escrow")
                    .validator(is_pubkey)
                    .index(1)
                    .value_name("ESCROW_ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Escrow address"),
            )
        )
        .subcommand(SubCommand::with_name("complete").about("Completes escrow")
            .arg(
                Arg::with_name("escrow")
                    .validator(is_pubkey)
                    .index(1)
                    .value_name("ESCROW_ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Escrow address"),
            )
        )
        .get_matches();

    let mut wallet_manager = None;
    let config = {
        let cli_config = if let Some(config_file) = matches.value_of("config_file") {
            solana_cli_config::Config::load(config_file).unwrap_or_default()
        } else {
            solana_cli_config::Config::default()
        };
        let json_rpc_url = value_t!(matches, "json_rpc_url", String)
            .unwrap_or_else(|_| cli_config.json_rpc_url.clone());

        let owner = signer_from_path(
            &matches,
            &cli_config.keypair_path,
            "owner",
            &mut wallet_manager,
        )
        .unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });
        let fee_payer = signer_from_path(
            &matches,
            &cli_config.keypair_path,
            "fee_payer",
            &mut wallet_manager,
        )
        .unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });
        let verbose = matches.is_present("verbose");

        Config {
            rpc_client: RpcClient::new(json_rpc_url),
            verbose,
            owner,
            fee_payer,
            commitment_config: CommitmentConfig::single(),
        }
    };

    solana_logger::setup_with_default("solana=info");

    let _ = match matches.subcommand() {
        ("create", Some(arg_matches)) => {
            let mint: Pubkey = pubkey_of(arg_matches, "mint").unwrap();
            let launcher: Option<Pubkey> = pubkey_of(arg_matches, "launcher");
            let canceler: Option<Pubkey> = pubkey_of(arg_matches, "canceler");
            let canceler_token: Option<Pubkey> = pubkey_of(arg_matches, "canceler_token");
            let duration = value_t_or_exit!(arg_matches, "duration", u64);
            command_create(
                &config,
                &mint,
                &launcher,
                &canceler,
                &canceler_token,
                duration,
            )
        }
        ("info", Some(arg_matches)) => {
            let escrow: Pubkey = pubkey_of(arg_matches, "escrow").unwrap();
            command_info(&config, &escrow)
        }
        ("setup", Some(arg_matches)) => {
            let escrow: Pubkey = pubkey_of(arg_matches, "escrow").unwrap();
            let reputation_oracle: Option<Pubkey> = pubkey_of(arg_matches, "reputation_oracle");
            let reputation_oracle_token: Option<Pubkey> =
                pubkey_of(arg_matches, "reputation_oracle_token");
            let reputation_oracle_stake =
                value_t_or_exit!(arg_matches, "reputation_oracle_stake", u8);
            let recording_oracle: Option<Pubkey> = pubkey_of(arg_matches, "recording_oracle");
            let recording_oracle_token: Option<Pubkey> =
                pubkey_of(arg_matches, "recording_oracle_token");
            let recording_oracle_stake =
                value_t_or_exit!(arg_matches, "recording_oracle_stake", u8);
            let manifest_url: String = value_of(arg_matches, "manifest_url").unwrap_or_default();
            let manifest_hash: Option<String> = value_of(arg_matches, "manifest_hash");
            command_setup(
                &config,
                &escrow,
                &reputation_oracle,
                &reputation_oracle_token,
                reputation_oracle_stake,
                &recording_oracle,
                &recording_oracle_token,
                recording_oracle_stake,
                &manifest_url,
                &manifest_hash,
            )
        }
        ("store-results", Some(arg_matches)) => {
            let escrow: Pubkey = pubkey_of(arg_matches, "escrow").unwrap();
            let amount = value_t_or_exit!(arg_matches, "amount", f64);
            let recipients = value_t_or_exit!(arg_matches, "recipients", u64);
            let results_url: String = value_of(arg_matches, "results_url").unwrap_or_default();
            let results_hash: Option<String> = value_of(arg_matches, "results_hash");
            command_store_results(
                &config,
                &escrow,
                amount,
                recipients,
                &results_url,
                &results_hash,
            )
        }
        ("payout", Some(arg_matches)) => {
            let escrow: Pubkey = pubkey_of(arg_matches, "escrow").unwrap();
            let file_name = value_t_or_exit!(arg_matches, "file_name", String);
            command_payout(&config, &escrow, &file_name)
        }
        ("cancel", Some(arg_matches)) => {
            let escrow: Pubkey = pubkey_of(arg_matches, "escrow").unwrap();
            command_cancel(&config, &escrow)
        }
        ("complete", Some(arg_matches)) => {
            let escrow: Pubkey = pubkey_of(arg_matches, "escrow").unwrap();
            command_complete(&config, &escrow)
        }
        _ => unreachable!(),
    }
    .and_then(|transaction| {
        if let Some(transaction) = transaction {
            // TODO: Upgrade to solana-client 1.3 and
            // `send_and_confirm_transaction_with_spinner_and_commitment()` with single
            // confirmation by default for better UX
            let signature = config
                .rpc_client
                .send_and_confirm_transaction_with_spinner_and_commitment(
                    &transaction,
                    config.commitment_config,
                )?;
            println!("Signature: {}", signature);
        }
        Ok(())
    })
    .map_err(|err| {
        eprintln!("{}", err);
        exit(1);
    });
}
