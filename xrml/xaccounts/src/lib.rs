// Copyright 2018-2019 Chainpool.

//! This module mainly stores some special kinds of account
//! that could be queried from various function modules.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;
mod tests;
pub mod types;

// Substrate
use primitives::traits::Hash;
use rstd::prelude::*;
use substrate_primitives::crypto::UncheckedFrom;
use support::{decl_module, decl_storage, dispatch::Result};

// ChainX
use xr_primitives::Name;

pub use self::types::IntentionProps;

pub trait Trait: system::Trait + consensus::Trait {
    /// Generate virtual AccountId for each (psedu) intention
    type DetermineIntentionJackpotAccountId: IntentionJackpotAccountIdFor<Self::AccountId>;
}

pub trait IntentionJackpotAccountIdFor<AccountId: Sized> {
    fn accountid_for(origin: &AccountId) -> AccountId;
}

pub struct SimpleAccountIdDeterminator<T: Trait>(::rstd::marker::PhantomData<T>);

impl<T: Trait> IntentionJackpotAccountIdFor<T::AccountId> for SimpleAccountIdDeterminator<T>
where
    T::AccountId: UncheckedFrom<T::Hash>,
{
    fn accountid_for(origin: &T::AccountId) -> T::AccountId {
        let name = Module::<T>::intention_name_of(origin)
            .expect("The original account must be an existing intention.");
        // name
        UncheckedFrom::unchecked_from(T::Hashing::hash(&name))
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as XAccounts {
        /// intention name => intention
        pub IntentionOf get(intention_of): map Name => Option<T::AccountId>;

        /// intention => intention name
        pub IntentionNameOf get(intention_name_of): map T::AccountId => Option<Name>;

        pub IntentionPropertiesOf get(intention_props_of): map T::AccountId => IntentionProps<T::SessionKey, T::BlockNumber>;

        pub TeamAccount get(team_account): T::AccountId;
        pub CouncilAccount get(council_account): T::AccountId;
    }
}

impl<T: Trait> Module<T> {
    pub fn is_intention(who: &T::AccountId) -> bool {
        Self::intention_name_of(who).is_some()
    }
}

impl<T: Trait> xsystem::Validator<T::AccountId> for Module<T> {
    fn get_validator_by_name(name: &[u8]) -> Option<T::AccountId> {
        Self::intention_of(name.to_vec())
    }
    fn get_validator_name(accountid: &T::AccountId) -> Option<Vec<u8>> {
        Self::intention_name_of(accountid)
    }
}

pub fn is_valid_name(name: &[u8]) -> Result {
    if name.len() > 12 || name.len() < 2 {
        return Err("The length of name must be in range [2, 12].");
    }

    Ok(())
}

pub fn is_valid_about(about: &[u8]) -> Result {
    if about.len() > 128 {
        return Err("The length of about must be in range [0, 128].");
    }

    Ok(())
}

pub fn is_valid_url(url: &[u8]) -> Result {
    if url.len() > 24 || url.len() < 4 {
        return Err("The length of url must be in range [4, 24].");
    }
    // ASCII alphanumeric character and '.'
    let is_valid = |n: &u8| -> bool { n.is_ascii_alphanumeric() || *n == b'.' };

    if url.iter().filter(|n| !is_valid(n)).count() > 0 {
        return Err("Only ASCII alphanumeric character and . are allowed.");
    }

    Ok(())
}
