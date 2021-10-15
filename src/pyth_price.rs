//! pyth client getting function

#![cfg(all(unix))]
use pyth_client::{cast, AccountType, Price, Product, MAGIC, VERSION_2};
use solana_client::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;

use crate::utils::DEFAULT_TOKEN_DECIMALS;

/// get selected product's price info
#[cfg(not(unix))]
pub fn get_pyth_price_info(_prod_pubkey: Pubkey) -> Option<(u64, u64)> {
    Some((0, 0))
}

/// get selected product's price info
#[cfg(all(unix))]
pub fn get_pyth_price_info(prod_pubkey: Pubkey) -> Option<(u64, u64)> {
    // get pyth mapping account
    let url = "https://api.mainnet-beta.solana.com";
    let clnt = RpcClient::new(url.to_string());
    let mut res_price: i64 = 0;
    let mut res_conf: u64 = 0;
    let mut res_exponent: i32 = 0;

    let prod_data = clnt.get_account_data(&prod_pubkey).unwrap();
    let prod_acct = cast::<Product>(&prod_data);
    assert_eq!(prod_acct.magic, MAGIC, "not a valid pyth account");
    assert_eq!(
        prod_acct.atype,
        AccountType::Product as u32,
        "not a valid pyth product account"
    );
    assert_eq!(
        prod_acct.ver, VERSION_2,
        "unexpected pyth product account version"
    );

    // print all Prices that correspond to this Product
    if prod_acct.px_acc.is_valid() {
        let mut px_pkey = Pubkey::new(&prod_acct.px_acc.val);
        loop {
            let pd = clnt.get_account_data(&px_pkey).unwrap();
            let pa = cast::<Price>(&pd);
            assert_eq!(pa.magic, MAGIC, "not a valid pyth account");
            assert_eq!(
                pa.atype,
                AccountType::Price as u32,
                "not a valid pyth price account"
            );
            assert_eq!(pa.ver, VERSION_2, "unexpected pyth price account version");
            res_price = pa.agg.price;
            res_conf = pa.agg.conf;
            res_exponent = pa.expo;

            // go to next price account in list
            if pa.next.is_valid() {
                px_pkey = Pubkey::new(&pa.next.val);
            } else {
                break;
            }
        }
    }

    let u_price;

    if res_exponent.checked_add(DEFAULT_TOKEN_DECIMALS as i32)? < 0 {
        u_price = res_price.checked_div(
            10i64.pow(
                res_exponent
                    .checked_add(DEFAULT_TOKEN_DECIMALS as i32)?
                    .abs() as u32,
            ),
        )? as u64;
    } else {
        u_price = res_price.checked_mul(
            10i64.pow(
                res_exponent
                    .checked_add(DEFAULT_TOKEN_DECIMALS as i32)?
                    .abs() as u32,
            ),
        )? as u64;
    }

    Some((u_price, res_conf))
}

#[cfg(test)]
mod tests {
    use uint::core_::str::FromStr;

    use super::*;

    #[test]
    fn basic_tests() {
        let sol_prod_pubkey =
            Pubkey::from_str("2Lg3b2UdD4hzrxHpcwhgShuUdccTKFoo2doAUZawEdPH").unwrap();
        let (price, conf) = get_pyth_price_info(sol_prod_pubkey).unwrap();
        assert_ne!((price, conf), (0, 0));
    }
}
