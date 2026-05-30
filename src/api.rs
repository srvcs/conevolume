use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::{OpenApi, ToSchema};

use crate::client::{self, DepError};

pub const SERVICE: &str = "srvcs-conevolume";
pub const CONCERN: &str = "geometry: volume of a cone";
pub const DEPENDS_ON: &[&str] = &["srvcs-pi", "srvcs-floatmultiply", "srvcs-floatdivide"];

/// Dependency endpoints, injected as router state so tests can point them at
/// mock services.
#[derive(Clone)]
pub struct Deps {
    pub pi_url: String,
    pub floatmultiply_url: String,
    pub floatdivide_url: String,
}

#[derive(Serialize, ToSchema)]
pub struct Info {
    pub service: &'static str,
    pub concern: &'static str,
    pub depends_on: Vec<&'static str>,
}

/// `GET /` — service identity (srvcs service standard).
#[utoipa::path(get, path = "/", responses((status = 200, body = Info)))]
pub async fn index() -> Json<Info> {
    Json(Info {
        service: SERVICE,
        concern: CONCERN,
        depends_on: DEPENDS_ON.to_vec(),
    })
}

#[derive(Deserialize, ToSchema)]
pub struct EvalRequest {
    /// The radius of the cone's circular base.
    #[schema(value_type = Object)]
    pub radius: Value,
    /// The height of the cone.
    #[schema(value_type = Object)]
    pub height: Value,
}

#[derive(Serialize, ToSchema)]
pub struct ConeVolumeResponse {
    #[schema(value_type = Object)]
    pub radius: Value,
    #[schema(value_type = Object)]
    pub height: Value,
    pub result: f64,
}

fn ok(radius: Value, height: Value, result: f64) -> Response {
    (
        StatusCode::OK,
        Json(json!({ "radius": radius, "height": height, "result": result })),
    )
        .into_response()
}

fn degraded(dependency: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "error": "dependency unavailable", "dependency": dependency })),
    )
        .into_response()
}

fn forward(status: u16, body: Value) -> Response {
    let code = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    (code, Json(body)).into_response()
}

/// A reachable dependency answered `200` but its body lacked a numeric
/// `result`. That is a contract violation we cannot recover from, so surface a
/// `500` rather than guessing.
fn malformed(dependency: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(
            json!({ "error": "dependency returned a malformed result", "dependency": dependency }),
        ),
    )
        .into_response()
}

/// Call one dependency at `url` with `body`, mapping its outcome to either the
/// parsed response body (on `200`) or an early-return `Response` the caller
/// should surface verbatim:
///
/// - unreachable / non-`200`/`422` -> `503` degraded
/// - `422` -> forwarded `422` (the dependency rejected the input)
async fn ask(url: &str, body: &Value, dependency: &str) -> Result<Value, Response> {
    match client::call(url, body).await {
        Err(DepError::Unreachable) => Err(degraded(dependency)),
        Ok((200, body)) => Ok(body),
        Ok((422, body)) => Err(forward(422, body)),
        Ok(_) => Err(degraded(dependency)),
    }
}

/// `POST /` — compute the volume of a cone, `V = (1/3) * pi * r^2 * height`.
///
/// This service owns the *control flow* but delegates every arithmetic step to
/// its float primitives, exactly as specified:
///
/// 1. `p = pi()` — `srvcs-pi`, a constant service called with an empty body;
/// 2. `r2 = floatmultiply(radius, radius)`;
/// 3. `base = floatmultiply(p, r2)`;
/// 4. `col = floatmultiply(base, height)` — the volume of the enclosing
///    cylinder;
/// 5. `result = floatdivide(col, 3)`.
///
/// It never validates operands itself: any `422` a dependency raises (e.g. a
/// non-numeric radius) is forwarded verbatim. If a dependency is unreachable it
/// reports itself degraded (`503`).
#[utoipa::path(
    post,
    path = "/",
    request_body = EvalRequest,
    responses(
        (status = 200, body = ConeVolumeResponse),
        (status = 422, description = "a dependency rejected an input (forwarded)"),
        (status = 500, description = "a dependency returned a malformed result"),
        (status = 503, description = "a dependency is unavailable")
    )
)]
pub async fn evaluate(State(deps): State<Deps>, Json(req): Json<EvalRequest>) -> Response {
    // 1. p = pi() — a constant service, called with an EMPTY body.
    let p_body = match ask(&deps.pi_url, &json!({}), "srvcs-pi").await {
        Ok(body) => body,
        Err(resp) => return resp,
    };
    let p = match p_body.get("result").and_then(Value::as_f64) {
        Some(v) => v,
        None => return malformed("srvcs-pi"),
    };

    // 2. r2 = radius * radius.
    let r2_body = match ask(
        &deps.floatmultiply_url,
        &json!({ "a": req.radius, "b": req.radius }),
        "srvcs-floatmultiply",
    )
    .await
    {
        Ok(body) => body,
        Err(resp) => return resp,
    };
    let r2 = match r2_body.get("result").and_then(Value::as_f64) {
        Some(v) => v,
        None => return malformed("srvcs-floatmultiply"),
    };

    // 3. base = p * r2.
    let base_body = match ask(
        &deps.floatmultiply_url,
        &json!({ "a": p, "b": r2 }),
        "srvcs-floatmultiply",
    )
    .await
    {
        Ok(body) => body,
        Err(resp) => return resp,
    };
    let base = match base_body.get("result").and_then(Value::as_f64) {
        Some(v) => v,
        None => return malformed("srvcs-floatmultiply"),
    };

    // 4. col = base * height (the volume of the enclosing cylinder).
    let col_body = match ask(
        &deps.floatmultiply_url,
        &json!({ "a": base, "b": req.height }),
        "srvcs-floatmultiply",
    )
    .await
    {
        Ok(body) => body,
        Err(resp) => return resp,
    };
    let col = match col_body.get("result").and_then(Value::as_f64) {
        Some(v) => v,
        None => return malformed("srvcs-floatmultiply"),
    };

    // 5. result = col / 3.
    let div_body = match ask(
        &deps.floatdivide_url,
        &json!({ "a": col, "b": 3 }),
        "srvcs-floatdivide",
    )
    .await
    {
        Ok(body) => body,
        Err(resp) => return resp,
    };
    let result = match div_body.get("result").and_then(Value::as_f64) {
        Some(v) => v,
        None => return malformed("srvcs-floatdivide"),
    };

    ok(req.radius, req.height, result)
}

#[derive(OpenApi)]
#[openapi(
    paths(index, evaluate),
    components(schemas(Info, EvalRequest, ConeVolumeResponse))
)]
pub struct ApiDoc;

/// Serve OpenAPI document
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_documents_routes() {
        let doc = ApiDoc::openapi();
        let root = doc.paths.paths.get("/").expect("path / present");
        assert!(root.get.is_some());
        assert!(root.post.is_some());
    }

    #[tokio::test]
    async fn index_reports_all_dependencies() {
        let Json(info) = index().await;
        assert_eq!(info.service, "srvcs-conevolume");
        assert_eq!(info.concern, "geometry: volume of a cone");
        assert_eq!(
            info.depends_on,
            vec!["srvcs-pi", "srvcs-floatmultiply", "srvcs-floatdivide"]
        );
    }
}
