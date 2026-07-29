use rusqlite::Connection;

const INITIAL: &str = include_str!("../migrations/0001_initial.sql");
const GITHUB_RELEASES: &str = include_str!("../migrations/0002_github_release_distribution.sql");
const CERTIFICATE_SERIAL: &str = include_str!("../migrations/0003_certificate_serial.sql");
const CERTIFICATE_IDENTITY: &str = include_str!("../migrations/0004_certificate_identity.sql");
const UUID_DEVELOPER_IDS: &str = include_str!("../migrations/0005_uuid_developer_ids.sql");

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
