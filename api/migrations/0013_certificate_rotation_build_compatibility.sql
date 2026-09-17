DROP INDEX idx_app_current_certificate;

ALTER TABLE app_certificates RENAME COLUMN is_current TO selected_for_new_builds;
ALTER TABLE app_certificates ADD COLUMN retired_at INTEGER;

CREATE UNIQUE INDEX idx_app_selected_certificate
  ON app_certificates(app_id) WHERE selected_for_new_builds=1;

ALTER TABLE releases ADD COLUMN architecture TEXT;
ALTER TABLE releases ADD COLUMN abi TEXT;
ALTER TABLE app_builds ADD COLUMN architecture TEXT;
ALTER TABLE app_builds ADD COLUMN abi TEXT;
ALTER TABLE app_builds ADD COLUMN certificate_serial TEXT NOT NULL DEFAULT '';
ALTER TABLE app_builds ADD COLUMN certificate_subject_public_key TEXT NOT NULL DEFAULT '';
ALTER TABLE app_builds ADD COLUMN certificate_subject_key_id TEXT NOT NULL DEFAULT '';
ALTER TABLE app_builds ADD COLUMN certificate_developer_id TEXT NOT NULL DEFAULT '';
ALTER TABLE app_builds ADD COLUMN certificate_issuer_key_id TEXT NOT NULL DEFAULT '';
ALTER TABLE app_builds ADD COLUMN certificate_issuer_public_key TEXT NOT NULL DEFAULT '';
ALTER TABLE app_builds ADD COLUMN certificate_issuance_source TEXT NOT NULL DEFAULT '';

UPDATE app_builds
   SET certificate_serial=COALESCE((
         SELECT developer_certificate_serial FROM releases WHERE release_id=app_builds.build_id
       ),''),
       certificate_subject_public_key=COALESCE((
         SELECT developer_public_key FROM releases WHERE release_id=app_builds.build_id
       ),''),
       certificate_subject_key_id=COALESCE((
         SELECT developer_certificate_subject_key_id FROM releases WHERE release_id=app_builds.build_id
       ),''),
       certificate_developer_id=COALESCE((
         SELECT developer_certificate_developer_id FROM releases WHERE release_id=app_builds.build_id
       ),''),
       certificate_issuer_key_id=COALESCE((
         SELECT developer_certificate_issuer_key_id FROM releases WHERE release_id=app_builds.build_id
       ),''),
       certificate_issuer_public_key=COALESCE((
         SELECT developer_certificate_issuer_public_key FROM releases WHERE release_id=app_builds.build_id
       ),''),
       certificate_issuance_source=COALESCE((
         SELECT developer_certificate_issuance_source FROM releases WHERE release_id=app_builds.build_id
       ),'');

CREATE TRIGGER app_build_certificate_identity_immutable
BEFORE UPDATE OF certificate_id,certificate_serial,certificate_subject_public_key,
  certificate_subject_key_id,certificate_developer_id,certificate_issuer_key_id,
  certificate_issuer_public_key,certificate_issuance_source ON app_builds
WHEN NEW.certificate_id IS NOT OLD.certificate_id
  OR NEW.certificate_serial IS NOT OLD.certificate_serial
  OR NEW.certificate_subject_public_key IS NOT OLD.certificate_subject_public_key
  OR NEW.certificate_subject_key_id IS NOT OLD.certificate_subject_key_id
  OR NEW.certificate_developer_id IS NOT OLD.certificate_developer_id
  OR NEW.certificate_issuer_key_id IS NOT OLD.certificate_issuer_key_id
  OR NEW.certificate_issuer_public_key IS NOT OLD.certificate_issuer_public_key
  OR NEW.certificate_issuance_source IS NOT OLD.certificate_issuance_source
BEGIN
  SELECT RAISE(ABORT, 'build certificate identity is immutable');
END;

CREATE TRIGGER release_certificate_identity_immutable
BEFORE UPDATE OF developer_certificate_id,developer_public_key,
  developer_certificate_serial,developer_certificate_subject_key_id,
  developer_certificate_developer_id,developer_certificate_issuer_key_id,
  developer_certificate_issuer_public_key,developer_certificate_issuance_source,
  registered_by ON releases
WHEN NEW.developer_certificate_id IS NOT OLD.developer_certificate_id
  OR NEW.developer_public_key IS NOT OLD.developer_public_key
  OR NEW.developer_certificate_serial IS NOT OLD.developer_certificate_serial
  OR NEW.developer_certificate_subject_key_id IS NOT OLD.developer_certificate_subject_key_id
  OR NEW.developer_certificate_developer_id IS NOT OLD.developer_certificate_developer_id
  OR NEW.developer_certificate_issuer_key_id IS NOT OLD.developer_certificate_issuer_key_id
  OR NEW.developer_certificate_issuer_public_key IS NOT OLD.developer_certificate_issuer_public_key
  OR NEW.developer_certificate_issuance_source IS NOT OLD.developer_certificate_issuance_source
  OR NEW.registered_by IS NOT OLD.registered_by
BEGIN
  SELECT RAISE(ABORT, 'release certificate identity is immutable');
END;

CREATE TRIGGER release_valid_compatibility_required
BEFORE UPDATE OF validation_status,architecture,abi ON releases
WHEN NEW.validation_status='valid'
  AND (NEW.architecture IS NULL OR length(NEW.architecture)=0 OR length(NEW.architecture)>64
    OR NEW.abi IS NULL OR length(NEW.abi)=0 OR length(NEW.abi)>64)
BEGIN
  SELECT RAISE(ABORT, 'valid release requires architecture and ABI');
END;

CREATE TRIGGER app_build_valid_compatibility_required
BEFORE UPDATE OF machine_status,architecture,abi ON app_builds
WHEN NEW.machine_status='valid'
  AND (NEW.architecture IS NULL OR length(NEW.architecture)=0 OR length(NEW.architecture)>64
    OR NEW.abi IS NULL OR length(NEW.abi)=0 OR length(NEW.abi)>64)
BEGIN
  SELECT RAISE(ABORT, 'valid build requires architecture and ABI');
END;

DROP TRIGGER legacy_release_validation_sync;

CREATE TRIGGER legacy_release_validation_sync
AFTER UPDATE OF validation_status,sha256,package_digest,manifest_hash,capabilities_json,
  payloads_json,reviewer_version,validated_at,architecture,abi ON releases
BEGIN
  UPDATE app_builds
     SET machine_status=NEW.validation_status,
         sha256=NEW.sha256,
         package_digest=NEW.package_digest,
         manifest_digest=NEW.manifest_hash,
         capabilities_json=NEW.capabilities_json,
         payloads_json=NEW.payloads_json,
         machine_message=NEW.validation_message,
         reviewer_version=NEW.reviewer_version,
         validated_at=NEW.validated_at,
         architecture=NEW.architecture,
         abi=NEW.abi
   WHERE build_id=NEW.release_id;
END;
