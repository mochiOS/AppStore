use super::*;
use serde::Deserialize;

#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Information {
    pub app_name: String,
    pub description: String,
    pub icon_url: String,
    pub icon_media_type: String,
    pub icon_width: u32,
    pub icon_height: u32,
    pub category: Option<String>,
    pub kind: String,
    pub age_rating: Option<String>,
    pub screenshots: Vec<ScreenshotInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    information: Information,
    revision: i64,
}

fn valid(information: &Information) -> bool {
    (information.app_name.is_empty() || workflow::valid_app_name(&information.app_name))
        && information.description.chars().count() <= 4000
        && valid_icon_url(Some(&information.icon_url))
        && matches!(
            information.icon_media_type.as_str(),
            "" | "image/png" | "image/jpeg"
        )
        && information.icon_width <= 8192
        && information.icon_height <= 8192
        && matches!(information.kind.as_str(), "app" | "game")
        && valid_optional_text(information.category.as_deref(), 80)
        && valid_optional_text(information.age_rating.as_deref(), 40)
        && information.screenshots.len() <= 20
        && information.screenshots.iter().all(|image| {
            !image.image_url.trim().is_empty()
                && valid_icon_url(Some(&image.image_url))
                && (1..=20).contains(&image.position)
        })
        && information
            .screenshots
            .iter()
            .map(|image| image.position)
            .collect::<HashSet<_>>()
            .len()
            == information.screenshots.len()
}

pub async fn handler(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let database = db(&ctx)?;
    let bundle_id = param(&ctx, "bundle_id");
    let Some(app) = store::developer_app(&database, &actor.developer_id, bundle_id).await? else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    let app_id = value_str(&app, "app_id").unwrap_or("");
    if req.method() == Method::Get {
        let row: Option<Value> = store::first(
            &database,
            "SELECT information_json,revision FROM app_store_information WHERE app_id=?1",
            &[store::value(app_id)],
        )
        .await?;
        let (information, revision) = match row {
            Some(row) => (
                serde_json::from_str::<Value>(value_str(&row, "information_json").unwrap_or("{}"))?,
                row["revision"].clone(),
            ),
            None => (
                serde_json::to_value(Information {
                    app_name: value_str(&app, "display_name").unwrap_or("").into(),
                    description: value_str(&app, "description").unwrap_or("").into(),
                    icon_url: value_str(&app, "icon_url").unwrap_or("").into(),
                    kind: value_str(&app, "kind").unwrap_or("app").into(),
                    ..Information::default()
                })?,
                json!(0),
            ),
        };
        return json_response(&json!({"information":information,"revision":revision}), 200);
    }
    if let Some(response) =
        rate_limited(&req, &ctx.env, "MUTATION_RATE_LIMITER", "store-information").await?
    {
        return Ok(response);
    }
    let update: Update = match bounded_json(&mut req).await? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    if update.revision < 0 || !valid(&update.information) {
        return error(
            "STORE_INFORMATION_INVALID",
            "Store Information contains invalid fields",
            422,
        );
    }
    let content = serde_json::to_string(&update.information)?;
    let timestamp = now();
    let row: Option<Value> = if update.revision == 0 {
        store::first(&database,
            "INSERT INTO app_store_information(app_id,information_json,revision,updated_by_account_id,updated_at)
             VALUES(?1,?2,1,?3,?4) ON CONFLICT(app_id) DO NOTHING RETURNING revision",
            &[store::value(app_id),store::value(&content),store::value(&actor.account_id),store::number(timestamp)]).await?
    } else {
        store::first(&database,
            "UPDATE app_store_information SET information_json=?1,revision=revision+1,updated_by_account_id=?2,updated_at=?3
             WHERE app_id=?4 AND revision=?5 RETURNING revision",
            &[store::value(&content),store::value(&actor.account_id),store::number(timestamp),store::value(app_id),store::number(update.revision)]).await?
    };
    let Some(row) = row else {
        return error(
            "STORE_INFORMATION_CHANGED",
            "Store Information was changed elsewhere. Reload before saving.",
            409,
        );
    };
    store::audit(
        &database,
        Some(&actor.account_id),
        "app.store_information_saved",
        "app",
        bundle_id,
        json!({"revision":row["revision"]}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"information":update.information,"revision":row["revision"]}),
        200,
    )
}

pub async fn snapshot_input(
    req: &mut Request,
    database: &D1Database,
    app_id: &str,
    actor: &auth::DeveloperActor,
) -> Result<std::result::Result<(SubmissionDraftInput, i64), Response>> {
    let body: Value = match bounded_json(req).await? {
        Ok(value) => value,
        Err(response) => return Ok(Err(response)),
    };
    let Some(mut body) = body.as_object().cloned() else {
        return Ok(Err(error(
            "SUBMISSION_INVALID",
            "Version fields must be an object",
            422,
        )?));
    };
    let row: Option<Value> = store::first(
        database,
        "SELECT information_json,revision FROM app_store_information WHERE app_id=?1",
        &[store::value(app_id)],
    )
    .await?;
    let Some(row) = row else {
        return Ok(Err(error(
            "STORE_INFORMATION_REQUIRED",
            "Save Store Information before preparing a Version",
            422,
        )?));
    };
    let information: Value =
        serde_json::from_str(value_str(&row, "information_json").unwrap_or("{}"))?;
    for (key, value) in information.as_object().into_iter().flatten() {
        body.insert(key.clone(), value.clone());
    }
    body.insert("developer_name".into(), json!(actor.display_name));
    if !body.contains_key("submission_kind") {
        let lifecycle: Option<Value> = store::first(database,
            "SELECT (SELECT COUNT(*) FROM published_versions WHERE app_id=?1) AS published,
                    COALESCE((SELECT status FROM app_availability WHERE app_id=?1),'not_available') AS availability",
            &[store::value(app_id)]).await?;
        let kind = match lifecycle.as_ref() {
            Some(value) if value["published"].as_f64().unwrap_or(0.0) > 0.0 => {
                if value_str(value, "availability") == Some("available") {
                    "update"
                } else {
                    "re_review"
                }
            }
            _ => "new_app",
        };
        body.insert("submission_kind".into(), json!(kind));
    }
    match serde_json::from_value::<SubmissionDraftInput>(Value::Object(body)) {
        Ok(input) => Ok(Ok((input, row["revision"].as_f64().unwrap_or(0.0) as i64))),
        Err(_) => Ok(Err(error(
            "SUBMISSION_INVALID",
            "Version fields are invalid",
            422,
        )?)),
    }
}
