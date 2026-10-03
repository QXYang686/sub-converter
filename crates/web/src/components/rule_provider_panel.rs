use leptos::prelude::*;
use leptos::task::spawn_local;

use contract::subscription::RuleProviderResponse;

use crate::api::user_message;
use crate::state::{AuthStatus, AuthStore};

use super::{Alert, CodeBlock, Spinner};

/// 某个订阅源的 rule-provider 列表与服务端快照内容。
#[component]
pub fn RuleProviderPanel(source_id: String) -> impl IntoView {
    let store = AuthStore::expect();
    let status = store.status;
    let providers = RwSignal::new(Option::<Vec<RuleProviderResponse>>::None);
    let error = RwSignal::new(None::<String>);
    let viewing = RwSignal::new(None::<String>);
    let content = RwSignal::new(None::<String>);

    let load_store = store.clone();
    let load_id = source_id.clone();
    Effect::new(move |_| {
        if !matches!(status.get(), AuthStatus::Authenticated(_)) {
            return;
        }
        let store = load_store.clone();
        let id = load_id.clone();
        spawn_local(async move {
            match store.source_providers(&id).await {
                Ok(list) => providers.set(Some(list)),
                Err(err) => error.set(Some(user_message(&err))),
            }
        });
    });

    let view_store = store.clone();
    let view_id = source_id.clone();
    let on_view = move |name: String| {
        if viewing.get_untracked() == Some(name.clone()) {
            viewing.set(None);
            content.set(None);
            return;
        }
        viewing.set(Some(name.clone()));
        content.set(None);
        error.set(None);
        let store = view_store.clone();
        let id = view_id.clone();
        spawn_local(async move {
            match store.source_provider_content(&id, &name).await {
                Ok(text) => content.set(Some(text)),
                Err(err) => {
                    viewing.set(None);
                    error.set(Some(user_message(&err)));
                }
            }
        });
    };

    view! {
        <div class="mt-2 rounded-lg border border-slate-800 bg-slate-950/40 p-3">
            <Alert message=error.into()/>
            {move || match providers.get() {
                None => view! { <Spinner/> }.into_any(),
                Some(list) if list.is_empty() => {
                    view! {
                        <p class="text-xs text-slate-500">"该订阅源没有 rule-providers"</p>
                    }
                        .into_any()
                }
                Some(list) => {
                    view! {
                        <ul class="space-y-1.5">
                            {list
                                .into_iter()
                                .map(|provider| {
                                    let name = provider.name.clone();
                                    let toggle_name = name.clone();
                                    let label_name = name.clone();
                                    let show_name = name.clone();
                                    let on_view = on_view.clone();
                                    let meta = {
                                        let mut parts = Vec::new();
                                        if let Some(behavior) = provider.behavior.clone() {
                                            parts.push(behavior);
                                        }
                                        if let Some(kind) = provider.provider_type.clone() {
                                            parts.push(kind);
                                        }
                                        parts.join(" · ")
                                    };
                                    let location = provider.url.clone().unwrap_or_default();
                                    let status_line = if provider.has_snapshot {
                                        format!(
                                            "{} 条规则 · 快照 {}",
                                            provider.rule_count,
                                            format_time(provider.fetched_at),
                                        )
                                    } else if provider.last_error.is_some() {
                                        "抓取失败".to_string()
                                    } else {
                                        "无快照（非 http provider 不代抓）".to_string()
                                    };
                                    let last_error = provider.last_error.clone();
                                    let viewable = provider.has_snapshot;
                                    let is_viewing = move || viewing.get() == Some(show_name.clone());
                                    view! {
                                        <li class="rounded border border-slate-800 px-2 py-1.5">
                                            <div class="flex items-start justify-between gap-2">
                                                <div class="min-w-0">
                                                    <p class="truncate text-xs text-slate-300">
                                                        {name.clone()}
                                                        {(!meta.is_empty())
                                                            .then(|| {
                                                                view! {
                                                                    <span class="ml-2 text-[10px] uppercase tracking-wide text-slate-500">
                                                                        {meta.clone()}
                                                                    </span>
                                                                }
                                                            })}
                                                    </p>
                                                    {(!location.is_empty())
                                                        .then(|| {
                                                            view! {
                                                                <p class="truncate text-[11px] text-slate-500">
                                                                    {location.clone()}
                                                                </p>
                                                            }
                                                        })}
                                                    <p class="text-[11px] text-slate-500">{status_line}</p>
                                                    {last_error
                                                        .map(|message| {
                                                            view! {
                                                                <p class="truncate text-[11px] text-red-300">
                                                                    {message}
                                                                </p>
                                                            }
                                                        })}
                                                </div>
                                                {viewable
                                                    .then(|| {
                                                        view! {
                                                            <button
                                                                type="button"
                                                                class="shrink-0 text-xs text-slate-400 transition hover:text-slate-200"
                                                                on:click=move |_| on_view(toggle_name.clone())
                                                            >
                                                                {move || {
                                                                    if viewing.get() == Some(label_name.clone()) {
                                                                        "收起"
                                                                    } else {
                                                                        "查看"
                                                                    }
                                                                }}
                                                            </button>
                                                        }
                                                    })}
                                            </div>
                                            <Show when=is_viewing.clone()>
                                                {move || match content.get() {
                                                    Some(text) => {
                                                        view! { <CodeBlock content=text/> }.into_any()
                                                    }
                                                    None => view! { <Spinner/> }.into_any(),
                                                }}
                                            </Show>
                                        </li>
                                    }
                                })
                                .collect_view()}
                        </ul>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

fn format_time(value: Option<i64>) -> String {
    let Some(value) = value.filter(|value| *value > 0) else {
        return "时间未知".to_string();
    };
    #[cfg(target_arch = "wasm32")]
    {
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(value as f64 * 1000.0));
        if let Some(iso) = date.to_iso_string().as_string() {
            return iso.chars().take(16).collect::<String>().replace('T', " ");
        }
    }
    value.to_string()
}
