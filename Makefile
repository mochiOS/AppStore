PHP = php
PUBLIC_PORT = 8000
API_PORT = 8001

run:
	@mkdir -p data/
	@cd src/api && $(PHP) -S localhost:$(API_PORT) router.php & \
	cd src/public && $(PHP) -S localhost:$(PUBLIC_PORT)

api:
	@cd src/api && $(PHP) -S localhost:$(API_PORT) router.php

public:
	@cd src/public && $(PHP) -S localhost:$(PUBLIC_PORT)

clean:
	rm -rf data/

migrate:
	@$(PHP) src/cli/migrate.php

.PHONY: run api public clean migrate
