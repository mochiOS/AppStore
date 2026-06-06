<?php

return [
    'env' => 'local',

    'local' => [
        'frontend_url' => 'http://localhost:3000',
        'api_url' => 'http://localhost:3001',
        'allowed_origins' => [
            'http://localhost:3000',
        ],
    ],

    'production' => [
        'frontend_url' => 'https://console.mochios.org',
        'api_url' => 'https://api.mochios.org',
        'allowed_origins' => [
            'https://console.mochios.org',
        ],
    ],
];