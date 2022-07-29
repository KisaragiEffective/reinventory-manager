use std::sync::{Arc, Mutex, MutexGuard};
use clap::Parser;
use log::{debug, info, warn};
use crate::cli::{AfterArgs, Args, ARGS, ToolSubCommand};
use crate::operation::Operation;
use crate::model::Record;

mod operation;
mod model;
mod cli;

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();
    let args = args.validate().unwrap();
    cli::init_fern().unwrap();
    debug!("fern initialized");

    ARGS.set(Arc::new(Mutex::new(args))).expect("once_cell error!!");

    debug!("login...");
    let login_res = Operation::login().await;
    debug!("done.");

    let sub_command = { &get_args_lock().sub_command };
    match sub_command {
        ToolSubCommand::List { max_depth, base_dir, target_user } => {
            println!("Inventory:");
            let xs = Operation::get_directory_items(
                target_user.clone().or_else(|| login_res.clone().map(|a| a.user_id)).expect("To perform this action, I must identify you."),
                base_dir.clone(),
                &login_res.clone().map(|a| a.using_token),
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
                &login_res.clone().map(|a| a.using_token),
            ).await;
            println!("{}", serde_json::to_string(&res).unwrap());
        }
        ToolSubCommand::Move { target_user, record_id, from, to } => {
            let owner_id = target_user.clone();
            let found_record: Option<Record> = None;
            if let Some(record) = found_record {
                Operation::move_record(
                    owner_id.clone(),
                    record.clone(),
                    from.clone(),
                    to.clone(),
                    &login_res.clone().map(|a| a.using_token),
                ).await;

                let from = record.path.clone();
                info!("moved {record_id} for {owner_id}, from {from} to {to}.", to = to.join("\\"));
            } else {
                warn!("{record_id} could not be found for {owner_id}");
            }
        }
    }

    if let Some(session) = login_res {
        let user_id = session.user_id;
        Operation::logout(user_id, session.using_token).await;
        info!("Logged out");
    }
}

fn get_args_lock<'a>() -> MutexGuard<'a, AfterArgs> {
    ARGS.get().unwrap().lock().unwrap()
}
