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
            'SELECT developer_id, created_at, status
             FROM developers
             WHERE developer_id = :developer_id
             LIMIT 1'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        $developer = $stmt->fetch(PDO::FETCH_ASSOC);

        return $developer === false ? null : $developer;
    }

    public function findByOAuthSubjectHash(string $provider, string $subjectHash): ?array
    {
        $stmt = $this->pdo->prepare(
            'SELECT d.developer_id, d.created_at, d.status
             FROM oauth_links ol
             INNER JOIN developers d
                 ON d.developer_id = ol.developer_id
             WHERE ol.provider = :provider
               AND ol.provider_subject_hash = :subject_hash
             LIMIT 1'
        );

        $stmt->execute([
            ':provider' => $provider,
            ':subject_hash' => $subjectHash,
        ]);

        $developer = $stmt->fetch(PDO::FETCH_ASSOC);

        return $developer === false ? null : $developer;
    }

    public function linkOAuth(string $developerId, string $provider, string $subjectHash): void
    {
        $stmt = $this->pdo->prepare(
            'INSERT INTO oauth_links (
                developer_id,
                provider,
                provider_subject_hash,
                linked_at
            ) VALUES (
                :developer_id,
                :provider,
                :subject_hash,
                :linked_at
            )'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
            ':provider' => $provider,
            ':subject_hash' => $subjectHash,
            ':linked_at' => date('c'),
        ]);
    }

    public function findOrCreateByOAuth(string $provider, string $providerSubject): array
    {
        $subjectHash = $this->hashProviderSubject($providerSubject);
        $developer = $this->findByOAuthSubjectHash($provider, $subjectHash);

        if ($developer !== null) {
            return $developer;
        }

        $this->pdo->beginTransaction();

        try {
            $developer = $this->create();
            $this->linkOAuth($developer['developer_id'], $provider, $subjectHash);
            $this->pdo->commit();
        } catch (Throwable $e) {
            if ($this->pdo->inTransaction()) {
                $this->pdo->rollBack();
            }

            $existing = $this->findByOAuthSubjectHash($provider, $subjectHash);
            if ($existing !== null) {
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
