use crate::*;
use solana_program::pubkey::Pubkey;
use solana_sdk::native_token::*;

pub fn check_fee_payer_balance(
    config: &Config,
    address: &Pubkey,
    required_balance: u64,
) -> Result<(), Error> {
    let balance = config.rpc_client.get_balance(address)?;
    if balance < required_balance {
        Err(format!(
            "Fee payer, {}, has insufficient balance: {} required, {} available",
            address,
            lamports_to_sol(required_balance),
            lamports_to_sol(balance)
        )
        .into())
    } else {
        Ok(())
    }
}
