<?php

final class ApiContext
{
    public function __construct(
        public readonly string $path,
        public readonly string $method,
        public readonly PDO $db,
        public readonly AppRepository $appRepo,
        public readonly ReleaseRepository $releaseRepo,
        public readonly AppCatalog $appCatalog,
        public readonly DeveloperRepository $developerRepo,
        public readonly DeveloperCertificateRepository $certificateRepo,
        public readonly CertificateAuthority $certificateAuthority,
        public readonly PackageStorage $storage,
        public readonly array $appConfig,
        public readonly int $limit,
        public readonly int $offset,
    ) {
    }
}