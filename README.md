<!-- PROJECT LOGO -->
<br />
<p align="center">
  <a href="https://www.3scale.net/">
    <img src="assets/img/threescale.png" alt="Logo" width="800" height="250">
  </a>

  <h3 align="center">3scale envoy proxy authorization cache</h3>

  <p align="center">
    GSoC 2021
    <br />
    <a href="#"><strong>Explore the docs »</strong></a>
    <br />
    <br />
    <a href="#">View Demo</a>
    ·
    <a href="#">Report Bug</a>
    ·
    <a href="#">Request Feature</a>
  </p>
</p>

<!-- TABLE OF CONTENTS -->
## Table of Contents

* [About the Project](#about-the-project)
* [Prerequisites](#prerequisites)
* [Installation](#installation)
* [Cache-filter Design](#cache-filter-design)
* [Integration tests](#writing-integration-tests)
* [License](#license)


<!-- ABOUT THE PROJECT -->
## About The Project

The project is done as a part of Google Summer of Code 2021 programme. The main intention of the project is to 
implement an in-proxy authorization cache for envoy proxy which performs authorization and rate limiting based on
the in-proxy cache reducing the request latency. Also, it will reduce the traffic on the threescale service management API by
synchronizing with the service management API based on various policies defined instead of making 1 HTTP call per request.       

## Prerequisites

###  Build Prerequisites for filters and singleton service
* Rust
* Cargo
* Make

### Prerequisites for integration tests
* Golang
* Docker
* Docker Compose

## Installation
 
1. Clone the repo 
```sh
git clone https://github.com/3scale-labs/gsoc-wasm-filters.git
cd gsoc-wasm-filters
```
2. Build the project

> Building the cache filter, singleton service and threescale auth filter.

* Development build
```sh
make build
```
* Production build
> For the production build `wasm-opt` and `wasm-snip` are required as the auth filter requires them for build optimizations. 
```sh
make build BUILD=release
```
* Building individual components

Individual components can be built using the following make commands.

```sh
make cache
make service
make auth
```
Build artifacts for the release can be built by passing `BUILD=release` to the individual commands. After a successful build, 
build artifacts generated will get placed in the deployments/docker-compose folder.

3. Run the integration tests.

For the integration tests, golang is required to be installed on the host.

> Please note that integration tests should be executed after a successful build when build artifacts are available in the `deployments/docker-compose folder`.

```sh
make integration
```

4. Start the services with docker-compose

```sh
make run
```

5. Send sample test requests for the following scenarios.

> Please note that the application id and service token related to the above tests are hard coded into `deployments/docker-compose/envoy.yaml`.

* Send a GET request with `app_id` and `app_key` pattern.

```sh
curl -X GET 'localhost:9095/' -H 'x-app-id: fcf4db29' -H 'x-app-key: 9a0435ee68f5d647f03a80480a97a326'
```

* Send a GET request with `user_key` pattern.

```sh
curl -X GET 'localhost:9095/?api_key=46de54605a1321aa3838480c5fa91bcc'
```

## Cache-filter Design
**Input requirements**

**Flow**

**Configuration option**

## Writing integration tests

Integration tests are written in golang and executed by starting related services in docker containers using docker-compose. 

In integration-tests directory,
* `main.go` contains helper method for generating custom config files, serial searching patterns through response headers, starting/stoping proxy and middleware service.
* `apisonator.go` contains helper methods for maintaining local state of Apisonator during tests.
* `middleware` directory contains source code of a service to add custom delay to the response from apisonator. Note: when using this service, you need to update `envoy.yaml` file to point to it and it will act as a proxy server in-between.

Integration tests can be implemented in 2 ways.

1. For general cases where no specific deployment pattern is required. The basic template with 1 envoy proxy and 1 solsson/http-echo can be used.

Here `docker-compose.yaml` and `Dockerfile` are not needed since it uses already available common template. Only `envoy.yaml` is required.
First use the `BuildnStartContainers()` helper method to start docker containers by passing the path of the required
`envoy.yaml`. eg: `BuildnStartContainers("./configs/app-id/envoy.yaml")`. Then implement related testing logic using testify suite and use `BuildStopContainers()` to stop
the docker containers. Examples can be found in `app_id_test.go`.

2. For special cases where a special docker-compose configuration is required. Need to provide `docker-compose.yaml`, `Dockerfile` and `envoy.yaml`.

First create a directory in the configs folder and add related `docker-compose.yaml`, `Dockerfile` and `envoy.yaml`. Then use `StartContainers()` helper to
start docker containers by providing the related configuration folder path. eg: `StartContainers("./configs/app-id/docker-compose.yaml")`. Then implement related
testing logic using testify suite and use `StopContainers("")` to stop the docker containers by providing the path of the related config folder. eg: `StopContainers("./configs/app-id/docker-compose.yaml")`. Examples can be found in `app_id_test.go`.

> For all the tests, it is important to add a delay after container initialization and testing in order to provide time for services to be available when running inside hosts with less performance, CI/CD pipelines.

**Using testify suite**

For each group of related independant tests, a test suite can be created as follows.

```go
type ExampleTestSuite struct {
    suite.Suite
}
```

For each group, `SetupSuite` method can be used to implement suite initialization like starting services using docker-compose, application, service initialization etc.

```go
func (suite *AppCredentialTestSuite) SetupSuite() {
	// Initialization logic goes here
}
```

Also for advance testing if each test requires some initialization, `SetupTest()`, `BeforeTest()` can be used.
The test cases can be implemented by implementing a test func for each case.

```go
func (suite *ExampleTestSuite) TestExample() {
    // Test logic goes here
    assert.Equal(suite.T(), 123, 123)
}

```

Finally we can run a clean up function using `TearDownSuite()`. Here we can stop the services that runs in docker.

```go
func (suite *AppCredentialTestSuite) TearDownSuite() {
  // Clean up logic goes here
}
```
Also for advanced testing cases, `TearDownTest()` can be used to clean up after every test.

**Generating custom envoy configs**

`GenerateConfig` allows you to generate custom envoy.yaml files during the test time phase. It takes two params: 1) name of the file (e.g. `temp.yaml`) 2) JSON formated key-value pairs. These keys should match those present in the `config_template.yaml` file and will be replaced with the value provided.

```json
{ 
	"UpstreamURL": "\"http://dogecoin.net:3000\"",
}
```
will replace the following key in the config:
```json
....
"upstream":
  ....
  "url": {{or .UpstreamURL "\"localhost:3000\""}},
....
// Note: if 'UpstreamURL key is not provided, next value i.e "localhost:3000" will be used instead.
````
For a working example, that generates a custom config file and starts proxy using that, please refer to `config_test.go` file.

For making any changes to `config_template.yaml`, please refer to https://golang.org/pkg/text/template/

<!-- LICENSE -->
## License

Distributed under the Apache License Version 2.0. See `LICENSE` for more information.
