# jbhttp

A Rust HTTP server framework with (almost) no dependencies. This is an educational project, use at your own risk.

## Read documentation

```bash
cargo doc --open
```

## Run file server example

```bash
cargo run --example file_service -- -vvd examples/html --threads 8
```

```bash
curl http://localhost:8080/index.html
```

## Run API server example

```bash
cargo run --example api_service -- -vv
```

```bash
curl -v 'http://localhost:8080/person/1'
curl -v -H 'X-Api-Key: secret' 'http://localhost:8080/person/1'
curl -v -H 'X-Api-Key: secret' -H 'Content-Type: application/json' -d '{"name": "John Smith", "age": 42}' 'http://localhost:8080/person/'
curl -v -H 'X-Api-Key: secret' 'http://localhost:8080/person/1'
curl -v -H 'X-Api-Key: secret' -X DELETE 'http://localhost:8080/person/1'
curl -v -H 'X-Api-Key: secret' 'http://localhost:8080/person/1'
```
