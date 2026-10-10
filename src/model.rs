use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use url::Url;
use derive_more::{Display, FromStr};
use std::str::FromStr;
use email_address::EmailAddress;
use serde::{Serialize, Deserialize, Deserializer};
use anyhow::ensure;
use base64::Engine;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use uuid::Uuid;
use crate::cli::OneTimePassword;

#[derive(Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct UserId(String);

impl FromStr for UserId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ensure!(s.starts_with("U-"), "An UserId must be prefixed with `U-`");
        Ok(Self(s.to_string()))
    }
}

// TODO: "R-" {GUID}という形式に沿ってパースする
#[derive(FromStr, Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
/// This is thin pointer to the actual Record. It is unique, and has one-by-one relation with Record.
pub struct RecordId(pub String);

#[derive(Display, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct GroupId(String);

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

impl SessionToken {
    pub const fn new(inner: String) -> Self {
        Self(inner)
    }
}

#[derive(Serialize, Debug, Eq, PartialEq, Clone)]
#[serde(untagged)]
pub enum LoginInfo {
    ByPassword {
        #[serde(flatten)]
        user_identify_pointer: UserIdentifyPointer,
        password: Password,
        totp: Option<OneTimePassword>,
    },
    ByTokenFromStdin {
        user_id: UserId,
    }
}

impl LoginInfo {
    pub const fn get_totp(&self) -> &Option<OneTimePassword> {
        match self {
            Self::ByPassword { totp, .. } => totp,
            Self::ByTokenFromStdin { .. } => &None,
        }
    }
}

#[derive(Serialize, Debug, Eq, PartialEq, Clone)]
#[serde(untagged)]
pub enum UserIdentifyPointer {
    Email {
        email: EmailAddress,
    },
    UserId {
        #[serde(rename = "ownerId")]
        user_id: UserId
    },
}

impl UserIdentifyPointer {
    pub const fn email(value: EmailAddress) -> Self {
        Self::Email {
            email: value
        }
    }

    pub const fn user_id(value: UserId) -> Self {
        Self::UserId {
            user_id: value
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
/// body: POST /userSessions
pub struct UserLoginPostBody {
    #[serde(flatten)]
    login_method: LoginInfo,
    #[serde(rename = "secretMachineId")]
    generated_machine_id: String,
    remember_me: bool,
}

/// response: POST /userSessions
impl UserLoginPostBody {
    pub fn for_platform(self, platform: crate::cli::Platform) -> serde_json::Value {
        if platform == crate::cli::Platform::Neos {
            return serde_json::to_value(self).expect("login body must be serializable");
        }
        let LoginInfo::ByPassword { user_identify_pointer, password, .. } = self.login_method else {
            unreachable!("token authentication does not create a login request")
        };
        let mut body = serde_json::json!({
            "authentication": { "$type": "password", "password": password },
            "secretMachineId": self.generated_machine_id,
            "rememberMe": self.remember_me,
        });
        match user_identify_pointer {
            UserIdentifyPointer::Email { email } => body["email"] = serde_json::json!(email),
            UserIdentifyPointer::UserId { user_id } => body["userId"] = serde_json::json!(user_id),
        }
        body
    }

    pub fn create(login_method: LoginInfo, remember_me: bool) -> Self {
        use base64::engine::GeneralPurpose as Base64Engine;
        let random_uuid = Uuid::new_v4().to_string();
        let random_uuid = random_uuid.as_bytes();
        static BASE64_ENGINE: Base64Engine = Base64Engine::new(
            &base64::alphabet::URL_SAFE,
            base64::engine::GeneralPurposeConfig::new().with_encode_padding(false)
        );
        let nonce = BASE64_ENGINE.encode(random_uuid);

        Self {
            login_method,
            generated_machine_id: nonce,
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
    pub fn as_authorization_header_value(&self, platform: crate::cli::Platform) -> String {
        let val = format!("{scheme} {owner_id}:{auth_token}", scheme = platform.authorization_scheme(), owner_id = self.owner_id.0, auth_token = self.token.0);
        val
    }

    pub const fn new(owner_id: UserId, token: SessionToken) -> Self {
        Self {
            owner_id,
            token,
        }
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
pub enum RecordOwner {
    User(UserId),
    Group(GroupId),
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum RecordType {
    Directory,
    Object,
    Texture,
    Audio,
    Link,
    Other(String),
}

impl Display for RecordType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Directory => "Directory",
            Self::Object => "Object",
            Self::Texture => "Texture",
            Self::Audio => "Audio",
            Self::Link => "Link",
            Self::Other(value) => value,
        })
    }
}

impl Serialize for RecordType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RecordType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(match value.to_ascii_lowercase().as_str() {
            // this is INTENTIONALLY loose, as the API *may* returns both uppercase and lowercase variant.
            // This does not seem to have rule(s), so I chose let this side be loose.
            "directory" | "Directory" => Self::Directory,
            "object" | "Object" => Self::Object,
            "texture" | "Texture" => Self::Texture,
            "audio" | "Audio" => Self::Audio,
            "link" | "Link" => Self::Link,
            _ => Self::Other(value),
        })
    }
}

// Retain the wire format so records can be sent back to the selected API.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum RecordVersion {
    Resonite { version: VersionMetadata },
    Neos(VersionMetadata),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetadata {
    pub global_version: i32,
    pub local_version: i32,
    #[serde(rename = "lastModifyingUserId", default)]
    // 壊れたフォルダーだと欠けている場合がある (?!)
    pub last_update_by: Option<UserId>,
    #[serde(rename = "lastModifyingMachineId", default)]
    // Essential Toolsだと欠けている
    pub last_update_machine: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
// fields are only for serde-integrations. I'd like to export them in JSON.
#[allow(dead_code, clippy::struct_excessive_bools)]
/// <https://neos-api.polylogix.studio/#tag/Records/operation/getRecordAtPath>
pub struct Record {
    pub id: RecordId,
    /// ## Format
    /// `neosdb:///...`, in URI format
    ///
    /// ## Note
    /// This field is absent when self.record_type == "directory"
    #[serde(default)]
    pub asset_uri: Option<Url>,
    #[serde(flatten)]
    pub version: RecordVersion,
    pub name: String,
    pub record_type: RecordType,
    #[serde(default)]
    // Essential Toolsだと欠けている
    pub owner_name: Option<String>,
    #[serde(default)]
    // Essential Toolsだと欠けている
    pub tags: Vec<String>,
    #[serde(default)]
    pub path: String,
    pub is_public: bool,
    pub is_for_patrons: bool,
    pub is_listed: bool,
    pub is_deleted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_read_only: Option<bool>,
    #[serde(default)]
    // Essential Toolsだと欠けている
    pub thumbnail_uri: Option<Url>,
    #[serde(rename = "creationTime", default)]
    // Essential Toolsだと欠けている
    created_at: Option<DateTime<Utc>>,
    #[serde(rename = "lastModificationTime", deserialize_with = "fallback_to_utc")]
    updated_at: DateTime<Utc>,
    pub random_order: i32,
    pub visits: i32,
    pub rating: f64,
    #[serde(default)]
    // Essential Toolsだと欠けていることがある
    pub owner_id: Option<RecordOwner>,
    #[serde(default)]
    pub submissions: Vec<Submission>
}

/// Essential Toolsだとタイムゾーンが欠けているのでパースに失敗する (?!)
/// see: <https://github.com/Neos-Metaverse/NeosPublic/issues/3714>
fn fallback_to_utc<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    /// see <https://users.rust-lang.org/t/serde-clone-deserializer/49568/3>
    enum LocalHack {
        WithTimeZone(DateTime<Utc>),
        WithoutTimeZone(NaiveDateTime),
    }

    let utc_date_time = match LocalHack::deserialize(deserializer)? {
        LocalHack::WithTimeZone(utc_date_time) => utc_date_time,
        LocalHack::WithoutTimeZone(naive_date_time) => {
            let dt = naive_date_time;
            Utc.from_local_datetime(&dt).unwrap()
        }
    };

    Ok(utc_date_time)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Submission {
    id: String,
    owner_id: UserId,
    target_record_id: RecordId,
    submission_time: DateTime<Utc>,
    submitted_by_id: String,
    submitted_by_name: String,
    #[serde(rename = "featured")]
    is_featured: bool,
    featured_by_user_id: String,
    #[serde(default)]
    featured_timestamp: Option<DateTime<Utc>>
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
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

/// インベントリのルートを起点とする絶対パスを表現する。
/// 要素に`.`や`..`が入っていても、特別な意味を持たず、文字通り扱われることに注意。
#[derive(Eq, PartialEq, Default, Debug, Clone)]
pub struct AbsoluteInventoryPath {
    inner: Vec<String>
}

impl AbsoluteInventoryPath {
    pub fn to_uri_query_value(&self) -> String {
        self.inner.join("%5C")
    }

    pub fn to_absolute_path(&self) -> String {
        self.inner.join("/")
    }
}

impl FromStr for AbsoluteInventoryPath {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self { inner: s.split('/').map(std::string::ToString::to_string).collect() })
    }
}

// For clap
impl Display for AbsoluteInventoryPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_absolute_path().as_str())
    }
}

#[cfg(test)]
mod platform_tests {
    use super::*;
    use crate::cli::Platform;

    #[test]
    fn authentication_matches_selected_platform() {
        let auth = AuthorizationInfo::new(UserId("U-test".into()), SessionToken::new("token".into()));
        assert_eq!(auth.as_authorization_header_value(Platform::Neos), "neos U-test:token");
        assert_eq!(auth.as_authorization_header_value(Platform::Resonite), "res U-test:token");
        for pointer in [UserIdentifyPointer::email("test@example.com".parse().unwrap()), UserIdentifyPointer::user_id(UserId("U-test".into()))] {
            let login = LoginInfo::ByPassword { user_identify_pointer: pointer.clone(), password: Password("secret".into()), totp: None };
            let neos = UserLoginPostBody::create(login.clone(), false).for_platform(Platform::Neos);
            assert_eq!(neos["password"], "secret");
            let resonite = UserLoginPostBody::create(login, false).for_platform(Platform::Resonite);
            assert_eq!(resonite["authentication"]["$type"], "password");
            assert_eq!(resonite["authentication"]["password"], "secret");
            assert!(resonite.get("password").is_none());
            match pointer {
                UserIdentifyPointer::Email { .. } => assert_eq!(resonite["email"], "test@example.com"),
                UserIdentifyPointer::UserId { .. } => assert_eq!(resonite["userId"], "U-test"),
            }
        }
    }
}


#[cfg(test)]
mod record_tests {
    use super::*;

    fn record_json() -> serde_json::Value {
        serde_json::json!({
            "id": "R-test", "name": "test", "recordType": "Object",
            "assetUri": "resdb:///test", "ownerId": "U-test",
            "isPublic": false, "isForPatrons": false, "isListed": false,
            "isDeleted": false, "isReadOnly": true,
            "lastModificationTime": "2026-10-11T00:00:00Z",
            "randomOrder": 0, "visits": 0, "rating": 0.0,
            "version": {"globalVersion": 7, "localVersion": 3, "lastModifyingUserId": "U-test"}
        })
    }

    #[test]
    fn resonite_records_preserve_nested_version_and_read_only() {
        let original = record_json();
        let record: Record = serde_json::from_value(original.clone()).unwrap();
        assert_eq!(record.path, "");
        assert!(record.tags.is_empty());
        let output = serde_json::to_value(record).unwrap();
        assert_eq!(output["version"]["globalVersion"], 7);
        assert_eq!(output["version"]["localVersion"], 3);
        assert_eq!(output["version"]["lastModifyingUserId"], "U-test");
        assert_eq!(output["isReadOnly"], true);
        assert!(output.get("globalVersion").is_none());
    }

    #[test]
    fn neos_records_keep_flat_version_and_existing_path() {
        let mut original = record_json();
        let version = original.as_object_mut().unwrap().remove("version").unwrap();
        original.as_object_mut().unwrap().extend(version.as_object().unwrap().clone());
        original.as_object_mut().unwrap().remove("isReadOnly");
        original["path"] = serde_json::json!("Inventory\\Test");
        let record: Record = serde_json::from_value(original).unwrap();
        assert_eq!(record.path, "Inventory\\Test");
        let output = serde_json::to_value(record).unwrap();
        assert_eq!(output["globalVersion"], 7);
        assert_eq!(output["localVersion"], 3);
        assert!(output.get("version").is_none());
        assert!(output.get("isReadOnly").is_none());
    }

    #[test]
    fn additional_record_types_round_trip_without_losing_their_names() {
        for kind in ["World", "world", "Mesh", "Material", "FutureRecordType"] {
            let mut json = record_json();
            json["recordType"] = serde_json::json!(kind);
            let record: Record = serde_json::from_value(json).unwrap();
            assert_eq!(record.record_type, RecordType::Other(kind.into()));
            assert_eq!(serde_json::to_value(record).unwrap()["recordType"], kind);
        }
        for kind in ["Directory", "directory", "DIRECTORY"] {
            let record_type: RecordType = serde_json::from_value(serde_json::json!(kind)).unwrap();
            assert_eq!(record_type, RecordType::Directory);
        }
    }

    #[test]
    fn malformed_or_missing_versions_are_rejected() {
        for version in [serde_json::Value::Null, serde_json::json!({"globalVersion":7}), serde_json::json!({"globalVersion":"invalid", "localVersion":3})] {
            let mut record = record_json();
            record["version"] = version;
            assert!(serde_json::from_value::<Record>(record).is_err());
        }
    }
}
