use rusqlite::Connection;

const INITIAL: &str = include_str!("../migrations/0001_initial.sql");
const GITHUB_RELEASES: &str = include_str!("../migrations/0002_github_release_distribution.sql");
const CERTIFICATE_SERIAL: &str = include_str!("../migrations/0003_certificate_serial.sql");
const CERTIFICATE_IDENTITY: &str = include_str!("../migrations/0004_certificate_identity.sql");
const UUID_DEVELOPER_IDS: &str = include_str!("../migrations/0005_uuid_developer_ids.sql");
const PACKAGE_SUSPENSIONS: &str = include_str!("../migrations/0006_package_suspensions.sql");
const RELEASE_VALIDATION_REPORTS: &str =
    include_str!("../migrations/0007_release_validation_reports.sql");
const VALIDATION_ATTEMPT_LEASES: &str =
    include_str!("../migrations/0008_validation_attempt_leases.sql");
const REMOVE_PRICE_AND_MINIMUM_OS: &str =
    include_str!("../migrations/0009_remove_price_and_minimum_os.sql");
const BACKFILL_BUNDLE_RESERVATION_AUDITS: &str =
    include_str!("../migrations/0010_backfill_bundle_reservation_audits.sql");
const NOTIFICATION_READS: &str = include_str!("../migrations/0011_notification_reads.sql");
const SUBMISSION_WORKFLOW: &str = include_str!("../migrations/0012_submission_workflow.sql");

fn apply_all_migrations(connection: &Connection) {
    for migration in [
        INITIAL,
        GITHUB_RELEASES,
        CERTIFICATE_SERIAL,
        CERTIFICATE_IDENTITY,
        UUID_DEVELOPER_IDS,
        PACKAGE_SUSPENSIONS,
        RELEASE_VALIDATION_REPORTS,
        VALIDATION_ATTEMPT_LEASES,
        REMOVE_PRICE_AND_MINIMUM_OS,
        BACKFILL_BUNDLE_RESERVATION_AUDITS,
        NOTIFICATION_READS,
        SUBMISSION_WORKFLOW,
    ] {
        connection
            .execute_batch(migration)
            .expect("apply migration");
    }
}

fn insert_workflow_fixture(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO bundle_ids VALUES('org.mochios.example','developer','Example','active',1);
         INSERT INTO apps(app_id,bundle_id,developer_id,display_name,created_at,updated_at)
           VALUES('app','org.mochios.example','developer','Example',1,1);
         INSERT INTO app_certificates(app_id,certificate_id,assigned_by_account_id,assigned_at)
           VALUES('app','cert-one','account',1);",
        )
        .unwrap();
}

#[test]
fn workflow_separates_builds_submissions_reviews_and_availability() {
    let connection = Connection::open_in_memory().expect("open migration fixture");
    apply_all_migrations(&connection);
    insert_workflow_fixture(&connection);

    connection.execute_batch(
        "INSERT INTO app_builds(build_id,app_id,certificate_id,version,build_number,
           github_repository_id,github_repository,github_release_id,github_release_tag,
           github_asset_id,asset_name,download_url,file_size,registered_by_account_id,created_at)
         VALUES
           ('build-one','app','cert-one','1.0.0',1,1,'example/app',10,'v1',100,
            'app-1.mpkg','https://github.com/example/app/releases/download/v1/app-1.mpkg',10,'account',1),
           ('build-two','app','cert-one','1.0.0',2,1,'example/app',11,'v1-retry',101,
            'app-2.mpkg','https://github.com/example/app/releases/download/v1-retry/app-2.mpkg',10,'account',2);
         INSERT INTO submissions(submission_id,app_id,build_id,version,submission_number,
           submission_kind,state,created_by_account_id,created_at,updated_at)
         VALUES
           ('submission-one','app','build-one','1.0.0',1,'new_app','changes_required','account',1,1),
           ('submission-two','app','build-two','1.0.0',2,'new_app','submitted','account',2,2);",
    ).unwrap();

    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM app_builds WHERE app_id='app' AND version='1.0.0'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM submissions WHERE app_id='app' AND version='1.0.0'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );

    connection
        .execute(
            "INSERT INTO published_versions VALUES('app','1.0.0','submission-one',3)",
            [],
        )
        .unwrap();
    assert!(
        connection
            .execute(
                "INSERT INTO published_versions VALUES('app','1.0.0','submission-two',4)",
                []
            )
            .is_err()
    );

    connection.execute(
        "INSERT INTO app_availability VALUES('app','available','submission-one',NULL,'reviewer',3)", []).unwrap();
    connection.execute(
        "UPDATE app_availability SET status='developer_unpublished',reason='Developer request',changed_at=4 WHERE app_id='app'", []).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT status FROM app_availability WHERE app_id='app'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "developer_unpublished"
    );
}

#[test]
fn workflow_enforces_certificate_domain_and_append_only_history_invariants() {
    let connection = Connection::open_in_memory().expect("open migration fixture");
    apply_all_migrations(&connection);
    insert_workflow_fixture(&connection);

    assert!(
        connection
            .execute(
                "INSERT INTO app_certificates VALUES('app','cert-two','account',2,NULL,'active')",
                []
            )
            .is_err()
    );

    connection.execute_batch(
        "INSERT INTO app_builds(build_id,app_id,certificate_id,version,build_number,
           github_repository_id,github_repository,github_release_id,github_release_tag,
           github_asset_id,asset_name,download_url,file_size,registered_by_account_id,created_at)
         VALUES('build','app','cert-one','1.0.0',1,1,'example/app',10,'v1',100,
           'app.mpkg','https://github.com/example/app/releases/download/v1/app.mpkg',10,'account',1);
         INSERT INTO submissions(submission_id,app_id,build_id,version,submission_number,
           submission_kind,state,created_by_account_id,created_at,updated_at)
         VALUES('submission','app','build','1.0.0',1,'new_app','in_review','account',1,1);
         INSERT INTO submission_reviews VALUES('review','submission','reviewer','changes_required','Fix metadata',2);
         INSERT INTO availability_history VALUES('event','app',NULL,'removed','Policy violation','reviewer',3);
         INSERT INTO app_acquisitions VALUES('app','customer',4);",
    ).unwrap();

    assert!(
        connection
            .execute(
                "INSERT INTO submission_network_domains VALUES('submission','*.example.com')",
                []
            )
            .is_err()
    );
    connection
        .execute(
            "INSERT INTO submission_network_domains VALUES('submission','api.example.com')",
            [],
        )
        .unwrap();
    assert!(
        connection
            .execute(
                "UPDATE submission_reviews SET reason='tampered' WHERE review_id='review'",
                []
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "DELETE FROM availability_history WHERE event_id='event'",
                []
            )
            .is_err()
    );
    assert!(
        connection
            .execute("DELETE FROM app_acquisitions WHERE app_id='app'", [])
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE submissions SET state='unknown' WHERE submission_id='submission'",
                []
            )
            .is_err()
    );
}

#[test]
fn package_suspension_is_reversible() {
    let connection = Connection::open_in_memory().expect("open migration fixture");
    connection.execute_batch(INITIAL).unwrap();
    connection.execute_batch("INSERT INTO bundle_ids VALUES ('org.mochios.example','developer','Example','active',1); INSERT INTO apps(app_id,bundle_id,developer_id,display_name,created_at,updated_at) VALUES('app','org.mochios.example','developer','Example',1,1);").unwrap();
    connection.execute_batch(PACKAGE_SUSPENSIONS).unwrap();
    connection.execute("INSERT INTO package_suspensions(bundle_id,suspended_by_account_id,reason,suspended_at) VALUES('org.mochios.example','admin','incident',2)",[]).unwrap();
    connection
        .execute(
            "UPDATE bundle_ids SET status='blocked' WHERE bundle_id='org.mochios.example'",
            [],
        )
        .unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT status FROM bundle_ids WHERE bundle_id='org.mochios.example'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "blocked"
    );
    connection
        .execute(
            "DELETE FROM package_suspensions WHERE bundle_id='org.mochios.example'",
            [],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE bundle_ids SET status='active' WHERE bundle_id='org.mochios.example'",
            [],
        )
        .unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM package_suspensions", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn validation_report_schema_and_audit_log_are_hardened() {
    let connection = Connection::open_in_memory().expect("open migration fixture");
    for migration in [
        INITIAL,
        GITHUB_RELEASES,
        CERTIFICATE_SERIAL,
        CERTIFICATE_IDENTITY,
        UUID_DEVELOPER_IDS,
        PACKAGE_SUSPENSIONS,
        RELEASE_VALIDATION_REPORTS,
        VALIDATION_ATTEMPT_LEASES,
        REMOVE_PRICE_AND_MINIMUM_OS,
        BACKFILL_BUNDLE_RESERVATION_AUDITS,
        NOTIFICATION_READS,
    ] {
        connection
            .execute_batch(migration)
            .expect("apply migration");
    }
    connection
        .execute(
            "INSERT INTO audit_logs VALUES('audit','account','release.validate','release','release',NULL,1)",
            [],
        )
        .unwrap();
    assert!(
        connection
            .execute("UPDATE audit_logs SET action='tampered'", [])
            .is_err()
    );
    assert!(connection.execute("DELETE FROM audit_logs", []).is_err());
    for column in [
        "registered_by_account_id",
        "developer_display_name",
        "package_digest",
        "capabilities_json",
        "payloads_json",
        "reviewer_version",
        "validation_error_code",
        "rejection_reason_code",
        "withdrawn_at",
        "last_integrity_checked_at",
        "validation_attempt_id",
        "validation_started_at",
    ] {
        let exists = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('releases') WHERE name=?1",
                [column],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "missing {column}");
    }
    for (table, column) in [
        ("apps", "price_label"),
        ("releases", "minimum_mochios_version"),
    ] {
        let exists = connection
            .query_row(
                &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name=?1"),
                [column],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(exists, 0, "obsolete {table}.{column} remains");
    }
}

#[test]
fn notification_reads_are_scoped_per_account_and_reference_audits() {
    let connection = Connection::open_in_memory().expect("open migration fixture");
    connection.execute_batch(INITIAL).unwrap();
    connection
        .execute_batch(RELEASE_VALIDATION_REPORTS)
        .unwrap();
    connection.execute_batch(NOTIFICATION_READS).unwrap();
    connection
        .execute(
            "INSERT INTO audit_logs VALUES('audit-one',NULL,'release.validation_failed','release','rel-one','{}',1)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO notification_reads VALUES('audit-one','account-a',2)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO notification_reads VALUES('audit-one','account-b',3)",
            [],
        )
        .unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM notification_reads", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert!(
        connection
            .execute(
                "INSERT INTO notification_reads VALUES('missing','account-a',4)",
                []
            )
            .is_err()
    );
}

#[test]
fn bundle_reservation_audit_backfill_is_idempotent() {
    let connection = Connection::open_in_memory().expect("open migration fixture");
    connection.execute_batch(INITIAL).unwrap();
    connection
        .execute_batch(
            "INSERT INTO bundle_ids VALUES ('org.mochios.example','developer','Example','reserved',1);",
        )
        .unwrap();

    connection
        .execute_batch(BACKFILL_BUNDLE_RESERVATION_AUDITS)
        .unwrap();
    connection
        .execute_batch(BACKFILL_BUNDLE_RESERVATION_AUDITS)
        .unwrap();

    let audit_count = connection
        .query_row(
            "SELECT COUNT(*) FROM audit_logs WHERE action='bundle.reserve' AND target_id='org.mochios.example'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(audit_count, 1);
}

#[test]
fn reviewer_lease_suppresses_duplicates_and_binds_results() {
    let connection = Connection::open_in_memory().expect("open migration fixture");
    for migration in [
        INITIAL,
        GITHUB_RELEASES,
        CERTIFICATE_SERIAL,
        CERTIFICATE_IDENTITY,
        UUID_DEVELOPER_IDS,
        PACKAGE_SUSPENSIONS,
        RELEASE_VALIDATION_REPORTS,
        VALIDATION_ATTEMPT_LEASES,
        REMOVE_PRICE_AND_MINIMUM_OS,
    ] {
        connection
            .execute_batch(migration)
            .expect("apply migration");
    }
    connection.execute_batch(
        "INSERT INTO bundle_ids VALUES('com.example.testapp','019fad830240772ba6fd5f50596afb4c','TestApp','active',1);
         INSERT INTO apps(app_id,bundle_id,developer_id,display_name,created_at,updated_at)
           VALUES('app','com.example.testapp','019fad830240772ba6fd5f50596afb4c','TestApp',1,1);
         INSERT INTO releases(release_id,bundle_id,version,github_asset_id,file_size,
           developer_certificate_id,developer_public_key,created_at)
           VALUES('rel','com.example.testapp','0.1.0',42,100,'cert','public',1);",
    ).unwrap();

    assert_eq!(
        connection
            .execute(
                "UPDATE releases SET validation_attempt_id='attempt-a',validation_started_at=1000
              WHERE release_id='rel' AND validation_status='pending' AND review_status='pending'
                AND publish_status='draft'
                AND (validation_started_at IS NULL OR validation_started_at<400)",
                [],
            )
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .execute(
                "UPDATE releases SET validation_attempt_id='attempt-b',validation_started_at=1001
              WHERE release_id='rel' AND validation_status='pending' AND review_status='pending'
                AND publish_status='draft'
                AND (validation_started_at IS NULL OR validation_started_at<401)",
                [],
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .execute(
                "UPDATE releases SET validation_status='valid',review_status='submitted'
              WHERE release_id='rel' AND validation_attempt_id='attempt-b'",
                [],
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .execute(
                "UPDATE releases SET validation_status='valid',review_status='submitted'
              WHERE release_id='rel' AND validation_attempt_id='attempt-a'",
                [],
            )
            .unwrap(),
        1
    );
}

#[test]
fn automatic_reviewer_queue_claims_oldest_available_release() {
    let connection = Connection::open_in_memory().expect("open migration fixture");
    for migration in [
        INITIAL,
        GITHUB_RELEASES,
        CERTIFICATE_SERIAL,
        CERTIFICATE_IDENTITY,
        UUID_DEVELOPER_IDS,
        PACKAGE_SUSPENSIONS,
        RELEASE_VALIDATION_REPORTS,
        VALIDATION_ATTEMPT_LEASES,
        REMOVE_PRICE_AND_MINIMUM_OS,
    ] {
        connection
            .execute_batch(migration)
            .expect("apply migration");
    }
    connection.execute_batch(
        "INSERT INTO bundle_ids VALUES('com.example.testapp','019fad830240772ba6fd5f50596afb4c','TestApp','active',1);
         INSERT INTO apps(app_id,bundle_id,developer_id,display_name,created_at,updated_at)
           VALUES('app','com.example.testapp','019fad830240772ba6fd5f50596afb4c','TestApp',1,1);
         INSERT INTO releases(release_id,bundle_id,version,github_asset_id,file_size,
           developer_certificate_id,developer_public_key,created_at)
           VALUES('rel-oldest','com.example.testapp','0.1.0',41,100,'cert','public',1),
                 ('rel-leased','com.example.testapp','0.2.0',42,100,'cert','public',2),
                 ('rel-newest','com.example.testapp','0.3.0',43,100,'cert','public',3);
         UPDATE releases SET validation_attempt_id='active-attempt',validation_started_at=950
           WHERE release_id='rel-leased';",
    ).unwrap();

    let claim = "UPDATE releases SET validation_attempt_id=?1,validation_started_at=?2
          WHERE release_id=(
            SELECT release_id FROM releases
             WHERE validation_status='pending' AND review_status='pending'
               AND publish_status='draft'
               AND (validation_started_at IS NULL OR validation_started_at<?3)
             ORDER BY created_at ASC,release_id ASC LIMIT 1
          )
            AND validation_status='pending' AND review_status='pending'
            AND publish_status='draft'
            AND (validation_started_at IS NULL OR validation_started_at<?3)
          RETURNING release_id";
    let first = connection
        .query_row(claim, ("attempt-a", 1000_i64, 400_i64), |row| {
            row.get::<_, String>(0)
        })
        .unwrap();
    let second = connection
        .query_row(claim, ("attempt-b", 1001_i64, 401_i64), |row| {
            row.get::<_, String>(0)
        })
        .unwrap();

    assert_eq!(first, "rel-oldest");
    assert_eq!(second, "rel-newest");
    assert_eq!(
        connection
            .query_row(
                "SELECT validation_attempt_id FROM releases WHERE release_id='rel-leased'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "active-attempt"
    );
}

#[test]
fn certificate_identity_migration_preserves_legacy_releases() {
    let connection = Connection::open_in_memory().expect("open migration fixture");
    connection
        .execute_batch(INITIAL)
        .expect("apply initial schema");
    connection
        .execute_batch(
            "INSERT INTO bundle_ids VALUES ('org.mochios.example','019f9e5a-c668-7902-b0e7-2fe53abfbef1','Example','active',1);
             INSERT INTO apps(app_id,bundle_id,developer_id,display_name,created_at,updated_at)
             VALUES ('app-1','org.mochios.example','019f9e5a-c668-7902-b0e7-2fe53abfbef1','Example',1,1);
             INSERT INTO releases(
               release_id,bundle_id,version,package_key,package_size,package_sha256,
               signature,certificate_id,status,created_at)
             VALUES ('release-1','org.mochios.example','1.0.0','legacy-key',12,'sha',
                     'signature','certificate-1','published',1);",
        )
        .expect("insert legacy release");
    connection
        .execute_batch(GITHUB_RELEASES)
        .expect("migrate GitHub release distribution");
    connection
        .execute_batch(CERTIFICATE_SERIAL)
        .expect("add certificate serial");
    connection
        .execute_batch(CERTIFICATE_IDENTITY)
        .expect("add certificate identity");
    connection.execute("UPDATE releases SET registered_by='019f9e5a-c668-7902-b0e7-2fe53abfbef1',developer_certificate_developer_id='org.mochios.developer.019f9e5ac6687902b0e72fe53abfbef1' WHERE release_id='release-1'",[]).unwrap();
    connection
        .execute_batch(UUID_DEVELOPER_IDS)
        .expect("normalize Developer IDs");

    let values = connection
        .query_row(
            "SELECT developer_certificate_id,developer_certificate_serial,
                    developer_certificate_subject_key_id,developer_certificate_developer_id,
                    developer_certificate_issuer_key_id,
                    developer_certificate_issuer_public_key,
                    developer_certificate_issuance_source
               FROM releases WHERE release_id='release-1'",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .expect("read migrated release");
    assert_eq!(values.0, "certificate-1");
    assert!(values.1.is_empty());
    assert!(values.2.is_empty());
    assert_eq!(values.3, "019f9e5ac6687902b0e72fe53abfbef1");
    assert!(values.4.is_empty());
    assert!(values.5.is_empty());
    assert_eq!(values.6, "legacy_root");
    assert_eq!(
        connection
            .query_row("SELECT developer_id FROM apps", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "019f9e5ac6687902b0e72fe53abfbef1"
    );
    assert_eq!(
        connection
            .query_row("SELECT registered_by FROM releases", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "019f9e5ac6687902b0e72fe53abfbef1"
    );
    assert_eq!(
        connection
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .expect("integrity check"),
        "ok"
    );
}
