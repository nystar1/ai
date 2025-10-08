use std::net::SocketAddr;

use axum::{
    body::{Body, to_bytes},
    extract::{ConnectInfo, Json, Request, State},
    http::{Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use futures::StreamExt;
use serde_json::{Value, from_slice};
use tokio::spawn;
use tracing::error;

use crate::{
    CLIENT, COMPLETIONS_URL, DEFAULT_MODEL,
    delegates::error::APIError,
    is_allowed_model,
    metrics::database::{MetricsState, extract_tokens},
};

pub async fn validate_model(req: Request, next: Next) -> Result<Response, APIError> {
    let (parts, body) = req.into_parts();

    let bytes = to_bytes(body, usize::MAX).await.map_err(|_| APIError {
        code: StatusCode::BAD_REQUEST,
        body: Some("Failed to read request body"),
    })?;

    let mut json: Value = from_slice(&bytes).map_err(|_| APIError {
        code: StatusCode::BAD_REQUEST,
        body: Some("Invalid JSON"),
    })?;

    if let Some(obj) = json.as_object_mut() {
        if let Some(tier) = obj.get("service_tier").and_then(Value::as_str) {
            if tier != "flex" && tier != "on_demand" {
                obj.remove("service_tier");
            }
        } else {
            obj.remove("service_tier");
        }

        let needs_update = obj
            .get("model")
            .and_then(Value::as_str)
            .map_or(true, |m| !is_allowed_model(m));

        if needs_update {
            obj.insert(
                "model".to_string(),
                Value::String(DEFAULT_MODEL.to_string()),
            );
        }
    }

    let body = serde_json::to_vec(&json).map_err(|_| APIError {
        code: StatusCode::INTERNAL_SERVER_ERROR,
        body: Some("Failed to serialize request"),
    })?;

    Ok(next.run(Request::from_parts(parts, Body::from(body))).await)
}

#[utoipa::path(
    post,
    path = "/chat/completions",
    request_body(
        content = serde_json::Value,
        example = json!({
            "messages": [{"role": "user", "content": "Tell me a joke!"}]
        })
    ),
    responses(
        (status = 200, description = "Chat completion successful", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 502, description = "Upstream service error")
    ),
    tag = "Chat",
    description = "Refer to [Groq](https://console.groq.com/docs/api-reference#chat-create) or [OpenAI](https://platform.openai.com/docs/api-reference/introduction) documentation for guidelines on how to call this endpoint."
)]
pub async fn completions(
    State(state): State<MetricsState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(request): Json<Value>,
) -> impl IntoResponse {
    let response = CLIENT
        .request(Method::POST, COMPLETIONS_URL)
        .json(&request)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to send request to Groq: {}", e);
            APIError {
                code: StatusCode::BAD_GATEWAY,
                body: Some("Failed to connect to upstream service"),
            }
        })?;

    if !response.status().is_success() {
        return Err(APIError {
            code: response.status(),
            body: Some("Upstream service error"),
        });
    }

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or(header::HeaderValue::from_static("application/json"));

    let is_streaming = request
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let ip = addr.ip();

    if is_streaming {
        let stream = response.bytes_stream().map(move |chunk_result| {
            if let Ok(ref chunk) = chunk_result {
                if let Some(json) = String::from_utf8_lossy(chunk)
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("data: ")
                            .filter(|&d| d != "[DONE]")
                            .and_then(|d| serde_json::from_str::<Value>(d).ok())
                            .filter(|j| j.get("x_groq").and_then(|x| x.get("usage")).is_some())
                    })
                {
                    let state = state.clone();
                    let request = request.clone();
                    let tokens = extract_tokens(&json, true);
                    spawn(async move {
                        state.log_request(&request, &json, ip, tokens).await;
                    });
                }
            }
            chunk_result
        });

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from_stream(stream))
            .unwrap())
    } else {
        let bytes = response.bytes().await.map_err(|e| {
            error!("Failed to read response body: {}", e);
            APIError {
                code: StatusCode::BAD_GATEWAY,
                body: Some("Failed to read upstream response"),
            }
        })?;

        let json: Value = from_slice(&bytes).map_err(|e| {
            error!("Failed to parse response JSON: {}", e);
            APIError {
                code: StatusCode::BAD_GATEWAY,
                body: Some("Invalid response from upstream service"),
            }
        })?;

        let tokens = extract_tokens(&json, false);
        state.log_request(&request, &json, ip, tokens).await;

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(bytes))
            .unwrap())
    }
}
