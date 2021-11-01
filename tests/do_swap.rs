#![cfg(feature = "test-bpf")]

mod utils;

use deltafi_swap::{
    instruction::SwapDirection,
    math::{Decimal, TryDiv},
    processor::process,
};

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use utils::*;

#[tokio::test]
async fn test_success() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(100_000);

    let swap_config = add_swap_config(&mut test);

    let sol_oracle = add_sol_oracle(&mut test);
    let srm_oracle = add_srm_oracle(&mut test);
    let srm_mint = add_srm_mint(&mut test);

    let user_account_owner = Keypair::new();
    let admin_account_owner = Keypair::new();

    let swap_info = add_swap_info(
        &mut test,
        &swap_config,
        &user_account_owner,
        &admin_account_owner,
        AddSwapInfoArgs {
            token_a_mint: spl_token::native_mint::id(),
            token_b_mint: srm_mint.pubkey,
            token_a_amount: 42_000_000_000,
            token_b_amount: 800_000_000_000,
            is_open_twap: true,
            oracle_a: sol_oracle.price_pubkey,
            oracle_b: srm_oracle.price_pubkey,
            market_price: sol_oracle.price.try_div(srm_oracle.price).unwrap(),
            slope: Decimal::one().try_div(2).unwrap(),
        },
    );

    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let user_account_owner = Keypair::new();
    let sol_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        spl_token::native_mint::id(),
        None,
        &payer,
        user_account_owner.pubkey(),
        10_000_000_000,
    )
    .await;

    let srm_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        srm_mint.pubkey,
        Some(&srm_mint.authority),
        &payer,
        user_account_owner.pubkey(),
        0,
    )
    .await;

    let deltafi_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        swap_config.deltafi_mint,
        None,
        &payer,
        user_account_owner.pubkey(),
        0,
    )
    .await;

    swap_info
        .swap(
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            sol_user_account,
            srm_user_account,
            deltafi_user_account,
            2_000_000_000,
            35_000_000_000,
            SwapDirection::SellBase,
            &payer,
        )
        .await;

    assert_eq!(
        get_token_balance(&mut banks_client, sol_user_account).await,
        8_000_000_000,
    );
    assert!(get_token_balance(&mut banks_client, srm_user_account).await > 35_000_000_000);
    assert!(get_token_balance(&mut banks_client, deltafi_user_account).await > 0);
}
