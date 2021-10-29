#![cfg(feature = "test-bpf")]

mod utils;

use deltafi_swap::{error::SwapError, instruction::initialize_config, processor::process};
use solana_program_test::*;
use solana_sdk::{
    instruction::InstructionError,
    pubkey::Pubkey,
    signature::Signer,
    transaction::{Transaction, TransactionError},
};
use utils::*;

#[tokio::test]
async fn test_success() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(20_000);

    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let test_swap_config = TestSwapConfig::init(&mut banks_client, &payer).await;

    test_swap_config.validate_state(&mut banks_client).await;
}

#[tokio::test]
async fn test_already_initialized() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    let existing_config = add_swap_config(&mut test);
    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    let mut transaction = Transaction::new_with_payer(
        &[initialize_config(
            deltafi_swap::id(),
            existing_config.pubkey,
            existing_config.market_authority,
            existing_config.deltafi_mint,
            existing_config.admin.pubkey(),
            existing_config.fees,
            existing_config.rewards,
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer, &existing_config.admin], recent_blockhash);
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
