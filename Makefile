PHP = php
PUBLIC_PORT = 8000
API_PORT = 8001
DATA_DIR = data

run:
	@mkdir -p $(DATA_DIR)
	@cd src/api && $(PHP) -S localhost:$(API_PORT) router.php & \
	cd src/public && npm run dev

api:
	@cd src/api && $(PHP) -S localhost:$(API_PORT) router.php

public:
	@cd src/public && npm run dev
	
clean:
	rm -rf $(DATA_DIR)/

migrate:
	@$(PHP) src/cli/migrate.php

test:
	@$(PHP) src/tests/run.php

.PHONY: run api public clean migrate test
