#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response, StdError, StdResult,
};
use cw0::maybe_addr;
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, GamesListResponse, InstantiateMsg, QueryMsg};
use crate::state::{Game, GameMove, GameResult, State, ADMIN, GAME, HOOKS, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:rps-dapp-v2";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        owner: info.sender.clone(),
        admin: msg.admin,
    };

    let api = deps.api;

    ADMIN.set(
        deps.branch(),
        maybe_addr(api, Some(info.sender.to_string()))?,
    )?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let api = deps.api;

    match msg {
        ExecuteMsg::UpdateAdmin { admin } => Ok(ADMIN.execute_update_admin(
            deps,
            info,
            maybe_addr(api, Some(admin.to_string()))?,
        )?),
        ExecuteMsg::StartGame {
            opponent,
            host_move,
        } => try_start_game(deps, info, opponent, host_move),
        ExecuteMsg::AddToBlacklist { address } => Ok(HOOKS.execute_add_hook(
            &ADMIN,
            deps,
            info,
            api.addr_validate(&address.as_str())?,
        )?),
        ExecuteMsg::RemoveFromBlacklist { address } => Ok(HOOKS.execute_remove_hook(
            &ADMIN,
            deps,
            info,
            api.addr_validate(&address.as_str())?,
        )?),
        ExecuteMsg::OpponentResponse {
            host,
            opponent,
            opp_move,
        } => try_opponent_response(deps, info, host, opponent, opp_move),
    }
}

pub fn try_start_game(
    deps: DepsMut,
    info: MessageInfo,
    opponent: Addr,
    host_move: GameMove,
) -> Result<Response, ContractError> {
    let blacklist = HOOKS.query_hooks(deps.as_ref())?.hooks;

    for address in blacklist {
        if address == info.sender {
            return Err(ContractError::OnTheBlacklist {});
        }
    }

    let _valid_addr = deps.api.addr_validate(opponent.as_str())?;

    let game_found = GAME.may_load(deps.storage, (&info.sender, &opponent))?;

    match game_found {
        Some(_) => return Err(ContractError::OneGameAtATime {}),
        None => {
            let g = Game {
                host: info.sender.clone(),
                opponent: opponent.clone(),
                host_move: host_move,
                opp_move: None,
                result: None,
            };

            GAME.save(deps.storage, (&info.sender, &opponent), &g)?;
        }
    };

    Ok(Response::new()
        .add_attribute("method", "try_start_game")
        .add_attribute("host", info.sender)
        .add_attribute("opponent", opponent))
}

pub fn try_opponent_response(
    deps: DepsMut,
    info: MessageInfo,
    host: Addr,
    opponent: Addr,
    opp_move: GameMove,
) -> Result<Response, ContractError> {
    //validate host and opp address
    let api = deps.api;

    let valid_host = api.addr_validate(&host.as_str())?;
    let valid_opp = api.addr_validate(&opponent.as_str())?;

    let key = (&valid_host, &valid_opp);

    //check opp & info sender are the same
    if &info.sender != &opponent {
        return Err(ContractError::Unauthorized {});
    }

    //load game
    let game_found =
        query_game_by_host_and_opponent(deps.as_ref(), host.clone(), opponent.clone())?;

    //compare host move and opp move
    let result = get_game_result(game_found.host_move, opp_move)?;

    //return the game result
    let result_str = match result {
        GameResult::HostWins => "Host Wins",
        GameResult::OpponentWins => "Opponent Wins",
        GameResult::Tie => "Tie",
    };

    //create closure for update method
    let update_game = |g: Option<Game>| -> Result<Game, ContractError> {
        match g {
            Some(_) => Ok(Game {
                host: game_found.host,
                opponent: game_found.opponent,
                host_move: game_found.host_move,
                opp_move: Some(opp_move),
                result: Some(result),
            }),
            None => Err(ContractError::NoGameFound {}),
        }
    };

    //update game to GAME state
    GAME.update(deps.storage, key, update_game)?;

    //delete the game from state
    GAME.remove(deps.storage, key);

    //optional: add a leaderboard

    Ok(Response::new()
        .add_attribute("method", "try_opponent_response")
        .add_attribute("host", valid_host)
        .add_attribute("opponent", valid_opp)
        .add_attribute("result", result_str))
}

pub fn get_game_result(
    host_move: GameMove,
    opp_move: GameMove,
) -> Result<GameResult, ContractError> {
    if host_move == opp_move {
        Ok(GameResult::Tie)
    } else if host_move == GameMove::Rock && opp_move == GameMove::Scissors
        || host_move == GameMove::Paper && opp_move == GameMove::Rock
        || host_move == GameMove::Scissors && opp_move == GameMove::Paper
    {
        Ok(GameResult::HostWins)
    } else {
        Ok(GameResult::OpponentWins)
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetGamesByHost { host } => to_binary(&query_games_by_host(deps, host)?),
        QueryMsg::GetGamesByOpponent { opponent } => {
            to_binary(&query_games_by_opponent(deps, opponent)?)
        }
        QueryMsg::GetGameByHostAndOpponent { host, opponent } => {
            to_binary(&query_game_by_host_and_opponent(deps, host, opponent)?)
        }
        QueryMsg::GetAdmin {} => to_binary(&ADMIN.get(deps)?),
    }
}

fn query_games_by_host(deps: Deps, host: Addr) -> StdResult<GamesListResponse> {
    let valid_addr = deps.api.addr_validate(host.as_str())?;

    let all_games: StdResult<Vec<_>> = GAME
        .prefix(&valid_addr)
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    let mut found_games: Vec<Game> = vec![];

    for all_games in &all_games? {
        found_games.push(all_games.1.clone());
    }

    Ok(GamesListResponse { games: found_games })
}

fn query_games_by_opponent(deps: Deps, opponent: Addr) -> StdResult<GamesListResponse> {
    let valid_addr = deps.api.addr_validate(opponent.as_str())?;

    let all_games: StdResult<Vec<_>> = GAME
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    let mut found_games: Vec<Game> = vec![];

    for all_games in &all_games? {
        if all_games.1.opponent == valid_addr {
            found_games.push(all_games.1.clone());
        }
    }

    Ok(GamesListResponse { games: found_games })
}

fn query_game_by_host_and_opponent(deps: Deps, host: Addr, opponent: Addr) -> StdResult<Game> {
    let valid_host = deps.api.addr_validate(host.as_str())?;
    let valid_opp = deps.api.addr_validate(opponent.as_str())?;

    let game = GAME.may_load(deps.storage, (&valid_host, &valid_opp))?;

    match game {
        Some(g) => Ok(Game {
            host: valid_host,
            opponent: valid_opp,
            host_move: g.host_move,
            opp_move: g.opp_move,
            result: g.result,
        }),
        None => Err(StdError::generic_err("No game found")),
    }
}

#[cfg(test)]
mod tests {
    use crate::state::GameMove;

    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: Addr::unchecked("creator"),
        };
        let info = mock_info("creator", &coins(1000, "earth"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        //confirm instantiation response
        assert_eq!("owner", res.attributes[1].key);
        assert_eq!("creator", res.attributes[1].value);
    }

    #[test]
    fn start_game() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: Addr::unchecked("creator"),
        };
        let info = mock_info("creator", &coins(1000, "earth"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        //confirm instantiation response
        assert_eq!("owner", res.attributes[1].key);
        assert_eq!("creator", res.attributes[1].value);

        // execute start game
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player"),
            host_move: GameMove::Rock,
        };

        let res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        assert_eq!("method", res.attributes[0].key);
        assert_eq!("try_start_game", res.attributes[0].value);
        assert_eq!("host", res.attributes[1].key);
        assert_eq!("creator", res.attributes[1].value);
        assert_eq!("opponent", res.attributes[2].key);
        assert_eq!("other_player", res.attributes[2].value);
    }

    #[test]
    fn get_games_by_host() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: Addr::unchecked("creator"),
        };
        let info = mock_info("creator", &coins(1000, "earth"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        //confirm instantiation response
        assert_eq!("owner", res.attributes[1].key);
        assert_eq!("creator", res.attributes[1].value);

        // execute start game 1
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // execute start game 2
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player_2"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // execute start game 3 - other creator
        let auth_info = mock_info("other_creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        //query game by non-host = zero games found
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetGamesByHost {
                host: Addr::unchecked("non_host"),
            },
        )
        .unwrap();
        let value: GamesListResponse = from_binary(&res).unwrap();
        assert_eq!(0, value.games.len());

        //query game by host = 2 games found
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetGamesByHost {
                host: Addr::unchecked("creator"),
            },
        )
        .unwrap();

        let value: GamesListResponse = from_binary(&res).unwrap();
        assert_eq!(2, value.games.len());
        assert_eq!(Addr::unchecked("creator"), value.games[0].host);
        assert_eq!(Addr::unchecked("other_player"), value.games[0].opponent);
        assert_eq!(GameMove::Rock, value.games[0].host_move);
        assert_eq!(None, value.games[0].opp_move);
        assert_eq!(None, value.games[0].result);
    }

    #[test]
    fn get_games_by_opp() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: Addr::unchecked("creator"),
        };
        let info = mock_info("creator", &coins(1000, "earth"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        //confirm instantiation response
        assert_eq!("owner", res.attributes[1].key);
        assert_eq!("creator", res.attributes[1].value);

        // execute start game 1
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // execute start game 2
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player_2"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // execute start game 3 - other creator
        let auth_info = mock_info("other_creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // execute start game 4 - another creator
        let auth_info = mock_info("another_creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        //query game by non-opponent = zero games found
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetGamesByOpponent {
                opponent: Addr::unchecked("non_opponent"),
            },
        )
        .unwrap();
        let value: GamesListResponse = from_binary(&res).unwrap();
        assert_eq!(0, value.games.len());

        //query game by opponent = 3 games found
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetGamesByOpponent {
                opponent: Addr::unchecked("other_player"),
            },
        )
        .unwrap();

        let value: GamesListResponse = from_binary(&res).unwrap();
        assert_eq!(3, value.games.len());
        assert_eq!(Addr::unchecked("creator"), value.games[0].host);
        assert_eq!(Addr::unchecked("other_player"), value.games[0].opponent);
        assert_eq!(GameMove::Rock, value.games[0].host_move);
        assert_eq!(None, value.games[0].opp_move);
        assert_eq!(None, value.games[0].result);
    }

    #[test]
    fn get_game_by_host_and_opp() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: Addr::unchecked("creator"),
        };
        let info = mock_info("creator", &coins(1000, "earth"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        //confirm instantiation response
        assert_eq!("owner", res.attributes[1].key);
        assert_eq!("creator", res.attributes[1].value);

        // execute start game 1
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // execute start game 2
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player_2"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // execute start game 3 - other creator
        let auth_info = mock_info("other_creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        //query game non-players = zero games found
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetGameByHostAndOpponent {
                host: Addr::unchecked("non_host"),
                opponent: Addr::unchecked("non_opponent"),
            },
        );

        match res {
            Err(_) => {}
            _ => panic!("Should error out."),
        }

        //query game by host and opponent = 1 game found
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetGameByHostAndOpponent {
                host: Addr::unchecked("creator"),
                opponent: Addr::unchecked("other_player"),
            },
        )
        .unwrap();

        let value: Game = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("creator"), value.host);
        assert_eq!(Addr::unchecked("other_player"), value.opponent);
        assert_eq!(GameMove::Rock, value.host_move);
        assert_eq!(None, value.opp_move);
        assert_eq!(None, value.result);
    }

    #[test]
    fn update_admin() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: Addr::unchecked("creator"),
        };
        let info = mock_info("creator", &coins(1000, "earth"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        //confirm instantiation response
        assert_eq!("owner", res.attributes[1].key);
        assert_eq!("creator", res.attributes[1].value);

        // execute start game 1
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        //query first admin
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetAdmin {}).unwrap();

        let value: Addr = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("creator"), value);

        // update admin - fail
        let auth_info = mock_info("not_admin", &coins(2, "token"));
        let msg = ExecuteMsg::UpdateAdmin {
            admin: Addr::unchecked("other_admin"),
        };
        let res = execute(deps.as_mut(), mock_env(), auth_info, msg);

        match res {
            Err(_) => {}
            _ => panic!("This is supposed to return a Contract Error"),
        };

        // update admin - success
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::UpdateAdmin {
            admin: Addr::unchecked("other_admin"),
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        //query updated admin
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetAdmin {}).unwrap();

        let value: Addr = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("other_admin"), value);
    }

    #[test]
    fn blacklist() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: Addr::unchecked("creator"),
        };
        let info = mock_info("creator", &coins(1000, "earth"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        //confirm instantiation response
        assert_eq!("owner", res.attributes[1].key);
        assert_eq!("creator", res.attributes[1].value);

        // add address to blacklist
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::AddToBlacklist {
            address: Addr::unchecked("bad_guy"),
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // execute start game - fail because host is blacklisted
        let auth_info = mock_info("bad_guy", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player"),
            host_move: GameMove::Rock,
        };
        let res = execute(deps.as_mut(), mock_env(), auth_info, msg);

        match res {
            Err(ContractError::OnTheBlacklist {}) => {}
            _ => panic!("OnTheBlacklist error should occur"),
        };

        // remove address from blacklist
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::RemoveFromBlacklist {
            address: Addr::unchecked("bad_guy"),
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // execute start game - success because host has been removed from blacklist
        let auth_info = mock_info("bad_guy", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("other_player"),
            host_move: GameMove::Rock,
        };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        //query game by host and opponent = 1 game found
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetGameByHostAndOpponent {
                host: Addr::unchecked("bad_guy"),
                opponent: Addr::unchecked("other_player"),
            },
        )
        .unwrap();

        let value: Game = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("bad_guy"), value.host);
        assert_eq!(Addr::unchecked("other_player"), value.opponent);
        assert_eq!(GameMove::Rock, value.host_move);
        assert_eq!(None, value.opp_move);
        assert_eq!(None, value.result);
    }

    #[test]
    fn full_game() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: Addr::unchecked("creator"),
        };
        let info = mock_info("creator", &coins(1000, "earth"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        //confirm instantiation response
        assert_eq!("owner", res.attributes[1].key);
        assert_eq!("creator", res.attributes[1].value);

        // execute start game
        let auth_info = mock_info("hosty", &coins(2, "token"));
        let msg = ExecuteMsg::StartGame {
            opponent: Addr::unchecked("toasty"),
            host_move: GameMove::Rock,
        };

        let res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();
        assert_eq!("host", res.attributes[1].key);
        assert_eq!("hosty", res.attributes[1].value);
        assert_eq!("opponent", res.attributes[2].key);
        assert_eq!("toasty", res.attributes[2].value);

        // execute opponent reponse
        let auth_info = mock_info("toasty", &coins(2, "token"));
        let msg = ExecuteMsg::OpponentResponse {
            host: Addr::unchecked("hosty"),
            opponent: Addr::unchecked("toasty"),
            opp_move: GameMove::Rock,
        };

        let res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        assert_eq!("result", res.attributes[3].key);
        assert_eq!("Tie", res.attributes[3].value);

        //query for game - fail
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetGameByHostAndOpponent {
                host: Addr::unchecked("hosty"),
                opponent: Addr::unchecked("toasty"),
            },
        );
        //confirms game data was deleted
        match res {
            Err(_) => {}
            _ => panic!("No Game Found Error should occur"),
        }
    }
}
