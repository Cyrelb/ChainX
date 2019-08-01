// Copyright 2018-2019 Chainpool.

#![cfg(test)]

use super::mock::*;
use super::*;

use runtime_io::with_externalities;
use support::{assert_noop, assert_ok};
use xassets::Chain;

#[test]
fn issue_sdot_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(1);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &1, 100));
        // amount: 0
        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 0,
                last_total_deposit_weight_update: 1
            }
        );
        // amount: 0
        assert_eq!(
            XTokens::deposit_records((1, sdot)),
            DepositVoteWeight {
                last_deposit_weight: 0,
                last_deposit_weight_update: 1
            }
        );

        System::set_block_number(2);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        // amount: 100
        assert_ok!(XAssets::issue(&sdot, &1, 100));
        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 100,
                last_total_deposit_weight_update: 2
            }
        );
        // amount: 100
        assert_eq!(
            XTokens::deposit_records((1, sdot)),
            DepositVoteWeight {
                last_deposit_weight: 100,
                last_deposit_weight_update: 2
            }
        );

        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &2, 100));
        // amount: 200
        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 100 + 200 * 1,
                last_total_deposit_weight_update: 3
            }
        );
        // amount: 0
        assert_eq!(
            XTokens::deposit_records((2, sdot)),
            DepositVoteWeight {
                last_deposit_weight: 0,
                last_deposit_weight_update: 3
            }
        );

        System::set_block_number(4);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &2, 100));
        // amount: 300
        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 300 + 300 * 1,
                last_total_deposit_weight_update: 4
            }
        );
        // amount: 100
        assert_eq!(
            XTokens::deposit_records((2, sdot)),
            DepositVoteWeight {
                last_deposit_weight: 100,
                last_deposit_weight_update: 4
            }
        );

        System::set_block_number(5);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &1, 100));
        // amount: 400
        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 600 + 400 * 1,
                last_total_deposit_weight_update: 5
            }
        );
        // amount: 200
        assert_eq!(
            XTokens::deposit_records((1, sdot)),
            DepositVoteWeight {
                last_deposit_weight: 100 + 200 * 3,
                last_deposit_weight_update: 5
            }
        );

        System::set_block_number(6);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &1, 100));
        // amount: 500
        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 1000 + 500 * 1,
                last_total_deposit_weight_update: 6
            }
        );
        // amount: 300
        assert_eq!(
            XTokens::deposit_records((1, sdot)),
            DepositVoteWeight {
                last_deposit_weight: 700 + 300 * 1,
                last_deposit_weight_update: 6
            }
        );
    });
}

#[test]
fn move_sdot_later_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &1, 100));
        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 0,
                last_total_deposit_weight_update: 3
            }
        );

        System::set_block_number(4);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &2, 100));
        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 100,
                last_total_deposit_weight_update: 4
            }
        );

        System::set_block_number(5);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        XAssets::move_balance(&sdot, &1, AssetType::Free, &2, AssetType::Free, 10).unwrap();

        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 100,
                last_total_deposit_weight_update: 4
            }
        );

        assert_eq!(
            XTokens::deposit_records((1, sdot.clone())),
            DepositVoteWeight {
                last_deposit_weight: 100 * 2,
                last_deposit_weight_update: 5
            }
        );

        assert_eq!(
            XTokens::deposit_records((2, sdot)),
            DepositVoteWeight {
                last_deposit_weight: 100 * 1,
                last_deposit_weight_update: 5
            }
        );
    });
}

#[test]
fn claim_sdot_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &100, 100));
        assert_ok!(XTokens::set_claim_restriction(sdot.clone(), (0, 0)));

        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 0,
                last_total_deposit_weight_update: 3
            }
        );

        System::set_block_number(4);
        XSession::check_rotate_session(System::block_number());
        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &200, 100));

        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 0 + 100 * 1,
                last_total_deposit_weight_update: 4
            }
        );

        assert_eq!(XAssets::pcx_free_balance(&10), 39503961);
        assert_eq!(XAssets::pcx_free_balance(&100), 0);
        XTokens::claim(Origin::signed(100), sdot.clone()).unwrap();
        // 10% goes to channel/council
        assert_eq!(XAssets::pcx_free_balance(&10), 0);
        assert_eq!(XAssets::pcx_free_balance(&100), 39503961 - 39503961 / 10);

        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 0,
                last_total_deposit_weight_update: 4
            }
        );

        assert_eq!(
            XTokens::deposit_records((100, sdot.clone())),
            DepositVoteWeight {
                last_deposit_weight: 0,
                last_deposit_weight_update: 4
            }
        );

        System::set_block_number(5);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        XAssets::move_balance(&sdot, &100, AssetType::Free, &200, AssetType::Free, 10).unwrap();

        assert_eq!(
            XTokens::deposit_records((100, sdot.clone())),
            DepositVoteWeight {
                last_deposit_weight: 0 + 100 * 1,
                last_deposit_weight_update: 5
            }
        );

        assert_eq!(
            XTokens::deposit_records((200, sdot.clone())),
            DepositVoteWeight {
                last_deposit_weight: 0 + 100 * 1,
                last_deposit_weight_update: 5
            }
        );

        assert_eq!(XAssets::pcx_free_balance(&10), 78431373);
        assert_eq!(XAssets::pcx_free_balance(&100), 35553565);
        XTokens::claim(Origin::signed(100), sdot.clone()).unwrap();
        assert_eq!(XAssets::pcx_free_balance(&10), 39215687);
        assert_eq!(XAssets::pcx_free_balance(&100), 70847683);

        assert_eq!(
            XTokens::psedu_intention_profiles(&sdot),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 0 + 200 * 1 - 100 * 1,
                last_total_deposit_weight_update: 5
            }
        );
    });
}

#[test]
fn move_sdot_to_an_account_never_deposited_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());
        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &100, 100));

        System::set_block_number(4);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        XAssets::move_balance(&sdot, &100, AssetType::Free, &200, AssetType::Free, 10).unwrap();

        assert_eq!(
            XTokens::deposit_records((100, sdot.clone())),
            DepositVoteWeight {
                last_deposit_weight: 0 + 100 * 1,
                last_deposit_weight_update: 4
            }
        );

        assert_eq!(
            XTokens::deposit_records((200, sdot.clone())),
            DepositVoteWeight {
                last_deposit_weight: 0,
                last_deposit_weight_update: 4
            }
        );

        System::set_block_number(5);
        XSession::check_rotate_session(System::block_number());
        XAssets::move_balance(&sdot, &100, AssetType::Free, &200, AssetType::Free, 10).unwrap();

        assert_eq!(
            XTokens::deposit_records((100, sdot.clone())),
            DepositVoteWeight {
                last_deposit_weight: 100 + 90 * 1,
                last_deposit_weight_update: 5
            }
        );

        assert_eq!(
            XTokens::deposit_records((200, sdot.clone())),
            DepositVoteWeight {
                last_deposit_weight: 0 + 10 * 1,
                last_deposit_weight_update: 5
            }
        );
    });
}

#[test]
fn vote_weight_update_on_withdraw_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());
        let btc = b"BTC".to_vec();

        // deposit
        assert_ok!(XRecords::deposit(&1, &btc, 100));
        assert_eq!(
            XTokens::deposit_records((1, btc.clone())),
            DepositVoteWeight {
                last_deposit_weight: 0,
                last_deposit_weight_update: 3
            }
        );
        assert_eq!(
            XTokens::psedu_intention_profiles(&btc),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 0,
                last_total_deposit_weight_update: 3
            }
        );
        assert_eq!(XAssets::free_balance_of(&1, &btc), 100);

        System::set_block_number(4);
        XSession::check_rotate_session(System::block_number());

        // withdraw
        assert_ok!(XRecords::withdrawal(
            &1,
            &btc,
            100,
            b"addr".to_vec(),
            b"ext".to_vec()
        ));

        let numbers = XRecords::withdrawal_application_numbers(Chain::Bitcoin, 10).unwrap();
        assert_eq!(numbers.len(), 1);
        assert_ok!(XRecords::withdrawal_processing(&numbers));
        for i in numbers {
            assert_ok!(XRecords::withdrawal_finish(i));
        }

        assert_eq!(
            XTokens::deposit_records((1, btc.clone())),
            DepositVoteWeight {
                last_deposit_weight: 0 + 100,
                last_deposit_weight_update: 4
            }
        );
        assert_eq!(
            XTokens::psedu_intention_profiles(&btc),
            PseduIntentionVoteWeight {
                last_total_deposit_weight: 0 + 100,
                last_total_deposit_weight_update: 4
            }
        );

        assert_eq!(XAssets::free_balance_of(&1, &btc), 0);
    });
}

#[test]
fn total_token_reward_should_be_right() {
    with_externalities(&mut new_test_ext(), || {
        // validators: 1, 2, 3, 4
        // jackpot: 101, 102, 103, 104
        // team: 666
        // council: 888
        // depositors: 100, 200, 300
        //
        // Initial state: all accounts' balance is 0.
        let all = vec![1, 2, 3, 4, 101, 102, 103, 104, 100, 200, 300, 10, 666, 888];
        assert_eq!(
            all.iter()
                .map(|x| XAssets::pcx_free_balance(x))
                .sum::<u64>(),
            0
        );

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XTokens::set_claim_restriction(sdot.clone(), (0, 0)));

        // 5_000_000_000 per session
        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());
        assert_eq!(XAssets::pcx_free_balance(&666), 1_000_000_000);
        assert_ok!(XAssets::issue(&sdot, &100, 100));

        assert_eq!(XAssets::pcx_free_balance(&1), 100_000_000);
        assert_eq!(XAssets::pcx_free_balance(&101), 900_000_000);

        assert_eq!(
            all.iter()
                .map(|x| XAssets::pcx_free_balance(x))
                .sum::<u64>(),
            5_000_000_000
        );

        System::set_block_number(4);
        XSession::check_rotate_session(System::block_number());
        assert_ok!(XAssets::issue(&sdot, &200, 200));

        assert_eq!(
            all.iter()
                .map(|x| XAssets::pcx_free_balance(x))
                .sum::<u64>(),
            5_000_000_000 * 2
        );

        System::set_block_number(5);
        XSession::check_rotate_session(System::block_number());
        assert_ok!(XAssets::issue(&sdot, &300, 300));

        assert_eq!(
            all.iter()
                .map(|x| XAssets::pcx_free_balance(x))
                .sum::<u64>(),
            5_000_000_000 * 3
        );

        System::set_block_number(6);
        XSession::check_rotate_session(System::block_number());
        XAssets::move_balance(&sdot, &100, AssetType::Free, &200, AssetType::Free, 10).unwrap();

        assert_eq!(
            all.iter()
                .map(|x| XAssets::pcx_free_balance(x))
                .sum::<u64>(),
            5_000_000_000 * 4
        );

        System::set_block_number(7);
        XSession::check_rotate_session(System::block_number());
        XAssets::move_balance(&sdot, &300, AssetType::Free, &100, AssetType::Free, 100).unwrap();

        XTokens::claim(Origin::signed(100), sdot.clone()).unwrap();
        XTokens::claim(Origin::signed(200), sdot.clone()).unwrap();
        XTokens::claim(Origin::signed(300), sdot.clone()).unwrap();

        assert_eq!(
            all.iter()
                .map(|x| XAssets::pcx_free_balance(x))
                .sum::<u64>(),
            5_000_000_000 * 5
        );
    });
}

#[test]
fn cross_chain_assets_grow_too_fast_should_work() {
    with_externalities(&mut new_test_ext(), || {
        XStaking::set_distribution_ratio((1, 1)).unwrap();
        assert_ok!(XAssets::pcx_issue(&1, 100_000_000_000));
        assert_ok!(XStaking::nominate(
            Origin::signed(1),
            1.into(),
            10_000_000_000,
            vec![]
        ));

        let trading_pair = XSpot::trading_pair_of(0).unwrap();
        assert_ok!(XAssets::issue(&trading_pair.quote(), &1, 10));
        assert_eq!(XAssets::free_balance_of(&1, &trading_pair.quote()), 10);
        assert_ok!(XSpot::put_order(
            Origin::signed(1),
            0,
            xspot::OrderType::Limit,
            xspot::Side::Buy,
            1000,
            1_000_000,
        ));

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &1, 10_000));

        let btc = <XBitcoin as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&btc, &1, 10_000_000));

        System::set_block_number(1);
        XSession::check_rotate_session(System::block_number());

        // BTC: 10000010000, SDOT, 500000000
        //
        // total_cross_chain_assets: 10500010000
        // total_staked: 10500000000
        //
        // BTC 1BTC = 999.99904761 PCX
        assert_eq!(
            XTokens::asset_power(&trading_pair.quote()),
            Some((100_000_000_000u128 * 10_500_000_000u128 / 10_500_010_000u128) as u64)
        );
        // PCX
        assert_eq!(
            XTokens::asset_power(&trading_pair.base()),
            Some(100_000_000)
        );

        assert_ok!(XSpot::put_order(
            Origin::signed(1),
            0,
            xspot::OrderType::Limit,
            xspot::Side::Buy,
            1000,
            10_000_000,
        ));

        // cross_chain_assets: staked: = 1:9
        XStaking::set_distribution_ratio((1, 9)).unwrap();
        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());

        // BTC 1BTC = 111.11100529 PCX
        assert_eq!(
            XTokens::asset_power(&trading_pair.quote()),
            Some((100_000_000_000u128 * 10_500_000_000u128 / 94_500_090_000u128) as u64)
        );
        // PCX
        assert_eq!(
            XTokens::asset_power(&trading_pair.base()),
            Some(100_000_000)
        );

        // cross_chain_assets: staked: = 9:1
        assert_ok!(XAssets::issue(&trading_pair.quote(), &1, 100_000_000_000));
        XStaking::set_distribution_ratio((9, 1)).unwrap();
        System::set_block_number(2);
        XSession::check_rotate_session(System::block_number());
        // BTC 1BTC = 0.94490078 PCX
        assert_eq!(
            XTokens::asset_power(&trading_pair.quote()),
            Some((100_000_000_000u128 * 94_500_000_000u128 / 100_010_500_010_000u128) as u64)
        );
        // PCX
        assert_eq!(
            XTokens::asset_power(&trading_pair.base()),
            Some(100_000_000)
        );
    });
}

#[test]
fn claim_need_enough_staking_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());

        // cross miner should have some stake.
        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &100, 100));
        assert_ok!(XTokens::set_claim_restriction(sdot.clone(), (10u32, 0)));

        System::set_block_number(4);
        XSession::check_rotate_session(System::block_number());
        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        // current dividend: 39603961
        assert_noop!(
            XTokens::claim(Origin::signed(100), sdot.clone()),
            "Cannot claim if what you have staked is too little."
        );
        assert_ok!(XAssets::pcx_issue(&1, 39603961 * 10));
        assert_ok!(XStaking::nominate(
            Origin::signed(1),
            1.into(),
            39603961 * 10,
            vec![]
        ));

        assert_ok!(XAssets::pcx_issue(&100, 39603961 * 5));
        assert_ok!(XStaking::nominate(
            Origin::signed(100),
            1.into(),
            39603961 * 5,
            vec![]
        ));
        assert_noop!(
            XTokens::claim(Origin::signed(100), sdot.clone()),
            "Cannot claim if what you have staked is too little."
        );

        assert_ok!(XAssets::pcx_issue(&100, 39603961 * 5));
        assert_ok!(XStaking::nominate(
            Origin::signed(100),
            1.into(),
            39603961 * 5,
            vec![]
        ));
        assert_ok!(XTokens::claim(Origin::signed(100), sdot.clone()));
    });
}

#[test]
fn claim_has_frequency_limit_should_work() {
    with_externalities(&mut new_test_ext(), || {
        System::set_block_number(3);
        XSession::check_rotate_session(System::block_number());

        let sdot = <XSdot as ChainT>::TOKEN.to_vec();
        assert_ok!(XAssets::issue(&sdot, &100, 100));
        assert_ok!(XTokens::set_claim_restriction(sdot.clone(), (0u32, 1)));

        System::set_block_number(4);
        XSession::check_rotate_session(System::block_number());
        XTokens::claim(Origin::signed(100), sdot.clone()).unwrap();

        System::set_block_number(5);
        XSession::check_rotate_session(System::block_number());
        assert_noop!(
            XTokens::claim(Origin::signed(100), sdot.clone()),
            "Can only claim once per claim limiting period."
        );

        System::set_block_number(6);
        XSession::check_rotate_session(System::block_number());
        XTokens::claim(Origin::signed(100), sdot.clone()).unwrap();
    });
}
