use axum::body::Body;
use axum::extract::Json as AxumJson;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::{Json, Router as AxumRouter};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use srvcs_conevolume::{api::Deps, health, router, telemetry};
use tower::ServiceExt;

const DEAD_URL: &str = "http://127.0.0.1:1";

// --- Computing mocks for every srvcs primitive this family composes over.
//
// Each reads its operands from the request body and returns the *real* answer,
// so the orchestration is genuinely exercised rather than fed a canned value.
// conevolume only calls `srvcs-pi`, `srvcs-floatmultiply` and
// `srvcs-floatdivide`; the rest are provided for completeness of the family's
// contract.

/// `srvcs-pi`: a constant service — returns `{"result": PI}` for *any* body.
async fn spawn_pi() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|| async move { Json(json!({ "result": std::f64::consts::PI })) }),
    );
    serve(app).await
}

/// `srvcs-floatadd`: reads `{a, b}` -> `{"result": a + b}` (as f64).
#[allow(dead_code)]
async fn spawn_floatadd() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_f64).unwrap_or(0.0);
            let b = body.get("b").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": a + b }))
        }),
    );
    serve(app).await
}

/// `srvcs-floatsubtract`: reads `{a, b}` -> `{"result": a - b}` (as f64).
#[allow(dead_code)]
async fn spawn_floatsubtract() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_f64).unwrap_or(0.0);
            let b = body.get("b").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": a - b }))
        }),
    );
    serve(app).await
}

/// `srvcs-floatmultiply`: reads `{a, b}` -> `{"result": a * b}` (as f64).
async fn spawn_floatmultiply() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_f64).unwrap_or(0.0);
            let b = body.get("b").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": a * b }))
        }),
    );
    serve(app).await
}

/// `srvcs-floatdivide`: reads `{a, b}` -> `{"result": a / b}` (as f64).
async fn spawn_floatdivide() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_f64).unwrap_or(0.0);
            let b = body.get("b").and_then(Value::as_f64).unwrap_or(1.0);
            Json(json!({ "result": a / b }))
        }),
    );
    serve(app).await
}

/// `srvcs-sqrt`: reads `{value}` -> `{"result": value.sqrt()}` (as f64).
#[allow(dead_code)]
async fn spawn_sqrt() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let value = body.get("value").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": value.sqrt() }))
        }),
    );
    serve(app).await
}

/// `srvcs-sin`: reads `{value}` -> `{"result": value.sin()}` (as f64).
#[allow(dead_code)]
async fn spawn_sin() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let value = body.get("value").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": value.sin() }))
        }),
    );
    serve(app).await
}

/// `srvcs-cos`: reads `{value}` -> `{"result": value.cos()}` (as f64).
#[allow(dead_code)]
async fn spawn_cos() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let value = body.get("value").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": value.cos() }))
        }),
    );
    serve(app).await
}

/// `srvcs-tan`: reads `{value}` -> `{"result": value.tan()}` (as f64).
#[allow(dead_code)]
async fn spawn_tan() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let value = body.get("value").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": value.tan() }))
        }),
    );
    serve(app).await
}

/// Spawn a mock returning a fixed status + body (used for error-path tests).
async fn spawn_fixed(status: StatusCode, body: Value) -> String {
    let app = AxumRouter::new().route(
        "/",
        post(move || {
            let body = body.clone();
            async move { (status, Json(body)) }
        }),
    );
    serve(app).await
}

async fn serve(app: AxumRouter) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

fn app(pi_url: &str, floatmultiply_url: &str, floatdivide_url: &str) -> axum::Router {
    router(
        telemetry::metrics_handle_for_tests(),
        Deps {
            pi_url: pi_url.to_string(),
            floatmultiply_url: floatmultiply_url.to_string(),
            floatdivide_url: floatdivide_url.to_string(),
        },
    )
}

async fn conevolume(
    pi_url: &str,
    floatmultiply_url: &str,
    floatdivide_url: &str,
    radius: Value,
    height: Value,
) -> (StatusCode, Value) {
    let res = app(pi_url, floatmultiply_url, floatdivide_url)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "radius": radius, "height": height }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn status_of(uri: &str) -> StatusCode {
    app(DEAD_URL, DEAD_URL, DEAD_URL)
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

fn result_f64(body: &Value) -> f64 {
    body["result"].as_f64().expect("result is a JSON number")
}

// --- Standard endpoints. ---

#[tokio::test]
async fn healthz_ok() {
    assert_eq!(status_of("/healthz").await, StatusCode::OK);
}

#[tokio::test]
async fn readyz_reflects_state() {
    health::set_ready(true);
    assert_eq!(status_of("/readyz").await, StatusCode::OK);
}

#[tokio::test]
async fn metrics_ok() {
    assert_eq!(status_of("/metrics").await, StatusCode::OK);
}

#[tokio::test]
async fn openapi_ok() {
    assert_eq!(status_of("/openapi.json").await, StatusCode::OK);
}

#[tokio::test]
async fn generates_request_id_when_absent() {
    let res = app(DEAD_URL, DEAD_URL, DEAD_URL)
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        res.headers().contains_key("x-request-id"),
        "response must carry a generated x-request-id"
    );
}

#[tokio::test]
async fn index_reports_identity() {
    let res = app(DEAD_URL, DEAD_URL, DEAD_URL)
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["service"], "srvcs-conevolume");
    assert_eq!(body["concern"], "geometry: volume of a cone");
    assert_eq!(
        body["depends_on"],
        json!(["srvcs-pi", "srvcs-floatmultiply", "srvcs-floatdivide"])
    );
}

// --- Correctness cases, against the computing mocks. ---

async fn deps() -> (String, String, String) {
    (
        spawn_pi().await,
        spawn_floatmultiply().await,
        spawn_floatdivide().await,
    )
}

#[tokio::test]
async fn conevolume_3_4_is_canonical() {
    let (pi, m, d) = deps().await;
    let (status, body) = conevolume(&pi, &m, &d, json!(3), json!(4)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["radius"], json!(3));
    assert_eq!(body["height"], json!(4));
    // (1/3) * pi * 9 * 4 = 12 * pi = 37.69911184307752
    assert!((result_f64(&body) - 37.69911184307752).abs() < 1e-9);
}

#[tokio::test]
async fn conevolume_unit_cone() {
    let (pi, m, d) = deps().await;
    let (status, body) = conevolume(&pi, &m, &d, json!(1), json!(1)).await;
    assert_eq!(status, StatusCode::OK);
    // (1/3) * pi * 1 * 1 = pi / 3
    assert!((result_f64(&body) - std::f64::consts::PI / 3.0).abs() < 1e-9);
}

#[tokio::test]
async fn conevolume_fractional() {
    let (pi, m, d) = deps().await;
    let (status, body) = conevolume(&pi, &m, &d, json!(2.5), json!(6.0)).await;
    assert_eq!(status, StatusCode::OK);
    // (1/3) * pi * 6.25 * 6 = 12.5 * pi
    let expected = (1.0 / 3.0) * std::f64::consts::PI * 6.25 * 6.0;
    assert!((result_f64(&body) - expected).abs() < 1e-9);
}

#[tokio::test]
async fn conevolume_zero_radius_is_zero() {
    let (pi, m, d) = deps().await;
    let (status, body) = conevolume(&pi, &m, &d, json!(0), json!(10)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(result_f64(&body).abs() < 1e-9);
}

#[tokio::test]
async fn conevolume_zero_height_is_zero() {
    let (pi, m, d) = deps().await;
    let (status, body) = conevolume(&pi, &m, &d, json!(7), json!(0)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(result_f64(&body).abs() < 1e-9);
}

// --- Degraded / error paths. ---

#[tokio::test]
async fn degrades_when_pi_unreachable() {
    let (m, d) = (spawn_floatmultiply().await, spawn_floatdivide().await);
    let (status, body) = conevolume(DEAD_URL, &m, &d, json!(3), json!(4)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-pi");
}

#[tokio::test]
async fn degrades_when_floatmultiply_unreachable() {
    // pi is reachable, so the pipeline reaches the first floatmultiply call.
    let (pi, d) = (spawn_pi().await, spawn_floatdivide().await);
    let (status, body) = conevolume(&pi, DEAD_URL, &d, json!(3), json!(4)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-floatmultiply");
}

#[tokio::test]
async fn degrades_when_floatdivide_unreachable() {
    // pi + floatmultiply reachable, so the pipeline reaches the divide call.
    let (pi, m) = (spawn_pi().await, spawn_floatmultiply().await);
    let (status, body) = conevolume(&pi, &m, DEAD_URL, json!(3), json!(4)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-floatdivide");
}

#[tokio::test]
async fn forwards_422_from_floatmultiply() {
    // pi answers, then floatmultiply rejects a non-numeric operand -> forward.
    let pi = spawn_pi().await;
    let d = spawn_floatdivide().await;
    let m = spawn_fixed(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({ "error": "value is not a number" }),
    )
    .await;
    let (status, body) = conevolume(&pi, &m, &d, json!("nope"), json!(4)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "value is not a number");
}

#[tokio::test]
async fn forwards_422_from_floatdivide() {
    let pi = spawn_pi().await;
    let m = spawn_floatmultiply().await;
    let d = spawn_fixed(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({ "error": "bad operand" }),
    )
    .await;
    let (status, _) = conevolume(&pi, &m, &d, json!(3), json!(4)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn malformed_pi_result_is_500() {
    let (m, d) = (spawn_floatmultiply().await, spawn_floatdivide().await);
    let pi = spawn_fixed(StatusCode::OK, json!({ "result": "not-a-number" })).await;
    let (status, body) = conevolume(&pi, &m, &d, json!(3), json!(4)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["dependency"], "srvcs-pi");
}

#[tokio::test]
async fn malformed_floatmultiply_result_is_500() {
    let pi = spawn_pi().await;
    let d = spawn_floatdivide().await;
    let m = spawn_fixed(StatusCode::OK, json!({ "result": "not-a-number" })).await;
    let (status, body) = conevolume(&pi, &m, &d, json!(3), json!(4)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["dependency"], "srvcs-floatmultiply");
}
