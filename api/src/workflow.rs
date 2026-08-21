const RESERVED_APP_NAMES: &[&str] = &[
    "app store",
    "mochios",
    "mochios app store",
    "system settings",
];

pub fn valid_app_name(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.chars().count() <= 128
        && !value.chars().any(is_emoji_character)
        && !RESERVED_APP_NAMES
            .iter()
            .any(|reserved| value.eq_ignore_ascii_case(reserved))
}

fn is_emoji_character(value: char) -> bool {
    matches!(value as u32,
        0x1F000..=0x1FAFF
        | 0x2600..=0x27BF
        | 0x2300..=0x23FF
        | 0x2B00..=0x2BFF
        | 0xFE0F
        | 0x200D
        | 0x20E3
    )
}

pub fn valid_icon(media_type: &str, width: u32, height: u32) -> bool {
    matches!(media_type, "image/png" | "image/jpeg") && width == 512 && height == 512
}

pub fn valid_screenshot_set(count: usize, actual_app_ui_count: usize) -> bool {
    count >= 3 && actual_app_ui_count > 0 && actual_app_ui_count <= count
}

pub fn valid_release_channel_name(name: &str, channel: &str) -> bool {
    match channel {
        "stable" => true,
        "alpha" | "beta" | "experimental" => name
            .split(|character: char| !character.is_alphanumeric())
            .any(|word| word.eq_ignore_ascii_case(channel)),
        _ => false,
    }
}

pub fn valid_external_domain(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 253
        || value != value.to_ascii_lowercase()
        || value.contains(['*', '/', ':'])
        || value.starts_with('.')
        || value.ends_with('.')
    {
        return false;
    }
    let labels = value.split('.').collect::<Vec<_>>();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

pub fn valid_submission_transition(from: &str, to: &str, actor: &str) -> bool {
    matches!(
        (from, to, actor),
        ("draft", "submitted", "developer")
            | ("submitted", "in_review", "reviewer")
            | ("in_review", "approved", "reviewer")
            | ("in_review", "changes_required", "reviewer")
            | ("in_review", "more_information_required", "reviewer")
            | ("in_review", "rejected", "reviewer")
            | ("more_information_required", "in_review", "developer")
    )
}

pub struct DeclarationSummary<'a> {
    pub external_communication: bool,
    pub external_communication_reason: Option<&'a str>,
    pub external_communication_purpose: Option<&'a str>,
    pub external_domains: &'a [&'a str],
    pub collects_data: bool,
    pub data_collection_description: Option<&'a str>,
    pub executes_dynamic_code: bool,
    pub dynamic_code_explanation: Option<&'a str>,
    pub uses_external_updates: bool,
    pub external_updates_explanation: Option<&'a str>,
    pub tracks_across_services: bool,
    pub tracking_user_consent: bool,
    pub uses_location_for_advertising: bool,
    pub requires_login: bool,
    pub test_account: Option<&'a str>,
    pub test_instructions: Option<&'a str>,
}

pub fn valid_declarations(value: &DeclarationSummary<'_>) -> bool {
    let present = |text: Option<&str>| text.is_some_and(|text| !text.trim().is_empty());
    (!value.external_communication
        || (present(value.external_communication_reason)
            && present(value.external_communication_purpose)
            && !value.external_domains.is_empty()
            && value
                .external_domains
                .iter()
                .all(|domain| valid_external_domain(domain))))
        && (!value.collects_data || present(value.data_collection_description))
        && (!value.executes_dynamic_code || present(value.dynamic_code_explanation))
        && (!value.uses_external_updates || present(value.external_updates_explanation))
        && (!value.tracks_across_services || value.tracking_user_consent)
        && !value.uses_location_for_advertising
        && (!value.requires_login
            || present(value.test_account)
            || present(value.test_instructions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_names_enforce_character_policy_and_reserved_words() {
        assert!(valid_app_name("Binder Beta"));
        assert!(valid_app_name(&"a".repeat(128)));
        assert!(!valid_app_name(&"a".repeat(129)));
        assert!(!valid_app_name("Binder 🍡"));
        assert!(!valid_app_name("mochiOS"));
        assert!(!valid_app_name("App Store"));
    }

    #[test]
    fn media_requirements_match_review_policy() {
        assert!(valid_icon("image/png", 512, 512));
        assert!(valid_icon("image/jpeg", 512, 512));
        assert!(!valid_icon("image/webp", 512, 512));
        assert!(!valid_icon("image/png", 1024, 1024));
        assert!(valid_screenshot_set(3, 1));
        assert!(!valid_screenshot_set(2, 1));
        assert!(!valid_screenshot_set(3, 0));
        assert!(valid_release_channel_name("Binder Beta", "beta"));
        assert!(!valid_release_channel_name("Binder", "beta"));
    }

    #[test]
    fn external_domains_must_be_explicit_fqdns() {
        assert!(valid_external_domain("api.example.com"));
        assert!(valid_external_domain("example.com"));
        for invalid in [
            "*.example.com",
            "API.example.com",
            "https://example.com",
            "example.com/path",
            "localhost",
            "-api.example.com",
            "api..example.com",
        ] {
            assert!(!valid_external_domain(invalid), "accepted {invalid}");
        }
    }

    #[test]
    fn submission_transitions_keep_resubmission_and_information_flows_distinct() {
        assert!(valid_submission_transition(
            "draft",
            "submitted",
            "developer"
        ));
        assert!(valid_submission_transition(
            "submitted",
            "in_review",
            "reviewer"
        ));
        assert!(valid_submission_transition(
            "more_information_required",
            "in_review",
            "developer"
        ));
        assert!(!valid_submission_transition(
            "changes_required",
            "submitted",
            "developer"
        ));
        assert!(!valid_submission_transition(
            "approved", "removed", "reviewer"
        ));
        assert!(!valid_submission_transition(
            "submitted",
            "approved",
            "reviewer"
        ));
    }

    #[test]
    fn conditional_declarations_require_review_details() {
        let domains = ["api.example.com"];
        let valid = DeclarationSummary {
            external_communication: true,
            external_communication_reason: Some("同期のため"),
            external_communication_purpose: Some("データ同期"),
            external_domains: &domains,
            collects_data: true,
            data_collection_description: Some("アカウントID"),
            executes_dynamic_code: true,
            dynamic_code_explanation: Some("プラグインを実行"),
            uses_external_updates: true,
            external_updates_explanation: Some("ゲームデータを取得"),
            tracks_across_services: true,
            tracking_user_consent: true,
            uses_location_for_advertising: false,
            requires_login: true,
            test_account: None,
            test_instructions: Some("ゲストアカウントを選択"),
        };
        assert!(valid_declarations(&valid));
        assert!(!valid_declarations(&DeclarationSummary {
            external_communication_reason: None,
            ..valid
        }));
    }
}
