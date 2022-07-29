use url::Url;
use derive_more::{Display, FromStr};
use std::str::FromStr;
use email_address::EmailAddress;
use serde::{Serialize, Deserialize, Deserializer};
use serde::de::Error;
use anyhow::ensure;
use chrono::{DateTime, Utc};
use log::debug;

#[derive(Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct UserId(String);

impl FromStr for UserId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        ensure!(s.starts_with("U-"), "An UserId must be prefixed with `U-`");
        Ok(Self(s.to_string()))
    }
}

// TODO: "R-" {GUID}という形式に沿ってパースする
#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct RecordId(String);

#[derive(Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
struct GroupId(String);

impl FromStr for GroupId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ensure!(s.starts_with("G-"), "A valid GroupId must be prefixed with `G-`");
        Ok(Self(s.to_string()))
    }
}

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct Password(String);

#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct SessionToken(String);

#[derive(Debug, Eq, PartialEq)]
pub struct LoginInfo {
    pub email: EmailAddress,
    pub password: Password,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
/// body: POST /userSessions
pub struct UserLoginPostBody {
    email: EmailAddress,
    password: Password,
    session_token: Option<()>,
    remember_me: bool,
}

/// response: POST /userSessions
impl UserLoginPostBody {
    pub fn create(email: EmailAddress, password: Password, remember_me: bool) -> Self {
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
pub struct UserLoginPostResponse {
    pub user_id: UserId,
    pub token: SessionToken,
}

impl UserLoginPostResponse {
    pub fn to_authorization_info(&self) -> AuthorizationInfo {
        AuthorizationInfo {
            owner_id: self.user_id.clone(),
            token: self.token.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthorizationInfo {
    pub owner_id: UserId,
    pub token: SessionToken,
}

impl AuthorizationInfo {
    pub fn as_authorization_header_value(&self) -> String {
        let val = format!("neos {owner_id}:{auth_token}", owner_id = self.owner_id.0, auth_token = self.token.0);
        debug!("auth: {val}");
        val
    }
}

#[derive(Debug, Clone)]
pub struct LoginResponse {
    pub using_token: AuthorizationInfo,
    pub user_id: UserId,
}

#[derive(Serialize)]
struct RecordGetBody {
    owner_user_id: UserId,
    record_id: RecordId,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum RecordOwner {
    User(UserId),
    Group(GroupId),
}

#[derive(Serialize, Debug, Clone, Eq, PartialEq)]
enum RecordType {
    Directory,
    Object,
    Texture,
    Audio,
    Link
}

impl<'de> Deserialize<'de> for RecordType {
    fn deserialize<D>(deserializer: D) -> anyhow::Result<Self, D::Error> where D: Deserializer<'de> {
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
// fields are only for serde-integrations. I'd like to export them in JSON.
#[allow(dead_code)]
/// https://neos-api.polylogix.studio/#tag/Records/operation/getRecordAtPath
pub struct RecordWithoutDescription {
    id: RecordId,
    owner_id: RecordOwner,
    /// neosdb:///...
    asset_uri: Url,
    name: String,
    record_type: RecordType,
    owner_name: String,
    tags: Vec<String>,
    path: String,
    is_public: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryMetadata {
    id: RecordId,
    global_version: u32,
    local_version: u32,
    #[serde(rename = "lastModifyingUserId")]
    last_modify_user: UserId,
    #[serde(rename = "lastModifyingMachineId")]
    last_modify_machine_user: String,
    name: String,
    // recordType is always directory, so omitted.
    owner_name: String,
    path: String,
    is_public: bool,
    is_for_patrons: bool,
    is_listed: bool,
    is_deleted: bool,
    #[serde(rename = "creationTime")]
    created_at: DateTime<Utc>,
    #[serde(rename = "lastModificationTime")]
    updated_at: DateTime<Utc>,

}
