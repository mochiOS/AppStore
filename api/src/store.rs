use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use uuid::{NoContext, Timestamp, Uuid};
use worker::{D1Database, Result, wasm_bindgen::JsValue};

use crate::model::{PublicApp, ReleaseView};

pub fn value(value: impl Into<JsValue>) -> JsValue {
    value.into()
}

pub fn number(value: i64) -> JsValue {
    JsValue::from_f64(value as f64)
}

pub fn id(prefix: &str, now: i64) -> String {
    let uuid = Uuid::new_v7(Timestamp::from_unix(NoContext, now.max(0) as u64, 0));
    format!("{prefix}_{}", uuid.simple())
}

pub async fn rows<T: DeserializeOwned>(
    db: &D1Database,
    sql: &str,
    params: &[JsValue],
) -> Result<Vec<T>> {
    db.prepare(sql).bind(params)?.all().await?.results::<T>()
}

pub async fn first<T: DeserializeOwned>(
    db: &D1Database,
    sql: &str,
    params: &[JsValue],
) -> Result<Option<T>> {
    db.prepare(sql).bind(params)?.first::<T>(None).await
}

pub async fn run(db: &D1Database, sql: &str, params: &[JsValue]) -> Result<()> {
    db.prepare(sql).bind(params)?.run().await?;
    Ok(())
}

const PUBLIC_APP_SELECT: &str =
    "SELECT a.bundle_id, a.display_name AS name, COALESCE(a.latest_version, '') AS version,
            COALESCE((SELECT rel.developer_display_name FROM releases rel
                       WHERE rel.bundle_id=a.bundle_id AND rel.publish_status='published'
                       ORDER BY rel.published_at DESC LIMIT 1), a.developer_id) AS developer,
            a.developer_id, a.description, a.icon_url AS icon, a.subtitle,
            a.category, a.kind, a.age_rating,
            CASE WHEN COALESCE(r.rating_count, 0) > 0
                 THEN CAST(r.rating_sum AS REAL) / r.rating_count ELSE NULL END AS rating,
            COALESCE(r.rating_count, 0) AS rating_count
       FROM apps a JOIN bundle_ids b ON b.bundle_id=a.bundle_id
       LEFT JOIN ratings r ON r.bundle_id = a.bundle_id";

pub async fn public_apps(
    db: &D1Database,
    kind: Option<&str>,
    category: Option<&str>,
    query: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<PublicApp>> {
    let sql = format!(
        "{PUBLIC_APP_SELECT}
          WHERE a.visibility='public' AND b.status='active'
            AND (?1 IS NULL OR a.kind=?1)
            AND (?2 IS NULL OR a.category=?2)
            AND (?3 IS NULL OR a.display_name LIKE '%' || ?3 || '%' OR a.description LIKE '%' || ?3 || '%' OR a.developer_id LIKE '%' || ?3 || '%')
          ORDER BY a.updated_at DESC LIMIT ?4 OFFSET ?5"
    );
    rows(
        db,
        &sql,
        &[
            kind.map_or(JsValue::NULL, value),
            category.map_or(JsValue::NULL, value),
            query.map_or(JsValue::NULL, value),
            number(limit),
            number(offset),
        ],
    )
    .await
}

pub async fn public_app(db: &D1Database, bundle_id: &str) -> Result<Option<PublicApp>> {
    first(
        db,
        &format!("{PUBLIC_APP_SELECT} WHERE a.visibility='public' AND b.status='active' AND a.bundle_id=?1 LIMIT 1"),
        &[value(bundle_id)],
    )
    .await
}

pub async fn public_releases(db: &D1Database, bundle_id: &str) -> Result<Vec<ReleaseView>> {
    rows(
        db,
        "SELECT r.release_id, r.bundle_id, r.version, r.file_size AS size, r.sha256,
                r.package_digest,
                r.changelog, r.review_status, r.publish_status, r.download_url,
                r.github_repository, r.github_release_tag, r.github_asset_id, r.asset_name,
                r.developer_certificate_id, r.created_at
           FROM releases r JOIN bundle_ids b ON b.bundle_id=r.bundle_id
          WHERE r.bundle_id=?1 AND b.status='active' AND validation_status='valid'
            AND review_status='approved' AND publish_status='published'
            AND download_url IS NOT NULL AND sha256 IS NOT NULL AND signature IS NOT NULL
          ORDER BY published_at DESC",
        &[value(bundle_id)],
    )
    .await
}

pub async fn developer_apps(db: &D1Database, developer_id: &str) -> Result<Vec<Value>> {
    rows(
        db,
        "SELECT * FROM apps WHERE developer_id=?1 ORDER BY created_at DESC",
        &[value(developer_id)],
    )
    .await
}

pub async fn developer_app(
    db: &D1Database,
    developer_id: &str,
    bundle_id: &str,
) -> Result<Option<Value>> {
    first(
        db,
        "SELECT * FROM apps WHERE developer_id=?1 AND bundle_id=?2 LIMIT 1",
        &[value(developer_id), value(bundle_id)],
    )
    .await
}

pub async fn release_by_id(db: &D1Database, release_id: &str) -> Result<Option<Value>> {
    first(
        db,
        "SELECT * FROM releases WHERE release_id=?1 LIMIT 1",
        &[value(release_id)],
    )
    .await
}

const DEVELOPER_NOTIFICATION_ACTIONS: &str =
    "'release.validation_succeeded','release.validation_failed','release.approve',
     'release.reject','release.withdraw','package.suspend','package.restore'";

const OPERATOR_NOTIFICATION_ACTIONS: &str =
    "'release.validation_succeeded','release.validation_failed','release.withdraw'";

pub async fn developer_notifications(
    db: &D1Database,
    developer_id: &str,
    account_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>> {
    rows(
        db,
        &format!(
            "SELECT a.audit_id AS notification_id,a.action,a.target_type,a.target_id,
                    a.metadata_json,a.created_at,n.read_at
               FROM audit_logs a
               LEFT JOIN notification_reads n
                 ON n.notification_id=a.audit_id AND n.account_id=?2
              WHERE a.action IN ({DEVELOPER_NOTIFICATION_ACTIONS})
                AND ((a.target_type='release' AND EXISTS(
                       SELECT 1 FROM releases r
                        WHERE r.release_id=a.target_id AND r.registered_by=?1))
                  OR (a.target_type='package' AND EXISTS(
                       SELECT 1 FROM apps p
                        WHERE p.bundle_id=a.target_id AND p.developer_id=?1)))
              ORDER BY a.created_at DESC,a.audit_id DESC LIMIT ?3 OFFSET ?4"
        ),
        &[
            value(developer_id),
            value(account_id),
            number(limit),
            number(offset),
        ],
    )
    .await
}

pub async fn developer_unread_count(
    db: &D1Database,
    developer_id: &str,
    account_id: &str,
) -> Result<i64> {
    let row: Option<Value> = first(
        db,
        &format!(
            "SELECT COUNT(*) AS count
               FROM audit_logs a
              WHERE a.action IN ({DEVELOPER_NOTIFICATION_ACTIONS})
                AND NOT EXISTS(SELECT 1 FROM notification_reads n
                                WHERE n.notification_id=a.audit_id AND n.account_id=?2)
                AND ((a.target_type='release' AND EXISTS(
                       SELECT 1 FROM releases r
                        WHERE r.release_id=a.target_id AND r.registered_by=?1))
                  OR (a.target_type='package' AND EXISTS(
                       SELECT 1 FROM apps p
                        WHERE p.bundle_id=a.target_id AND p.developer_id=?1)))"
        ),
        &[value(developer_id), value(account_id)],
    )
    .await?;
    Ok(row
        .and_then(|value| {
            value.get("count").and_then(Value::as_i64).or_else(|| {
                value
                    .get("count")
                    .and_then(Value::as_f64)
                    .map(|value| value as i64)
            })
        })
        .unwrap_or(0))
}

pub async fn operator_notifications(
    db: &D1Database,
    account_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>> {
    rows(
        db,
        &format!(
            "SELECT a.audit_id AS notification_id,a.action,a.target_type,a.target_id,
                    a.metadata_json,a.created_at,n.read_at
               FROM audit_logs a
               LEFT JOIN notification_reads n
                 ON n.notification_id=a.audit_id AND n.account_id=?1
              WHERE a.action IN ({OPERATOR_NOTIFICATION_ACTIONS})
              ORDER BY a.created_at DESC,a.audit_id DESC LIMIT ?2 OFFSET ?3"
        ),
        &[value(account_id), number(limit), number(offset)],
    )
    .await
}

pub async fn operator_unread_count(db: &D1Database, account_id: &str) -> Result<i64> {
    let row: Option<Value> = first(
        db,
        &format!(
            "SELECT COUNT(*) AS count FROM audit_logs a
              WHERE a.action IN ({OPERATOR_NOTIFICATION_ACTIONS})
                AND NOT EXISTS(SELECT 1 FROM notification_reads n
                                WHERE n.notification_id=a.audit_id AND n.account_id=?1)"
        ),
        &[value(account_id)],
    )
    .await?;
    Ok(row
        .and_then(|value| {
            value.get("count").and_then(Value::as_i64).or_else(|| {
                value
                    .get("count")
                    .and_then(Value::as_f64)
                    .map(|value| value as i64)
            })
        })
        .unwrap_or(0))
}

pub async fn mark_developer_notification_read(
    db: &D1Database,
    notification_id: &str,
    developer_id: &str,
    account_id: &str,
    now: i64,
) -> Result<bool> {
    let marked: Option<Value> = first(
        db,
        &format!(
            "INSERT INTO notification_reads(notification_id,account_id,read_at)
             SELECT a.audit_id,?3,?4 FROM audit_logs a
              WHERE a.audit_id=?1 AND a.action IN ({DEVELOPER_NOTIFICATION_ACTIONS})
                AND ((a.target_type='release' AND EXISTS(
                       SELECT 1 FROM releases r
                        WHERE r.release_id=a.target_id AND r.registered_by=?2))
                  OR (a.target_type='package' AND EXISTS(
                       SELECT 1 FROM apps p
                        WHERE p.bundle_id=a.target_id AND p.developer_id=?2)))
             ON CONFLICT(notification_id,account_id) DO UPDATE SET read_at=excluded.read_at
             RETURNING notification_id"
        ),
        &[
            value(notification_id),
            value(developer_id),
            value(account_id),
            number(now),
        ],
    )
    .await?;
    Ok(marked.is_some())
}

pub async fn mark_all_developer_notifications_read(
    db: &D1Database,
    developer_id: &str,
    account_id: &str,
    now: i64,
) -> Result<()> {
    run(
        db,
        &format!(
            "INSERT OR IGNORE INTO notification_reads(notification_id,account_id,read_at)
             SELECT a.audit_id,?2,?3 FROM audit_logs a
              WHERE a.action IN ({DEVELOPER_NOTIFICATION_ACTIONS})
                AND ((a.target_type='release' AND EXISTS(
                       SELECT 1 FROM releases r
                        WHERE r.release_id=a.target_id AND r.registered_by=?1))
                  OR (a.target_type='package' AND EXISTS(
                       SELECT 1 FROM apps p
                        WHERE p.bundle_id=a.target_id AND p.developer_id=?1)))"
        ),
        &[value(developer_id), value(account_id), number(now)],
    )
    .await
}

pub async fn mark_operator_notification_read(
    db: &D1Database,
    notification_id: &str,
    account_id: &str,
    now: i64,
) -> Result<bool> {
    let marked: Option<Value> = first(
        db,
        &format!(
            "INSERT INTO notification_reads(notification_id,account_id,read_at)
             SELECT a.audit_id,?2,?3 FROM audit_logs a
              WHERE a.audit_id=?1 AND a.action IN ({OPERATOR_NOTIFICATION_ACTIONS})
             ON CONFLICT(notification_id,account_id) DO UPDATE SET read_at=excluded.read_at
             RETURNING notification_id"
        ),
        &[value(notification_id), value(account_id), number(now)],
    )
    .await?;
    Ok(marked.is_some())
}

pub async fn mark_all_operator_notifications_read(
    db: &D1Database,
    account_id: &str,
    now: i64,
) -> Result<()> {
    run(
        db,
        &format!(
            "INSERT OR IGNORE INTO notification_reads(notification_id,account_id,read_at)
             SELECT a.audit_id,?1,?2 FROM audit_logs a
              WHERE a.action IN ({OPERATOR_NOTIFICATION_ACTIONS})"
        ),
        &[value(account_id), number(now)],
    )
    .await
}

pub async fn app_history(db: &D1Database, bundle_id: &str) -> Result<Vec<Value>> {
    rows(
        db,
        "SELECT audit_id,action,target_type,target_id,metadata_json,created_at
           FROM audit_logs a
          WHERE (a.target_id=?1 AND a.target_type IN ('bundle_id','app','package'))
             OR (a.target_type='release' AND EXISTS(
                  SELECT 1 FROM releases r
                   WHERE r.release_id=a.target_id AND r.bundle_id=?1))
          ORDER BY created_at DESC,audit_id DESC LIMIT 100",
        &[value(bundle_id)],
    )
    .await
}

pub async fn release_history(db: &D1Database, release_id: &str) -> Result<Vec<Value>> {
    rows(
        db,
        "SELECT audit_id,action,target_type,target_id,metadata_json,created_at
           FROM audit_logs WHERE target_type='release' AND target_id=?1
          ORDER BY created_at DESC,audit_id DESC LIMIT 100",
        &[value(release_id)],
    )
    .await
}

pub async fn audit(
    db: &D1Database,
    actor: Option<&str>,
    action: &str,
    target_type: &str,
    target_id: &str,
    metadata: Value,
    now: i64,
) -> Result<()> {
    audit_statement(db, actor, action, target_type, target_id, metadata, now)?
        .run()
        .await?;
    Ok(())
}

pub fn audit_statement(
    db: &D1Database,
    actor: Option<&str>,
    action: &str,
    target_type: &str,
    target_id: &str,
    metadata: Value,
    now: i64,
) -> Result<worker::D1PreparedStatement> {
    db.prepare(
        "INSERT INTO audit_logs (audit_id,actor_id,action,target_type,target_id,metadata_json,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
    )
    .bind(&[
        value(id("audit", now)),
        actor.map_or(JsValue::NULL, value),
        value(action),
        value(target_type),
        value(target_id),
        value(metadata.to_string()),
        number(now),
    ])
}

pub async fn storefront(db: &D1Database) -> Result<Value> {
    let apps = public_apps(db, Some("app"), None, None, 12, 0).await?;
    let games = public_apps(db, Some("game"), None, None, 12, 0).await?;
    let categories: Vec<Value> = rows(
        db,
        "SELECT lower(replace(a.category, ' ', '-')) AS slug, a.category AS name, NULL AS artwork
         FROM apps a JOIN bundle_ids b ON b.bundle_id=a.bundle_id
         WHERE a.visibility='public' AND b.status='active' AND a.category IS NOT NULL
         GROUP BY a.category ORDER BY a.category",
        &[],
    )
    .await?;
    let mut sections = Vec::new();
    if !apps.is_empty() {
        sections.push(json!({"id":"apps","title":"アプリ","layout":"row","apps":apps}));
    }
    if !games.is_empty() {
        sections.push(json!({"id":"games","title":"ゲーム","layout":"row","apps":games}));
    }
    Ok(json!({"featured":[],"sections":sections,"categories":categories}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_uuid_v7_without_system_clock_access() {
        let identifier = id("audit", 1_700_000_000);
        let parsed = Uuid::parse_str(identifier.trim_start_matches("audit_")).unwrap();
        assert_eq!(parsed.get_version_num(), 7);
    }
}
