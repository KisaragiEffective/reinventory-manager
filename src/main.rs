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

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct UserId(String);

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct RecordId(String);

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct GroupId(String);

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct Password(String);

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone)]
struct AuthToken(String);

// TODO: 他人のインベントリも見れるようにしたら面白いのでは？
#[derive(Parser, Debug)]
struct Args {
    #[clap(short, long)]
    email: EmailAddress,
    #[clap(short, long)]
    password: Password,
    #[clap(subcommand)]
    sub_command: ToolSubCommand,
}

#[derive(Subcommand, Debug)]
enum ToolSubCommand {
    List {
        #[clap(short = 'd', long, default_value_t = 1)]
        max_depth: usize,
        #[clap(default_values_t = Vec::<String>::new())]
        base_dir: Vec<String>
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
    token: AuthToken,
}

impl UserLoginPostResponse {
    fn to_authorization_info(&self) -> AuthorizationInfo {
        AuthorizationInfo {
            owner_id: self.user_id.clone(),
            token: self.token.clone(),
        }
    }
}

struct AuthorizationInfo {
    owner_id: UserId,
    token: AuthToken,
}

impl AuthorizationInfo {
    fn as_authorization_header_value(&self) -> String {
        let val = format!("neos {owner_id}:{auth_token}", owner_id = self.owner_id.0, auth_token = self.token.0);
        debug!("auth: {val}");
        val
    }
}

struct LoginResponse {
    using_token: AuthorizationInfo,
    auth_token: AuthToken,
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
    async fn login() -> LoginResponse {
        let client = reqwest::Client::new();
        debug!("post");
        let email = { get_args_lock().email.clone() };
        let password = { get_args_lock().password.clone() };
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
        let auth_token = (&token_res.token).clone();
        let user_id = token_res.user_id;

        debug!("post 4");
        LoginResponse {
            using_token,
            auth_token,
            user_id,
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

    async fn get_record_at_path(owner_id: UserId, path: Vec<String>, authorization_info: &AuthorizationInfo) -> Vec<PathPointedRecordResponse> {
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
        let res = client.get(endpoint)
            .header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value())
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

static ARGS: OnceCell<Arc<Mutex<Args>>> = OnceCell::new();
static BASE_POINT: &str = "https://api.neos.com/api";

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();
    init_fern().unwrap();
    debug!("fern initialized");

    ARGS.set(Arc::new(Mutex::new(args))).expect("once_cell error!!");

    debug!("login...");
    let login_res = Operation::login().await;
    debug!("done.");

    let sub_command = { &get_args_lock().sub_command };
    match sub_command {
        ToolSubCommand::List { max_depth, base_dir } => {
            println!("Inventory:");
            for x in Operation::get_record_at_path(login_res.user_id.clone(), base_dir.clone(), &login_res.using_token).await {
                println!("{:?}", x);
            }
        }
    }
    let user_id = login_res.user_id;
    Operation::logout(user_id, login_res.using_token).await;
    info!("Logged out");
}

fn get_args_lock<'a>() -> MutexGuard<'a, Args> {
    ARGS.get().unwrap().lock().unwrap()
}
