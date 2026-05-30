# srvcs-conevolume

The cone-volume orchestrator of the srvcs.cloud distributed standard library.

Its single concern: **geometry: volume of a cone.** It owns the *control flow* —
composing three float primitives — but does no arithmetic of its own. It asks
[`srvcs-pi`](https://github.com/srvcs/pi) for the constant, squares the radius
and chains the cylinder volume through
[`srvcs-floatmultiply`](https://github.com/srvcs/floatmultiply), then divides by
three with [`srvcs-floatdivide`](https://github.com/srvcs/floatdivide).

```
conevolume(radius, height):
    p    = pi()                          # srvcs-pi, called with an empty body
    r2   = floatmultiply(radius, radius)
    base = floatmultiply(p, r2)
    col  = floatmultiply(base, height)   # volume of the enclosing cylinder
    return floatdivide(col, 3)           # V = (1/3) * pi * r^2 * height
```

The result is an `f64` — a JSON number that may be fractional. For example
`conevolume(3, 4) == 37.69911184307752`.

Validation is not handled here. This service never calls `srvcs-isnumber`
directly; instead its dependencies validate their own operands, and any `422`
they raise is forwarded verbatim.

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/` | Service identity, concern, and dependency list |
| `POST` | `/` | Compute the volume of a cone from `radius` and `height` |
| `GET` | `/healthz` `/readyz` `/metrics` `/openapi.json` | srvcs service standard surface |

```sh
curl -s -X POST localhost:8080/ -H 'content-type: application/json' -d '{"radius": 3, "height": 4}'
# {"radius":3,"height":4,"result":37.69911184307752}
```

Responses:

- `200 {"radius": r, "height": h, "result": n}` — evaluated; `result` is a float.
- `422` — a dependency rejected an input (forwarded verbatim).
- `500` — a reachable dependency returned a `200` without a numeric `result`
  (a contract violation).
- `503` — a dependency is unavailable.

## Dependencies

- [`srvcs-pi`](https://github.com/srvcs/pi)
- [`srvcs-floatmultiply`](https://github.com/srvcs/floatmultiply)
- [`srvcs-floatdivide`](https://github.com/srvcs/floatdivide)

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SRVCS_BIND_ADDR` | `0.0.0.0:8080` | Bind address |
| `SRVCS_PI_URL` | `http://127.0.0.1:8090` | Base URL of `srvcs-pi` |
| `SRVCS_FLOATMULTIPLY_URL` | `http://127.0.0.1:8091` | Base URL of `srvcs-floatmultiply` |
| `SRVCS_FLOATDIVIDE_URL` | `http://127.0.0.1:8092` | Base URL of `srvcs-floatdivide` |
| `SRVCS_ENV` | `development` | Environment label for logs |
| `RUST_LOG` | `info,tower_http=info` | Tracing filter |

## Local checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Orchestration tests stand up *computing* mock dependency services in-process —
they read the request body and return the real `pi` / `a * b` / `a / b`, so the
composition is genuinely exercised against the asserted cases (compared
approximately, since the result is a float). See
[`srvcs/platform`](https://github.com/srvcs/platform) for the shared standard.

> Note: the `cargoHash` in `flake.nix` is inherited from the template and must be
> refreshed with a `nix build` before the Nix gates pass.
