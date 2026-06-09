PHP = php
PUBLIC_PORT = 3000
API_PORT = 3001
DATA_DIR = data

run:
	@mkdir -p $(DATA_DIR)
	@cd src/api && $(PHP) -S localhost:$(API_PORT) router.php & \
	cd src/public/console && $(PHP) -S localhost:$(PUBLIC_PORT)

api:
	@cd src/api && $(PHP) -S localhost:$(API_PORT) router.php

public:
	@cd src/public/console && $(PHP) -S localhost:$(PUBLIC_PORT)

clean:
	rm -rf $(DATA_DIR)/

migrate:
	@$(PHP) src/cli/migrate.php

ci:
	docker compose exec -T appstore php src/cli/migrate.php

data: migrate

test:
	@$(PHP) src/tests/run.php

admin:
	php src/cli/admin.php $(filter-out $@,$(MAKECMDGOALS))

%:
	@:

.PHONY: run api public clean data migrate test admin
