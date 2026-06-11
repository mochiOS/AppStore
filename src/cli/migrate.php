<?php

require_once __DIR__ . '/../helper/Paths.php';
require_once __DIR__ . '/../helper/Database.php';
require_once __DIR__ . '/../helper/PackageStorage.php';

$db = Database::get();

foreach (glob(__DIR__ . '/../migrations/*.sql') as $sqlFile) {
    $sql = file_get_contents($sqlFile);

    if ($sql === false || trim($sql) === '') {
        throw new RuntimeException("Migration file is empty: $sqlFile");
    }

    $db->exec($sql);
}

$storage = new PackageStorage(Paths::repoRoot());
$placeholderPath = $storage->ensurePlaceholderPackage(
    'data/releases/com.example/0.1.0.pkg',
    "mochiOS placeholder package for com.example 0.1.0\n"
);

$db->prepare(
    'INSERT OR IGNORE INTO apps (
        bundle_id,
        name,
        version,
        developer,
        description,
        icon
    ) VALUES (
        :bundle_id,
        :name,
        :version,
        :developer,
        :description,
        :icon
    )'
)->execute([
    ':bundle_id' => 'com.example',
    ':name' => 'Example App',
    ':version' => '0.1.0',
    ':developer' => 'mochiOS',
    ':description' => 'Example app for AppStore development.',
    ':icon' => null,
]);

$db->prepare(
    'INSERT OR IGNORE INTO releases (
        bundle_id,
        version,
        size,
        sha256,
        changelog,
        download_path,
        created_at
    ) VALUES (
        :bundle_id,
        :version,
        :size,
        :sha256,
        :changelog,
        :download_path,
        :created_at
    )'
)->execute([
    ':bundle_id' => 'com.example',
    ':version' => '0.1.0',
    ':size' => filesize($placeholderPath) ?: 0,
    ':sha256' => hash_file('sha256', $placeholderPath),
    ':changelog' => 'Initial example release.',
    ':download_path' => 'data/releases/com.example/0.1.0.pkg',
    ':created_at' => '2026-06-06T00:00:00+00:00',
]);

echo "Migration complete\n";

