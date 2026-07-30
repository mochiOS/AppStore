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
