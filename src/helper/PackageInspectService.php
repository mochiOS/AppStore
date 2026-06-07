<?php

class PackageInspectService
{
    public function inspect(string $packagePath): array
    {
        if (!is_file($packagePath)) {
            throw new RuntimeException('Package file not found');
        }

        $phar = $this->openPackage($packagePath);
        $files = $this->listFiles($phar);

        if (!isset($files['about.toml'])) {
            throw new RuntimeException('about.toml is missing');
        }

        $about = $this->parseTomlSubset($files['about.toml']);

        $manifest = null;
        if (isset($files['manifest.toml'])) {
            $manifest = $this->parseTomlSubset($files['manifest.toml']);
        }

        $signature = null;
        if (isset($files['META/signature.toml'])) {
            $signature = $this->parseTomlSubset($files['META/signature.toml']);
        }

        $bundleId = $this->stringValue(
            $about['bundle_id']
            ?? $about['id']
            ?? $about['package']['id']
            ?? $about['app']['id']
            ?? null
        );

        $version = $this->stringValue(
            $about['version']
            ?? $about['package']['version']
            ?? null
        );

        $name = $this->stringValue(
            $about['name']
            ?? $about['package']['name']
            ?? $about['app']['name']
            ?? null
        );

        $entry = $this->stringValue(
            $about['entry']
            ?? $about['app']['entry']
            ?? null
        );

        $developer = $this->stringValue(
            $about['developer']
            ?? $about['package']['developer']
            ?? null
        );

        $description = $this->stringValue(
            $about['description']
            ?? $about['package']['description']
            ?? null
        );

        $icon = $this->stringValue(
            $about['icon']
            ?? $about['app']['icon']
            ?? null
        );

        if ($bundleId === null || $bundleId === '') {
            throw new RuntimeException('bundle_id is missing');
        }

        if ($version === null || $version === '') {
            throw new RuntimeException('version is missing');
        }

        if ($name === null || $name === '') {
            throw new RuntimeException('name is missing');
        }

        if ($entry === null || $entry === '') {
            throw new RuntimeException('entry is missing');
        }

        $this->assertSafeArchivePath($entry);

        if (!isset($files[$entry])) {
            throw new RuntimeException('entry file is missing');
        }

        if ($icon !== null && $icon !== '') {
            $this->assertSafeArchivePath($icon);
        }

        $manifestAppId = null;
        if ($manifest !== null) {
            $manifestAppId = $this->stringValue(
                $manifest['app']['id']
                ?? $manifest['package']['id']
                ?? null
            );

            if ($manifestAppId !== null && $manifestAppId !== $bundleId) {
                throw new RuntimeException('bundle_id does not match manifest app id');
            }
        }

        return [
            'about' => [
                'bundle_id' => $bundleId,
                'version' => $version,
                'name' => $name,
                'entry' => $entry,
                'developer' => $developer,
                'description' => $description,
                'icon' => $icon,
            ],
            'manifest' => $manifest,
            'signature' => $signature,
            'hashes' => [
                'package_sha256' => hash_file('sha256', $packagePath),
                'content_hash' => $this->calculateContentHashFromFiles($files),
                'manifest_hash' => isset($files['manifest.toml'])
                    ? hash('sha256', $files['manifest.toml'])
                    : null,
            ],
            'files' => array_keys($files),
        ];
    }

    public function calculateContentHash(string $packagePath): string
    {
        $phar = $this->openPackage($packagePath);
        $files = $this->listFiles($phar);

        return $this->calculateContentHashFromFiles($files);
    }

    private function openPackage(string $packagePath): PharData
    {
        $tmp = tempnam(sys_get_temp_dir(), 'mochi_pkg_');

        if ($tmp === false) {
            throw new RuntimeException('Failed to create temporary package path');
        }

        $tmpTarGz = $tmp . '.tar.gz';
        unlink($tmp);

        if (!copy($packagePath, $tmpTarGz)) {
            throw new RuntimeException('Failed to copy package for inspection');
        }

        try {
            return new PharData($tmpTarGz);
        } catch (Throwable $e) {
            @unlink($tmpTarGz);
            throw new RuntimeException('Package is not a valid tar.gz archive');
        }
    }

    private function listFiles(PharData $phar): array
    {
        $files = [];

        $iterator = new RecursiveIteratorIterator($phar);

        foreach ($iterator as $item) {
            if (!$item instanceof SplFileInfo) {
                continue;
            }

            $path = $this->archivePathFromItem($item);
            $this->assertSafeArchivePath($path);

            if ($item->isLink()) {
                throw new RuntimeException('Links are not allowed in package');
            }

            if ($item->isDir()) {
                continue;
            }

            if (!$item->isFile()) {
                throw new RuntimeException('Only regular files are allowed in package');
            }

            $content = file_get_contents($item->getPathname());
            if ($content === false) {
                throw new RuntimeException('Failed to read package file: ' . $path);
            }

            $files[$path] = $content;
        }

        ksort($files);

        return $files;
    }

    private function archivePathFromItem(SplFileInfo $item): string
    {
        $path = str_replace('\\', '/', $item->getPathname());

        $pos = strpos($path, '.tar.gz/');
        if ($pos !== false) {
            return substr($path, $pos + strlen('.tar.gz/'));
        }

        $parts = explode('/', $path);
        return end($parts) ?: '';
    }

    private function assertSafeArchivePath(string $path): void
    {
        if (
            $path === ''
            || str_starts_with($path, '/')
            || str_contains($path, "\0")
            || str_contains($path, '\\')
        ) {
            throw new RuntimeException('Unsafe package path: ' . $path);
        }

        $parts = explode('/', $path);

        foreach ($parts as $part) {
            if ($part === '' || $part === '.' || $part === '..') {
                throw new RuntimeException('Unsafe package path: ' . $path);
            }
        }
    }

    private function calculateContentHashFromFiles(array $files): string
    {
        unset($files['META/signature.toml']);

        ksort($files);

        $context = hash_init('sha256');

        foreach ($files as $path => $content) {
            hash_update($context, $path);
            hash_update($context, "\0");
            hash_update($context, hash('sha256', $content));
            hash_update($context, "\0");
        }

        return hash_final($context);
    }

    private function parseTomlSubset(string $source): array
    {
        $result = [];
        $section = [];
        $arrayKey = null;
        $arrayValues = [];

        foreach (preg_split('/\R/', $source) as $line) {
            $line = trim($this->stripTomlComment($line));

            if ($line === '') {
                continue;
            }

            if ($arrayKey !== null) {
                if (str_contains($line, ']')) {
                    $beforeClose = trim(substr($line, 0, strpos($line, ']')));
                    $arrayValues = array_merge($arrayValues, $this->parseTomlArrayItems($beforeClose));

                    $this->setTomlValue($result, $section, $arrayKey, $arrayValues);

                    $arrayKey = null;
                    $arrayValues = [];
                    continue;
                }

                $arrayValues = array_merge($arrayValues, $this->parseTomlArrayItems($line));
                continue;
            }

            if (preg_match('/^\[([A-Za-z0-9_.-]+)]$/', $line, $matches) === 1) {
                $section = explode('.', $matches[1]);
                continue;
            }

            if (preg_match('/^([A-Za-z0-9_.-]+)\s*=\s*(.+)$/', $line, $matches) !== 1) {
                continue;
            }

            $key = $matches[1];
            $value = trim($matches[2]);

            if (str_starts_with($value, '[') && !str_contains($value, ']')) {
                $arrayKey = $key;
                $arrayValues = $this->parseTomlArrayItems(substr($value, 1));
                continue;
            }

            $this->setTomlValue(
                $result,
                $section,
                $key,
                $this->parseTomlValue($value)
            );
        }

        return $result;
    }

    private function setTomlValue(array &$result, array $section, string $key, mixed $value): void
    {
        $target = &$result;

        foreach ($section as $part) {
            if (!isset($target[$part]) || !is_array($target[$part])) {
                $target[$part] = [];
            }

            $target = &$target[$part];
        }

        $target[$key] = $value;
    }

    private function parseTomlValue(string $value): mixed
    {
        $value = trim($value);

        if (str_starts_with($value, '[') && str_ends_with($value, ']')) {
            return $this->parseTomlArrayItems(substr($value, 1, -1));
        }

        if (
            (str_starts_with($value, '"') && str_ends_with($value, '"'))
            || (str_starts_with($value, "'") && str_ends_with($value, "'"))
        ) {
            return stripcslashes(substr($value, 1, -1));
        }

        if ($value === 'true') {
            return true;
        }

        if ($value === 'false') {
            return false;
        }

        return $value;
    }

    private function parseTomlArrayItems(string $value): array
    {
        $items = [];
        $parts = preg_split('/,/', $value);

        foreach ($parts as $part) {
            $part = trim($part);

            if ($part === '') {
                continue;
            }

            $items[] = $this->parseTomlValue($part);
        }

        return $items;
    }

    private function stripTomlComment(string $line): string
    {
        $inString = false;
        $quote = '';

        $length = strlen($line);

        for ($i = 0; $i < $length; $i++) {
            $char = $line[$i];

            if (($char === '"' || $char === "'") && ($i === 0 || $line[$i - 1] !== '\\')) {
                if (!$inString) {
                    $inString = true;
                    $quote = $char;
                    continue;
                }

                if ($quote === $char) {
                    $inString = false;
                    $quote = '';
                    continue;
                }
            }

            if ($char === '#' && !$inString) {
                return substr($line, 0, $i);
            }
        }

        return $line;
    }

    private function stringValue(mixed $value): ?string
    {
        if (is_string($value)) {
            return $value;
        }

        return null;
    }
}