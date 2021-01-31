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

pub mod api;
pub mod filters;
pub mod handlers;
pub mod model;

use crate::deepq::model::GameId;
use crate::db::DbConn;

use tokio::sync::broadcast;
use warp::{
    filters::BoxedFilter,
    reply::Reply,
};

#[derive(Debug, Clone)]
pub enum FishnetMsg {
    JobAcquired(GameId),
    JobAborted(GameId),
    JobCompleted(GameId),
}


pub struct Actor {
    pub tx: broadcast::Sender<FishnetMsg>,
}

impl Actor {
    pub fn new(channel_size: usize) -> Actor {
        // TODO: make the amount of backlog configurable
        let (tx, _) = broadcast::channel(channel_size);
        Actor {tx}
    }

    pub fn handlers(&self, db: DbConn) -> BoxedFilter<(impl Reply,)> { 
        handlers::mount(db.clone(), self.tx.clone())
    }
}

