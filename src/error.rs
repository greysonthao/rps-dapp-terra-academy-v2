use cosmwasm_std::StdError;
use cw_controllers::{AdminError, HookError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("No Admin Found")]
    Admin(#[from] AdminError),

    #[error("{0}")]
    Hook(#[from] HookError),

    #[error("Caller is not admin")]
    NotAdmin {},

    #[error("Only One Game At A Time For The Same Host And Opponent")]
    OneGameAtATime {},

    #[error("No Game Found")]
    NoGameFound {},

    #[error("Given Address Already Registered On The Blacklist")]
    OnTheBlacklist {},

    #[error("Given Address Is Not Registered On The Blacklist")]
    NotOnTheBlacklist {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
