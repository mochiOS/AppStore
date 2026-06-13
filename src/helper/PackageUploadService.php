<?php

class PackageUploadService
{
    private const MAX_PACKAGE_SIZE = 128 * 1024 * 1024;

    public function validateUploadedPackage(array $file): void
    {
        if (!isset($file['error'], $file['tmp_name'], $file['size'])) {
            throw new RuntimeException('Invalid upload payload');
        }

        if ((int) $file['error'] !== UPLOAD_ERR_OK) {
            throw new RuntimeException('Package upload failed');
        }

        $size = (int) $file['size'];

        if ($size <= 0) {
            throw new RuntimeException('Package is empty');
        }

        if ($size > self::MAX_PACKAGE_SIZE) {
            throw new RuntimeException('Package is too large');
        }

        if (!is_file((string) $file['tmp_name'])) {
            throw new RuntimeException('Uploaded package file is missing');
        }
    }

    public function storeUploadedPackage(
        array $file,
        string $bundleId,
        string $version
    ): array {
        $this->assertValidPathSegment($bundleId, 'bundle_id');
        $this->assertValidVersion($version);
        $this->validateUploadedPackage($file);

        $tmpPath = (string) $file['tmp_name'];
        $size = (int) $file['size'];

        $relativePath = 'data/packages/' . $bundleId . '/' . $version . '.pkg';
        $absolutePath = Paths::dataDir() . '/packages/' . $bundleId . '/' . $version . '.pkg';
        $directory = dirname($absolutePath);

        if (!is_dir($directory)) {
            mkdir($directory, 0777, true);
        }

        if (is_file($absolutePath)) {
            throw new RuntimeException('Package already exists');
        }

        if (is_uploaded_file($tmpPath)) {
            if (!move_uploaded_file($tmpPath, $absolutePath)) {
                throw new RuntimeException('Failed to store uploaded package');
            }
        } else {
            if (!rename($tmpPath, $absolutePath)) {
                throw new RuntimeException('Failed to store package');
            }
        }

        return [
            'relative_path' => $relativePath,
            'absolute_path' => $absolutePath,
            'size' => filesize($absolutePath) ?: $size,
            'sha256' => hash_file('sha256', $absolutePath),
        ];
    }

    private function assertValidPathSegment(string $value, string $name): void
    {
        if (
            $value === ''
            || str_contains($value, '/')
            || str_contains($value, '\\')
            || str_contains($value, '..')
            || !preg_match('/^[a-z0-9.-]+$/', $value)
        ) {
            throw new RuntimeException($name . ' is invalid');
        }
    }

    private function assertValidVersion(string $version): void
    {
        if (
            $version === ''
            || str_contains($version, '/')
            || str_contains($version, '\\')
            || str_contains($version, '..')
            || !preg_match('/^[0-9A-Za-z.+_-]+$/', $version)
        ) {
            throw new RuntimeException('version is invalid');
        }
    }
}
