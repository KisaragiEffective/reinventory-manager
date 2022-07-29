use std::str::FromStr;
use std::sync::{Arc, Mutex, MutexGuard};
use clap::{Parser, Subcommand};
use derive_more::{FromStr, Display};
use log::{debug, info};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize};
use serde::de::Error;
use url::Url;
use email_address::EmailAddress;
use reqwest::header::AUTHORIZATION;
use anyhow::{ensure, Result};

#[derive(Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct UserId(String);

impl FromStr for UserId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        ensure!(s.starts_with("U-"), "An UserId must be prefixed with `U-`");
        Ok(Self(s.to_string()))
    }
}

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct RecordId(String);

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct GroupId(String);

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct Password(String);

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct SessionToken(String);

#[derive(Parser, Debug)]
struct Args {
    #[clap(short, long)]
    email: Option<EmailAddress>,
    #[clap(short, long)]
    password: Option<Password>,
    #[clap(subcommand)]
    sub_command: ToolSubCommand,
}

impl Args {
    fn validate(self) -> Result<AfterArgs> {
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

#[derive(Debug, Eq, PartialEq)]
struct LoginInfo {
    email: EmailAddress,
    password: Password,
}

#[derive(Debug)]
struct AfterArgs {
    login_info: Option<LoginInfo>,
    sub_command: ToolSubCommand,
}

#[derive(Subcommand, Debug)]
enum ToolSubCommand {
    List {
        #[clap(short = 'd', long, default_value_t = 1)]
        max_depth: usize,
        #[clap(default_values_t = Vec::<String>::new())]
        base_dir: Vec<String>,
        #[clap(short = 'u', long)]
        target_user: Option<UserId>,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
/// body: POST /userSessions
struct UserLoginPostBody {
    email: EmailAddress,
    password: Password,
    session_token: Option<()>,
    remember_me: bool,
}

/// response: POST /userSessions
impl UserLoginPostBody {
    fn create(email: EmailAddress, password: Password, remember_me: bool) -> Self {
        Self {
            email,
            password,
            session_token: None,
            remember_me
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserLoginPostResponse {
    user_id: UserId,
    token: SessionToken,
}

impl UserLoginPostResponse {
    fn to_authorization_info(&self) -> AuthorizationInfo {
        AuthorizationInfo {
            owner_id: self.user_id.clone(),
            token: self.token.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct AuthorizationInfo {
    owner_id: UserId,
    token: SessionToken,
}

impl AuthorizationInfo {
    fn as_authorization_header_value(&self) -> String {
        let val = format!("neos {owner_id}:{auth_token}", owner_id = self.owner_id.0, auth_token = self.token.0);
        debug!("auth: {val}");
        val
    }
}

#[derive(Debug, Clone)]
struct LoginResponse {
    using_token: AuthorizationInfo,
    user_id: UserId,
}

#[derive(Serialize)]
struct RecordGetBody {
    owner_user_id: UserId,
    record_id: RecordId,
}

#[derive(Deserialize, Debug, Clone)]
enum RecordOwner {
    User(UserId),
    Group(GroupId),
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum RecordType {
    Directory,
    Object,
    Texture,
    Audio,
    Link
}

impl<'de> Deserialize<'de> for RecordType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        match String::deserialize(deserializer)?.as_str() {
            "directory" => Ok(Self::Directory),
            "object" => Ok(Self::Object),
            "texture" => Ok(Self::Texture),
            "audio" => Ok(Self::Audio),
            "link" => Ok(Self::Link),
            _ => Err(D::Error::custom("dir | obj | text | aud | lnk")),
        }
    }
}

#[derive(Deserialize, Debug)]
/// https://neos-api.polylogix.studio/#tag/Records/operation/getRecordAtPath
struct PathPointedRecordResponse {
    id: RecordId,
    owner_id: RecordOwner,
    /// neosdb:///...
    asset_uri: Url,
    name: String,
    desciption: String,
    record_type: RecordType,
    owner_name: String,
    tags: Vec<String>,
    path: String,
    is_public: bool,
}

struct Operation;

impl Operation {
    async fn login() -> Option<LoginResponse> {
        let client = reqwest::Client::new();
        debug!("post");
        if let Some(auth) = &get_args_lock().login_info {
            let email = auth.email.clone();
            let password = auth.password.clone();
            let token_res = client
                .post(format!("{BASE_POINT}/userSessions"))
                .json(&UserLoginPostBody::create(email, password, false))
                .send();

            debug!("post 2");
            let token_res = token_res
                .await
                .unwrap()
                .json::<UserLoginPostResponse>()
                .await
                .unwrap();

            debug!("post 3");
            let using_token = (&token_res).to_authorization_info();
            let user_id = token_res.user_id;

            debug!("post 4");
            Some(LoginResponse {
                using_token,
                user_id,
            })
        } else {
            None
        }
    }

    async fn logout(owner_id: UserId, authorization_info: AuthorizationInfo) {
        let client = reqwest::Client::new();
        client
            .delete(format!("{BASE_POINT}/userSessions/{owner_id}/{auth_token}", auth_token = authorization_info.token))
            .header(AUTHORIZATION, authorization_info.as_authorization_header_value())
            .send()
            .await
            .unwrap();
    }

    async fn get_record_at_path(owner_id: UserId, path: Vec<String>, authorization_info: &Option<AuthorizationInfo>) -> Vec<PathPointedRecordResponse> {
        let client = reqwest::Client::new();
        let path = path.join("%5C");
        // NOTE:
        // https://api.neos.com/api/users/U-kisaragi-marine/records/root/Inventory/Test <-- これはディレクトリのメタデータを単体で返す


        // NOTE: これはドキュメントが古い。 (thanks kazu)
        // あと、Personalから始めるのではなく、Inventoryから先頭のバックスラッシュなしでパスを指定する点に注意
        let endpoint = if path.is_empty() {
            format!("{BASE_POINT}/users/{owner_id}/records/root")
        } else {
            format!("{BASE_POINT}/users/{owner_id}/records?path={path}")
        };

        debug!("endpoint: {endpoint}", endpoint = &endpoint);
        let mut res = client.get(endpoint);

        if let Some(authorization_info) = authorization_info {
            res = res.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
        }

        let res = res
            .send()
            .await
            .unwrap()
            .json::<Vec<PathPointedRecordResponse>>()
            .await
            .unwrap();

        res
    }
}

fn init_fern() -> Result<(), fern::InitError> {
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
        .chain(std::io::stdout())
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

static ARGS: OnceCell<Arc<Mutex<AfterArgs>>> = OnceCell::new();
static BASE_POINT: &str = "https://api.neos.com/api";

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();
    let args = args.validate().unwrap();
    init_fern().unwrap();
    debug!("fern initialized");

    ARGS.set(Arc::new(Mutex::new(args))).expect("once_cell error!!");

    debug!("login...");
    let login_res = Operation::login().await;
    debug!("done.");

    let sub_command = { &get_args_lock().sub_command };
    match sub_command {
        ToolSubCommand::List { max_depth, base_dir, target_user } => {
            println!("Inventory:");
            let xs = Operation::get_record_at_path(
                // TODO: ここのエラー表示が終わってるので要改善
                target_user.clone().unwrap_or_else(|| login_res.clone().unwrap().user_id),
                base_dir.clone(),
                &login_res.clone().map(|a| a.using_token)
            ).await;
            for x in xs {
                println!("{:?}", x);
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
