<?php

require_once __DIR__ . '/../helper/Paths.php';
require_once __DIR__ . '/../helper/Database.php';

$db = Database::get();

foreach (glob(__DIR__ . '/../migrations/*.sql') as $sqlFile) {
    $sql = file_get_contents($sqlFile);

    if ($sql === false || trim($sql) === '') {
        throw new RuntimeException("Migration file is empty: $sqlFile");
    }

    $db->exec($sql);
}

echo "Migration complete\n";
