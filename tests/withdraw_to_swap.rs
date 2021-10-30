#![cfg(feature = "test-bpf")]

mod utils;

use deltafi_swap::{
    math::{Decimal, TryDiv},
    processor::process,
};

use solana_program_test::*;
use solana_sdk::{
    program_pack::Pack,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use spl_token::state::{Account as Token, Mint};
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

    let liquidity_provider =
        add_position(&mut test, &swap_info, &user_account_owner, 2_000_000_000);

    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let sol_withdraw_account = create_and_mint_to_token_account(
        &mut banks_client,
        spl_token::native_mint::id(),
        None,
        &payer,
        user_account_owner.pubkey(),
        0,
    )
    .await;

    let srm_withdraw_account = create_and_mint_to_token_account(
        &mut banks_client,
        srm_mint.pubkey,
        Some(&srm_mint.authority),
        &payer,
        user_account_owner.pubkey(),
        0,
    )
    .await;

    swap_info
        .withdraw(
            &mut banks_client,
            &liquidity_provider,
            &user_account_owner,
            sol_withdraw_account,
            srm_withdraw_account,
            swap_info.pool_token,
            2_000_000_000,
            2_000_000_000,
            40_000_000_000,
            &payer,
        )
        .await;

    assert!(get_token_balance(&mut banks_client, sol_withdraw_account).await > 2_000_000_000);
    assert!(get_token_balance(&mut banks_client, srm_withdraw_account).await > 40_000_000_000);

    let pool_token_account = banks_client
        .get_account(swap_info.pool_token)
        .await
        .unwrap()
        .unwrap();
    let pool_token = Token::unpack(&pool_token_account.data[..]).unwrap();

    let pool_mint_account = banks_client
        .get_account(swap_info.pool_mint)
        .await
        .unwrap()
        .unwrap();
    let pool_mint = Mint::unpack(&pool_mint_account.data[..]).unwrap();

    assert_eq!(pool_token.amount, pool_mint.supply);
}
