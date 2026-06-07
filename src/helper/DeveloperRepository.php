<?php

class DeveloperRepository
{
    public function __construct(
        private PDO $pdo
    ) {
    }

    public function create(): array
    {
        $developerId = 'dev_' . bin2hex(random_bytes(16));
        $createdAt = date('c');

        $stmt = $this->pdo->prepare(
            'INSERT INTO developers (
                developer_id,
                created_at,
                status
            ) VALUES (
                :developer_id,
                :created_at,
                :status
            )'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
            ':created_at' => $createdAt,
            ':status' => 'active',
        ]);

        return $this->findById($developerId);
    }

    public function findById(string $developerId): ?array
    {
        $stmt = $this->pdo->prepare(
            'SELECT
            d.developer_id,
            d.created_at,
            d.status,
            ol.provider,
            ol.provider_username,
            ol.linked_at AS oauth_linked_at,
            ol.updated_at AS oauth_updated_at
         FROM developers d
         LEFT JOIN oauth_links ol
            ON ol.developer_id = d.developer_id
         WHERE d.developer_id = :developer_id
         ORDER BY ol.updated_at DESC, ol.linked_at DESC
         LIMIT 1'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        $developer = $stmt->fetch(PDO::FETCH_ASSOC);

        return $developer === false ? null : $developer;
    }

    public function findOAuthLink(string $provider, string $subjectHash): ?array
    {
        $stmt = $this->pdo->prepare(
            'SELECT
                developer_id,
                provider,
                provider_subject_hash,
                provider_username,
                linked_at,
                updated_at
             FROM oauth_links
             WHERE provider = :provider
               AND provider_subject_hash = :subject_hash
             LIMIT 1'
        );

        $stmt->execute([
            ':provider' => $provider,
            ':subject_hash' => $subjectHash,
        ]);

        $link = $stmt->fetch(PDO::FETCH_ASSOC);

        return $link === false ? null : $link;
    }

    public function linkOAuth(
        string $developerId,
        string $provider,
        string $subjectHash,
        ?string $providerUsername
    ): void {
        $now = date('c');

        $stmt = $this->pdo->prepare(
            'INSERT INTO oauth_links (
                developer_id,
                provider,
                provider_subject_hash,
                provider_username,
                linked_at,
                updated_at
            ) VALUES (
                :developer_id,
                :provider,
                :subject_hash,
                :provider_username,
                :linked_at,
                :updated_at
            )'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
            ':provider' => $provider,
            ':subject_hash' => $subjectHash,
            ':provider_username' => $providerUsername,
            ':linked_at' => $now,
            ':updated_at' => $now,
        ]);
    }

    public function updateOAuthUsername(
        string $provider,
        string $subjectHash,
        ?string $providerUsername
    ): void {
        $stmt = $this->pdo->prepare(
            'UPDATE oauth_links
             SET provider_username = :provider_username,
                 updated_at = :updated_at
             WHERE provider = :provider
               AND provider_subject_hash = :subject_hash'
        );

        $stmt->execute([
            ':provider_username' => $providerUsername,
            ':updated_at' => date('c'),
            ':provider' => $provider,
            ':subject_hash' => $subjectHash,
        ]);
    }

    public function findOrCreateByOAuth(
        string $provider,
        string $providerSubject,
        ?string $providerUsername = null
    ): array {
        $subjectHash = $this->hashProviderSubject($providerSubject);
        $developer = $this->findByOAuthSubjectHash($provider, $subjectHash);

        if ($developer !== null) {
            $this->updateOAuthUsername($provider, $subjectHash, $providerUsername);
            return $developer;
        }

        $this->pdo->beginTransaction();

        try {
            $developer = $this->create();

            $this->linkOAuth(
                $developer['developer_id'],
                $provider,
                $subjectHash,
                $providerUsername
            );

            $this->pdo->commit();
        } catch (Throwable $e) {
            if ($this->pdo->inTransaction()) {
                $this->pdo->rollBack();
            }

            $existing = $this->findByOAuthSubjectHash($provider, $subjectHash);
            if ($existing !== null) {
                $this->updateOAuthUsername($provider, $subjectHash, $providerUsername);
                return $existing;
            }

            throw $e;
        }

        return $developer;
    }

    private function hashProviderSubject(string $providerSubject): string
    {
        $salt = $this->oauthSubjectSalt();

        return hash_hmac('sha256', $providerSubject, $salt);
    }

    private function oauthSubjectSalt(): string
    {
        $oauthConfigPath = __DIR__ . '/../../config/oauth.php';
        if (is_file($oauthConfigPath)) {
            $oauthConfig = require $oauthConfigPath;
            if (is_array($oauthConfig) && isset($oauthConfig['oauth_subject_salt'])) {
                return (string) $oauthConfig['oauth_subject_salt'];
            }
        }

        $appConfig = AppConfig::get();

        return (string) ($appConfig['oauth_subject_salt'] ?? '');
    }
}