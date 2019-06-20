// Copyright 2018-2019 Chainpool.
//! Tests for the module.

#![cfg(test)]

use super::mock::*;
use super::*;

use primitives::testing::UintAuthorityId;
use runtime_io::with_externalities;
use support::{assert_noop, assert_ok};

#[test]
fn register_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(1);
        XSession::check_rotate_session(System::block_number());

        assert_ok!(XStaking::register(Origin::signed(1), b"name".to_vec(),));

        assert_noop!(
            XStaking::register(Origin::signed(1), b"name".to_vec(),),
            "Cannot register if transactor is an intention already."
        );
    });
}

#[test]
fn register_an_existing_name_should_not_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(1);
        XSession::check_rotate_session(System::block_number());

        assert_ok!(XStaking::register(Origin::signed(1), b"name".to_vec(),));
        assert_noop!(
            XStaking::register(Origin::signed(2), b"name".to_vec()),
            "This name has already been taken."
        );
    });
}

#[test]
fn refresh_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(1);
        XSession::check_rotate_session(System::block_number());

        assert_ok!(XStaking::register(Origin::signed(1), b"name".to_vec(),));

        assert_ok!(XStaking::refresh(
            Origin::signed(1),
            Some(b"new.name".to_vec()),
            Some(true),
            Some(UintAuthorityId(123).into()),
            None
        ));
        assert_eq!(XAccounts::intention_props_of(&1).is_active, true);
        assert_eq!(XAccounts::intention_props_of(&1).url, b"new.name".to_vec());

        assert_noop!(
            XStaking::refresh(
                Origin::signed(2),
                Some(b"new.url".to_vec()),
                Some(false),
                Some(UintAuthorityId(124).into()),
                None
            ),
            "Cannot refresh if transactor is not an intention."
        );
    });
}

#[test]
fn nominate_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(1);
        XSession::check_rotate_session(System::block_number());

        assert_ok!(XStaking::register(Origin::signed(1), b"name".to_vec(),));

        System::set_block_number(2);
        XSession::check_rotate_session(System::block_number());
        assert_ok!(XStaking::nominate(Origin::signed(2), 1.into(), 15, vec![]));

        assert_eq!(XAssets::pcx_free_balance(&2), 20 - 15);
        assert_eq!(
            XStaking::nomination_record_of(&2, &1),
            NominationRecord {
                nomination: 15,
                last_vote_weight: 0,
                last_vote_weight_update: 2,
                revocations: vec![],
            }
        );
    });
}

#[test]
fn renominate_by_intention_should_not_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(1);
        XSession::check_rotate_session(System::block_number());

        assert_ok!(XStaking::register(Origin::signed(1), b"name".to_vec(),));
        assert_ok!(XStaking::register(Origin::signed(3), b"name3".to_vec(),));

        System::set_block_number(2);
        XSession::check_rotate_session(System::block_number());
        assert_ok!(XStaking::nominate(Origin::signed(1), 1.into(), 5, vec![]));

        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());
        assert_noop!(
            XStaking::renominate(Origin::signed(1), 1.into(), 3.into(), 3, b"memo".to_vec()),
            "Cannot renominate the intention self-bonded."
        );
    });
}

#[test]
fn renominate_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(1);
        XSession::check_rotate_session(System::block_number());

        assert_ok!(XStaking::register(Origin::signed(1), b"name".to_vec(),));
        assert_ok!(XStaking::register(Origin::signed(3), b"name3".to_vec(),));

        System::set_block_number(2);
        XSession::check_rotate_session(System::block_number());
        assert_ok!(XStaking::nominate(Origin::signed(2), 1.into(), 15, vec![]));

        assert_eq!(XAssets::pcx_free_balance(&2), 20 - 15);
        assert_eq!(
            XStaking::nomination_record_of(&2, &1),
            NominationRecord {
                nomination: 15,
                last_vote_weight: 0,
                last_vote_weight_update: 2,
                revocations: vec![],
            }
        );

        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());
        assert_ok!(XStaking::renominate(
            Origin::signed(2),
            1.into(),
            3.into(),
            10,
            b"memo".to_vec()
        ));
        assert_eq!(
            XStaking::nomination_record_of(&2, &1),
            NominationRecord {
                nomination: 5,
                last_vote_weight: 15,
                last_vote_weight_update: 3,
                revocations: vec![],
            }
        );
        assert_eq!(
            XStaking::nomination_record_of(&2, &3),
            NominationRecord {
                nomination: 10,
                last_vote_weight: 0,
                last_vote_weight_update: 3,
                revocations: vec![],
            }
        );

        System::set_block_number(4);
        XSession::check_rotate_session(System::block_number());

        assert_ok!(XStaking::renominate(
            Origin::signed(2),
            1.into(),
            3.into(),
            5,
            b"memo".to_vec()
        ));
        assert_eq!(
            XStaking::nomination_record_of(&2, &1),
            NominationRecord {
                nomination: 0,
                last_vote_weight: 20,
                last_vote_weight_update: 4,
                revocations: vec![],
            }
        );
        assert_eq!(
            XStaking::nomination_record_of(&2, &3),
            NominationRecord {
                nomination: 15,
                last_vote_weight: 10,
                last_vote_weight_update: 4,
                revocations: vec![],
            }
        );
    });
}

#[test]
fn unnominate_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(1);
        XSession::check_rotate_session(System::block_number());

        assert_ok!(XStaking::register(Origin::signed(1), b"name".to_vec(),));
        assert_ok!(XStaking::nominate(Origin::signed(2), 1.into(), 15, vec![]));

        System::set_block_number(2);
        XSession::check_rotate_session(System::block_number());
        assert_noop!(
            XStaking::unnominate(Origin::signed(2), 1.into(), 10_000, vec![]),
            "Cannot unnominate if greater than your revokable nomination."
        );

        System::set_block_number(28801);
        XSession::check_rotate_session(System::block_number());
        assert_noop!(
            XStaking::unnominate(Origin::signed(2), 1.into(), 10_000, vec![]),
            "Cannot unnominate if greater than your revokable nomination."
        );

        System::set_block_number(28802);
        XSession::check_rotate_session(System::block_number());
        assert_ok!(XStaking::unnominate(
            Origin::signed(2),
            1.into(),
            10,
            vec![]
        ));

        assert_eq!(
            XStaking::nomination_record_of(&2, &1),
            NominationRecord {
                nomination: 5,
                last_vote_weight: 432015,
                last_vote_weight_update: 28802,
                revocations: vec![(28803, 10)],
            }
        );

        System::set_block_number(28803);
        XSession::check_rotate_session(System::block_number());
        assert_ok!(XStaking::unnominate(Origin::signed(2), 1.into(), 5, vec![]));

        assert_eq!(
            XStaking::nomination_record_of(&2, &1),
            NominationRecord {
                nomination: 0,
                last_vote_weight: 432020,
                last_vote_weight_update: 28803,
                revocations: vec![(28803, 10), (28804, 5)],
            }
        );
    });
}

#[test]
fn claim_should_work() {
    with_externalities(&mut new_test_ext(), || {
        assert_ok!(XStaking::register(Origin::signed(2), b"name".to_vec(),));
        assert_ok!(XStaking::refresh(
            Origin::signed(2),
            None,
            Some(true),
            None,
            None
        ));
        assert_eq!(XAccounts::intention_props_of(&2).is_active, true);
        assert_eq!(XAssets::pcx_free_balance(&2), 20);
        System::set_block_number(1);
        assert_eq!(XAssets::pcx_free_balance(&2), 20);
        XSession::check_rotate_session(System::block_number());
        assert_eq!(XAssets::pcx_free_balance(&2), 20);
        System::set_block_number(2);
        XSession::check_rotate_session(System::block_number());
        assert_eq!(XAssets::pcx_free_balance(&2), 20);
        assert_ok!(XStaking::nominate(Origin::signed(2), 2.into(), 10, vec![]));
        assert_eq!(XAssets::pcx_free_balance(&2), 10);
        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());
        assert_eq!(XAssets::pcx_free_balance(&2), 400000010);
        assert_ok!(XStaking::claim(Origin::signed(2), 2.into()));
        assert_eq!(XAssets::pcx_free_balance(&2), 4000000010);
    });
}

#[test]
fn offline_should_slash_and_kick() {
    // Test that an offline validator gets slashed and kicked
    with_externalities(&mut new_test_ext(), || {
        assert_eq!(XAssets::pcx_free_balance(&6), 30);
        assert_ok!(XStaking::register(Origin::signed(6), b"name".to_vec(),));
        assert_ok!(XStaking::refresh(
            Origin::signed(6),
            None,
            Some(true),
            None,
            None
        ));

        assert_ok!(XStaking::register(Origin::signed(10), b"name1".to_vec(),));
        assert_ok!(XStaking::refresh(
            Origin::signed(10),
            None,
            Some(true),
            None,
            None
        ));

        assert_ok!(XStaking::register(Origin::signed(20), b"name2".to_vec(),));
        assert_ok!(XStaking::refresh(
            Origin::signed(20),
            None,
            Some(true),
            None,
            None
        ));

        assert_ok!(XStaking::register(Origin::signed(30), b"name3".to_vec(),));
        assert_ok!(XStaking::refresh(
            Origin::signed(30),
            None,
            Some(true),
            None,
            None
        ));

        assert_ok!(XStaking::register(Origin::signed(40), b"name4".to_vec(),));
        assert_ok!(XStaking::refresh(
            Origin::signed(40),
            None,
            Some(true),
            None,
            None
        ));

        assert_ok!(XStaking::nominate(Origin::signed(1), 20.into(), 5, vec![]));
        assert_ok!(XStaking::nominate(Origin::signed(2), 30.into(), 15, vec![]));
        assert_ok!(XStaking::nominate(Origin::signed(3), 40.into(), 15, vec![]));
        assert_ok!(XStaking::nominate(Origin::signed(4), 10.into(), 15, vec![]));

        assert_eq!(XAccounts::intention_props_of(&6).is_active, true);
        System::set_block_number(1);
        XSession::check_rotate_session(System::block_number());

        assert_ok!(XStaking::nominate(Origin::signed(4), 6.into(), 5, vec![]));
        let jackpot_addr = XStaking::jackpot_accountid_for(&6);
        assert_eq!(XAssets::pcx_free_balance(&jackpot_addr), 0);

        System::set_block_number(2);
        XSession::check_rotate_session(System::block_number());

        // Account 6 is a validator
        assert_eq!(
            XStaking::validators(),
            vec![(40, 15), (30, 15), (10, 15), (20, 5), (6, 5)]
        );
        let mut total_active_stake = 15 + 15 + 15 + 5 + 5;
        let mut rewards = 5_000_000_000 * 8 / 10;
        let mut reward_of = Vec::new();
        for (val, stakes) in XStaking::validators() {
            let reward = rewards * stakes / total_active_stake;
            reward_of.push((val, reward));
            rewards -= reward;
            total_active_stake -= stakes;
        }
        let reward = reward_of[4].1;
        let jackpot1 = reward - reward / 10;
        assert_eq!(XAssets::pcx_free_balance(&jackpot_addr), jackpot1);

        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());
        // Validator 6 get slashed immediately
        XStaking::on_offline_validator(&6);
        assert_eq!(
            XStaking::validators(),
            vec![(40, 15), (30, 15), (10, 15), (20, 5), (6, 5)]
        );

        let mut total_active_stake = 15 + 15 + 15 + 5 + 5;
        let mut rewards = 5_000_000_000 * 8 / 10;
        let mut reward_of = Vec::new();
        for (val, stakes) in XStaking::validators() {
            let reward = rewards * stakes / total_active_stake;
            reward_of.push((val, reward));
            rewards -= reward;
            total_active_stake -= stakes;
        }
        let reward = reward_of[4].1;
        let jackpot2 = reward - reward / 10;
        assert_eq!(
            XAssets::pcx_free_balance(&jackpot_addr),
            jackpot2 + jackpot1
        );

        System::set_block_number(4);
        XSession::check_rotate_session(System::block_number());

        // Validator 6 be kicked
        assert_eq!(
            XStaking::validators(),
            vec![(40, 15), (30, 15), (10, 15), (20, 5)]
        );
        assert_eq!(XAssets::pcx_free_balance(&jackpot_addr), 0);
        assert_eq!(XAccounts::intention_props_of(&2).is_active, false);
    });
}
