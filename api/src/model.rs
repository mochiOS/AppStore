use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct BundleInput {
    pub bundle_id: String,
    pub app_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[serde(default)]
    pub age_rating: Option<String>,
}

fn default_kind() -> String {
    "app".into()
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseInput {
    pub version: String,
    pub repository: String,
    pub release_tag: String,
    pub asset: String,
    pub certificate_id: String,
    #[serde(default)]
    pub changelog: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GitHubReleaseAssetRequest<'a> {
    pub owner: &'a str,
    pub repository: &'a str,
    pub release_tag: &'a str,
    pub asset_name: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct AccountsReleaseAssetEnvelope {
    pub account_id: String,
    pub release_asset: GitHubReleaseAsset,
}

#[derive(Debug)]
pub struct VerifiedGitHubReleaseAsset {
    pub account_id: String,
    pub release_asset: GitHubReleaseAsset,
}

#[derive(Debug, Deserialize)]
pub struct GitHubReleaseAsset {
    pub repository_id: u64,
    pub repository: String,
    pub repository_permission: String,
    pub release_id: u64,
    pub release_tag: String,
    pub immutable: bool,
    pub prerelease: bool,
    pub asset_id: u64,
    pub asset_name: String,
    pub download_url: String,
    pub file_size: u64,
    pub github_digest: Option<String>,
    pub asset_created_at: String,
    pub asset_updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationInput {
    pub release_id: String,
    pub asset_id: u64,
    pub validation_attempt_id: String,
    pub reviewer_version: String,
    pub validated_at: u64,
    pub package_id: String,
    pub version: String,
    pub file_size: u64,
    pub asset_sha256: String,
    pub package_digest: String,
    pub manifest_digest: String,
    pub signature: String,
    pub certificate_id: String,
    pub certificate_serial: String,
    pub certificate_subject_key_id: String,
    pub certificate_developer_id: String,
    pub certificate_issuer_key_id: String,
    pub capabilities: Vec<String>,
    pub payloads: Vec<PayloadReport>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationFailureInput {
    pub release_id: String,
    pub asset_id: u64,
    pub validation_attempt_id: String,
    pub reviewer_version: String,
    pub validated_at: u64,
    pub error_code: String,
    pub error_summary: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadReport {
    pub file_id: String,
    pub container_path: String,
    pub install_path: String,
    pub size: u64,
    pub sha256: String,
    pub mode: String,
}

#[derive(Debug, Deserialize)]
pub struct RejectInput {
    pub reason_code: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuspensionInput {
    pub reason: String,
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
    pub developer_id: String,
    pub description: String,
    pub icon: Option<String>,
    pub subtitle: Option<String>,
    pub category: Option<String>,
    pub kind: String,
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
    pub package_digest: String,
    pub changelog: Option<String>,
    pub review_status: String,
    pub publish_status: String,
    pub download_url: String,
    pub github_repository: String,
    pub github_release_tag: String,
    pub github_asset_id: i64,
    pub asset_name: String,
    pub developer_certificate_id: String,
    pub created_at: i64,
}
