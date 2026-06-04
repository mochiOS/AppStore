<?php

class PackageStorage
{
    public function __construct(
        private string $root
    ) {}

    public function absolutePath(string $relativePath): string
    {
        if (str_starts_with($relativePath, 'data/') || $relativePath === 'data') {
            $dataDir = Paths::dataDir();
            if ($relativePath === 'data') {
                return $dataDir;
            }

            return $dataDir . '/' . ltrim(substr($relativePath, 5), '/');
        }

        return $this->root . ltrim($relativePath, '/');
    }

    public function ensurePlaceholderPackage(string $relativePath, string $content): string
    {
        $path = $this->absolutePath($relativePath);
        $directory = dirname($path);

        if (!is_dir($directory)) {
            mkdir($directory, 0777, true);
        }

        if (!file_exists($path)) {
            file_put_contents($path, $content);
        }

        return $path;
    }
}

?>
