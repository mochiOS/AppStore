<?php

class AppCatalog
{
    public function __construct(
        private AppRepository $appRepo,
        private ReleaseRepository $releaseRepo
    ) {}

    public function listApps(int $limit, int $offset): array
    {
        return [
            'apps' => $this->appRepo->findAll($limit, $offset),
        ];
    }

    public function hasApp(string $bundleId): bool
    {
        return $this->appRepo->findByBundleId($bundleId) !== null;
    }

    public function findAppDetail(string $bundleId): ?array
    {
        $app = $this->appRepo->findByBundleId($bundleId);
        if ($app === null) {
            return null;
        }

        $app['releases'] = $this->releaseRepo->findAllByBundleId($bundleId);

        return $app;
    }

    public function listReleases(string $bundleId): ?array
    {
        if ($this->appRepo->findByBundleId($bundleId) === null) {
            return null;
        }

        return [
            'bundle_id' => $bundleId,
            'releases' => $this->releaseRepo->findAllByBundleId($bundleId),
        ];
    }

    public function findRelease(string $bundleId, string $version): ?array
    {
        $release = $this->releaseRepo->findByBundleIdAndVersion($bundleId, $version);
        if ($release === null) {
            return null;
        }

        return $this->releaseRepo->toApiRelease($release);
    }

    public function findDownloadRelease(string $bundleId, ?string $version): ?array
    {
        if ($version === null || $version === '') {
            return $this->releaseRepo->findLatestByBundleId($bundleId);
        }

        return $this->releaseRepo->findByBundleIdAndVersion($bundleId, $version);
    }
}

?>
