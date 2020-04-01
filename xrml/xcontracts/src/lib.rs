// Copyright 2018-2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate. If not, see <http://www.gnu.org/licenses/>.

//! # Contract Module
//!
//! The Contract module provides functionality for the runtime to deploy and execute WebAssembly smart-contracts.
//!
//! - [`contracts::Trait`](./trait.Trait.html)
//! - [`Call`](./enum.Call.html)
//!
//! ## Overview
//!
//! This module extends accounts based on the `Currency` trait to have smart-contract functionality. It can
//! be used with other modules that implement accounts based on `Currency`. These "smart-contract accounts"
//! have the ability to instantiate smart-contracts and make calls to other contract and non-contract accounts.
//!
//! The smart-contract code is stored once in a `code_cache`, and later retrievable via its `code_hash`.
//! This means that multiple smart-contracts can be instantiated from the same `code_cache`, without replicating
//! the code each time.
//!
//! When a smart-contract is called, its associated code is retrieved via the code hash and gets executed.
//! This call can alter the storage entries of the smart-contract account, instantiate new smart-contracts,
//! or call other smart-contracts.
//!
//! Finally, when an account is reaped, its associated code and storage of the smart-contract account
//! will also be deleted.
//!
//! ### Gas
//!
//! Senders must specify a gas limit with every call, as all instructions invoked by the smart-contract require gas.
//! Unused gas is refunded after the call, regardless of the execution outcome.
//!
//! If the gas limit is reached, then all calls and state changes (including balance transfers) are only
//! reverted at the current call's contract level. For example, if contract A calls B and B runs out of gas mid-call,
//! then all of B's calls are reverted. Assuming correct error handling by contract A, A's other calls and state
//! changes still persist.
//!
//! ### Notable Scenarios
//!
//! Contract call failures are not always cascading. When failures occur in a sub-call, they do not "bubble up",
//! and the call will only revert at the specific contract level. For example, if contract A calls contract B, and B
//! fails, A can decide how to handle that failure, either proceeding or reverting A's changes.
//!
//! ## Interface
//!
//! ### Dispatchable functions
//!
//! * `put_code` - Stores the given binary Wasm code into the chain's storage and returns its `code_hash`.
//! * `instantiate` - Deploys a new contract from the given `code_hash`, optionally transferring some balance.
//! This instantiates a new smart contract account and calls its contract deploy handler to
//! initialize the contract.
//! * `call` - Makes a call to an account, optionally transferring some balance.
//!
//! ### Signed Extensions
//!
//! The contracts module defines the following extension:
//!
//!   - [`CheckBlockGasLimit`]: Ensures that the transaction does not exceeds the block gas limit.
//!
//! The signed extension needs to be added as signed extra to the transaction type to be used in the
//! runtime.
//!
//! ## Usage
//!
//! The Contract module is a work in progress. The following examples show how this Contract module
//! can be used to instantiate and call contracts.
//!
//! * [`ink`](https://github.com/paritytech/ink) is
//! an [`eDSL`](https://wiki.haskell.org/Embedded_domain_specific_language) that enables writing
//! WebAssembly based smart contracts in the Rust programming language. This is a work in progress.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
mod gas;

mod account_db;
mod exec;
mod rent;
mod wasm;

#[cfg(test)]
mod tests;

use crate::account_db::{AccountDb, DirectAccountDb};
use crate::exec::ExecutionContext;
use crate::wasm::{WasmLoader, WasmVm};

pub use crate::exec::{ExecError, ExecResult, ExecReturnValue, StatusCode};
pub use crate::gas::{Gas, GasMeter};

use codec::{Codec, Decode, Encode};
use primitives::crypto::UncheckedFrom;
use primitives::storage::well_known_keys::CHILD_STORAGE_KEY_PREFIX;
use rstd::{collections::btree_map::BTreeMap, marker::PhantomData, prelude::*};
use runtime_io::blake2_256;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sr_primitives::traits::{Hash, MaybeSerializeDebug, Member, StaticLookup, Zero};
use support::dispatch::{Dispatchable, Result};
use support::{
    decl_event, decl_module, decl_storage, parameter_types, storage::child, Parameter, StorageMap,
    StorageValue,
};
use support::{
    traits::{Get, OnFreeBalanceZero},
    IsSubType,
};
use system::{ensure_root, ensure_signed, RawOrigin};

use xassets::{AssetType, Token};
pub use xr_primitives::XRC20Selector; // re-export
use xsupport::{debug, ensure_with_errorlog, error, info};
#[cfg(feature = "std")]
use xsupport::{token, try_hex_or_str};

pub type CodeHash<T> = <T as system::Trait>::Hash;
pub type TrieId = Vec<u8>;
pub type Selector = [u8; 4];

/// A function that generates an `AccountId` for a contract upon instantiation.
pub trait ContractAddressFor<CodeHash, AccountId> {
    fn contract_address_for(code_hash: &CodeHash, data: &[u8], origin: &AccountId) -> AccountId;
}

/// A function that returns the fee for dispatching a `Call`.
pub trait ComputeDispatchFee<Call, Balance> {
    fn compute_dispatch_fee(call: &Call) -> Option<Balance>;
}

/// Information for managing an acocunt and its sub trie abstraction.
/// This is the required info to cache for an account
#[derive(Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum ContractInfo<T: Trait> {
    Alive(AliveContractInfo<T>),
    Tombstone(TombstoneContractInfo<T>),
}

impl<T: Trait> ContractInfo<T> {
    /// If contract is alive then return some alive info
    pub fn get_alive(self) -> Option<AliveContractInfo<T>> {
        if let ContractInfo::Alive(alive) = self {
            Some(alive)
        } else {
            None
        }
    }
    /// If contract is alive then return some reference to alive info
    pub fn as_alive(&self) -> Option<&AliveContractInfo<T>> {
        if let ContractInfo::Alive(ref alive) = self {
            Some(alive)
        } else {
            None
        }
    }
    /// If contract is alive then return some mutable reference to alive info
    pub fn as_alive_mut(&mut self) -> Option<&mut AliveContractInfo<T>> {
        if let ContractInfo::Alive(ref mut alive) = self {
            Some(alive)
        } else {
            None
        }
    }

    /// If contract is tombstone then return some tombstone info
    pub fn get_tombstone(self) -> Option<TombstoneContractInfo<T>> {
        if let ContractInfo::Tombstone(tombstone) = self {
            Some(tombstone)
        } else {
            None
        }
    }
    /// If contract is tombstone then return some reference to tombstone info
    pub fn as_tombstone(&self) -> Option<&TombstoneContractInfo<T>> {
        if let ContractInfo::Tombstone(ref tombstone) = self {
            Some(tombstone)
        } else {
            None
        }
    }
    /// If contract is tombstone then return some mutable reference to tombstone info
    pub fn as_tombstone_mut(&mut self) -> Option<&mut TombstoneContractInfo<T>> {
        if let ContractInfo::Tombstone(ref mut tombstone) = self {
            Some(tombstone)
        } else {
            None
        }
    }
}

pub type AliveContractInfo<T> = RawAliveContractInfo<
    CodeHash<T>,
    <T as xassets::Trait>::Balance,
    <T as system::Trait>::BlockNumber,
>;

/// Information for managing an account and its sub trie abstraction.
/// This is the required info to cache for an account.
// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Encode, Decode, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct RawAliveContractInfo<CodeHash, Balance, BlockNumber> {
    /// Unique ID for the subtree encoded as a bytes vector.
    pub trie_id: TrieId,
    /// The size of stored value in octet.
    pub storage_size: u32,
    /// The code associated with a given account.
    pub code_hash: CodeHash,
    /// Pay rent at most up to this value.
    pub rent_allowance: Balance,
    /// Last block rent has been payed.
    pub deduct_block: BlockNumber,
    /// Last block child storage has been written.
    pub last_write: Option<BlockNumber>,
}

pub type TombstoneContractInfo<T> =
    RawTombstoneContractInfo<<T as system::Trait>::Hash, <T as system::Trait>::Hashing>;

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Encode, Decode, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct RawTombstoneContractInfo<H, Hasher>(H, PhantomData<Hasher>);

impl<H, Hasher> RawTombstoneContractInfo<H, Hasher>
where
    H: Member
        + MaybeSerializeDebug
        + AsRef<[u8]>
        + AsMut<[u8]>
        + Copy
        + Default
        + rstd::hash::Hash
        + Codec,
    Hasher: Hash<Output = H>,
{
    fn new(storage_root: &[u8], code_hash: H) -> Self {
        let mut buf = Vec::new();
        storage_root.using_encoded(|encoded| buf.extend_from_slice(encoded));
        buf.extend_from_slice(code_hash.as_ref());
        RawTombstoneContractInfo(Hasher::hash(&buf[..]), PhantomData)
    }
}

/// Get a trie id (trie id must be unique and collision resistant depending upon its context).
/// Note that it is different than encode because trie id should be collision resistant
/// (being a proper unique identifier).
pub trait TrieIdGenerator<AccountId> {
    /// Get a trie id for an account, using reference to parent account trie id to ensure
    /// uniqueness of trie id.
    ///
    /// The implementation must ensure every new trie id is unique: two consecutive calls with the
    /// same parameter needs to return different trie id values.
    ///
    /// Also, the implementation is responsible for ensuring that `TrieId` starts with
    /// `:child_storage:`.
    /// TODO: We want to change this, see https://github.com/paritytech/substrate/issues/2325
    fn trie_id(account_id: &AccountId) -> TrieId;
}

/// Get trie id from `account_id`.
pub struct TrieIdFromParentCounter<T: Trait>(PhantomData<T>);

/// This generator uses inner counter for account id and applies the hash over `AccountId +
/// accountid_counter`.
impl<T: Trait> TrieIdGenerator<T::AccountId> for TrieIdFromParentCounter<T>
where
    T::AccountId: AsRef<[u8]>,
{
    fn trie_id(account_id: &T::AccountId) -> TrieId {
        // Note that skipping a value due to error is not an issue here.
        // We only need uniqueness, not sequence.
        let new_seed = AccountCounter::<T>::mutate(|v| {
            *v = v.wrapping_add(1);
            *v
        });

        let mut buf = Vec::new();
        buf.extend_from_slice(account_id.as_ref());
        buf.extend_from_slice(&new_seed.to_le_bytes()[..]);

        // TODO: see https://github.com/paritytech/substrate/issues/2325
        let trie_id = CHILD_STORAGE_KEY_PREFIX
            .iter()
            .chain(b"default:")
            .chain(T::Hashing::hash(&buf[..]).as_ref().iter())
            .cloned()
            .collect::<Vec<_>>();

        debug!(
            "[TrieIdGenerator]|contract:{:?}|new_seed:{:}|trie_id:{:}",
            account_id,
            new_seed,
            try_hex_or_str(&trie_id)
        );
        trie_id
    }
}

parameter_types! {
    /// A reasonable default value for [`Trait::SignedClaimedHandicap`].
    pub const DefaultSignedClaimHandicap: u32 = 2;
    /// A reasonable default value for [`Trait::TombstoneDeposit`].
    pub const DefaultTombstoneDeposit: u32 = 16;
    /// A reasonable default value for [`Trait::StorageSizeOffset`].
    pub const DefaultStorageSizeOffset: u32 = 8;
    /// A reasonable default value for [`Trait::RentByteFee`].
    pub const DefaultRentByteFee: u32 = 4;
    /// A reasonable default value for [`Trait::RentDepositOffset`].
    pub const DefaultRentDepositOffset: u32 = 1000;
    /// A reasonable default value for [`Trait::MaxDepth`].
    pub const DefaultMaxDepth: u32 = 32;
    /// A reasonable default value for [`Trait::MaxValueSize`].
    pub const DefaultMaxValueSize: u32 = 16_384;
    /// A reasonable default value for [`Trait::BlockGasLimit`].
    pub const DefaultBlockGasLimit: u32 = 10_000_000;
}

pub trait Trait:
    system::Trait + timestamp::Trait + xassets::Trait + xaccounts::Trait + xsystem::Trait
{
    /// The outer call dispatch type.
    type Call: Parameter
        + Dispatchable<Origin = <Self as system::Trait>::Origin>
        + IsSubType<Module<Self>>;

    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    /// A function type to get the contract address given the instantiator.
    type DetermineContractAddress: ContractAddressFor<CodeHash<Self>, Self::AccountId>;

    /// A function type that computes the fee for dispatching the given `Call`.
    ///
    /// It is recommended (though not required) for this function to return a fee that would be
    /// taken by the Executive module for regular dispatch.
    type ComputeDispatchFee: ComputeDispatchFee<
        <Self as Trait>::Call,
        <Self as xassets::Trait>::Balance,
    >;

    /// trie id generator
    type TrieIdGenerator: TrieIdGenerator<Self::AccountId>;

    /// Number of block delay an extrinsic claim surcharge has.
    ///
    /// When claim surcharge is called by an extrinsic the rent is checked
    /// for current_block - delay
    type SignedClaimHandicap: Get<Self::BlockNumber>;

    /// The minimum amount required to generate a tombstone.
    type TombstoneDeposit: Get<Self::Balance>;

    /// Size of a contract at the time of instantiation. This is a simple way to ensure
    /// that empty contracts eventually gets deleted.
    type StorageSizeOffset: Get<u32>;

    /// Price of a byte of storage per one block interval. Should be greater than 0.
    type RentByteFee: Get<Self::Balance>;

    /// The amount of funds a contract should deposit in order to offset
    /// the cost of one byte.
    ///
    /// Let's suppose the deposit is 1,000 BU (balance units)/byte and the rent is 1 BU/byte/day,
    /// then a contract with 1,000,000 BU that uses 1,000 bytes of storage would pay no rent.
    /// But if the balance reduced to 500,000 BU and the storage stayed the same at 1,000,
    /// then it would pay 500 BU/day.
    type RentDepositOffset: Get<Self::Balance>;

    /// The maximum nesting level of a call/instantiate stack.
    type MaxDepth: Get<u32>;

    /// The maximum size of a storage value in bytes.
    type MaxValueSize: Get<u32>;

    /// The maximum amount of gas that could be expended per block.
    type BlockGasLimit: Get<Gas>;
}

/// Simple contract address determiner.
///
/// Address calculated from the code (of the constructor), input data to the constructor,
/// and the account id that requested the account creation.
///
/// Formula: `blake2_256(blake2_256(code) + blake2_256(data) + origin)`
pub struct SimpleAddressDeterminer<T: Trait>(PhantomData<T>);
impl<T: Trait> ContractAddressFor<CodeHash<T>, T::AccountId> for SimpleAddressDeterminer<T>
where
    T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
{
    fn contract_address_for(
        code_hash: &CodeHash<T>,
        data: &[u8],
        origin: &T::AccountId,
    ) -> T::AccountId {
        let data_hash = T::Hashing::hash(data);

        let mut buf = Vec::new();
        buf.extend_from_slice(code_hash.as_ref());
        buf.extend_from_slice(data_hash.as_ref());
        buf.extend_from_slice(origin.as_ref());

        UncheckedFrom::unchecked_from(T::Hashing::hash(&buf[..]))
    }
}

decl_module! {
    /// Contracts module.
    pub struct Module<T: Trait> for enum Call where origin: <T as system::Trait>::Origin {
        fn deposit_event<T>() = default;

        /// Updates the schedule for metering contracts.
        ///
        /// The schedule must have a greater version than the stored schedule.
        pub fn update_schedule(origin, schedule: Schedule) -> Result {
            ensure_root(origin)?;
            if <Module<T>>::current_schedule().version >= schedule.version {
                return Err("new schedule must have a greater version than current");
            }

            Self::deposit_event(RawEvent::ScheduleUpdated(schedule.version));
            CurrentSchedule::<T>::put(schedule);

            Ok(())
        }

        /// Stores the given binary Wasm code into the chain's storage and returns its `codehash`.
        /// You can instantiate contracts only with stored code.
        pub fn put_code(
            origin,
            #[compact] gas_limit: Gas,
            code: Vec<u8>
        ) -> Result {
            let origin = ensure_signed(origin)?;

            let (network, _) = xsystem::Module::<T>::network_props();
            match network {
                xsystem::NetworkType::Mainnet => {
                    let council = xaccounts::Module::<T>::council_account();
                    ensure_with_errorlog!(
                        origin == council,
                        "[put_code]|in mainnet, only council account could do `put_code`.",
                        "[put_code]|in mainnet, only council account could do `put_code`|current:{:?}|council:{:?}",
                        origin, council
                    );
                    info!("[put_code]|mainnet put_code, from account:{:?}", origin);
                },
                xsystem::NetworkType::Testnet => {
                    // do nothing
                },
            }

            let mut gas_meter = gas::buy_gas::<T>(&origin, gas_limit)?;

            let schedule = <Module<T>>::current_schedule();
            let result = wasm::save_code::<T>(code, &mut gas_meter, &schedule);
            if let Ok(code_hash) = result {
                info!("[put_code]|set new code|code_hash:{:?}", code_hash);
                Self::deposit_event(RawEvent::CodeStored(code_hash));
            }

            gas::refund_unused_gas::<T>(&origin, gas_meter);

            result.map(|_| ())
        }

        /// Makes a call to an account, optionally transferring some balance.
        ///
        /// * If the account is a smart-contract account, the associated code will be
        /// executed and any value will be transferred.
        /// * If the account is a regular account, any value will be transferred.
        /// * If no account exists and the call value is not less than `existential_deposit`,
        /// a regular account will be created and any value will be transferred.
        pub fn call(
            origin,
            dest: <T::Lookup as StaticLookup>::Source,
            #[compact] value: T::Balance,
            #[compact] gas_limit: Gas,
            data: Vec<u8>
        ) -> Result {
            let origin = ensure_signed(origin)?;
            let dest = T::Lookup::lookup(dest)?;
            debug!("[call]|call contract|from:{:?}|dest:{:?}|value:{:?}|data:{:}", origin, dest, value, try_hex_or_str(&data));

            Self::bare_call(origin, dest.clone(), value, gas_limit, data)
                .and_then(|output| {
                    if output.is_success() {
                        debug!("[call]|call contract success|result:{:}|contract addr:{:?}", try_hex_or_str(&output.data), dest);
                        Ok(()) // just drop output
                    } else {
                        Err(ExecError{
                            reason: "fail to call the contract, please check input_data and contract",
                            buffer: Vec::new(),
                        })
                    }
                })
                .map_err(|e| e.reason)
        }

        /// Instantiates a new contract from the `codehash` generated by `put_code`, optionally transferring some balance.
        ///
        /// Instantiation is executed as follows:
        ///
        /// - The destination address is computed based on the sender and hash of the code.
        /// - The smart-contract account is created at the computed address.
        /// - The `ctor_code` is executed in the context of the newly-created account. Buffer returned
        ///   after the execution is saved as the `code` of the account. That code will be invoked
        ///   upon any call received by this account.
        /// - The contract is initialized.
        pub fn instantiate(
            origin,
            #[compact] endowment: T::Balance,
            #[compact] gas_limit: Gas,
            code_hash: CodeHash<T>,
            data: Vec<u8>
        ) -> Result {
            let origin = ensure_signed(origin)?;
            info!("[instantiate]|create new contract|from:{:?}|endowment:{:}|code_hash:{:?}|data:{:}", origin, endowment, code_hash, try_hex_or_str(&data));
            Self::execute_wasm(origin, None, gas_limit, |ctx, gas_meter| {
                ctx.instantiate(endowment, gas_meter, &code_hash, data)
                    .map(|(_address, output)| {
                        if output.is_success() {
                            info!("[instantiate]|succeed to create contract:{:?}", _address);
                        } else {
                            info!("[instantiate]|fail to create contract:{:?}|status:{:}|data:{:?}", _address, output.status, try_hex_or_str(&output.data));
                        }
                        output
                    })
            })
            .and_then(|output| {
                if output.is_success() {
                    Ok(()) // just drop output
                } else {
                    Err(ExecError{
                        reason: "fail to create contract, maybe instantiate data decode error",
                        buffer: Vec::new(),
                    })
                }
            })
            .map_err(|e| e.reason)
        }

        /// Allows block producers to claim a small reward for evicting a contract. If a block producer
        /// fails to do so, a regular users will be allowed to claim the reward.
        ///
        /// If contract is not evicted as a result of this call, no actions are taken and
        /// the sender is not eligible for the reward.
        fn claim_surcharge(origin, _dest: T::AccountId, aux_sender: Option<T::AccountId>) {
            let origin = origin.into();
            let (signed, _rewarded) = match (origin, aux_sender) {
                (Ok(system::RawOrigin::Signed(account)), None) => {
                    (true, account)
                },
                (Ok(system::RawOrigin::None), Some(aux_sender)) => {
                    (false, aux_sender)
                },
                _ => return Err(
                    "Invalid surcharge claim: origin must be signed or \
                    inherent and auxiliary sender only provided on inherent"
                ),
            };

            // Add some advantage for block producers (who send unsigned extrinsics) by
            // adding a handicap: for signed extrinsics we use a slightly older block number
            // for the eviction check. This can be viewed as if we pushed regular users back in past.
            let _handicap = if signed {
                T::SignedClaimHandicap::get()
            } else {
                Zero::zero()
            };

            // If poking the contract has lead to eviction of the contract, give out the rewards.
            // if rent::try_evict::<T>(&dest, handicap) == rent::RentOutcome::Evicted {
            //     T::Currency::deposit_into_existing(&rewarded, T::SurchargeReward::get())?;
            // }
        }

        /// Set gas price by root
        pub fn set_gas_price(#[compact] price: T::Balance) {
            info!("[set_gas_price]|set new gas price:{:}", price);
            GasPrice::<T>::mutate(|p| *p = price);
        }

        /// Enable of Off println for contract. Just for debug.
        pub fn set_println(state: bool) {
            CurrentSchedule::<T>::mutate(|s| {
                s.enable_println = state;
            });
        }

        // xrc20 and runtime assets
        /// Convert asset balance to xrc20 token. This function would call xrc20 `issue` interface.
        /// The gas cast would deduct the caller.
        pub fn convert_to_xrc20(origin, token: Token, #[compact] value: T::Balance, #[compact] gas_limit: Gas) -> Result {
            let origin = ensure_signed(origin)?;
            Self::issue_to_xrc20(token, origin, value, gas_limit)
        }

        /// Convert xrc20 token to asset balance. This function could not be called from an extrinsic,
        /// just could be called inside the xrc20, XRC777 and etc contract instance.
        pub fn convert_to_asset(origin, to: T::AccountId, #[compact] value: T::Balance) -> Result {
            let origin = ensure_signed(origin)?;
            // check token xrc20 is exist
            Self::refund_to_asset(origin, to, value)
        }

        /// Set the xrc20 addr and selectors for a token name.
        pub fn set_token_xrc20(token: Token, xrc20_addr: T::AccountId, selectors: BTreeMap<XRC20Selector, Selector>) {
            XRC20InfoOfToken::<T>::insert(token.clone(), (xrc20_addr.clone(), selectors));
            TokenOfAddr::<T>::insert(xrc20_addr, token);
        }

        /// Set the xrc20 selectors for a token name.
        pub fn set_xrc20_selector(token: Token, selectors: BTreeMap<XRC20Selector, Selector>) {
            XRC20InfoOfToken::<T>::mutate(token, |info| {
                if let Some(ref mut data) = info {
                    data.1 = selectors;
                }
            })
        }

        /// Remove xrc20 relationship for a token name.
        pub fn remove_token_xrc20(token: Token) {
            if let Some(info) = XRC20InfoOfToken::<T>::take(&token) {
                let _ = TokenOfAddr::<T>::take(info.0);
            }
        }

        /// Force issue xrc20 token.
        pub fn force_issue_xrc20(token: Token, issues: Vec<(T::AccountId, T::Balance)>, gas_limit: Gas) -> Result {
            for (origin, value)  in issues {
                let params = (origin.clone(), value).encode();

                if let Err(_e) = Self::call_for_xrc20(token.clone(), origin.clone(), gas_limit, XRC20Selector::Issue, params.clone()) {
                    error!("[force_issue_xrc20]|{:}|who:{:?}|value:{:}|gas_limit:{:}|params:{:}", _e.reason, origin, value, gas_limit, try_hex_or_str(&params))
                }
            }
            Ok(())
        }

        fn on_finalize() {
            GasSpent::<T>::kill();
        }
    }
}

/// The possible errors that can happen querying the storage of a contract.
pub enum GetStorageError {
    /// The given address doesn't point on a contract.
    ContractDoesntExist,
    /// The specified contract is a tombstone and thus cannot have any storage.
    IsTombstone,
}

/// Public APIs provided by the contracts module.
impl<T: Trait> Module<T> {
    /// Perform a call to a specified contract.
    ///
    /// This function is similar to `Self::call`, but doesn't perform any address lookups and better
    /// suitable for calling directly from Rust.
    pub fn bare_call(
        origin: T::AccountId,
        dest: T::AccountId,
        value: T::Balance,
        gas_limit: Gas,
        input_data: Vec<u8>,
    ) -> ExecResult {
        if <ContractInfoOf<T>>::get(&dest).is_none() {
            return Err(ExecError {
                reason: "unable to call dest contract as it does not exist",
                buffer: input_data,
            });
        }
        Self::execute_wasm(origin, None, gas_limit, |ctx, gas_meter| {
            ctx.call(dest, value, gas_meter, input_data)
        })
    }

    /// Query storage of a specified contract under a specified key.
    pub fn get_storage(
        address: T::AccountId,
        key: [u8; 32],
    ) -> rstd::result::Result<Option<Vec<u8>>, GetStorageError> {
        let contract_info = <ContractInfoOf<T>>::get(&address)
            .ok_or(GetStorageError::ContractDoesntExist)?
            .get_alive()
            .ok_or(GetStorageError::IsTombstone)?;

        let maybe_value = AccountDb::<T>::get_storage(
            &DirectAccountDb,
            &address,
            Some(&contract_info.trie_id),
            &key,
        );
        Ok(maybe_value)
    }

    /// Query a call to a specified xrc20 token.
    /// notice this function just allow to be called in runtime api, not allow in an extrinsic
    pub fn call_xrc20(
        token: Token,
        pay_gas: T::AccountId,
        gas_limit: Gas,
        selector: XRC20Selector,
        data: Vec<u8>,
    ) -> ExecResult {
        match selector {
            XRC20Selector::Issue | XRC20Selector::Destroy => {
                return Err(ExecError {
                    reason: "not allow selector 'Issue' or `Destroy` in call_xrc20",
                    buffer: Vec::new(),
                })
            }
            _ => {}
        }

        Self::call_for_xrc20(token, pay_gas, gas_limit, selector, data)
    }

    fn issue_to_xrc20(
        token: Token,
        origin: T::AccountId,
        value: T::Balance,
        gas_limit: Gas,
    ) -> Result {
        // check
        ensure_with_errorlog!(
            xassets::Module::<T>::free_balance_of(&origin, &token) >= value,
            "not enough balance for this token to convert to xrc20 token",
            "token:{:}|who:{:?}|value:{:}",
            token!(token),
            origin,
            value
        );

        let params = (origin.clone(), value).encode();

        // call xrc20 contract to issue xrc20 token
        let exec_value = Self::call_for_xrc20(
            token.clone(),
            origin.clone(),
            gas_limit,
            XRC20Selector::Issue,
            params,
        )
        .and_then(|output| {
            if output.is_success() {
                Ok(output)
            } else {
                Err(ExecError {
                    reason: "fail to call the contract, please check params and xrc20",
                    buffer: Vec::new(),
                })
            }
        })
        .map_err(|e| e.reason)?;

        // notice when standard xrc20 return chech, this decode method should also change
        let result: bool = Decode::decode(&mut exec_value.data.as_slice()).ok_or_else(|| {
            error!(
                "[issue_to_xrc20]|fail to decode wasm result|data:{:}",
                try_hex_or_str(&exec_value.data)
            );
            "fail decode wasm result to bool"
        })?;
        if !result {
            return Err("fail to issue token in xrc20 contract");
        }

        let xrc20_addr = Self::xrc20_of_token(&token)
            .expect("xrc20 info must be existed at here")
            .0;
        // success, transfer to the xrc20 contract
        let _ = xassets::Module::<T>::move_balance(
            &token,
            &origin,
            AssetType::Free,
            &xrc20_addr,
            AssetType::ReservedXRC20,
            value,
        )
        .map_err(|e| e.info())?;
        Ok(())
    }

    fn call_for_xrc20(
        token: Token,
        pay_gas: T::AccountId,
        gas_limit: Gas,
        enum_selector: XRC20Selector,
        input_data: Vec<u8>,
    ) -> ExecResult {
        let info = Self::xrc20_of_token(&token).ok_or_else(|| {
            error!("no xrc20 instance for this token|token:{:}", token!(token));
            ExecError {
                reason: "no xrc20 instance for this token",
                buffer: Vec::new(),
            }
        })?;
        let xrc20_addr = info.0;
        let selectors = info.1;
        let selector = selectors.get(&enum_selector).ok_or_else(|| {
            error!(
                "no issue selector in xrc20 info for this token|token:{:}",
                token!(token)
            );
            ExecError {
                reason: "no issue selector in xrc20 info for this token",
                buffer: Vec::new(),
            }
        })?;

        let mut data = selector.to_vec(); // provide selector
        data.extend_from_slice(input_data.as_slice());

        debug!("[call_for_xrc20]|call xrc20 instance|token:{:}|xrc20:{:?}|pay gas:{:?}|selector:{:?}|data:{:}",
            token!(token), xrc20_addr, pay_gas, enum_selector, try_hex_or_str(&data));

        Self::execute_wasm(
            xrc20_addr.clone(),
            Some(pay_gas),
            gas_limit,
            |ctx, gas_meter| ctx.call(xrc20_addr.clone(), Zero::zero(), gas_meter, data),
        )
    }

    fn refund_to_asset(contract_addr: T::AccountId, to: T::AccountId, value: T::Balance) -> Result {
        let token: Token = Self::token_of_addr(&contract_addr).ok_or_else(|| {
            error!(
                "no token for this xrc20 address|xrc20 addr:{:?}",
                contract_addr
            );
            "no token for this xrc20 address"
        })?;
        let current_reserved = xassets::Module::<T>::asset_balance_of(
            &contract_addr,
            &token,
            AssetType::ReservedXRC20,
        );
        ensure_with_errorlog!(
            current_reserved >= value,
            "not enough balance for this xrc20 instance to refund asset",
            "token:{:}|xrc20:{:?}|value:{:}|current:{:}",
            token!(token),
            contract_addr,
            value,
            current_reserved
        );

        // success, refund asset to this account
        let _ = xassets::Module::<T>::move_balance(
            &token,
            &contract_addr,
            AssetType::ReservedXRC20,
            &to,
            AssetType::Free,
            value,
        )
        .map_err(|e| e.info())?;
        Ok(())
    }
}

impl<T: Trait> Module<T> {
    fn execute_wasm(
        origin: T::AccountId,
        buy_gas_account: Option<T::AccountId>,
        gas_limit: Gas,
        func: impl FnOnce(&mut ExecutionContext<T, WasmVm, WasmLoader>, &mut GasMeter<T>) -> ExecResult,
    ) -> ExecResult {
        // Pay for the gas upfront.
        //
        // NOTE: it is very important to avoid any state changes before
        // paying for the gas.
        let pay_gas = buy_gas_account.unwrap_or(origin.clone());
        let mut gas_meter = try_or_exec_error!(
            gas::buy_gas::<T>(&pay_gas, gas_limit),
            // We don't have a spare buffer here in the first place, so create a new empty one.
            Vec::new()
        );

        let cfg = Config::preload();
        let vm = WasmVm::new(&cfg.schedule);
        let loader = WasmLoader::new(&cfg.schedule);
        let mut ctx = ExecutionContext::top_level(origin.clone(), &cfg, &vm, &loader);

        let result = func(&mut ctx, &mut gas_meter);

        if result
            .as_ref()
            .map(|output| output.is_success())
            .unwrap_or(false)
        {
            // Commit all changes that made it thus far into the persistent storage.
            DirectAccountDb.commit(ctx.overlay.into_change_set());
        }

        // Refund cost of the unused gas.
        //
        // NOTE: This should go after the commit to the storage, since the storage changes
        // can alter the balance of the caller.
        gas::refund_unused_gas::<T>(&pay_gas, gas_meter);

        // Execute deferred actions.
        ctx.deferred.into_iter().for_each(|deferred| {
            use self::exec::DeferredAction::*;
            match deferred {
                DepositEvent { topics, event } => {
                    debug!(
                        "[deferred_deposit_event]topics:{:?}|event:{:?}",
                        topics, event
                    );
                    <system::Module<T>>::deposit_event_indexed(
                        &*topics,
                        <T as Trait>::Event::from(event).into(),
                    );
                }
                DispatchRuntimeCall { origin: who, call } => {
                    debug!(
                        "[deferred_dispatch_runtime_call]origin:{:?}|call:{:?}",
                        who, call
                    );
                    let result = call.dispatch(RawOrigin::Signed(who.clone()).into());
                    Self::deposit_event(RawEvent::Dispatched(who, result.is_ok()));
                }
                RestoreTo {
                    donor,
                    dest,
                    code_hash,
                    rent_allowance,
                    delta,
                } => {
                    let _result = Self::restore_to(donor, dest, code_hash, rent_allowance, delta);
                }
            }
        });

        result
    }

    fn restore_to(
        origin: T::AccountId,
        dest: T::AccountId,
        code_hash: CodeHash<T>,
        rent_allowance: T::Balance,
        delta: Vec<exec::StorageKey>,
    ) -> Result {
        let mut origin_contract = <ContractInfoOf<T>>::get(&origin)
            .and_then(|c| c.get_alive())
            .ok_or("Cannot restore from inexisting or tombstone contract")?;

        let current_block = <system::Module<T>>::block_number();

        if origin_contract.last_write == Some(current_block) {
            return Err("Origin TrieId written in the current block");
        }

        let dest_tombstone = <ContractInfoOf<T>>::get(&dest)
            .and_then(|c| c.get_tombstone())
            .ok_or("Cannot restore to inexisting or alive contract")?;

        let last_write = if !delta.is_empty() {
            Some(current_block)
        } else {
            origin_contract.last_write
        };

        let key_values_taken = delta
            .iter()
            .filter_map(|key| {
                child::get_raw(&origin_contract.trie_id, &blake2_256(key)).map(|value| {
                    child::kill(&origin_contract.trie_id, &blake2_256(key));
                    (key, value)
                })
            })
            .collect::<Vec<_>>();

        let tombstone = <TombstoneContractInfo<T>>::new(
            // This operation is cheap enough because last_write (delta not included)
            // is not this block as it has been checked earlier.
            &runtime_io::child_storage_root(&origin_contract.trie_id)[..],
            code_hash,
        );

        if tombstone != dest_tombstone {
            for (key, value) in key_values_taken {
                child::put_raw(&origin_contract.trie_id, &blake2_256(key), &value);
            }

            return Err("Tombstones don't match");
        }

        origin_contract.storage_size -= key_values_taken
            .iter()
            .map(|(_, value)| value.len() as u32)
            .sum::<u32>();

        <ContractInfoOf<T>>::remove(&origin);
        <ContractInfoOf<T>>::insert(
            &dest,
            ContractInfo::Alive(RawAliveContractInfo {
                trie_id: origin_contract.trie_id,
                storage_size: origin_contract.storage_size,
                code_hash,
                rent_allowance,
                deduct_block: current_block,
                last_write,
            }),
        );

        let all_value = xassets::Module::<T>::pcx_free_balance(&origin);
        xassets::Module::<T>::pcx_move_free_balance(&origin, &dest, all_value)
            .map_err(|e| e.info())?;

        Ok(())
    }

    fn transfer_to_council(slashed_account: &T::AccountId, value: T::Balance) {
        let council = xaccounts::Module::<T>::council_account();
        let _ = <xassets::Module<T>>::pcx_move_free_balance(&slashed_account, &council, value);
    }
}

decl_event! {
    pub enum Event<T>
    where
        <T as xassets::Trait>::Balance,
        <T as system::Trait>::AccountId,
        <T as system::Trait>::Hash
    {
        /// Transfer happened `from` to `to` with given `value` as part of a `call` or `instantiate`.
        Transfer(AccountId, AccountId, Balance),

        /// Contract deployed by address at the specified address.
        Instantiated(AccountId, AccountId),

        /// Code with the specified hash has been stored.
        CodeStored(Hash),

        /// Triggered when the current schedule is updated.
        ScheduleUpdated(u32),

        /// A call was dispatched from the given account. The bool signals whether it was
        /// successful execution or not.
        Dispatched(AccountId, bool),

        /// An event deposited upon execution of a contract from the account.
        ContractExecution(AccountId, Vec<u8>),
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as XContracts {
        /// Gas spent so far in this block.
        GasSpent get(gas_spent): Gas;
        /// Current cost schedule for contracts.
        CurrentSchedule get(current_schedule) config(): Schedule = Schedule::default();
        /// A mapping from an original code hash to the original code, untouched by instrumentation.
        pub PristineCode: map CodeHash<T> => Option<Vec<u8>>;
        /// A mapping between an original code hash and instrumented wasm code, ready for execution.
        pub CodeStorage: map CodeHash<T> => Option<wasm::PrefabWasmModule>;
        /// The subtrie counter.
        pub AccountCounter: u64 = 0;
        /// The code associated with a given account.
        pub ContractInfoOf: map T::AccountId => Option<ContractInfo<T>>;
        /// The price of one unit of gas.
        pub GasPrice get(gas_price) config(): T::Balance = 5.into();

        // ChainX modify
        // the map of token and token contract instance
        // addr <--xrc20--> token
        // addr <---XRC777---^

        /// The Token name of a token contract instance address.
        /// notice the address could be xrc20, XRC777, or other type contract
        pub TokenOfAddr get(token_of_addr): map T::AccountId => Option<Token>;
        // xrc20
        /// The XRC20 contract of a token name.
        pub XRC20InfoOfToken get(xrc20_of_token): map Token => Option<(T::AccountId, BTreeMap<XRC20Selector, Selector>)>;
        // XRC777 (in future)
    }
}

impl<T: Trait> OnFreeBalanceZero<T::AccountId> for Module<T> {
    fn on_free_balance_zero(who: &T::AccountId) {
        if let Some(ContractInfo::Alive(info)) = <ContractInfoOf<T>>::take(who) {
            child::kill_storage(&info.trie_id);
        }
    }
}

/// In-memory cache of configuration values.
///
/// We assume that these values can't be changed in the
/// course of transaction execution.
pub struct Config<T: Trait> {
    pub schedule: Schedule,
    pub existential_deposit: T::Balance,
    pub max_depth: u32,
    pub max_value_size: u32,
    pub contract_account_instantiate_fee: T::Balance,
    pub account_create_fee: T::Balance,
    pub transfer_fee: T::Balance,
}

impl<T: Trait> Config<T> {
    fn preload() -> Config<T> {
        let existential_deposit = {
            #[cfg(not(test))]
            {
                T::Balance::zero()
            }
            #[cfg(test)]
            {
                use tests::ExistentialDeposit;
                ExistentialDeposit::get().into()
            }
        };

        Config {
            schedule: <Module<T>>::current_schedule(),
            existential_deposit,
            max_depth: T::MaxDepth::get(),
            max_value_size: T::MaxValueSize::get(),
            contract_account_instantiate_fee: T::Balance::zero(),
            account_create_fee: T::Balance::zero(),
            transfer_fee: T::Balance::zero(),
        }
    }
}

/// Definition of the cost schedule and other parameterizations for wasm vm.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
#[derive(Clone, Encode, Decode, PartialEq, Eq)]
pub struct Schedule {
    /// Version of the schedule.
    pub version: u32,

    /// Cost of putting a byte of code into storage.
    pub put_code_per_byte_cost: Gas,

    /// Gas cost of a growing memory by single page.
    pub grow_mem_cost: Gas,

    /// Gas cost of a regular operation.
    pub regular_op_cost: Gas,

    /// Gas cost per one byte returned.
    pub return_data_per_byte_cost: Gas,

    /// Gas cost to deposit an event; the per-byte portion.
    pub event_data_per_byte_cost: Gas,

    /// Gas cost to deposit an event; the cost per topic.
    pub event_per_topic_cost: Gas,

    /// Gas cost to deposit an event; the base.
    pub event_base_cost: Gas,

    /// Base gas cost to call into a contract.
    pub call_base_cost: Gas,

    /// Base gas cost to instantiate a contract.
    pub instantiate_base_cost: Gas,

    /// Gas cost per one byte read from the sandbox memory.
    pub sandbox_data_read_cost: Gas,

    /// Gas cost per one byte written to the sandbox memory.
    pub sandbox_data_write_cost: Gas,

    /// The maximum number of topics supported by an event.
    pub max_event_topics: u32,

    /// Maximum allowed stack height.
    ///
    /// See https://wiki.parity.io/WebAssembly-StackHeight to find out
    /// how the stack frame cost is calculated.
    pub max_stack_height: u32,

    /// Maximum number of memory pages allowed for a contract.
    pub max_memory_pages: u32,

    /// Maximum allowed size of a declared table.
    pub max_table_size: u32,

    /// Whether the `ext_println` function is allowed to be used contracts.
    /// MUST only be enabled for `dev` chains, NOT for production chains
    pub enable_println: bool,

    /// The maximum length of a subject used for PRNG generation.
    pub max_subject_len: u32,
}

impl Default for Schedule {
    fn default() -> Schedule {
        if cfg!(test) {
            Schedule {
                version: 0,
                put_code_per_byte_cost: 1,
                grow_mem_cost: 1,
                regular_op_cost: 1,
                return_data_per_byte_cost: 1,
                event_data_per_byte_cost: 1,
                event_per_topic_cost: 1,
                event_base_cost: 1,
                call_base_cost: 135,
                instantiate_base_cost: 175,
                sandbox_data_read_cost: 1,
                sandbox_data_write_cost: 1,
                max_event_topics: 4,
                max_stack_height: 64 * 1024,
                max_memory_pages: 16,
                max_table_size: 16 * 1024,
                enable_println: false,
                max_subject_len: 32,
            }
        } else {
            Schedule {
                version: 0,
                put_code_per_byte_cost: 200,
                grow_mem_cost: 1,
                regular_op_cost: 1,
                return_data_per_byte_cost: 1,
                event_data_per_byte_cost: 20,
                event_per_topic_cost: 1,
                event_base_cost: 1,
                call_base_cost: 60000,
                instantiate_base_cost: 200000,
                sandbox_data_read_cost: 1,
                sandbox_data_write_cost: 1,
                max_event_topics: 4,
                max_stack_height: 64 * 1024,
                max_memory_pages: 16,
                max_table_size: 16 * 1024,
                enable_println: false,
                max_subject_len: 32,
            }
        }
    }
}
