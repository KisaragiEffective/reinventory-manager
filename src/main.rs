use std::sync::{Arc, Mutex, MutexGuard};
use clap::Parser;
use log::{debug, info};
use crate::cli::{AfterArgs, Args, ARGS, ToolSubCommand};
use crate::operation::Operation;

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
                // TODO: ここのエラー表示が終わってるので要改善
                target_user.clone().unwrap_or_else(|| login_res.clone().unwrap().user_id),
                base_dir.clone(),
                &login_res.clone().map(|a| a.using_token),
            ).await;
            for x in xs {
                println!("{:?}", x);
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
            println!("{:?}", serde_json::to_string(&res));
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
