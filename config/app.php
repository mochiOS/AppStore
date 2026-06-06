<?php

return [
    'env' => 'local',
    'oauth_subject_salt' => '',

    'local' => [
        'frontend_url' => 'http://localhost:3000',
        'api_url' => 'http://localhost:3001',
        'allowed_origins' => [
            'http://localhost:3000',
            'https://console.mochios.org',
        ],
    ],

    'production' => [
        'frontend_url' => 'https://console.mochios.org',
        'api_url' => 'https://api.mochios.org',
        'allowed_origins' => [
            'http://localhost:3000',
            'https://console.mochios.org',
        ],
    ],
];
