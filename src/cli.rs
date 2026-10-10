use std::io::IsTerminal;
use clap::{Parser, Subcommand};
use email_address::EmailAddress;
use anyhow::{bail, Result};
use fern::colors::ColoredLevelConfig;
use log::{debug, LevelFilter, warn};
use derive_more::{Display, FromStr};
use strum::{EnumString, Display as StrumDisplay};
use serde::Serialize;
use crate::model::{AbsoluteInventoryPath, LoginInfo, Password, RecordId, UserId, UserIdentifyPointer};

#[derive(Parser, Debug)]
pub struct Args {
    #[clap(short, long)]
    email: Option<EmailAddress>,
    #[clap(short, long)]
    password: Option<Password>,
    #[clap(short, long)]
    totp: Option<OneTimePassword>,
    #[clap(short, long)]
    user_id: Option<UserId>,
    #[clap(long, default_value_t = LogLevel::Warn)]
    log_level: LogLevel,
    #[clap(long)]
    read_token_from_stdin: bool,
    #[clap(long)]
    keep_record_id: bool,
    #[clap(short, long = "color", default_value_t = ColorPolicy::Auto)]
    color_policy: ColorPolicy,
    #[clap(long)]
    platform: Option<Platform>,
    #[clap(subcommand)]
    sub_command: ToolSubCommand,
}

#[derive(EnumString, Display, Copy, Clone, Eq, PartialEq, Debug)]
#[strum(ascii_case_insensitive)]
pub enum ColorPolicy {
    Always,
    Auto,
    Never,
}

#[derive(EnumString, Display, Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum Platform {
    Neos,
    Resonite,
}

impl Platform {
    pub const fn base_url(self) -> &'static str {
        match self {
            Self::Neos => "https://api.neos.com/api",
            Self::Resonite => "https://api.resonite.com",
        }
    }

    pub const fn authorization_scheme(self) -> &'static str {
        match self {
            Self::Neos => "neos",
            Self::Resonite => "res",
        }
    }
}

#[derive(Serialize, Display, FromStr, Debug, Eq, PartialEq, Clone)]
pub struct OneTimePassword(pub String);

impl Args {
    pub fn validate(self) -> Result<AfterArgs> {
        let login_info = if let Some(password) = self.password {
            match (self.email, self.user_id) {
                (Some(_), Some(_)) => {
                    bail!("You can not provide both --email and --user-id.")
                }
                (Some(email), None) => {
                    Some(LoginInfo::ByPassword {
                        user_identify_pointer: UserIdentifyPointer::email(email),
                        password,
                        totp: self.totp
                    })
                }
                (None, Some(user_id)) => {
                    Some(LoginInfo::ByPassword {
                        user_identify_pointer: UserIdentifyPointer::user_id(user_id),
                        password,
                        totp: self.totp
                    })
                }
                (None, None) => {
                    bail!("You must provide --email or --user-id if --password is given.")
                }
            }
        } else if self.read_token_from_stdin {
            if let Some(user_id) = self.user_id {
                debug!("auth: userid+token");
                Some(LoginInfo::ByTokenFromStdin {
                    user_id
                })
            } else {
                bail!("You must provide --user-id if --read-token-from-stdin is given.")
            }
        } else {
            debug!("auth: no login");
            None
        };

        let colored = match self.color_policy {
            ColorPolicy::Always => true,
            ColorPolicy::Auto => std::io::stdout().is_terminal(),
            ColorPolicy::Never => false
        };

        let platform = self.platform.unwrap_or_else(|| {
            warn!("Deprecated (implicitly implying --platform): in the next major version, the --platform flag would be require to set manually.\
                To fix this warning, include `--platform Neos` your command line.");
            Platform::Neos
        });

        Ok(AfterArgs {
            login_info,
            sub_command: self.sub_command,
            log_level: self.log_level,
            read_token_from_stdin: self.read_token_from_stdin,
            keep_record_id: self.keep_record_id,
            colored,
            platform,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AfterArgs {
    pub login_info: Option<LoginInfo>,
    pub sub_command: ToolSubCommand,
    pub log_level: LogLevel,
    pub read_token_from_stdin: bool,
    pub keep_record_id: bool,
    pub colored: bool,
    pub platform: Platform,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ToolSubCommand {
    List {
        #[clap(short = 'd', long, default_value_t = 1)]
        max_depth: usize,
        #[clap(short = 'u', long)]
        target_user: Option<UserId>,
        #[clap(default_value_t = Default::default())]
        base_dir: AbsoluteInventoryPath,
    },
    Metadata {
        #[clap(short = 'u', long)]
        target_user: Option<UserId>,
        #[clap(default_value_t = Default::default())]
        base_dir: AbsoluteInventoryPath,
    },
    Move {
        #[clap(short = 'u', long)]
        target_user: UserId,
        #[clap(short, long)]
        record_id: Vec<RecordId>,
        #[clap(long)]
        to: Vec<String>,
    },
}

pub fn init_fern(log_level: LogLevel) -> Result<(), fern::InitError> {
    let colors = ColoredLevelConfig::new();

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                colors.color(record.level()),
                message
            ));
        })
        .level(log_level.into())
        .chain(std::io::stderr())
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

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

impl From<LogLevel> for LevelFilter {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::None => Self::Off,
            LogLevel::Error => Self::Error,
            LogLevel::Warn => Self::Warn,
            LogLevel::Info => Self::Info,
            LogLevel::Debug => Self::Debug,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_both_platforms_and_preserves_default() {
        for (option, expected) in [(Some("Resonite"), Platform::Resonite), (Some("Neos"), Platform::Neos), (None, Platform::Neos)] {
            let mut arguments = vec!["reinventory-manager"];
            if let Some(platform) = option {
                arguments.extend(["--platform", platform]);
            }
            arguments.push("list");
            assert_eq!(Args::try_parse_from(arguments).unwrap().validate().unwrap().platform, expected);
        }
    }
}
