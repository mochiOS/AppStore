use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct BundleInput {
    pub bundle_id: String,
    pub app_name: String,
}

#[derive(Debug, Deserialize)]
pub struct AppInput {
    pub bundle_id: String,
    pub display_name: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default = "default_price")]
    pub price_label: String,
    #[serde(default)]
    pub age_rating: Option<String>,
}

fn default_kind() -> String {
    "app".into()
}
fn default_price() -> String {
    "入手".into()
}

#[derive(Debug, Deserialize)]
pub struct ReleaseInput {
    pub version: String,
    pub package_size: u64,
    pub package_sha256: String,
    #[serde(default)]
    pub manifest_hash: Option<String>,
    pub signature: String,
    pub certificate_id: String,
    #[serde(default)]
    pub changelog: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RejectInput {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct KeyInput {
    pub key_id: String,
    pub public_key: String,
    pub fingerprint: String,
}

#[derive(Debug, Deserialize)]
pub struct TeamInput {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MemberInput {
    pub developer_id: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct TeamAssignment {
    pub team_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublicApp {
    pub bundle_id: String,
    pub name: String,
    pub version: String,
    pub developer: String,
    pub description: String,
    pub icon: Option<String>,
    pub subtitle: Option<String>,
    pub category: Option<String>,
    pub kind: String,
    pub price_label: String,
    pub age_rating: Option<String>,
    pub rating: Option<f64>,
    pub rating_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseView {
    pub release_id: String,
    pub bundle_id: String,
    pub version: String,
    pub size: i64,
    pub sha256: String,
    pub changelog: Option<String>,
    pub status: String,
    pub download_url: String,
    pub created_at: i64,
}
