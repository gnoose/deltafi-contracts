#![cfg(feature = "test-bpf")]

mod utils;

use deltafi_swap::{error::SwapError, instruction::init_liquidity_provider, processor::process};

use solana_program_test::*;
use solana_sdk::{
    instruction::InstructionError,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use utils::*;

#[tokio::test]
async fn test_success() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(3_000);

    let user_account_owner = Keypair::new();

    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let test_liquidity_provider =
        TestLiquidityProvider::init(&mut banks_client, &user_account_owner, &payer).await;

    test_liquidity_provider
        .validate_state(&mut banks_client)
        .await;
}

#[tokio::test]
async fn test_already_initialized() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    let liquidity_owner = Keypair::new();
    let existing_liquidity_provider = add_liquidity_provider(&mut test, &liquidity_owner);
    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    let mut transaction = Transaction::new_with_payer(
        &[init_liquidity_provider(
            deltafi_swap::id(),
            existing_liquidity_provider.pubkey,
            existing_liquidity_provider.owner,
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer, &liquidity_owner], recent_blockhash);
    assert_eq!(
        banks_client
            .process_transaction(transaction)
            .await
            .unwrap_err()
            .unwrap(),
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(SwapError::AlreadyInUse as u32)
        )
    );
}
