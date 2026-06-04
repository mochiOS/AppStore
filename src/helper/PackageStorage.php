<?php

class PackageStorage
{
    public function __construct(
        private string $root
    ) {}

    public function absolutePath(string $relativePath): string
    {
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
