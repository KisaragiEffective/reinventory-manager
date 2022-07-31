use once_cell::sync::OnceCell;
use std::sync::{Arc, Mutex};
use clap::{Parser, Subcommand};
use email_address::EmailAddress;
use anyhow::{bail, ensure, Result};
use fern::colors::ColoredLevelConfig;
use log::{debug, LevelFilter};
use derive_more::{Display, FromStr};
use strum::{EnumString, Display as StrumDisplay};
use crate::model::{LoginInfo, Password, RecordId, UserId};

#[derive(Parser, Debug)]
pub struct Args {
    #[clap(short, long)]
    email: Option<EmailAddress>,
    #[clap(short, long)]
    password: Option<Password>,
    #[clap(short, long)]
    totp: Option<OneTimePassword>,
    #[clap(long, default_value_t = LogLevel::Warn)]
    log_level: LogLevel,
    #[clap(long)]
    read_token_from_stdin: bool,
    #[clap(subcommand)]
    sub_command: ToolSubCommand,
}

#[derive(Display, FromStr, Debug, Eq, PartialEq)]
pub struct OneTimePassword(pub String);

impl Args {
    pub fn validate(self) -> Result<AfterArgs> {
        if self.email.is_some() && self.password.is_some() {
            debug!("auth: email+password");
        } else if self.read_token_from_stdin {
            debug!("auth: token");
        } else if self.email.is_none() && self.email.is_none() {
            debug!("auth: *no auth*");
        } else {
            bail!(r#"You must combine switch in valid way.
Possible situation:
a) provide both email and password
b) provide token (can be grubbed from external tool)
c) leave blank all switch (no login)"#)
        }
        Ok(AfterArgs {
            login_info: self.email.and_then(|email| self.password.map(|password| LoginInfo {
                email,
                password,
                totp: self.totp,
            })),
            sub_command: self.sub_command,
            log_level: self.log_level,
            read_token_from_stdin: self.read_token_from_stdin,
        })
    }
}

#[derive(Debug)]
pub struct AfterArgs {
    pub login_info: Option<LoginInfo>,
    pub sub_command: ToolSubCommand,
    pub log_level: LogLevel,
    pub read_token_from_stdin: bool,
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
        to: Vec<String>,
    },
}

pub fn init_fern(log_level: LogLevel) -> anyhow::Result<(), fern::InitError> {
    let colors = ColoredLevelConfig::new();

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                colors.color(record.level()),
                message
            ))
        })
        .level(log_level.into())
        .chain(std::io::stderr())
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

pub static ARGS: OnceCell<Arc<Mutex<AfterArgs>>> = OnceCell::new();

#[derive(EnumString, StrumDisplay, Copy, Clone, Debug, Eq, PartialEq)]
pub enum LogLevel {
    #[strum(serialize = "none")]
    None,
    #[strum(serialize = "error")]
    Error,
    #[strum(serialize = "warn")]
    Warn,
    #[strum(serialize = "info")]
    Info,
    #[strum(serialize = "debug")]
    Debug,
}

impl Into<LevelFilter> for LogLevel {
    fn into(self) -> LevelFilter {
        match self {
            LogLevel::None => LevelFilter::Off,
            LogLevel::Error => LevelFilter::Error,
            LogLevel::Warn => LevelFilter::Warn,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Debug => LevelFilter::Debug,
        }
    }
}