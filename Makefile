ROOT_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
DOCKER_IMAGE_NAME=fiction
DOCKER_CONTAINER_NAME=fiction
HOST_PORT=8080
CONTAINER_PORT=8080

.PHONY: image start-containers stop-containers

build:
	go build $(ROOT_DIR)

image:
	docker build --tag=$(DOCKER_IMAGE_NAME) $(ROOT_DIR)

start-containers: image
	docker run --detach=true \
		--publish=$(HOST_PORT):$(CONTAINER_PORT) \
		--name=$(DOCKER_CONTAINER_NAME) \
		$(DOCKER_IMAGE_NAME)

stop-containers:
	docker rm --force=true $(DOCKER_CONTAINER_NAME)
