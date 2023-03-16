#![deny(clippy::all)]
#![warn(clippy::pedantic, clippy::nursery)]

use std::io::stdin;
use std::process::exit;
use clap::Parser;
use log::{debug, error, info, warn};
use crate::cli::{Args, LogLevel, ToolSubCommand};
use crate::model::{AuthorizationInfo, LoginInfo, SessionToken};
use crate::operation::Operation;

mod operation;
mod model;
mod cli;

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();
    let args = args.validate().unwrap();
    if args.log_level != LogLevel::None {
        cli::init_fern(args.log_level).unwrap();
    }

    debug!("fern initialized");

    let read_token_from_stdin = args.read_token_from_stdin;
    let auth_info = args.login_info.clone();

    let authorization_info = if read_token_from_stdin {
        if let Some(LoginInfo::ByTokenFromStdin { user_id }) = auth_info {
            let mut buf = String::new();
            let read_size = stdin().read_line(&mut buf).unwrap();
            if read_size == 0 {
                error!("Please provide token from stdin!");
                exit(1)
            }
            let auth = AuthorizationInfo::new(user_id, SessionToken::new(buf));
            Some(auth)
        } else {
            unreachable!("Arguments validation must be done at this point")
        }
    } else {
        debug!("login...");
        let login_res = Operation::login(args.login_info).await;
        debug!("done.");
        login_res.map(|a| a.using_token)
    };

    let sub_command = args.sub_command;
    match sub_command {
        ToolSubCommand::List { max_depth: _, base_dir, target_user } => {
            debug!("Inventory:");
            let xs = Operation::get_directory_items(
                target_user
                    .clone()
                    .or_else(|| authorization_info.clone().map(|a| a.owner_id))
                    .expect("To perform this action, I must know user, to see inventory contents."),
                base_dir.clone(),
                &authorization_info,
            ).await;

            debug!("record count: {len}", len = xs.len());
            if xs.is_empty() {
                warn!("response is empty! You may want to login?");
            }
            for x in xs {
                println!("{}", serde_json::to_string(&x).unwrap());
            }
        }
        ToolSubCommand::Metadata { target_user, base_dir } => {
            debug!("Directory metadata:");
            let res = Operation::get_directory_metadata(
                target_user
                    .clone()
                    .or_else(|| authorization_info.clone().map(|a| a.owner_id))
                    .expect("To perform this action, I must know user, to see inventory contents."),
                base_dir.clone(),
                &authorization_info,
            ).await;
            println!("{}", serde_json::to_string(&res).unwrap());
        }
        ToolSubCommand::Move { target_user, record_id: record_id_list, to } => {
            let owner_id = target_user.clone();
            Operation::move_records(
                owner_id.clone(),
                record_id_list.clone(),
                to.clone(),
                &authorization_info,
                args.keep_record_id
            ).await;
        }
    }

    if let Some(session) = authorization_info {
        Operation::logout(session).await;
        info!("Logged out");
    }
}
