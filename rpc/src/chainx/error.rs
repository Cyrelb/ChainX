// Copyright 2018-2019 Chainpool.

//! Error helpers for ChainX RPC module.

use std::str;

use crate::errors;
use crate::rpc;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, derive_more::Display, derive_more::From)]
pub enum Error {
    /// Client error.
    Client(client::error::Error),

    #[display(fmt = "Method not yet implemented")]
    Unimplemented,

    #[display(fmt = "Quotations Piece Err: piece:{}", _0)]
    QuotationsPieceErr(u32),

    #[display(fmt = "TradingPair Index error or not exist: pair index:{}", _0)]
    TradingPairIndexErr(u32),

    #[display(fmt = "Page Size Must Between 0~100, size:{}", _0)]
    PageSizeErr(u32),

    #[display(fmt = "Page Index Error, index:{}", _0)]
    PageIndexErr(u32),

    #[display(fmt = "Decode Data Error")]
    DecodeErr,

    #[display(fmt = "Start With 0x")]
    BinaryStartErr,

    #[display(fmt = "Decode Hex Err")]
    HexDecodeErr,

    #[display(
        fmt = "Runtime error, reason: {{{:}}}",
        "str::from_utf8(&_0).unwrap_or_default()"
    )]
    RuntimeErr(Vec<u8>, Option<String>),

    #[display(fmt = "{:} is Deprecated, Please Use {:}V1 Instead", _0, _0)]
    DeprecatedV0Err(String),

    #[display(fmt = "Cache fetch lock error")]
    CacheErr,

    #[display(fmt = "Storage record does not exist or not in archive")]
    StorageNotExistErr,

    #[display(fmt = "BlockNumber not exist for this hash")]
    BlockNumberErr,

    #[display(fmt = "InvalidParams")]
    InvalidParams(String),

    ContractGetStorageError(xr_primitives::GetStorageError),
}

const ERROR: i64 = 1600;

impl From<Error> for rpc::Error {
    fn from(e: Error) -> Self {
        match e {
            Error::Unimplemented => rpc::Error {
                code: rpc::ErrorCode::ServerError(1),
                message: format!("{:}", e),
                data: None,
            },
            Error::QuotationsPieceErr(_) => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 5),
                message: format!("{:}", e),
                data: None,
            },
            Error::TradingPairIndexErr(_) => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 6),
                message: format!("{:}", e),
                data: None,
            },
            Error::PageSizeErr(_) => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 7),
                message: format!("{:}", e),
                data: None,
            },
            Error::PageIndexErr(_) => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 8),
                message: format!("{:}", e),
                data: None,
            },
            Error::DecodeErr => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 9),
                message: format!("{:}", e),
                data: None,
            },
            Error::BinaryStartErr => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 10),
                message: format!("{:}", e),
                data: None,
            },
            Error::HexDecodeErr => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 11),
                message: format!("{:}", e),
                data: None,
            },
            Error::RuntimeErr(e, msg) => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 13),
                message: format!(
                    "Runtime error, reason: {{{:}}}",
                    str::from_utf8(&e).unwrap_or_default()
                ),
                data: msg.map(Into::into),
            },
            Error::DeprecatedV0Err(_) => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 14),
                message: format!("{:}", e),
                data: None,
            },
            Error::StorageNotExistErr => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 15),
                message: format!("{:}", e),
                data: None,
            },
            Error::BlockNumberErr => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 16),
                message: format!("{:}", e),
                data: None,
            },
            Error::InvalidParams(ref s) => rpc::Error {
                code: rpc::ErrorCode::ServerError(ERROR + 17),
                message: format!("{:}|reason:{}", &e, s),
                data: None,
            },
            Error::ContractGetStorageError(e) => {
                use xr_primitives::GetStorageError::*;
                match e {
                    ContractDoesntExist => rpc::Error {
                        code: rpc::ErrorCode::ServerError(ERROR + 100),
                        message: "The specified contract doesn't exist.".into(),
                        data: None,
                    },
                    IsTombstone => rpc::Error {
                        code: rpc::ErrorCode::ServerError(ERROR + 101),
                        message: "The contract is a tombstone and doesn't have any storage.".into(),
                        data: None,
                    },
                }
            }
            e => errors::internal(e),
        }
    }
}
