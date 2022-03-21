use cosmwasm_std::Addr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Game, GameMove};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub admin: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    StartGame {
        opponent: Addr,
        host_move: GameMove,
    },
    UpdateAdmin {
        admin: Addr,
    },
    AddToBlacklist {
        address: Addr,
    },
    RemoveFromBlacklist {
        address: Addr,
    },
    OpponentResponse {
        host: Addr,
        opponent: Addr,
        opp_move: GameMove,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    GetGamesByHost { host: Addr },
    GetGamesByOpponent { opponent: Addr },
    GetGameByHostAndOpponent { host: Addr, opponent: Addr },
    GetAdmin {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GamesListResponse {
    pub games: Vec<Game>,
}
