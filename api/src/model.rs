use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppUpdateInput {
    pub display_name: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotInput {
    pub image_url: String,
    #[serde(default)]
    pub contains_actual_app_ui: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataCategoryInput {
    pub category: String,
    #[serde(default)]
    pub details: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmissionDraftInput {
    pub build_id: String,
    pub submission_kind: String,
    #[serde(default)]
    pub previous_submission_id: Option<String>,
    pub app_name: String,
    pub developer_name: String,
    pub description: String,
    pub icon_url: String,
    pub icon_media_type: String,
    pub icon_width: u32,
    pub icon_height: u32,
    #[serde(default)]
    pub category: Option<String>,
    pub kind: String,
    #[serde(default = "default_release_channel")]
    pub release_channel: String,
    #[serde(default = "default_primary_purpose")]
    pub primary_purpose: String,
    #[serde(default)]
    pub age_rating: Option<String>,
    pub screenshots: Vec<ScreenshotInput>,
    #[serde(default)]
    pub capability_reasons: HashMap<String, String>,
    #[serde(default)]
    pub external_communication: bool,
    #[serde(default)]
    pub external_communication_reason: Option<String>,
    #[serde(default)]
    pub external_communication_purpose: Option<String>,
    #[serde(default)]
    pub external_domains: Vec<String>,
    #[serde(default)]
    pub collects_data: bool,
    #[serde(default)]
    pub data_collection_description: Option<String>,
    #[serde(default)]
    pub data_categories: Vec<DataCategoryInput>,
    #[serde(default)]
    pub uses_advertising: bool,
    #[serde(default)]
    pub uses_analytics: bool,
    #[serde(default)]
    pub tracks_across_services: bool,
    #[serde(default)]
    pub tracking_user_consent: bool,
    #[serde(default)]
    pub uses_location_for_advertising: bool,
    #[serde(default)]
    pub has_payments: bool,
    #[serde(default)]
    pub content_declarations: serde_json::Value,
    #[serde(default)]
    pub executes_dynamic_code: bool,
    #[serde(default)]
    pub dynamic_code_explanation: Option<String>,
    #[serde(default)]
    pub uses_external_updates: bool,
    #[serde(default)]
    pub external_updates_explanation: Option<String>,
    #[serde(default)]
    pub is_emulator: bool,
    #[serde(default)]
    pub is_virtual_machine: bool,
    #[serde(default)]
    pub supports_plugins: bool,
    #[serde(default)]
    pub is_external_app_store: bool,
    #[serde(default)]
    pub uses_ai_generated_content: bool,
    #[serde(default)]
    pub disclose_ai_generated_content: bool,
    #[serde(default)]
    pub reviewer_notes: Option<String>,
    #[serde(default)]
    pub requires_login: bool,
    #[serde(default)]
    pub test_account: Option<String>,
    #[serde(default)]
    pub test_instructions: Option<String>,
}

fn default_release_channel() -> String {
    "stable".into()
}

fn default_primary_purpose() -> String {
    "general".into()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmissionMessageInput {
    pub body: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppealInput {
    pub submission_id: Option<String>,
    pub appealed_action: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnpublishInput {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CertificateReplacementInput {
    pub certificate_id: String,
    pub confirmation: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDecisionInput {
    pub decision: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemovalInput {
    pub reason: String,
    pub confirmation: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppealResolutionInput {
    pub outcome: String,
    pub reason: String,
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
