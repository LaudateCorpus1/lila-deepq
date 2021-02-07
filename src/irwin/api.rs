//
// Copyright 2021 Lakin Wecker
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
//
//

use std::convert::{TryFrom, TryInto};
use std::iter::Iterator;
use std::result::Result as StdResult;

use futures::future::try_join_all;
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, SpaceSeparator, StringWithSeparator};
use shakmaty::{san::San, uci::Uci, CastlingMode, Chess, Position};
use tokio::sync::broadcast::{self, error::RecvError};

use crate::db::DbConn;
use crate::deepq::api::{
    find_report, insert_many_games, insert_one_report, precedence_for_origin, CreateGame,
    CreateReport,
};
use crate::deepq::model::{GameId, ReportOrigin, ReportType, Score, UserId};
use crate::error::{Error, Result};
use crate::fishnet::api::{get_job, insert_many_jobs, CreateJob};
use crate::fishnet::model::AnalysisType;
use crate::fishnet::FishnetMsg;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub titled: bool,
    pub engine: bool,
    pub games: i32,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Game {
    pub id: GameId,
    pub white: UserId,
    pub black: UserId,
    pub emts: Option<Vec<i32>>,

    #[serde_as(as = "StringWithSeparator::<SpaceSeparator, San>")]
    pub pgn: Vec<San>,
    pub analysis: Option<Vec<Score>>,
}

fn uci_from_san(pgn: &Vec<San>) -> Result<Vec<Uci>> {
    let mut pos = Chess::default();
    let mut ret_val = Vec::new();
    for san in pgn.iter() {
        let m = san.to_move(&pos)?;
        // TODO: the castling mode needs to come from the game!!
        ret_val.push(Uci::from_move(&m, CastlingMode::Standard));
        pos = pos.play(&m).map_err(|_pos| Error::PositionError)?;
    }
    Ok(ret_val)
}

impl TryFrom<&Game> for CreateGame {
    type Error = Error;

    fn try_from(g: &Game) -> StdResult<CreateGame, Self::Error> {
        let g = g.clone();
        Ok(CreateGame {
            game_id: g.id,
            emts: g.emts.unwrap_or_else(Vec::new),
            pgn: uci_from_san(&g.pgn)?,
            black: Some(g.black),
            white: Some(g.white),
        })
    }
}

// TODO: Consider using an enum for the Request/KeepAlive pair here.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Request {
    pub t: String,
    pub origin: ReportOrigin,
    pub user: User,
    pub games: Vec<Game>,
}

impl From<Request> for CreateReport {
    fn from(request: Request) -> CreateReport {
        CreateReport {
            user_id: request.user.id,
            origin: request.origin,
            report_type: ReportType::Irwin,
            games: request.games.iter().map(|g| g.id.clone()).collect(),
        }
    }
}

impl From<Request> for Vec<CreateJob> {
    fn from(request: Request) -> Vec<CreateJob> {
        request
            .games
            .iter()
            .map(|g| CreateJob {
                game_id: g.id.clone(),
                report_id: None,
                analysis_type: AnalysisType::Deep,
                precedence: precedence_for_origin(request.clone().origin),
            })
            .collect()
    }
}

pub async fn add_to_queue(db: DbConn, request: Request) -> Result<()> {
    let games_with_uci = request
        .games
        .iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<CreateGame>>>()?;
    try_join_all(insert_many_games(
        db.clone(),
        games_with_uci.iter().cloned(),
    ))
    .await?;

    let report_id = insert_one_report(db.clone(), request.clone().into()).await?;

    let fishnet_jobs: Vec<CreateJob> = request.into();
    let fishnet_jobs: Vec<CreateJob> = fishnet_jobs
        .iter()
        .map(|j: &CreateJob| CreateJob {
            game_id: j.game_id.clone(),
            report_id: Some(report_id.clone()),
            analysis_type: j.analysis_type.clone(),
            precedence: j.precedence,
        })
        .collect();

    try_join_all(insert_many_jobs(db.clone(), fishnet_jobs.iter().by_ref())).await?;
    Ok(())
}

pub fn fishnet_listener(db: DbConn, tx: broadcast::Sender<FishnetMsg>) -> Result<()> {
    let p = "irwin::api::fishnet_listener >";
    tokio::spawn(async move {
        let mut should_stop: bool = false;
        let mut rx = tx.subscribe();
        while !should_stop {
            let db = db.clone();
            let msg = rx.recv().await;
            if let Ok(msg) = msg {
                match msg {
                    FishnetMsg::JobAcquired(id) => {
                        // TODO: do something with this?
                        debug!("{} Fishnet::JobAcquired({})", p, id);
                    }
                    FishnetMsg::JobAborted(id) => {
                        // TODO: do something with this?
                        debug!("{} Fishnet::JobAborted({})", p, id);
                    }
                    FishnetMsg::JobCompleted(id) => {
                        tokio::spawn(async move {
                            debug!("{} Fishnet::JobCompleted({})", p, id);
                            match get_job(db.clone(), id.clone().into()).await {
                                Result::Err(err) => {
                                    error!("{} Unable find job for {:?}. Error: {:?}", p, id.clone(), err);
                                }
                                Result::Ok(None) => {
                                    error!("{} Unable find job for {:?}.", p, id.clone());
                                }
                                Result::Ok(Some(job)) => {
                                    if let Some(report_id) = job.report_id {
                                        match find_report(db.clone(), report_id.clone()).await {
                                            Result::Err(err) => {
                                                error!("{} Unable find report for {:?}. Error: {:?}", p, report_id.clone(), err);
                                            }
                                            Result::Ok(None) => {
                                                error!("{} Unable find report for {:?}.", p, report_id.clone());
                                            }
                                            Result::Ok(Some(report)) => {
                                                debug!("{} Fishnet::JobCompleted({})", p, id);
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    }
                }
            } else if let Err(e) = msg {
                match e {
                    RecvError::Lagged(n) => {
                        warn!(
                            "irwin::api::fishnet_listener unable to keep up. Skip {} messages",
                            n
                        );
                    }
                    RecvError::Closed => {
                        should_stop = true;
                    }
                }
            }
        }
    });
    Ok(())
}
