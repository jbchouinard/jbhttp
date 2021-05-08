# jbhttp

A Rust web service framework with (almost) no dependencies. Also includes a poorly tested,
non-spec compliant development HTTP server.

I wrote this as a Rust learning project, this is not in any way production-tested.

[api_service.rs](/examples/api_service.rs) is a heavily commented example
service which showcases most of the features of the library.

## Usage

### Read documentation

```bash
cargo doc --open
```

### Run file server example

```bash
cargo run --example file_service -- -vvd examples/html --threads 8
```

```bash
curl http://localhost:8080/index.html
```

### Run API server example

```bash
cargo run --example api_service -- -vv
```

```bash
curl -v -H 'X-Api-Key: secret' -H 'Content-Type: application/json' -d '{"name": "John Smith", "age": 42}' 'http://localhost:8080/person/'
curl -v -H 'X-Api-Key: secret' 'http://localhost:8080/person/1'
```
