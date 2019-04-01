// Copyright 2018-2019 Chainpool.

//! this module is for chainx system

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;
mod tests;
pub mod types;

// Substrate
use inherents::{InherentData, InherentIdentifier, MakeFatalError, ProvideInherent, RuntimeString};
use rstd::{prelude::Vec, result::Result as StdResult};
use support::{decl_module, decl_storage, dispatch::Result, StorageValue};
use system::ensure_inherent;

// ChainX
use xsupport::{error, info};

#[cfg(feature = "std")]
pub use self::types::InherentDataProvider;
pub use self::types::InherentError;

pub const INHERENT_IDENTIFIER: InherentIdentifier = *b"producer";

pub trait Trait: system::Trait {
    type ValidatorList: ValidatorList<Self::AccountId>;
    type Validator: Validator<Self::AccountId>;
}

pub trait ValidatorList<AccountId> {
    fn validator_list() -> Vec<AccountId>;
}

pub trait Validator<AccountId> {
    fn get_validator_by_name(name: &[u8]) -> Option<AccountId>;
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn set_block_producer(origin, producer: T::AccountId) -> Result {
            ensure_inherent(origin)?;
            info!("height:{:}, blockproducer: {:}", system::Module::<T>::block_number(), producer);

            if Self::is_validator(&producer) == false {
                error!("producer:{:} not in current validators!, validators is:{:?}", producer, T::ValidatorList::validator_list());
                panic!("producer not in current validators!");
            }

            BlockProducer::<T>::put(producer);
            Ok(())
        }
        fn on_finalise(_n: T::BlockNumber) {
            BlockProducer::<T>::kill();
        }
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as XSystem {
        pub BlockProducer get(block_producer): Option<T::AccountId>;
        pub DeathAccount get(death_account) config(): T::AccountId;
        // TODO remove this to other module
        pub BurnAccount get(burn_account) config(): T::AccountId;
    }
}

impl<T: Trait> Module<T> {
    fn is_validator(producer: &T::AccountId) -> bool {
        let validators = T::ValidatorList::validator_list();
        validators.contains(&producer)
    }
}

impl<T: Trait> ProvideInherent for Module<T> {
    type Call = Call<T>;
    type Error = MakeFatalError<RuntimeString>;
    const INHERENT_IDENTIFIER: InherentIdentifier = INHERENT_IDENTIFIER;
    fn create_inherent(data: &InherentData) -> Option<Self::Call> {
        let r = data
            .get_data::<Vec<u8>>(&INHERENT_IDENTIFIER)
            .expect("gets and decodes producer inherent data");
        let producer_name = r.expect("producer must set before");

        let producer: T::AccountId = if let Some(a) =
            T::Validator::get_validator_by_name(&producer_name)
        {
            a
        } else {
            error!("[create_inherent] producer_name:{:} do not have accountid on chain, may not be registerd or do not have current storage", std::str::from_utf8(&producer_name).unwrap_or(&format!("{:?}", producer_name)));
            panic!("[create_inherent] do not have accountid on chain, may not be registerd or do not have current storage");
        };

        if !Self::is_validator(&producer) {
            error!(
                "[create_inherent] producer_name:{:?}, producer:{:} not in current validators!, validators is:{:?}",
                std::str::from_utf8(&producer_name).unwrap_or(&format!("{:?}", producer_name)),
                producer,
                T::ValidatorList::validator_list()
            );
            panic!("[create_inherent] producer not in current validators!");
        }

        Some(Call::set_block_producer(producer))
    }

    fn check_inherent(call: &Self::Call, _data: &InherentData) -> StdResult<(), Self::Error> {
        let producer = match call {
            Call::set_block_producer(ref p) => p.clone(),
            _ => return Err(RuntimeString::from("not found producer in call").into()),
        };

        if !Self::is_validator(&producer) {
            error!(
                "[check_inherent] producer:{:} not in current validators!, validators is:{:?}",
                producer,
                T::ValidatorList::validator_list()
            );
            return Err(
                RuntimeString::from("[check_inherent] producer not in current validators").into(),
            );
        }
        Ok(())
    }
}
