use axum::{
    extract::State,
    response::{Html, IntoResponse},
};
use maud::html;

use crate::{ALLOWED_MODELS, DEFAULT_MODEL, metrics::database::MetricsState};

#[utoipa::path(
    get,
    path = "/",
    responses(
        (status = 200, description = "Metrics page", body = String)
    ),
    tag = "Metrics"
)]
pub async fn index(State(state): State<MetricsState>) -> impl IntoResponse {
    let mut total: i64 = 0;

    if let Some(pool) = &state.db {
        if let Ok(client) = pool.get().await {
            if let Ok(rows) = client
                .query("SELECT COALESCE(SUM(tokens), 0) AS sum FROM api_logs", &[])
                .await
            {
                if let Some(row) = rows.first() {
                    total = row.get::<_, i64>("sum");
                }
            }
        }
    }

    Html(html! {
        (maud::DOCTYPE) // important lol
        html {
            head {
                title { "Hack Club | AI" }
                script src="https://cdn.tailwindcss.com" {}
            }
            body class="bg-black text-white min-h-screen p-6 font-mono" {
                div class="max-w-4xl mx-auto space-y-8" {
                    div class="space-y-4" {
                        h1 class="text-4xl font-bold" { "ai.hackclub.com" }
                        p class="text-lg" {
                            "Unlimited "
                            code class="bg-gray-900 px-2 py-1 rounded text-blue-400" { "/chat/completions" }
                            " for teens in "
                            a href="https://hackclub.com/" target="_blank" { "Hack Club" }
                            ". No API key needed."
                            br;
                            span class="font-semibold" { (total.to_string()) }
                            " tokens processed since October 2025."
                        }
                        div class="text-sm" {
                            "Available models: "
                            span class="inline-flex flex-wrap gap-2" {
                                @for model in ALLOWED_MODELS.split(',') {
                                    @if model.trim() == DEFAULT_MODEL {
                                        code class="bg-gray-900 px-2 py-1 rounded text-white" {
                                            (model.trim()) " " span class="text-blue-400 text-xs" { "default" }
                                        }
                                    } @else {
                                        code class="bg-gray-900 px-2 py-1 rounded text-white" { (model.trim()) }
                                    }
                                }
                            }
                        }
                        p class="text-sm" {
                            "Open source " a href="https://github.com/hackclub/ai" class="text-blue-400" { "here" }
                        }
                    }
                    div class="space-y-4" {
                        h2 class="text-2xl font-semibold" { "Usage" }
                        div class="space-y-3" {
                            h3 class="text-lg" { "> Chat Completions" }
                            pre class="bg-gray-900 p-4 rounded overflow-x-auto" {
                                code class="text-sm" {
                                    span class="text-white" { "curl" } " "
                                    span class="text-gray-500" { "-X" } " "
                                    span class="text-white" { "POST" }
                                    " https://ai.hackclub.com/chat/completions \\\n  "
                                    span class="text-gray-500" { "-H" } " "
                                    span class="text-gray-400" { "\"Content-Type: application/json\"" }
                                    " \\\n  "
                                    span class="text-gray-500" { "-d" } " "
                                    span class="text-gray-400" { "'{\"messages\": [{\"role\": \"user\", \"content\": \"Tell me a joke!\"}]}'" }
                                }
                            }
                        }
                        div class="space-y-3" {
                            h3 class="text-lg" { "> Get Models" }
                            pre class="bg-gray-900 p-4 rounded overflow-x-auto" {
                                code class="text-sm" {
                                    span class="text-white" { "curl" } " https://ai.hackclub.com/model"
                                }
                            }
                        }
                    }
                    div class="space-y-4" {
                        h2 class="text-2xl font-semibold" { "Terms" }
                        p {
                            "You must be a teenager in the "
                            a href="https://hackclub.com/slack" { "Hack Club Slack" }
                            ". All requests and responses are logged to prevent abuse. Projects only - no personal use. This means you can't use it in Cursor or anything similar for the moment! Abuse means this will get shut down - we're a nonprofit funded by donations."
                        }
                    }
                    div class="space-y-4" {
                        h2 class="text-2xl font-semibold" { "Documentation" }
                        a href="/docs" class="inline-block px-6 py-3 border border-white" { "View API Docs" }
                    }
                }
            }
        }
    }.into_string())
}
