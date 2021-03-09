#![cfg(feature = "test-bpf")]

use hmt_escrow::state::DataHash;
use hmt_escrow::state::DataUrl;
use hmt_escrow::*;
use solana_program::{hash::Hash, program_pack::Pack, pubkey::Pubkey, system_instruction};
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;
const DECIMALS: u8 = 9;

const DEFAULT_FACTORY_VERSION: u8 = 1;

fn program_test() -> ProgramTest {
    let mut pc = ProgramTest::new(
        "hmt_escrow",
        id(),
        processor!(processor::Processor::process),
    );

    // Add SPL Token program
    pc.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );

    pc
}

async fn create_mint(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    token_mint: &Keypair,
    owner: &Pubkey,
) {
    let rent = banks_client.get_rent().await.unwrap();
    let mint_rent = rent.minimum_balance(spl_token::state::Mint::LEN);

    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &token_mint.pubkey(),
                mint_rent,
                spl_token::state::Mint::LEN as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_mint(
                &spl_token::id(),
                &token_mint.pubkey(),
                &owner,
                None,
                DECIMALS,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, token_mint], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

async fn create_token_account(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    account: &Keypair,
    token_mint: &Pubkey,
    owner: &Pubkey,
) {
    let rent = banks_client.get_rent().await.unwrap();
    let account_rent = rent.minimum_balance(spl_token::state::Account::LEN);

    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &account.pubkey(),
                account_rent,
                spl_token::state::Account::LEN as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_account(
                &spl_token::id(),
                &account.pubkey(),
                token_mint,
                owner,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, account], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

async fn create_escrow(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    escrow_account: &Keypair,
    factory_account: &Keypair,
    escrow_token_account: &Keypair,
    launcher: &Pubkey,
    canceler: &Pubkey,
    canceler_token: &Keypair,
    token_mint: &Pubkey,
    duration: &u64,
) {
    let rent = banks_client.get_rent().await.unwrap();
    let account_rent = rent.minimum_balance(state::Escrow::LEN);

    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &escrow_account.pubkey(),
                account_rent,
                hmt_escrow::state::Escrow::LEN as u64,
                &id(),
            ),
            instruction::initialize(
                &id(),
                &escrow_account.pubkey(),
                &factory_account.pubkey(),
                token_mint,
                &escrow_token_account.pubkey(),
                &launcher,
                &canceler,
                &canceler_token.pubkey(),
                *duration,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, escrow_account], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

async fn create_factory(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    factory_account: &Keypair,
    version: u8,
) {
    let rent = banks_client.get_rent().await.unwrap();
    let account_rent = rent.minimum_balance(state::Factory::LEN);

    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &factory_account.pubkey(),
                account_rent,
                state::Factory::LEN as u64,
                &id(),
            ),
            instruction::factory_initialize(&id(), &factory_account.pubkey(), version).unwrap(),
        ],
        Some(&payer.pubkey()),
    );

    transaction.sign(&[payer, factory_account], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

async fn setup_escrow(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    escrow_account: &Keypair,
    trust_handler: &Keypair,
    reputation_oracle: &Keypair,
    reputation_oracle_token: &Keypair,
    reputation_oracle_stake: &u8,
    recording_oracle: &Keypair,
    recording_oracle_token: &Keypair,
    recording_oracle_stake: &u8,
    manifest_url: &DataUrl,
    manifest_hash: &DataHash,
) {
    let mut transaction = Transaction::new_with_payer(
        &[instruction::setup(
            &id(),
            &escrow_account.pubkey(),
            &trust_handler.pubkey(),
            &reputation_oracle.pubkey(),
            &reputation_oracle_token.pubkey(),
            *reputation_oracle_stake,
            &recording_oracle.pubkey(),
            &recording_oracle_token.pubkey(),
            *recording_oracle_stake,
            manifest_url,
            manifest_hash,
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, trust_handler], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

async fn store_results(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    escrow_account: &Keypair,
    trust_handler: &Keypair,
    total_amount: &f64,
    total_recipients: &u64,
    final_results_url: &DataUrl,
    final_results_hash: &DataHash,
) {
    let mut transaction = Transaction::new_with_payer(
        &[instruction::store_results(
            &id(),
            &escrow_account.pubkey(),
            &trust_handler.pubkey(),
            spl_token::ui_amount_to_amount(*total_amount, DECIMALS),
            *total_recipients,
            &final_results_url,
            final_results_hash,
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, trust_handler], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

async fn payout(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    escrow_account: &Keypair,
    trust_handler: &Keypair,
    escrow_token_account: &Keypair,
    escrow_authority: &Pubkey,
    recipient_token_account: &Keypair,
    reputation_oracle_token_account: &Keypair,
    recording_oracle_token_account: &Keypair,
    amount: &f64,
) {
    let mut transaction = Transaction::new_with_payer(
        &[instruction::payout(
            &id(),
            &escrow_account.pubkey(),
            &trust_handler.pubkey(),
            &escrow_token_account.pubkey(),
            &escrow_authority,
            &recipient_token_account.pubkey(),
            &reputation_oracle_token_account.pubkey(),
            &recording_oracle_token_account.pubkey(),
            &spl_token::id(),
            spl_token::ui_amount_to_amount(*amount, DECIMALS),
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, trust_handler], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

async fn cancel(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    escrow_account: &Keypair,
    trust_handler: &Keypair,
    escrow_token_account: &Keypair,
    escrow_authority: &Pubkey,
    canceler_token_account: &Keypair,
) {
    let mut transaction = Transaction::new_with_payer(
        &[instruction::cancel(
            &id(),
            &escrow_account.pubkey(),
            &trust_handler.pubkey(),
            &escrow_token_account.pubkey(),
            &escrow_authority,
            &canceler_token_account.pubkey(),
            &spl_token::id(),
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, trust_handler], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

async fn complete(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    escrow_account: &Keypair,
    trust_handler: &Keypair,
) {
    let mut transaction = Transaction::new_with_payer(
        &[
            instruction::complete(&id(), &escrow_account.pubkey(), &trust_handler.pubkey())
                .unwrap(),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, trust_handler], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

async fn mint_to_escrow(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: &Hash,
    mint_pubkey: &Keypair,
    account_pubkey: &Keypair,
    owner_pubkey: &Keypair,
    amount: f64,
) {
    let mut transaction = Transaction::new_with_payer(
        &[spl_token::instruction::mint_to(
            &spl_token::id(),
            &mint_pubkey.pubkey(),
            &account_pubkey.pubkey(),
            &owner_pubkey.pubkey(),
            &[],
            spl_token::ui_amount_to_amount(amount, DECIMALS),
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer, owner_pubkey], *recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
}

struct EscrowAccount {
    pub escrow: Keypair,
    pub factory: Keypair,
    pub token_mint: Keypair,
    pub escrow_token_account: Keypair,
    pub launcher: Keypair,
    pub canceler: Keypair,
    pub canceler_token_account: Keypair,
    pub duration: u64,
    pub escrow_authority: Pubkey,
    pub bump_seed: u8,
    pub reputation_oracle: Keypair,
    pub reputation_oracle_token: Keypair,
    pub reputation_oracle_stake: u8,
    pub recording_oracle: Keypair,
    pub recording_oracle_token: Keypair,
    pub recording_oracle_stake: u8,
    pub manifest_url: DataUrl,
    pub manifest_hash: DataHash,
    pub final_results_url: DataUrl,
    pub final_results_hash: DataHash,
    pub total_amount: f64,
    pub total_recipients: u64,
    pub payout_amount: f64,
    pub mint_authority: Keypair,
}

impl EscrowAccount {
    pub fn new() -> Self {
        let escrow = Keypair::new();
        let factory = Keypair::new();
        let token_mint = Keypair::new();
        let escrow_token_account = Keypair::new();
        let launcher = Keypair::new();
        let canceler = Keypair::new();
        let canceler_token_account = Keypair::new();

        let reputation_oracle = Keypair::new();
        let reputation_oracle_token = Keypair::new();
        let recording_oracle = Keypair::new();
        let recording_oracle_token = Keypair::new();
        let mint_authority = Keypair::new();

        let manifest_array_for_hash = [1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 9, 8, 7, 6, 5, 4, 3, 9, 9];
        let final_array_for_hash = [7, 3, 4, 3, 4, 5, 6, 7, 2, 1, 6, 6, 0, 2, 2, 2, 3, 2, 4, 7];
        let manifest_hash = DataHash::new_from_array(manifest_array_for_hash);
        let final_results_hash = DataHash::new_from_array(final_array_for_hash);
        //find authority bumpseed
        let (escrow_authority, bump_seed) =
            hmt_escrow::processor::Processor::find_authority_bump_seed(&id(), &escrow.pubkey());

        let manifest_url: DataUrl = match DataUrl::from_str("http://somemanifest.com") {
            Ok(url) => url,
            _ => Default::default(),
        };

        let final_results_url: DataUrl = match DataUrl::from_str("http://result.com") {
            Ok(url) => url,
            _ => Default::default(),
        };
        Self {
            escrow,
            factory,
            token_mint,
            escrow_token_account,
            launcher,
            canceler,
            canceler_token_account,
            duration: 100000 as u64,
            escrow_authority,
            bump_seed,
            reputation_oracle,
            reputation_oracle_token,
            reputation_oracle_stake: 10 as u8,
            recording_oracle,
            recording_oracle_token,
            recording_oracle_stake: 15 as u8,
            total_amount: 30.0 as f64,
            total_recipients: 1 as u64,
            payout_amount: 30.0 as f64,
            mint_authority,
            manifest_url,
            manifest_hash,
            final_results_url,
            final_results_hash,
        }
    }

    pub async fn initialize_escrow(
        &self,
        mut banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,
    ) {
        create_mint(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.token_mint,
            &self.mint_authority.pubkey(),
        )
        .await;

        //Creating token account for escrow
        create_token_account(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.escrow_token_account,
            &self.token_mint.pubkey(),
            &self.escrow_authority,
        )
        .await;

        //Creating token account for canceler
        create_token_account(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.canceler_token_account,
            &self.token_mint.pubkey(),
            &self.canceler.pubkey(),
        )
        .await;
        create_factory(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.factory,
            DEFAULT_FACTORY_VERSION,
        )
        .await;
        create_escrow(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.escrow,
            &self.factory,
            &self.escrow_token_account,
            &self.launcher.pubkey(),
            &self.canceler.pubkey(),
            &self.canceler_token_account,
            &self.token_mint.pubkey(),
            &self.duration,
        )
        .await;
    }

    pub async fn setup_escrow(
        &self,
        mut banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,
    ) {
        //Creating token account for reputation_oracle_token
        create_token_account(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.reputation_oracle_token,
            &self.token_mint.pubkey(),
            &self.reputation_oracle.pubkey(),
        )
        .await;

        //Creating token account for recording_oracle_token
        create_token_account(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.recording_oracle_token,
            &self.token_mint.pubkey(),
            &self.recording_oracle.pubkey(),
        )
        .await;
        setup_escrow(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.escrow,
            &self.launcher,
            &self.reputation_oracle,
            &self.reputation_oracle_token,
            &self.reputation_oracle_stake,
            &self.recording_oracle,
            &self.recording_oracle_token,
            &self.recording_oracle_stake,
            &self.manifest_url,
            &self.manifest_hash,
        )
        .await;
    }

    pub async fn store_results(
        &self,
        mut banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,
    ) {
        store_results(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &self.escrow,
            &self.launcher,
            &self.total_amount,
            &self.total_recipients,
            &self.final_results_url,
            &self.final_results_hash,
        )
        .await;
    }

    pub async fn payout_escrow(
        &self,
        mut banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,
        recipient_token_account: &Keypair,
    ) {
        payout(
            &mut banks_client,
            payer,
            &recent_blockhash,
            &self.escrow,
            &self.launcher,
            &self.escrow_token_account,
            &self.escrow_authority,
            recipient_token_account,
            &self.reputation_oracle_token,
            &self.recording_oracle_token,
            &self.payout_amount,
        )
        .await;
    }

    pub async fn cancel_escrow(
        &self,
        mut banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,
    ) {
        cancel(
            &mut banks_client,
            payer,
            &recent_blockhash,
            &self.escrow,
            &self.launcher,
            &self.escrow_token_account,
            &self.escrow_authority,
            &self.canceler_token_account,
        )
        .await;
    }

    pub async fn complete_escrow(
        &self,
        mut banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: &Hash,
    ) {
        complete(
            &mut banks_client,
            payer,
            &recent_blockhash,
            &self.escrow,
            &self.launcher,
        )
        .await;
    }
}

async fn check_escrow_account_info<F>(f: F, escrow: &EscrowAccount, banks_client: &mut BanksClient)
where
    F: Fn(state::Escrow),
{
    let escrow = banks_client
        .get_account(escrow.escrow.pubkey())
        .await
        .expect("get_account")
        .expect("cannot read escrow account data");

    assert_eq!(escrow.data.len(), hmt_escrow::state::Escrow::LEN);
    match state::Escrow::unpack_from_slice(escrow.data.as_slice()) {
        Ok(escrow) => {
            f(escrow);
        }
        Err(_) => assert!(false),
    };
}

async fn check_token_account_info<F>(f: F, account: &Keypair, banks_client: &mut BanksClient)
where
    F: Fn(spl_token::state::Account),
{
    let account = banks_client
        .get_account(account.pubkey())
        .await
        .expect("get_account")
        .expect("cannot read token account data");

    match spl_token::state::Account::unpack_from_slice(account.data.as_slice()) {
        Ok(token_account) => {
            f(token_account);
        }
        Err(_) => assert!(false),
    };
}

#[tokio::test]
async fn test_hmt_escrow_initialize() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let escrow_account = EscrowAccount::new();
    escrow_account
        .initialize_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let initialize_check = |escrow: state::Escrow| {
        assert_eq!(escrow.state, state::EscrowState::Launched);
        assert_eq!(escrow.bump_seed, escrow_account.bump_seed);
        assert_eq!(escrow.token_mint, escrow_account.token_mint.pubkey());
        assert_eq!(
            escrow.token_account,
            escrow_account.escrow_token_account.pubkey()
        );
        assert_eq!(escrow.canceler, escrow_account.canceler.pubkey());
        assert_eq!(
            escrow.canceler_token_account,
            escrow_account.canceler_token_account.pubkey()
        );
    };

    check_escrow_account_info(initialize_check, &escrow_account, &mut banks_client).await;
}

#[tokio::test]
async fn test_hmt_escrow_setup() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let escrow_account = EscrowAccount::new();
    escrow_account
        .initialize_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;

    escrow_account
        .setup_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let setup_check = |escrow: state::Escrow| {
        assert_eq!(escrow.state, state::EscrowState::Pending);
        assert_eq!(escrow.bump_seed, escrow_account.bump_seed);
        assert_eq!(escrow.token_mint, escrow_account.token_mint.pubkey());
        assert_eq!(
            escrow.token_account,
            escrow_account.escrow_token_account.pubkey()
        );

        assert_eq!(escrow.canceler, escrow_account.canceler.pubkey());
        assert_eq!(
            escrow.canceler_token_account,
            escrow_account.canceler_token_account.pubkey()
        );

        assert_eq!(
            escrow.reputation_oracle.unwrap(),
            escrow_account.reputation_oracle.pubkey()
        );
        assert_eq!(
            escrow.reputation_oracle_token_account.unwrap(),
            escrow_account.reputation_oracle_token.pubkey()
        );

        assert_eq!(
            escrow.recording_oracle.unwrap(),
            escrow_account.recording_oracle.pubkey()
        );
        assert_eq!(
            escrow.recording_oracle_token_account.unwrap(),
            escrow_account.recording_oracle_token.pubkey()
        );

        assert_eq!(
            escrow.reputation_oracle_stake,
            escrow_account.reputation_oracle_stake
        );
        assert_eq!(
            escrow.recording_oracle_stake,
            escrow_account.recording_oracle_stake
        );
    };

    check_escrow_account_info(setup_check, &escrow_account, &mut banks_client).await;
}

#[tokio::test]
async fn test_hmt_escrow_store_results() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let escrow_account = EscrowAccount::new();
    escrow_account
        .initialize_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;

    escrow_account
        .setup_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;
    escrow_account
        .store_results(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let store_check = |escrow: state::Escrow| {
        assert_eq!(escrow.state, state::EscrowState::Pending);
        assert_eq!(
            escrow.total_amount,
            spl_token::ui_amount_to_amount(escrow_account.total_amount, DECIMALS)
        );
        assert_eq!(escrow.total_recipients, escrow_account.total_recipients);
        assert_eq!(escrow.manifest_url, escrow_account.manifest_url);
        assert_eq!(escrow.manifest_hash, escrow_account.manifest_hash);
        assert_eq!(escrow.final_results_url, escrow_account.final_results_url);
        assert_eq!(escrow.final_results_hash, escrow_account.final_results_hash);
    };

    check_escrow_account_info(store_check, &escrow_account, &mut banks_client).await;
}

#[tokio::test]
async fn test_hmt_escrow_payout() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let escrow_account = EscrowAccount::new();
    escrow_account
        .initialize_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let recipient = Keypair::new();
    let recipient_token_account = Keypair::new();

    create_token_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &recipient_token_account,
        &escrow_account.token_mint.pubkey(),
        &recipient.pubkey(),
    )
    .await;

    escrow_account
        .setup_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;
    escrow_account
        .store_results(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let escrow_token_for_payout = 5000.0;
    let escrow_token_for_payout_to_mint =
        spl_token::ui_amount_to_amount(escrow_token_for_payout, DECIMALS);

    mint_to_escrow(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &escrow_account.token_mint,
        &escrow_account.escrow_token_account,
        &escrow_account.mint_authority,
        escrow_token_for_payout,
    )
    .await;

    escrow_account
        .payout_escrow(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &recipient_token_account,
        )
        .await;

    let stake_percent: f64 =
        (escrow_account.reputation_oracle_stake + escrow_account.recording_oracle_stake).into();
    let payout_amount_to_mint =
        spl_token::ui_amount_to_amount(escrow_account.payout_amount, DECIMALS);
    let result_amount =
        escrow_account.payout_amount - (escrow_account.payout_amount * (stake_percent / 100.0));
    let result_amount_to_mint = spl_token::ui_amount_to_amount(result_amount, DECIMALS);

    let amount_check = |token_account: spl_token::state::Account| {
        assert_eq!(token_account.amount, result_amount_to_mint)
    };

    check_token_account_info(amount_check, &recipient_token_account, &mut banks_client).await;

    let store_check = |token_account: spl_token::state::Account| {
        assert_eq!(
            token_account.amount,
            escrow_token_for_payout_to_mint - payout_amount_to_mint
        );
    };

    check_token_account_info(
        store_check,
        &escrow_account.escrow_token_account,
        &mut banks_client,
    )
    .await;

    let reputation_oracle_stake: f64 = escrow_account.reputation_oracle_stake.into();
    let result_amount = escrow_account.payout_amount * (reputation_oracle_stake / 100.0);
    let reputation_oracle_payout = spl_token::ui_amount_to_amount(result_amount, DECIMALS);

    let amount_check = |token_account: spl_token::state::Account| {
        assert_eq!(token_account.amount, reputation_oracle_payout);
    };
    check_token_account_info(
        amount_check,
        &escrow_account.reputation_oracle_token,
        &mut banks_client,
    )
    .await;

    let recording_oracle_stake: f64 = escrow_account.recording_oracle_stake.into();
    let result_amount = escrow_account.payout_amount * (recording_oracle_stake / 100.0);
    let recording_oracle_payout = spl_token::ui_amount_to_amount(result_amount, DECIMALS);

    let amount_check = |token_account: spl_token::state::Account| {
        assert_eq!(token_account.amount, recording_oracle_payout);
    };
    check_token_account_info(
        amount_check,
        &escrow_account.recording_oracle_token,
        &mut banks_client,
    )
    .await;
}

#[tokio::test]
async fn test_hmt_escrow_cancel() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let escrow_account = EscrowAccount::new();
    escrow_account
        .initialize_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let recipient = Keypair::new();
    let recipient_token_account = Keypair::new();

    create_token_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &recipient_token_account,
        &escrow_account.token_mint.pubkey(),
        &recipient.pubkey(),
    )
    .await;

    escrow_account
        .setup_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;
    escrow_account
        .store_results(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let escrow_token_for_payout = 5000.0;
    let escrow_token_for_payout_to_mint =
        spl_token::ui_amount_to_amount(escrow_token_for_payout, DECIMALS);

    mint_to_escrow(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &escrow_account.token_mint,
        &escrow_account.escrow_token_account,
        &escrow_account.mint_authority,
        escrow_token_for_payout,
    )
    .await;

    escrow_account
        .cancel_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let initialize_check = |escrow: state::Escrow| {
        assert_eq!(escrow.state, state::EscrowState::Cancelled);
    };
    check_escrow_account_info(initialize_check, &escrow_account, &mut banks_client).await;

    let store_check = |token_account: spl_token::state::Account| {
        assert_eq!(token_account.amount, escrow_token_for_payout_to_mint);
    };
    check_token_account_info(
        store_check,
        &escrow_account.canceler_token_account,
        &mut banks_client,
    )
    .await;
}

#[tokio::test]
async fn test_hmt_escrow_complete() {
    let (mut banks_client, payer, recent_blockhash) = program_test().start().await;
    let escrow_account = EscrowAccount::new();
    escrow_account
        .initialize_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let recipient = Keypair::new();
    let recipient_token_account = Keypair::new();

    create_token_account(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &recipient_token_account,
        &escrow_account.token_mint.pubkey(),
        &recipient.pubkey(),
    )
    .await;

    escrow_account
        .setup_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;
    escrow_account
        .store_results(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let escrow_token_for_payout = 5000.0;

    mint_to_escrow(
        &mut banks_client,
        &payer,
        &recent_blockhash,
        &escrow_account.token_mint,
        &escrow_account.escrow_token_account,
        &escrow_account.mint_authority,
        escrow_token_for_payout,
    )
    .await;

    escrow_account
        .payout_escrow(
            &mut banks_client,
            &payer,
            &recent_blockhash,
            &recipient_token_account,
        )
        .await;

    escrow_account
        .complete_escrow(&mut banks_client, &payer, &recent_blockhash)
        .await;

    let initialize_check = |escrow: state::Escrow| {
        assert_eq!(escrow.state, state::EscrowState::Complete);
    };
    check_escrow_account_info(initialize_check, &escrow_account, &mut banks_client).await;
}
