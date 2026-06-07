<?php

final readonly class ApiContext
{
    public function __construct(
        public string                         $path,
        public string                         $method,
        public PDO                            $db,
        public AppRepository                  $appRepo,
        public ReleaseRepository              $releaseRepo,
        public AppCatalog                     $appCatalog,
        public DeveloperRepository            $developerRepo,
        public PublicKeyRepository            $publicKeyRepo,
        public BundleIdRepository             $bundleIdRepo,
        public DeveloperAppRepository         $developerAppRepo,
        public DeveloperReleaseRepository     $developerReleaseRepo,
        public PackageUploadService           $packageUploadService,
        public PackageInspectService          $packageInspectService,
        public DeveloperCertificateRepository $certificateRepo,
        public CertificateAuthority           $certificateAuthority,
        public PackageStorage                 $storage,
        public AdminRepository                $adminRepo,
        public PackageSignatureVerifier       $packageSignatureVerifier,
        public TeamRepository                 $teamRepo,
        public array                          $appConfig,
        public int                            $limit,
        public int                            $offset,
    ) {
    }
}