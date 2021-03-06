// Copyright 2018-2019 Chainpool.

mod header_proof;

// Substrate
use rstd::result;
use support::StorageMap;

// ChainX
use xsupport::{debug, error, info};

// light-bitcoin
use btc_chain::BlockHeader;
use btc_primitives::H256;

use super::tx::{handle_tx, remove_unused_tx};
use super::types::BlockHeaderInfo;
use super::{BlockHashFor, BlockHeaderFor, Module, Trait};

pub use self::header_proof::HeaderVerifier;

#[derive(PartialEq, Eq)]
pub enum ChainErr {
    /// Unknown parent
    UnknownParent,
    /// Not Found
    NotFound,
    /// Ancient fork
    AncientFork,
    #[allow(unused)]
    OtherErr(&'static str),
}

impl ChainErr {
    pub fn info(&self) -> &'static str {
        match *self {
            ChainErr::UnknownParent => "Block parent is unknown",
            ChainErr::NotFound => "Not to find orphaned side chain in header collection; qed",
            ChainErr::AncientFork => "Fork is too long to proceed",
            ChainErr::OtherErr(s) => s,
        }
    }
}

pub fn check_prev_and_convert<T: Trait>(
    header: BlockHeader,
) -> result::Result<BlockHeaderInfo, ChainErr> {
    let prev_hash = &header.previous_header_hash;
    let prev_info = Module::<T>::block_header_for(prev_hash).ok_or_else(|| {
        error!(
            "[check_prev_and_convert]|not find prev header|current header:{:?}",
            header
        );
        ChainErr::UnknownParent
    })?;
    let prev_height = prev_info.height;

    let best_header_hash = Module::<T>::best_index();
    let best_info = Module::<T>::block_header_for(&best_header_hash).ok_or_else(|| {
        error!(
            "[check_prev_and_convert]|not find best|current best hash:{:}",
            best_header_hash
        );
        ChainErr::NotFound
    })?;
    let best_height = best_info.height;

    //      confirmed = best_height - (confirmations - 1)
    //           |--------- confirmations = 6 ------------|
    // b(prev) - b(confirm) - b - b - b - b - b(best_index)
    //      \    b_fork(ancient_fork)
    let confirmations = Module::<T>::confirmation_number();
    let this_height = prev_height + 1;
    if this_height <= best_height - (confirmations - 1) {
        error!("[check_prev_and_convert]|fatal error for bitcoin fork|best:{:?}|header:{:?}|confirmations:{:?}|height:{:} <= best_height - confirmations:{:}",
               best_info, header, confirmations, this_height, best_height - (confirmations - 1));
        return Err(ChainErr::AncientFork);
    }
    Ok(BlockHeaderInfo {
        header,
        height: this_height,
        confirmed: false,
        txid_list: [].to_vec(),
    })
}

pub fn remove_unused_headers<T: Trait>(header_info: &BlockHeaderInfo) {
    //delete old header info
    let reserved = Module::<T>::reserved_block();
    if header_info.height > reserved {
        let del = header_info.height - reserved;
        let v = Module::<T>::block_hash_for(&del);
        // remove all block for this height
        for h in v.iter() {
            if let Some(header_info) = Module::<T>::block_header_for(h) {
                // remove related tx for this block
                for txid in header_info.txid_list.iter() {
                    remove_unused_tx::<T>(txid);
                }
            }

            BlockHeaderFor::<T>::remove(h);
            debug!(
                "[remove_unused_headers]|remove old header|height:{:}|hash:{:}",
                del, h
            );
        }
        BlockHashFor::<T>::remove(&del);
    }
}

///      confirmed = best_height - (confirmations - 1)
///           |--------- confirmations = 6 ------------|
/// b(prev) - b(confirm) - b - b - b - b - b(best_index)
/// #issue 501 https://github.com/chainpool/ChainX/issues/501
pub fn update_confirmed_header<T: Trait>(header_info: &BlockHeaderInfo) -> (H256, u32) {
    // update confirmd status
    let confirmations = Module::<T>::confirmation_number();
    let mut prev_hash = header_info.header.previous_header_hash;
    // start from prev, thus start from 1,when confirmations = 6, it's 1..5 => [1,2,3,4]
    // b(100)(confirmed) - b(101)(need_confirmed) - b(102) - b(103) - b(104) - b(105)(best) - b(106)(current)
    //                                                                           prev        current 0
    //                                                                 prev     current 1 (start loop from this)
    //                                                       prev     current 2
    //                                              prev     current 3
    //                                  prev     current 4
    for _i in 1..(confirmations - 1) {
        if let Some(current_info) = Module::<T>::block_header_for(&prev_hash) {
            prev_hash = current_info.header.previous_header_hash
        } else {
            // if not find current header info, jump out of loop
            info!(
                "[update_confirmed_header]|not find for hash:{:?}, current reverse count:{:}",
                prev_hash, _i
            );
            break;
        }
    }

    if let Some(mut header) = Module::<T>::block_header_for(&prev_hash) {
        handle_confirmed_block::<T>(&header);
        header.confirmed = true;
        BlockHeaderFor::<T>::insert(&prev_hash, header);
    } else {
        // no not have prev hash in storage, return genesis header info
        info!(
            "[update_confirmed_header]|not find prev header, use genesis instead|prev:{:?}",
            prev_hash
        );
        let (header, height) = Module::<T>::genesis_info();
        return (header.hash(), height);
    }

    // e.g. header_info.height = 106
    // 106 - (6 - 1) = 101
    (prev_hash, header_info.height - (confirmations - 1))
}

fn handle_confirmed_block<T: Trait>(confirmed_header: &BlockHeaderInfo) {
    debug!(
        "[handle_confirmed_block]|Confirmed: height:{:}|hash:{:}",
        u64::from(confirmed_header.height),
        confirmed_header.header.hash(),
    );
    for txid in confirmed_header.txid_list.iter() {
        // deposit & withdraw
        match handle_tx::<T>(txid) {
            Err(_e) => {
                error!(
                    "[handle_confirmed_block]|Handle tx failed, the error info:{:}|tx_hash:{:}",
                    _e, txid,
                );
            }
            Ok(()) => (),
        }
    }
}

/// not include confirmed block, when confirmations = 6, it's 0..5 => [0,1,2,3,4]
/// b(100)(confirmed) - b(101) - b(102) - b(103) - b(104) - b(105)(best)
///                                                         current 0
///                                              current 1
///                                    current 2
///                           current 3
///       prev        current 4
pub fn find_confirmed_block<T: Trait>(current: &H256) -> BlockHeaderInfo {
    let confirmations = Module::<T>::confirmation_number();
    let mut current_hash = *current;
    for _ in 0..(confirmations - 1) {
        if let Some(info) = Module::<T>::block_header_for(current_hash) {
            if info.confirmed {
                return info;
            }

            current_hash = info.header.previous_header_hash
        } else {
            break;
        }
    }

    if let Some(info) = Module::<T>::block_header_for(current_hash) {
        info
    } else {
        let (header, _) = Module::<T>::genesis_info();
        Module::<T>::block_header_for(header.hash()).expect("genesis hash must exist!")
    }
}
