// Copyright 2019-2020 ChainX Project Authors. Licensed under GPL-3.0.

use super::*;
use crate::mock::*;
use std::collections::HashMap;

#[test]
fn test_validator_total_nomination() {
    ExtBuilder::default().build_and_execute(|| {
        use xpallet_mining_staking::{Nominations, ValidatorLedgers};

        let accounts = crate::mock::get_accounts();

        let mut calculated_nomination_info = HashMap::new();
        for who in accounts {
            for (validator, ledger) in Nominations::<Test>::iter_prefix(&who) {
                *calculated_nomination_info.entry(validator).or_insert(0u128) += ledger.nomination;
            }
        }

        let validators_nomination_info = ValidatorLedgers::<Test>::iter()
            .map(|(validator, validator_profile)| (validator, validator_profile.total_nomination))
            .collect::<HashMap<_, _>>();

        for (v1, nomination1) in calculated_nomination_info {
            let nomination2 = validators_nomination_info.get(&v1).unwrap();
            assert!(nomination1 == *nomination2);
        }
    });
}

#[test]
fn test_genesis_state() {
    ExtBuilder::default().build_and_execute(|| {
        let accounts = crate::mock::get_accounts();

        let mut calculated_total_weight = 0u128;
        let mut calculated_total_nomination = 0u128;
        let mut calculated_info = HashMap::new();
        let mut calculated_nomination_info = HashMap::new();
        for who in accounts {
            for (validator, ledger) in
                xpallet_mining_staking::Nominations::<Test>::iter_prefix(&who)
            {
                calculated_total_weight += ledger.last_vote_weight;
                calculated_total_nomination += ledger.nomination;
                *calculated_nomination_info
                    .entry(format!("{:?}", validator))
                    .or_insert(0u128) += ledger.nomination;
                *calculated_info
                    .entry(format!("{:?}", validator))
                    .or_insert(0u128) += ledger.last_vote_weight;
            }
        }
        println!(
            "calculated size:{:?}, {:#?}",
            calculated_info.len(),
            calculated_info
        );
        let mut total_validator_weights = 0u128;
        let mut total_nomination = 0u128;
        let mut validators_info = HashMap::new();
        let mut validators_nomination_info = HashMap::new();
        for (validator, validator_profile) in
            xpallet_mining_staking::ValidatorLedgers::<Test>::iter()
        {
            validators_info.insert(
                format!("{:?}", validator),
                validator_profile.last_total_vote_weight,
            );
            validators_nomination_info.insert(
                format!("{:?}", validator),
                validator_profile.total_nomination,
            );
            total_validator_weights += validator_profile.last_total_vote_weight;
            total_nomination += validator_profile.total_nomination;
        }

        println!(
            "validatos size:{:?}, {:#?}",
            validators_info.len(),
            validators_info
        );

        for (v1, weight1) in calculated_info {
            let weight2 = validators_info.get(&v1).unwrap();

            if weight1 == *weight2 {
            } else {
                println!(
                    "ERROR! v1:{:?}, calculated weight1: {:?}, weight2: {:?}",
                    v1, weight1, weight2
                );
            }
        }

        for (v1, nomination1) in calculated_nomination_info {
            let nomination2 = validators_nomination_info.get(&v1).unwrap();

            if nomination1 == *nomination2 {
                println!("PASS nomination")
            } else {
                println!(
                    "ERROR! v1:{:?}, calculated nomination1: {:?}, nomination2: {:?}",
                    v1, nomination1, nomination2
                );
            }
        }

        println!(
            "calculated_total_weight:{:?}, total_validator_weights:{:?}",
            calculated_total_weight, total_validator_weights
        );

        println!(
            "calculated total nomination: {:?}, total_nomination: {:?}",
            calculated_total_nomination, total_nomination
        );

        if calculated_total_nomination == total_nomination {
            println!("---------- Congratulations");
        }
    });
}
