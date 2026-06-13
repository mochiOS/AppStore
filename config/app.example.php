<?php

return [
    'env' => 'local',
    'oauth_subject_salt' => '',
    'admin_api_token' => '',
    'ca_cert_path' => '',
    'ca_key_path' => '',
    'ca_key_passphrase' => '',
    'ca_certificate_days' => 365,
    'msign_timeout_seconds' => 10,
    'msign_max_output_bytes' => 65536,
    'session_cookie_name' => 'mochios_appstore_session',

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
            'https://console.mochios.org',
        ],
    ],
];
