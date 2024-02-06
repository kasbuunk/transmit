DOCKER_COMPOSE_FILE := docker-compose.yaml
DOCKER_COMPOSE := docker-compose -f $(DOCKER_COMPOSE_FILE)
DOCKER_COMPOSE_FILE_E2E := tests/docker-compose.yaml
DOCKER_COMPOSE_E2E := docker-compose -f $(DOCKER_COMPOSE_FILE_E2E)

# Define targets and their commands
.PHONY: setup
setup:
	$(DOCKER_COMPOSE_E2E) up -d --build

.PHONY: test
test:
	cargo test --test end_to_end

.PHONY: teardown
teardown:
	$(DOCKER_COMPOSE_E2E) down

# Define a target that combines all the steps
.PHONY: e2e
e2e: setup test teardown
