use std::sync::{Arc, Mutex, MutexGuard};
use clap::Parser;
use derive_more::{FromStr, Display};
use log::{debug, info};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct UserId(String);

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct Password(String);

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone)]
struct AuthToken(String);

#[derive(Parser, Debug)]
struct Args {
    user_id: UserId,
    password: Password,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
/// body: POST /userSessions
struct UserLoginPostBody {
    owner_id: UserId,
    username: Option<()>,
    email: Option<()>,
    password: Password,
    session_token: Option<()>,
    remember_me: bool,
}

/// response: POST /userSessions
impl UserLoginPostBody {
    fn create(owner_id: UserId, password: Password, remember_me: bool) -> Self {
        Self {
            owner_id,
            username: None,
            email: None,
            password,
            session_token: None,
            remember_me
        }
    }
}

#[derive(Deserialize)]
struct UserLoginPostResponse {
    token: AuthToken,
}

impl UserLoginPostResponse {
    fn to_authorization_info(&self, owner: UserId) -> AuthorizationInfo {
        AuthorizationInfo {
            owner_id: owner,
            token: self.token.clone()
        }
    }
}

struct AuthorizationInfo {
    owner_id: UserId,
    token: AuthToken,
}

impl AuthorizationInfo {
    fn as_authorization_header_value(&self) -> String {
        format!("neos {owner_id}:{auth_token}", owner_id = self.owner_id.0, auth_token = self.token.0)
    }
}

struct LoginResponse {
    using_token: AuthorizationInfo,
    auth_token: AuthToken,
}

struct Operation;

impl Operation {
    async fn login() -> LoginResponse {
        let client = reqwest::Client::new();
        let token_res_1 = client.post(format!("{BASE_POINT}/userSessions"))
            .json(&UserLoginPostBody::create(get().user_id.clone(), get().password.clone(), false))
            .send();

        let token_res = token_res_1
            .await
            .unwrap()
            .json::<UserLoginPostResponse>()
            .await
            .unwrap();

        let using_token = (&token_res).to_authorization_info(get().user_id.clone());
        let auth_token = (&token_res.token).clone();

        LoginResponse {
            using_token,
            auth_token,
        }
    }

    async fn logout(owner_id: UserId, auth_token: AuthToken) {
        let client = reqwest::Client::new();
        client
            .delete(format!("{BASE_POINT}/userSessions/{owner_id}/{auth_token}"))
            .send()
            .await
            .unwrap();
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

    let login_res = Operation::login().await;

    todo!("display user information");
    todo!("inventory operation");

    let user_id = get().user_id.clone();
    Operation::logout(user_id, login_res.auth_token).await;
    info!("Logged out");
}

fn get<'a>() -> MutexGuard<'a, Args> {
    ARGS.get().unwrap().lock().unwrap()
}
