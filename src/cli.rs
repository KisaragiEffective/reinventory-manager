use once_cell::sync::OnceCell;
use std::sync::{Arc, Mutex};
use clap::{Parser, Subcommand};
use email_address::EmailAddress;
use anyhow::{ensure, Result};
use crate::model::{LoginInfo, Password, RecordId, UserId};

#[derive(Parser, Debug)]
pub struct Args {
    #[clap(short, long)]
    email: Option<EmailAddress>,
    #[clap(short, long)]
    password: Option<Password>,
    #[clap(subcommand)]
    sub_command: ToolSubCommand,
}

impl Args {
    pub fn validate(self) -> Result<AfterArgs> {
        ensure!(self.email.is_some() == self.password.is_some(), r#"You can not provide only one of authorization info.
You must:
a) provide both email and password
b) leave blank both email and password (no login)"#);
        Ok(AfterArgs {
            login_info: self.email.and_then(|email| self.password.map(|password| LoginInfo {
                email,
                password,
            })),
            sub_command: self.sub_command,
        })
    }
}

#[derive(Debug)]
pub struct AfterArgs {
    pub login_info: Option<LoginInfo>,
    pub sub_command: ToolSubCommand,
}

#[derive(Subcommand, Debug)]
pub enum ToolSubCommand {
    List {
        #[clap(short = 'd', long, default_value_t = 1)]
        max_depth: usize,
        #[clap(short = 'u', long)]
        target_user: Option<UserId>,
        #[clap(default_values_t = Vec::<String>::new())]
        base_dir: Vec<String>,
    },
    Metadata {
        #[clap(short = 'u', long)]
        target_user: Option<UserId>,
        #[clap(default_values_t = Vec::<String>::new())]
        base_dir: Vec<String>,
    },
    Move {
        #[clap(short = 'u', long)]
        target_user: UserId,
        #[clap(short, long)]
        record_id: RecordId,
        #[clap(long)]
        from: Vec<String>,
        #[clap(long)]
        to: Vec<String>,
    },
}

pub fn init_fern() -> anyhow::Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stderr())
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

pub static ARGS: OnceCell<Arc<Mutex<AfterArgs>>> = OnceCell::new();
