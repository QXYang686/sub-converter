use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;

use contract::subscription::{SourceResponse, UpdateSourceRequest};

use crate::api::user_message;
use crate::components::{Alert, RequireAuth, Spinner, TextField};
use crate::forms::validate_source;
use crate::state::{AuthStatus, AuthStore};

#[component]
pub fn SourcesPage() -> impl IntoView {
    let store = AuthStore::expect();
    let status = store.status;
    let sources = RwSignal::new(Option::<Vec<SourceResponse>>::None);
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let reload = RwSignal::new(0u32);

    let list_store = store.clone();
    Effect::new(move |_| {
        if !matches!(status.get(), AuthStatus::Authenticated(_)) {
            return;
        }
        reload.get();
        let store = list_store.clone();
        spawn_local(async move {
            match store.list_sources().await {
                Ok(list) => sources.set(Some(list)),
                Err(err) => error.set(Some(user_message(&err))),
            }
        });
    });

    let name = RwSignal::new(String::new());
    let url = RwSignal::new(String::new());

    let create_store = store.clone();
    let on_create = move |ev: SubmitEvent| {
        ev.prevent_default();
        match validate_source(&name.get_untracked(), &url.get_untracked()) {
            Err(message) => error.set(Some(message)),
            Ok((clean_name, clean_url)) => {
                error.set(None);
                busy.set(true);
                let store = create_store.clone();
                spawn_local(async move {
                    match store.create_source(&clean_name, &clean_url).await {
                        Ok(_) => {
                            name.set(String::new());
                            url.set(String::new());
                            reload.update(|value| *value += 1);
                        }
                        Err(err) => error.set(Some(user_message(&err))),
                    }
                    busy.set(false);
                });
            }
        }
    };

    let editing = RwSignal::new(None::<String>);
    let edit_name = RwSignal::new(String::new());
    let edit_url = RwSignal::new(String::new());

    let save_store = store.clone();
    let on_save = move |_| {
        let Some(id) = editing.get() else {
            return;
        };
        match validate_source(&edit_name.get_untracked(), &edit_url.get_untracked()) {
            Err(message) => error.set(Some(message)),
            Ok((clean_name, clean_url)) => {
                let request = UpdateSourceRequest {
                    name: Some(clean_name),
                    url: Some(clean_url),
                    enabled: None,
                };
                error.set(None);
                busy.set(true);
                let store = save_store.clone();
                spawn_local(async move {
                    match store.update_source(&id, &request).await {
                        Ok(_) => {
                            editing.set(None);
                            reload.update(|value| *value += 1);
                        }
                        Err(err) => error.set(Some(user_message(&err))),
                    }
                    busy.set(false);
                });
            }
        }
    };

    let toggle_store = store.clone();
    let on_toggle = move |source: SourceResponse| {
        let id = source.id.clone();
        let request = UpdateSourceRequest {
            name: None,
            url: None,
            enabled: Some(!source.enabled),
        };
        error.set(None);
        busy.set(true);
        let store = toggle_store.clone();
        spawn_local(async move {
            match store.update_source(&id, &request).await {
                Ok(_) => reload.update(|value| *value += 1),
                Err(err) => error.set(Some(user_message(&err))),
            }
            busy.set(false);
        });
    };

    let delete_store = store.clone();
    let on_delete = move |id: String| {
        error.set(None);
        busy.set(true);
        let store = delete_store.clone();
        spawn_local(async move {
            match store.delete_source(&id).await {
                Ok(()) => reload.update(|value| *value += 1),
                Err(err) => error.set(Some(user_message(&err))),
            }
            busy.set(false);
        });
    };

    let on_edit = move |source: SourceResponse| {
        editing.set(Some(source.id.clone()));
        edit_name.set(source.name);
        edit_url.set(source.url);
    };

    view! {
        <RequireAuth>
            <div class="space-y-6">
                <div>
                    <h1 class="text-2xl font-semibold">"订阅源"</h1>
                    <p class="mt-1 text-sm text-slate-400">
                        "添加需要导入的外部订阅，供发布订阅组合使用。"
                    </p>
                </div>
                <Alert message=error.into()/>
                <section class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <h2 class="text-lg font-medium">"新建订阅源"</h2>
                    <form class="mt-4 space-y-3" on:submit=on_create>
                        <TextField label="名称" value=name/>
                        <TextField label="订阅链接" value=url/>
                        <button
                            type="submit"
                            disabled=move || busy.get()
                            class="w-full rounded-lg bg-slate-100 px-4 py-2 font-medium text-slate-900 transition hover:bg-white disabled:cursor-not-allowed disabled:opacity-50"
                        >
                            "添加"
                        </button>
                    </form>
                </section>
                <section class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <h2 class="text-lg font-medium">"已添加"</h2>
                    {move || match sources.get() {
                        None => view! { <Spinner/> }.into_any(),
                        Some(list) if list.is_empty() => {
                            view! {
                                <p class="mt-4 text-sm text-slate-500">"还没有订阅源"</p>
                            }
                                .into_any()
                        }
                        Some(list) => {
                            view! {
                                <ul class="mt-4 space-y-2">
                                    {list
                                        .into_iter()
                                        .map(|source| {
                                            let id = source.id.clone();
                                            let is_editing = editing.get() == Some(id.clone());
                                            let on_toggle = on_toggle.clone();
                                            let on_delete = on_delete.clone();
                                            let on_edit = on_edit.clone();
                                            let row = if is_editing {
                                                view! {
                                                    <div class="space-y-2">
                                                        <TextField label="名称" value=edit_name/>
                                                        <TextField label="订阅链接" value=edit_url/>
                                                        <div class="flex gap-2">
                                                            <button
                                                                type="button"
                                                                disabled=move || busy.get()
                                                                class="rounded-lg bg-slate-100 px-3 py-1.5 text-sm font-medium text-slate-900 transition hover:bg-white disabled:opacity-50"
                                                                on:click=on_save.clone()
                                                            >
                                                                "保存"
                                                            </button>
                                                            <button
                                                                type="button"
                                                                class="rounded-lg border border-slate-700 px-3 py-1.5 text-sm text-slate-200 transition hover:bg-slate-800"
                                                                on:click=move |_| editing.set(None)
                                                            >
                                                                "取消"
                                                            </button>
                                                        </div>
                                                    </div>
                                                }
                                                    .into_any()
            } else {
                let toggle_source = source.clone();
                let edit_source = source.clone();
                let delete_id = id.clone();
                let enabled = source.enabled;
                let snapshot = source.snapshot.clone();
                let traffic = snapshot.as_ref().and_then(|snap| {
                    let total = snap.total.filter(|total| *total > 0)?;
                    let used = snap
                        .upload
                        .unwrap_or(0)
                        .saturating_add(snap.download.unwrap_or(0));
                    let percent =
                        ((used as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as u32;
                    Some((
                        format!(
                            "已用 {} / {} · 剩余 {}",
                            format_bytes(used),
                            format_bytes(total),
                            format_bytes(total.saturating_sub(used))
                        ),
                        percent,
                    ))
                });
                let nodes = snapshot.as_ref().map(|snap| {
                    let protocols = snap
                        .protocol_counts
                        .iter()
                        .map(|entry| format!("{} {}", entry.protocol, entry.count))
                        .collect::<Vec<_>>()
                        .join(" · ");
                    let mut line = format!(
                        "节点 {} · 分组 {} · 规则 {}",
                        snap.proxy_count, snap.group_count, snap.rule_count
                    );
                    if !protocols.is_empty() {
                        line.push_str(&format!("（{protocols}）"));
                    }
                    line
                });
                let expire = snapshot
                    .as_ref()
                    .and_then(|snap| snap.expire.filter(|expire| *expire > 0))
                    .map(|expire| format!("到期 {}", format_expire(expire)));
                let provider = snapshot
                    .as_ref()
                    .and_then(|snap| snap.provider_name.clone())
                    .map(|name| format!("机场 {name}"));
                let last_error = snapshot.as_ref().and_then(|snap| snap.last_error.clone());
                let fetched = snapshot.is_some();
                view! {
                    <div class="flex items-center justify-between gap-3">
                        <div class="min-w-0">
                            <p class="truncate text-sm">
                                {source.name.clone()}
                                <Show when=move || !enabled>
                                    <span class="ml-2 text-xs text-amber-300">
                                        "已停用"
                                    </span>
                                </Show>
                            </p>
                            <p class="truncate text-xs text-slate-500">
                                {source.url.clone()}
                            </p>
                            <Show when=move || !fetched>
                                <p class="mt-1 text-xs text-slate-500">"尚无抓取数据"</p>
                            </Show>
                            {traffic
                                .map(|(label, percent)| {
                                    view! {
                                        <div class="mt-1 w-48">
                                            <div class="h-1.5 overflow-hidden rounded bg-slate-800">
                                                <div
                                                    class="h-full rounded bg-sky-500"
                                                    style=format!("width:{percent}%")
                                                ></div>
                                            </div>
                                            <p class="mt-0.5 text-xs text-slate-400">{label}</p>
                                        </div>
                                    }
                                })}
                            {nodes
                                .map(|line| {
                                    view! { <p class="mt-1 text-xs text-slate-400">{line}</p> }
                                })}
                            <div class="mt-1 flex flex-wrap gap-x-3 text-xs text-slate-500">
                                {expire.map(|line| view! { <span>{line}</span> })}
                                {provider.map(|name| view! { <span>{name}</span> })}
                            </div>
                            {last_error
                                .map(|message| {
                                    view! {
                                        <p class="mt-1 truncate text-xs text-red-300">
                                            {message}
                                        </p>
                                    }
                                })}
                        </div>
                        <div class="flex shrink-0 items-center gap-3 text-sm">
                            <button
                                type="button"
                                disabled=move || busy.get()
                                class="text-slate-400 transition hover:text-slate-200 disabled:opacity-50"
                                on:click=move |_| on_toggle(toggle_source.clone())
                            >
                                {if enabled { "停用" } else { "启用" }}
                            </button>
                            <button
                                type="button"
                                disabled=move || busy.get()
                                class="text-slate-400 transition hover:text-slate-200 disabled:opacity-50"
                                on:click=move |_| on_edit(edit_source.clone())
                            >
                                "编辑"
                            </button>
                            <button
                                type="button"
                                disabled=move || busy.get()
                                class="text-red-300 transition hover:text-red-200 disabled:opacity-50"
                                on:click=move |_| on_delete(delete_id.clone())
                            >
                                "删除"
                            </button>
                        </div>
                    </div>
                }
                    .into_any()
            };
                                            view! {
                                                <li class="rounded-lg border border-slate-800 px-3 py-2">
                                                    {row}
                                                </li>
                                            }
                                        })
                                        .collect_view()}
                                </ul>
                            }
                                .into_any()
                        }
                    }}
                </section>
            </div>
        </RequireAuth>
    }
}

fn format_bytes(value: i64) -> String {
    const UNITS: [(&str, f64); 4] = [
        ("TB", 1024.0 * 1024.0 * 1024.0 * 1024.0),
        ("GB", 1024.0 * 1024.0 * 1024.0),
        ("MB", 1024.0 * 1024.0),
        ("KB", 1024.0),
    ];
    let value = value.max(0) as f64;
    for (unit, scale) in UNITS {
        if value >= scale {
            return format!("{:.1} {}", value / scale, unit);
        }
    }
    format!("{value:.0} B")
}

fn format_expire(expire: i64) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(expire as f64 * 1000.0));
        if let Some(iso) = date.to_iso_string().as_string() {
            return iso.chars().take(10).collect();
        }
    }
    expire.to_string()
}
