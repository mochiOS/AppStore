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
$storage->ensurePlaceholderPackage(
    'data/releases/com.example/0.1.0.pkg',
    "mochiOS placeholder package for com.example 0.1.0\n"
);

echo "Migration complete\n";


