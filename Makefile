PHP = php
PUBLIC_PORT = 3000
API_PORT = 3001
DATA_DIR = data

run:
	@mkdir -p $(DATA_DIR)
	@cd src/api && $(PHP) -S localhost:$(API_PORT) router.php

api:
	@cd src/api && $(PHP) -S localhost:$(API_PORT) router.php
	
clean:
	rm -rf $(DATA_DIR)/

migrate:
	@$(PHP) src/cli/migrate.php

data: migrate

test:
	@$(PHP) src/tests/run.php

.PHONY: run api public clean data migrate test
