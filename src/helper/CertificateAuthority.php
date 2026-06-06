<?php

class CertificateAuthority
{
    public function __construct(
        private readonly array $config
    ) {
    }

    public static function fromAppConfig(array $appConfig): self
    {
        return new self([
            'cert_path' => getenv('APPSTORE_CA_CERT_PATH') ?: ($appConfig['ca_cert_path'] ?? ''),
            'key_path' => getenv('APPSTORE_CA_KEY_PATH') ?: ($appConfig['ca_key_path'] ?? ''),
            'key_passphrase' => getenv('APPSTORE_CA_KEY_PASSPHRASE') ?: ($appConfig['ca_key_passphrase'] ?? ''),
            'certificate_days' => (int) (getenv('APPSTORE_CA_CERTIFICATE_DAYS') ?: ($appConfig['ca_certificate_days'] ?? 365)),
        ]);
    }

    public function isConfigured(): bool
    {
        return $this->certPath() !== ''
            && $this->keyPath() !== ''
            && is_file($this->certPath())
            && is_file($this->keyPath());
    }

    public function rootCertificatePem(): string
    {
        $this->assertConfigured();

        $pem = file_get_contents($this->certPath());
        if ($pem === false || trim($pem) === '') {
            throw new RuntimeException('CA certificate could not be read');
        }

        return $pem;
    }

    public function rootFingerprint(): string
    {
        $fingerprint = openssl_x509_fingerprint($this->rootCertificatePem(), 'sha256');
        if (!is_string($fingerprint) || $fingerprint === '') {
            throw new RuntimeException('Failed to calculate CA fingerprint');
        }

        return strtolower($fingerprint);
    }

    public function parseCsr(string $csrPem): array
    {
        $subject = openssl_csr_get_subject($csrPem, false);
        if (!is_array($subject) || $subject === []) {
            throw new RuntimeException('Invalid CSR subject');
        }

        $publicKey = openssl_csr_get_public_key($csrPem);
        if ($publicKey === false) {
            throw new RuntimeException('Invalid CSR public key');
        }

        $details = openssl_pkey_get_details($publicKey);
        if (!is_array($details) || !isset($details['key']) || !is_string($details['key'])) {
            throw new RuntimeException('Failed to export CSR public key');
        }

        return [
            'subject_dn' => self::dnToString($subject),
            'public_key' => $details['key'],
            'public_key_fingerprint' => hash('sha256', $details['key']),
        ];
    }

    public function issueCertificate(string $csrPem): array
    {
        $this->assertConfigured();

        $csrInfo = $this->parseCsr($csrPem);
        $caCertificatePem = $this->rootCertificatePem();
        $privateKey = openssl_pkey_get_private(
            'file://' . $this->keyPath(),
            $this->keyPassphrase()
        );

        if ($privateKey === false) {
            throw new RuntimeException('Failed to load CA private key');
        }

        $serialNumber = (string) random_int(1, PHP_INT_MAX);
        $certificate = openssl_csr_sign(
            $csrPem,
            $caCertificatePem,
            $privateKey,
            $this->certificateDays(),
            [],
            (int) $serialNumber
        );

        if ($certificate === false) {
            throw new RuntimeException('Failed to sign certificate');
        }

        $certificatePem = '';
        if (!openssl_x509_export($certificate, $certificatePem)) {
            throw new RuntimeException('Failed to export certificate');
        }

        $parsed = openssl_x509_parse($certificatePem);
        if (!is_array($parsed)) {
            throw new RuntimeException('Failed to parse issued certificate');
        }

        return [
            'certificate_pem' => $certificatePem,
            'serial_number' => $serialNumber,
            'ca_fingerprint' => $this->rootFingerprint(),
            'public_key' => $csrInfo['public_key'],
            'public_key_fingerprint' => $csrInfo['public_key_fingerprint'],
            'subject_dn' => $csrInfo['subject_dn'],
            'issued_at' => date('c', (int) ($parsed['validFrom_time_t'] ?? time())),
            'expires_at' => date('c', (int) ($parsed['validTo_time_t'] ?? time())),
        ];
    }

    public static function dnToString(array $subject): string
    {
        $parts = [];

        foreach ($subject as $key => $value) {
            $parts[] = $key . '=' . $value;
        }

        return implode(', ', $parts);
    }

    private function assertConfigured(): void
    {
        if (!$this->isConfigured()) {
            throw new RuntimeException('CA is not configured');
        }
    }

    private function certPath(): string
    {
        return (string) ($this->config['cert_path'] ?? '');
    }

    private function keyPath(): string
    {
        return (string) ($this->config['key_path'] ?? '');
    }

    private function keyPassphrase(): string
    {
        return (string) ($this->config['key_passphrase'] ?? '');
    }

    private function certificateDays(): int
    {
        $days = (int) ($this->config['certificate_days'] ?? 365);

        return $days > 0 ? $days : 365;
    }
}
