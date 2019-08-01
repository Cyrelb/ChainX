// Copyright 2018-2019 Chainpool.
//! Virtual mining for holding tokens.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod cross_mining;
mod mock;
mod tests;
pub mod types;

// Substrate
use primitives::traits::{As, Zero};
use rstd::{prelude::*, result};
use support::{
    decl_event, decl_module, decl_storage, dispatch::Result, ensure, StorageMap, StorageValue,
};

// ChainX
use xassets::{AssetErr, AssetType, ChainT, Token, TokenJackpotAccountIdFor};
use xassets::{OnAssetChanged, OnAssetRegisterOrRevoke};
use xstaking::{ClaimType, VoteWeight};
#[cfg(feature = "std")]
use xsupport::token;
use xsupport::{debug, ensure_with_errorlog, warn};

pub use self::types::*;

pub trait Trait:
    xsystem::Trait
    + xstaking::Trait
    + xspot::Trait
    + xsdot::Trait
    + xbridge_common::Trait
    + xbitcoin::lockup::Trait
{
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_event!(
    pub enum Event<T> where <T as xassets::Trait>::Balance, <T as system::Trait>::AccountId {
        DepositorReward(AccountId, Token, Balance),
        DepositorClaim(AccountId, Token, u64, u64, Balance),
    }
);

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;

        fn claim(origin, token: Token) {
            let who = system::ensure_signed(origin)?;

            ensure!(
                <xassets::Module<T> as ChainT>::TOKEN.to_vec() != token,
                "Cannot claim from native asset via tokens module."
            );
            ensure!(
                Self::psedu_intentions().contains(&token),
                "Cannot claim from unsupport token."
            );

            debug!("[claim] who: {:?}, token: {:?}", who, token!(token));
            let key = (who.clone(), token.clone());

            let mut p = <PseduIntentionProfiles<T>>::get(&token);
            let mut d = Self::deposit_records(&key);

            let mut prof = PseduIntentionProfs::<T>::new(&token, &mut p);
            let mut record = DepositRecord::<T>::new(&who, &token, &mut d);

            let jackpot = T::DetermineTokenJackpotAccountId::accountid_for_unsafe(&token);

            let (source_vote_weight, target_vote_weight, dividend) =
                <xstaking::Module<T>>::compute_dividend(&mut record, &mut prof, &jackpot)?;

            let current_block = <system::Module<T>>::block_number();

            Self::can_claim(&who, &token, dividend, current_block)?;

            <xstaking::Module<T>>::claim_transfer(ClaimType::PseduIntention(token.clone()), &jackpot, &who, dividend)?;

            record.set_state_on_claim(0, current_block);
            prof.set_state_on_claim(target_vote_weight - source_vote_weight, current_block);

            <DepositRecords<T>>::insert(&key, d);
            <PseduIntentionProfiles<T>>::insert(&token, p);

            <LastClaimOf<T>>::insert(&key, current_block);

            Self::deposit_event(RawEvent::DepositorClaim(
                who,
                token,
                source_vote_weight,
                target_vote_weight,
                dividend,
            ));

        }

        /// Set the discount for converting the cross-chain asset to PCX based on the market value.
        fn set_token_discount(token: Token, value: u32) {
            ensure!(value <= 100, "TokenDiscount cannot exceed 100.");
            <TokenDiscount<T>>::insert(token, value);
        }

        /// Set the reward for the newly issued cross-chain assets.
        fn set_deposit_reward(value: T::Balance) {
            DepositReward::<T>::put(value);
        }

        fn set_claim_restriction(token: Token, new: (u32, T::BlockNumber)) {
            <ClaimRestrictionOf<T>>::insert(token, new);
        }
    }
}

/// 302_400 blocks per week.
pub const BLOCKS_PER_WEEK: u64 = 60 * 60 * 24 * 7 / 2;

decl_storage! {
    trait Store for Module<T: Trait> as XTokens {
        pub TokenDiscount get(token_discount) build(|config: &GenesisConfig<T>| {
            config.token_discount.clone()
        }): map Token => u32;

        /// Cross-chain assets that are able to participate in the assets mining.
        pub PseduIntentions get(psedu_intentions) : Vec<Token>;

        pub ClaimRestrictionOf get(claim_restriction_of): map Token => (u32, T::BlockNumber) = (10u32, T::BlockNumber::sa(BLOCKS_PER_WEEK));

        /// Block height of last claim for some cross miner per token.
        pub LastClaimOf get(last_claim_of): map (T::AccountId, Token) => Option<T::BlockNumber>;

        pub PseduIntentionProfiles get(psedu_intention_profiles): map Token => PseduIntentionVoteWeight<T::BlockNumber>;

        pub DepositRecords get(deposit_records): map (T::AccountId, Token) => DepositVoteWeight<T::BlockNumber>;

        /// when deposit success, reward some pcx to user for claiming. Default is 100000 = 0.001 PCX; 0.001*100000000
        pub DepositReward get(deposit_reward): T::Balance = As::sa(100_000);
    }

    add_extra_genesis {
        config(token_discount): Vec<(Token, u32)>;
    }
}

impl<T: Trait> OnAssetChanged<T::AccountId, T::Balance> for Module<T> {
    fn on_move_before(
        token: &Token,
        from: &T::AccountId,
        _: AssetType,
        to: &T::AccountId,
        _: AssetType,
        _value: T::Balance,
    ) {
        // Exclude PCX and asset type changes on same account.
        if <xassets::Module<T> as ChainT>::TOKEN == token.as_slice() || from.clone() == to.clone() {
            return;
        }

        Self::try_init_receiver_vote_weight(to, token);

        Self::update_depositor_vote_weight_only(from, token);
        Self::update_depositor_vote_weight_only(to, token);
    }

    fn on_move(
        _token: &Token,
        _from: &T::AccountId,
        _: AssetType,
        _to: &T::AccountId,
        _: AssetType,
        _value: T::Balance,
    ) -> result::Result<(), AssetErr> {
        Ok(())
    }

    fn on_issue_before(target: &Token, source: &T::AccountId) {
        // Exclude PCX
        if <xassets::Module<T> as ChainT>::TOKEN == target.as_slice() {
            return;
        }

        Self::try_init_receiver_vote_weight(source, target);

        debug!(
            "[on_issue_before] deposit_records: ({:?}, {:?}) = {:?}",
            token!(target),
            source,
            Self::deposit_records((source.clone(), target.clone()))
        );

        Self::update_bare_vote_weight(source, target);
    }

    fn on_issue(target: &Token, source: &T::AccountId, value: T::Balance) -> Result {
        // Exclude PCX
        if <xassets::Module<T> as ChainT>::TOKEN == target.as_slice() {
            return Ok(());
        }

        debug!(
            "[on_issue] token: {:?}, who: {:?}, vlaue: {:?}",
            token!(target),
            source,
            value
        );

        Self::issue_reward(source, target, value)
    }

    fn on_destroy_before(target: &Token, source: &T::AccountId) {
        Self::update_bare_vote_weight(source, target);
    }

    fn on_destroy(_target: &Token, _source: &T::AccountId, _value: T::Balance) -> Result {
        Ok(())
    }
}

impl<T: Trait> Module<T> {
    pub fn last_claim(who: &T::AccountId, token: &Token) -> Option<T::BlockNumber> {
        Self::last_claim_of(&(who.clone(), token.clone()))
    }

    /// This rule doesn't take effect if the interval is zero.
    fn passed_enough_interval(
        who: &T::AccountId,
        token: &Token,
        interval: T::BlockNumber,
        current_block: T::BlockNumber,
    ) -> Result {
        if !interval.is_zero() {
            if let Some(last_claim) = Self::last_claim(who, token) {
                if current_block <= last_claim + interval {
                    return Err("Can only claim once per claim limiting period.");
                }
            }
        }
        Ok(())
    }

    /// This rule doesn't take effect if the staking requirement is zero.
    fn contribute_enough_staking(
        who: &T::AccountId,
        dividend: T::Balance,
        staking_requirement: u32,
    ) -> Result {
        if !staking_requirement.is_zero() {
            let staked = <xassets::Module<T>>::pcx_type_balance(who, AssetType::ReservedStaking);
            if staked < T::Balance::sa(u64::from(staking_requirement)) * dividend {
                warn!(
                    "cannot claim due to the insufficient staking, current dividend: {:?}, current staking: {:?}, required staking: {:?}",
                    dividend,
                    staked,
                    T::Balance::sa(u64::from(staking_requirement)) * dividend
                );
                return Err("Cannot claim if what you have staked is too little.");
            }
        }
        Ok(())
    }

    /// Whether the claimer is able to claim the dividend at the given height.
    fn can_claim(
        who: &T::AccountId,
        token: &Token,
        dividend: T::Balance,
        current_block: T::BlockNumber,
    ) -> Result {
        let (staking_requirement, interval) = Self::claim_restriction_of(token);
        Self::contribute_enough_staking(who, dividend, staking_requirement)?;
        Self::passed_enough_interval(who, token, interval, current_block)?;
        Ok(())
    }

    /// Ensure the vote weight of some depositor or transfer receiver is initialized.
    fn try_init_receiver_vote_weight(who: &T::AccountId, token: &Token) {
        let key = (who.clone(), token.clone());
        if !<DepositRecords<T>>::exists(&key) {
            <DepositRecords<T>>::insert(
                &key,
                DepositVoteWeight::new(0, <system::Module<T>>::block_number()),
            );
        }
    }

    fn issue_reward(source: &T::AccountId, token: &Token, _value: T::Balance) -> Result {
        ensure_with_errorlog!(
            Self::psedu_intentions().contains(&token),
            "Cannot issue deposit reward since this token is not a psedu intention.",
            "Cannot issue deposit reward since this token is not a psedu intention.|token:{:}",
            token!(token)
        );

        // when deposit(issue) success, reward some pcx for account to claim
        let reward_value = Self::deposit_reward();
        xbridge_common::Module::<T>::reward_from_jackpot(token, source, reward_value);

        Self::deposit_event(RawEvent::DepositorReward(
            source.clone(),
            token.clone(),
            reward_value,
        ));

        Ok(())
    }

    fn update_depositor_vote_weight_only(from: &T::AccountId, target: &Token) {
        let key = (from.clone(), target.clone());
        let mut d = Self::deposit_records(&key);
        let mut record = DepositRecord::<T>::new(from, target, &mut d);

        <xstaking::Module<T>>::generic_update_vote_weight(&mut record);

        <DepositRecords<T>>::insert(&key, d);
    }

    fn update_bare_vote_weight(source: &T::AccountId, target: &Token) {
        let key = (source.clone(), target.clone());
        let mut p = <PseduIntentionProfiles<T>>::get(target);
        let mut d = Self::deposit_records(&key);

        let mut prof = PseduIntentionProfs::<T>::new(target, &mut p);
        let mut record = DepositRecord::<T>::new(source, target, &mut d);

        <xstaking::Module<T>>::update_bare_vote_weight_both_way(&mut prof, &mut record);

        <PseduIntentionProfiles<T>>::insert(target, p);
        <DepositRecords<T>>::insert(&key, d);
    }

    #[cfg(feature = "std")]
    pub fn bootstrap_update_vote_weight(source: &T::AccountId, target: &Token) {
        Self::try_init_receiver_vote_weight(source, target);
        Self::update_bare_vote_weight(source, target);
    }
}

impl<T: Trait> Module<T> {
    pub fn token_jackpot_accountid_for_unsafe(token: &Token) -> T::AccountId {
        T::DetermineTokenJackpotAccountId::accountid_for_unsafe(token)
    }

    pub fn multi_token_jackpot_accountid_for_unsafe(tokens: &[Token]) -> Vec<T::AccountId> {
        tokens
            .iter()
            .map(|t| T::DetermineTokenJackpotAccountId::accountid_for_unsafe(t))
            .collect()
    }
}
