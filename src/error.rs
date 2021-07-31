// Copyright 2020 Lakin Wecker
//
// This file is part of lila-deepq.
//
// lila-deepq is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// lila-deepq is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with lila-deepq.  If not, see <https://www.gnu.org/licenses/>.

use std::env::VarError;
use std::num::TryFromIntError;

use mongodb::bson::{
    de::Error as _BsonDeError, document::ValueAccessError as _BsonValueAccessError,
    ser::Error as _BsonSeError,
};
use mongodb::error::Error as _MongoDBError;
//use serde::de::{Error as _SerdeDeError};
use shakmaty::uci::IllegalUciError;
use shakmaty::{Chess, PlayError};

use tokio::task::JoinError;
use warp::reject;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum HttpError {
    #[error("Unauthorized")]
    MalformedHeader,

    #[error("Unauthenticated")]
    Unauthenticated,

    #[error("Forbidden")]
    Forbidden, // Insufficient permissions
}

impl reject::Reject for HttpError {}

// TODO: this desperately needs to be cleaned up.
#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid command line arguments")]
    InvalidCommandLineArguments,

    // #[error("Serde Deserialization Error")]
    // SerdeDeserializationError(#[from] _SerdeDeError),
    #[error("I am somehow unable to create a record in the database.")]
    CreateError,

    #[error("I am somehow unable to find a record in the database.")]
    NotFoundError,

    #[error("BSON Error")]
    BsonSerializationError(#[from] _BsonSeError),

    #[error("BSON Error")]
    BsonDeserializationError(#[from] _BsonDeError),

    #[error("BSON Error")]
    BsonValueAccessError(#[from] _BsonValueAccessError),

    #[error("Mongo Database Error")]
    MongoDBError(#[from] _MongoDBError),

    #[error("Converstion Error")]
    TryFromIntError(#[from] TryFromIntError),
    #[error("Mongo Database Error")]
    HttpError(#[from] HttpError),

    #[error("IrwinStreamError")]
    IrwinStreamError(#[from] reqwest::Error),

    #[error("serde_json Error")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("std::io::Error")]
    IoError(#[from] std::io::Error),

    #[error("env::VarError")]
    VarError(#[from] VarError),

    #[error("mongodb::bson::oid::Error")]
    BsonOidError(#[from] mongodb::bson::oid::Error),

    #[error("shakmaty::san::SanError")]
    SanError(#[from] shakmaty::san::SanError),

    #[error("shakmaty::Chess")]
    PositionError,

    #[error("Unable to deserialize something")]
    DeserializationError,

    #[error("unknown data store error")]
    Unknown,

    #[error("I haven't implemented this yet")]
    Unimplemented,

    #[error("Unable to join tokio task")]
    JoinError(#[from] JoinError),

    #[error("Illegal UCI for a given position")]
    IllegalUciError(#[from] IllegalUciError),

    #[error("Illegal ChessMove for a given position")]
    IllegalChessMove(#[from] PlayError<Chess>),

    #[error("Irwin analysis has specific requirements")]
    IncompleteIrwinAnalysis,
}

impl reject::Reject for Error {}

pub type Result<T> = std::result::Result<T, Error>;
