#![cfg(feature = "test-bpf")]

mod utils;

use deltafi_swap::{error::SwapError, instruction::commit_new_admin, processor::process};
use solana_program::{instruction::InstructionError, program_pack::Pack};
use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use spl_token::state::Mint;
use utils::*;

#[tokio::test]
async fn test_success() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(20_000);

    let swap_config = add_swap_config(&mut test);
    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    let new_admin_key = Pubkey::new_unique();

    let mut transaction = Transaction::new_with_payer(
        &[commit_new_admin(
            deltafi_swap::id(),
            swap_config.pubkey,
            swap_config.admin.pubkey(),
            swap_config.deltafi_mint,
            new_admin_key,
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );

    transaction.sign(&[&payer, &swap_config.admin], recent_blockhash);

    banks_client
        .process_transaction(transaction)
        .await
        .map_err(|e| e.unwrap())
        .unwrap();

    let swap_config_info = swap_config.get_state(&mut banks_client).await;
    assert_eq!(swap_config_info.admin_key, new_admin_key);

    let deltafi_mint = banks_client
        .get_account(swap_config.deltafi_mint)
        .await
        .unwrap()
        .unwrap();
    let deltafi_mint_info = Mint::unpack(&deltafi_mint.data[..]).unwrap();
    assert_eq!(deltafi_mint_info.freeze_authority.unwrap(), new_admin_key);
}

#[tokio::test]
async fn test_invalid_owner() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    let swap_config = add_swap_config(&mut test);
    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    let invalid_owner = Keypair::new();
    let new_admin_key = Pubkey::new_unique();

    let mut transaction = Transaction::new_with_payer(
        &[commit_new_admin(
            deltafi_swap::id(),
            swap_config.pubkey,
            invalid_owner.pubkey(),
            swap_config.deltafi_mint,
            new_admin_key,
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );

    transaction.sign(&[&payer, &invalid_owner], recent_blockhash);

    assert_eq!(
        banks_client
            .process_transaction(transaction)
            .await
            .unwrap_err()
            .unwrap(),
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(SwapError::Unauthorized as u32)
        )
    );
}
