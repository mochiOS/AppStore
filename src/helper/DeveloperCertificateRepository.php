<?php

class DeveloperCertificateRepository
{
    public function __construct(
        private readonly PDO $db
    ) {
    }

    public function findVerificationByDeveloperId(string $developerId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT developer_id, verification_status, note, verified_at, verified_by, updated_at
             FROM developer_verifications
             WHERE developer_id = :developer_id
             LIMIT 1'
        );
        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        $row = $stmt->fetch(PDO::FETCH_ASSOC);

        return $row === false ? null : $row;
    }

    public function requestVerification(string $developerId, ?string $note): array
    {
        $existing = $this->findVerificationByDeveloperId($developerId);
        if ($existing !== null) {
            return $existing;
        }

        $now = date('c');
        $stmt = $this->db->prepare(
            'INSERT INTO developer_verifications (
                developer_id,
                verification_status,
                note,
                verified_at,
                verified_by,
                updated_at
            ) VALUES (
                :developer_id,
                :verification_status,
                :note,
                NULL,
                NULL,
                :updated_at
            )'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
            ':verification_status' => 'pending',
            ':note' => $note,
            ':updated_at' => $now,
        ]);

        return $this->findVerificationByDeveloperId($developerId);
    }

    public function updateVerification(
        string $developerId,
        string $status,
        ?string $note,
        string $verifiedBy
    ): array {
        $now = date('c');
        $verifiedAt = $status === 'verified' ? $now : null;

        $stmt = $this->db->prepare(
            'INSERT INTO developer_verifications (
                developer_id,
                verification_status,
                note,
                verified_at,
                verified_by,
                updated_at
            ) VALUES (
                :developer_id,
                :verification_status,
                :note,
                :verified_at,
                :verified_by,
                :updated_at
            )
            ON CONFLICT(developer_id) DO UPDATE SET
                verification_status = excluded.verification_status,
                note = excluded.note,
                verified_at = excluded.verified_at,
                verified_by = excluded.verified_by,
                updated_at = excluded.updated_at'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
            ':verification_status' => $status,
            ':note' => $note,
            ':verified_at' => $verifiedAt,
            ':verified_by' => $verifiedBy,
            ':updated_at' => $now,
        ]);

        return $this->findVerificationByDeveloperId($developerId);
    }

    public function createCertificateSigningRequest(
        string $developerId,
        string $csrPem,
        array $csrInfo
    ): array {
        $csrId = 'csr_' . bin2hex(random_bytes(16));
        $createdAt = date('c');

        $stmt = $this->db->prepare(
            'INSERT INTO certificate_signing_requests (
                csr_id,
                developer_id,
                public_key,
                public_key_fingerprint,
                csr_pem,
                subject_dn,
                status,
                created_at,
                processed_at,
                processed_by,
                rejection_reason
            ) VALUES (
                :csr_id,
                :developer_id,
                :public_key,
                :public_key_fingerprint,
                :csr_pem,
                :subject_dn,
                :status,
                :created_at,
                NULL,
                NULL,
                NULL
            )'
        );

        $stmt->execute([
            ':csr_id' => $csrId,
            ':developer_id' => $developerId,
            ':public_key' => $csrInfo['public_key'],
            ':public_key_fingerprint' => $csrInfo['public_key_fingerprint'],
            ':csr_pem' => $csrPem,
            ':subject_dn' => $csrInfo['subject_dn'],
            ':status' => 'pending',
            ':created_at' => $createdAt,
        ]);

        return $this->findCertificateSigningRequestById($csrId);
    }

    public function listCertificateSigningRequestsByDeveloperId(string $developerId): array
    {
        $stmt = $this->db->prepare(
            'SELECT csr_id, developer_id, public_key, public_key_fingerprint, csr_pem, subject_dn, status, created_at, processed_at, processed_by, rejection_reason
             FROM certificate_signing_requests
             WHERE developer_id = :developer_id
             ORDER BY created_at DESC'
        );
        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }

    public function findCertificateSigningRequestById(string $csrId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT csr_id, developer_id, public_key, public_key_fingerprint, csr_pem, subject_dn, status, created_at, processed_at, processed_by, rejection_reason
             FROM certificate_signing_requests
             WHERE csr_id = :csr_id
             LIMIT 1'
        );
        $stmt->execute([
            ':csr_id' => $csrId,
        ]);

        $row = $stmt->fetch(PDO::FETCH_ASSOC);

        return $row === false ? null : $row;
    }

    public function rejectCertificateSigningRequest(
        string $csrId,
        string $processedBy,
        ?string $reason
    ): ?array {
        $stmt = $this->db->prepare(
            'UPDATE certificate_signing_requests
             SET status = :status,
                 processed_at = :processed_at,
                 processed_by = :processed_by,
                 rejection_reason = :rejection_reason
             WHERE csr_id = :csr_id
               AND status = :pending_status'
        );

        $stmt->execute([
            ':status' => 'rejected',
            ':processed_at' => date('c'),
            ':processed_by' => $processedBy,
            ':rejection_reason' => $reason,
            ':csr_id' => $csrId,
            ':pending_status' => 'pending',
        ]);

        return $this->findCertificateSigningRequestById($csrId);
    }

    public function issueCertificate(
        string $csrId,
        string $processedBy,
        array $issuedCertificate
    ): array {
        $csr = $this->findCertificateSigningRequestById($csrId);
        if ($csr === null) {
            throw new RuntimeException('CSR not found');
        }

        if ($csr['status'] !== 'pending') {
            throw new RuntimeException('CSR is not pending');
        }

        $certificateId = 'cert_' . bin2hex(random_bytes(16));
        $now = date('c');

        $this->db->beginTransaction();

        try {
            $insert = $this->db->prepare(
                'INSERT INTO developer_certificates (
                    certificate_id,
                    developer_id,
                    csr_id,
                    serial_number,
                    certificate_pem,
                    ca_fingerprint,
                    public_key,
                    public_key_fingerprint,
                    subject_dn,
                    issued_at,
                    expires_at,
                    status,
                    revoked_at,
                    revocation_reason
                ) VALUES (
                    :certificate_id,
                    :developer_id,
                    :csr_id,
                    :serial_number,
                    :certificate_pem,
                    :ca_fingerprint,
                    :public_key,
                    :public_key_fingerprint,
                    :subject_dn,
                    :issued_at,
                    :expires_at,
                    :status,
                    NULL,
                    NULL
                )'
            );

            $insert->execute([
                ':certificate_id' => $certificateId,
                ':developer_id' => $csr['developer_id'],
                ':csr_id' => $csrId,
                ':serial_number' => $issuedCertificate['serial_number'],
                ':certificate_pem' => $issuedCertificate['certificate_pem'],
                ':ca_fingerprint' => $issuedCertificate['ca_fingerprint'],
                ':public_key' => $issuedCertificate['public_key'],
                ':public_key_fingerprint' => $issuedCertificate['public_key_fingerprint'],
                ':subject_dn' => $issuedCertificate['subject_dn'],
                ':issued_at' => $issuedCertificate['issued_at'],
                ':expires_at' => $issuedCertificate['expires_at'],
                ':status' => 'active',
            ]);

            $update = $this->db->prepare(
                'UPDATE certificate_signing_requests
                 SET status = :status,
                     processed_at = :processed_at,
                     processed_by = :processed_by,
                     rejection_reason = NULL
                 WHERE csr_id = :csr_id'
            );
            $update->execute([
                ':status' => 'approved',
                ':processed_at' => $now,
                ':processed_by' => $processedBy,
                ':csr_id' => $csrId,
            ]);

            $this->db->commit();
        } catch (Throwable $e) {
            if ($this->db->inTransaction()) {
                $this->db->rollBack();
            }

            throw $e;
        }

        return $this->findCertificateById($certificateId);
    }

    public function listCertificatesByDeveloperId(string $developerId): array
    {
        $stmt = $this->db->prepare(
            'SELECT certificate_id, developer_id, csr_id, serial_number, certificate_pem, ca_fingerprint, public_key, public_key_fingerprint, subject_dn, issued_at, expires_at, status, revoked_at, revocation_reason
             FROM developer_certificates
             WHERE developer_id = :developer_id
             ORDER BY issued_at DESC'
        );
        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }

    public function findCertificateById(string $certificateId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT certificate_id, developer_id, csr_id, serial_number, certificate_pem, ca_fingerprint, public_key, public_key_fingerprint, subject_dn, issued_at, expires_at, status, revoked_at, revocation_reason
             FROM developer_certificates
             WHERE certificate_id = :certificate_id
             LIMIT 1'
        );
        $stmt->execute([
            ':certificate_id' => $certificateId,
        ]);

        $row = $stmt->fetch(PDO::FETCH_ASSOC);

        return $row === false ? null : $row;
    }

    public function revokeCertificate(
        string $certificateId,
        string $reason
    ): ?array {
        $now = date('c');

        $this->db->beginTransaction();

        try {
            $stmt = $this->db->prepare(
                'UPDATE developer_certificates
                 SET status = :status,
                     revoked_at = :revoked_at,
                     revocation_reason = :revocation_reason
                 WHERE certificate_id = :certificate_id
                   AND status = :active_status'
            );
            $stmt->execute([
                ':status' => 'revoked',
                ':revoked_at' => $now,
                ':revocation_reason' => $reason,
                ':certificate_id' => $certificateId,
                ':active_status' => 'active',
            ]);

            $cert = $this->findCertificateById($certificateId);
            if ($cert !== null) {
                $revocation = $this->db->prepare(
                    'INSERT INTO revocations (
                        revocation_id,
                        target_type,
                        target_id,
                        reason,
                        created_at
                    ) VALUES (
                        :revocation_id,
                        :target_type,
                        :target_id,
                        :reason,
                        :created_at
                    )'
                );

                $revocation->execute([
                    ':revocation_id' => 'rev_' . bin2hex(random_bytes(16)),
                    ':target_type' => 'certificate',
                    ':target_id' => $certificateId,
                    ':reason' => $reason,
                    ':created_at' => $now,
                ]);
            }

            $this->db->commit();
        } catch (Throwable $e) {
            if ($this->db->inTransaction()) {
                $this->db->rollBack();
            }

            throw $e;
        }

        return $this->findCertificateById($certificateId);
    }
}
