<?php

require_once __DIR__ . '/../../../app/Database.php';
require_once __DIR__ . '/../../../app/AppRepository.php';

$repo = new AppRepository(Database::get());

echo json_encode([
    'apps' => $repo->findAll()
]);

?>