use std::io::stdin;
use std::process::exit;
use std::sync::{Arc, Mutex, MutexGuard};
use clap::Parser;
use log::{debug, error, info, warn};
use crate::cli::{AfterArgs, Args, ARGS, LogLevel, ToolSubCommand};
use crate::model::SessionToken;
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
    ARGS.set(Arc::new(Mutex::new(args))).expect("once_cell error!!");

    debug!("login...");
    let login_res = Operation::login().await;
    debug!("done.");

    let authorization_info = login_res.clone().map(|a| a.using_token);
    if read_token_from_stdin {
        if let Some(mut auth) = authorization_info.clone() {
            let mut buf = String::new();
            let read_size = stdin().read_line(&mut buf).unwrap();
            if read_size == 0 {
                error!("Please provide token from stdin!");
                exit(1)
            }
            auth.token = SessionToken::new(buf);
        }
    }
    let sub_command = { &get_args_lock().sub_command };
    match sub_command {
        ToolSubCommand::List { max_depth, base_dir, target_user } => {
            println!("Inventory:");
            let xs = Operation::get_directory_items(
                target_user.clone().or_else(|| login_res.clone().map(|a| a.user_id)).expect("To perform this action, I must identify you."),
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
            println!("Directory metadata:");
            let owner_id = target_user.clone().unwrap_or_else(|| login_res.clone().unwrap().user_id);
            let res = Operation::get_directory_metadata(
                owner_id,
                base_dir.clone(),
                &authorization_info,
            ).await;
            println!("{}", serde_json::to_string(&res).unwrap());
        }
        ToolSubCommand::Move { target_user, record_id, to } => {
            let owner_id = target_user.clone();
            Operation::move_record(
                owner_id.clone(),
                record_id.clone(),
                to.clone(),
                &authorization_info,
            ).await;
        }
    }

    if let Some(session) = authorization_info {
        Operation::logout(session).await;
        info!("Logged out");
    }
}

fn get_args_lock<'a>() -> MutexGuard<'a, AfterArgs> {
    ARGS.get().unwrap().lock().unwrap()
}
