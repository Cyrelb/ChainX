// Copyright 2018-2019 Chainpool.

//! this module is for funds-withdrawal

#![allow(clippy::ptr_arg)]
#![cfg_attr(not(feature = "std"), no_std)]

mod mock;
mod tests;

use parity_codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde_derive::{Deserialize, Serialize};

// Substrate
use rstd::prelude::Vec;
use support::{decl_module, decl_storage, dispatch::Result, StorageValue};
use system::ensure_signed;

// ChainX
use xassets::{Chain, ChainT, Memo, Token};
use xr_primitives::AddrStr;
#[cfg(feature = "std")]
use xsupport::token;
use xsupport::{debug, ensure_with_errorlog};

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct WithdrawalLimit<Balance> {
    pub minimal_withdrawal: Balance,
    pub fee: Balance,
}

pub trait Trait: xassets::Trait + xrecords::Trait + xbitcoin::Trait {}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn withdraw(origin, token: Token, value: T::Balance, addr: AddrStr, ext: Memo) -> Result {
            let who = ensure_signed(origin)?;

            Self::can_withdraw(&token)?;

            debug!("[withdraw]withdraw|who:{:?}|token:{:}|value:{:}", who, token!(token), value);

            let asset = xassets::Module::<T>::get_asset(&token)?;
            if asset.chain() == Chain::ChainX {
                return Err("Can't withdraw the asset on ChainX")
            }

            Self::verify_addr(&token, &addr, &ext)?;

            let limit = Self::withdrawal_limit(&token).ok_or("token should has withdrawal limit")?;
            // withdrawal value should larger than minimal_withdrawal, allow equal
            if value < limit.minimal_withdrawal {
                return Err("withdrawal value should larger than requirement")
            }

            xrecords::Module::<T>::withdrawal(&who, &token, value, addr, ext)?;
            Ok(())
        }

        fn revoke_withdraw(origin, id: u32) -> Result {
            let from = ensure_signed(origin)?;
            xrecords::Module::<T>::withdrawal_revoke(&from, id)
        }

        pub fn modify_token_black_list(token :Token) {
            TokenBlackList::<T>::mutate(|v| {
                if v.contains(&token) {
                    v.retain(|i| *i != token);
                } else {
                    v.push(token);
                }
            });
        }
    }
}

// bugfix:
// notice the old version is `Withdrawal`, it's a wrong naming.
// we fix it to `XAssetsProcess`, and it would affect genesis init for `TokenBlackList`
decl_storage! {
    trait Store for Module<T: Trait> as XAssetsProcess {
        TokenBlackList get(token_black_list) config(): Vec<Token>;
    }
}

impl<T: Trait> Module<T> {
    #[inline]
    fn can_withdraw(token: &Token) -> Result {
        ensure_with_errorlog!(
            xassets::Module::<T>::can_do(token, xassets::AssetLimit::CanWithdraw),
            "this asset do not allow withdraw",
            "token:{:}",
            token!(token),
        );
        Ok(())
    }

    fn verify_addr(token: &Token, addr: &[u8], _ext: &[u8]) -> Result {
        match token.as_slice() {
            <xbitcoin::Module<T> as ChainT>::TOKEN => xbitcoin::Module::<T>::check_addr(&addr, b""),
            _ => Err("not found match token Token addr checker"),
        }
    }

    pub fn verify_address(token: Token, addr: AddrStr, ext: Memo) -> Result {
        Self::verify_addr(&token, &addr, &ext)
    }

    pub fn withdrawal_limit(token: &Token) -> Option<WithdrawalLimit<T::Balance>> {
        match token.as_slice() {
            <xbitcoin::Module<T> as ChainT>::TOKEN => {
                let fee = xbitcoin::Module::<T>::btc_withdrawal_fee().into();
                let limit = WithdrawalLimit::<T::Balance> {
                    minimal_withdrawal: fee * 3.into() / 2.into(),
                    fee,
                };
                Some(limit)
            }
            _ => None,
        }
    }
}
