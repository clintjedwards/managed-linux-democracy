# Make settings
# Mostly copied from: https://tech.davis-hansson.com/p/make/

# Use Bash
SHELL := bash

# If one of the commands fails just fail properly and don't run the other commands.
.SHELLFLAGS := -eu -o pipefail -c

# Allows me to use a single shell session so you can do things like 'cd' without doing hacks.
.ONESHELL:

# Tells make not to do crazy shit.
MAKEFLAGS += --no-builtin-rules

# Allows me to replace tabs with > characters. This makes the things a bit easier to use things like forloops in bash.
ifeq ($(origin .RECIPEPREFIX), undefined)
  $(error This Make does not support .RECIPEPREFIX. Please use GNU Make 4.0 or later)
endif
.RECIPEPREFIX = >

# App Vars
APP_NAME = astra-web
GIT_COMMIT = $(shell git rev-parse --short HEAD)
SHELL = /bin/bash
SEMVER = 0.0.0
VERSION = ${SEMVER}_${GIT_COMMIT}

## build: run tests and compile application
build:
> cd docs
> mdbook build
> cd ..
> npx tailwindcss -i ./src/main.css -o ./public/css/main.css --minify
> cargo test
> cargo build --release --target x86_64-unknown-linux-musl && cp ../target/x86_64-unknown-linux-musl/release/astra_web /tmp/astra-web
.PHONY: build

## run: build application and run server
run:
> @$(MAKE) -j run-tailwind run-backend
.PHONY: run

run-tailwind:
> npx tailwindcss -i ./src/main.css -o ./public/css/main.css --watch >/dev/null 2>&1

run-backend:
> cargo run -- start-web-service

deploy:
> scp /tmp/astra_web ubuntu@astra.orreri.dev:/tmp/astra_web
> ssh ubuntu@astra.orreri.dev 'sudo systemctl stop astra_web.service && \
sudo cp /tmp/astra_web /usr/bin/astra_web && \
sudo chmod +x /usr/bin/astra_web && \
sudo chown astra:astra /usr/bin/astra_web && \
sudo setcap "cap_net_bind_service=+ep" /usr/bin/astra_web && \
sudo systemctl start astra_web && \
sudo systemctl status astra_web'

install:
> make build OUTPUT=/tmp/astra-web
> cp /tmp/astra-web ~/.bin/astra-web

build-docker: check-semver-included
> cp /tmp/astra-web ./astra-web-bin
> docker build -t 645654336763.dkr.ecr.us-east-2.amazonaws.com/astra:latest .
> docker tag 645654336763.dkr.ecr.us-east-2.amazonaws.com/astra:latest 645654336763.dkr.ecr.us-east-2.amazonaws.com/astra:${SEMVER}
> rm ./astra-web-bin

push-docker: check-semver-included
> aws ecr get-login-password --region us-east-2 | docker login --username AWS --password-stdin 645654336763.dkr.ecr.us-east-2.amazonaws.com
> docker push 645654336763.dkr.ecr.us-east-2.amazonaws.com/astra:${SEMVER}
> docker push 645654336763.dkr.ecr.us-east-2.amazonaws.com/astra:latest

## build-docs: build final documentation site artifacts
build-docs:
> cd docs
> mdbook build
.PHONY: build-docs

## run-docs: build and run documentation website for development
run-docs:
> cd docs
> mdbook serve --open
.PHONY: run-docs

## help: prints this help message
help:
> @echo "Usage: "
> @sed -n 's/^##//p' ${MAKEFILE_LIST} | column -t -s ':' |  sed -e 's/^/ /'
.PHONY: help

check-path-included:
ifndef OUTPUT
>	$(error OUTPUT is undefined; ex. OUTPUT=/tmp/${APP_NAME})
endif

check-semver-included:
ifeq ($(SEMVER), 0.0.0)
>	$(error SEMVER is undefined; ex. SEMVER=0.0.1)
endif
