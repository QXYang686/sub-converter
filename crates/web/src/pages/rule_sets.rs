use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;

use contract::rule_set::{CreateRuleSetRequest, RuleSetResponse, UpdateRuleSetRequest};

use crate::api::user_message;
use crate::components::{Alert, RequireAuth, Spinner, TextField};
use crate::forms::validate_rule_set_name;
use crate::state::{AuthStatus, AuthStore};

const SOURCE_KINDS: [(&str, &str); 3] = [
    ("remote", "远程 URL"),
    ("inline", "内联内容"),
    ("local", "本地路径"),
];

const CONTENT_FORMATS: [(&str, &str); 5] = [
    ("yaml", "YAML"),
    ("text", "文本"),
    ("source-json", "Source JSON"),
    ("binary", "二进制"),
    ("adblock", "Adblock"),
];

const CATEGORIES: [(&str, &str); 4] = [
    ("", "无"),
    ("domain", "domain"),
    ("ip", "ip"),
    ("mixed", "mixed"),
];

#[component]
pub fn RuleSetsPage() -> impl IntoView {
    let store = AuthStore::expect();
    let status = store.status;
    let rule_sets = RwSignal::new(Option::<Vec<RuleSetResponse>>::None);
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
            match store.list_rule_sets().await {
                Ok(list) => rule_sets.set(Some(list)),
                Err(err) => error.set(Some(user_message(&err))),
            }
        });
    });

    let name = RwSignal::new(String::new());
    let source_kind = RwSignal::new("remote".to_string());
    let url = RwSignal::new(String::new());
    let path = RwSignal::new(String::new());
    let category = RwSignal::new(String::new());
    let content_format = RwSignal::new("yaml".to_string());
    let interval = RwSignal::new(String::new());
    let content = RwSignal::new(String::new());

    let create_store = store.clone();
    let on_create = move |ev: SubmitEvent| {
        ev.prevent_default();
        let request = build_create_request(
            &name.get_untracked(),
            &source_kind.get_untracked(),
            &url.get_untracked(),
            &path.get_untracked(),
            &category.get_untracked(),
            &content_format.get_untracked(),
            &interval.get_untracked(),
            &content.get_untracked(),
        );
        match request {
            Err(message) => error.set(Some(message)),
            Ok(request) => {
                error.set(None);
                busy.set(true);
                let store = create_store.clone();
                spawn_local(async move {
                    match store.create_rule_set(&request).await {
                        Ok(_) => {
                            name.set(String::new());
                            url.set(String::new());
                            path.set(String::new());
                            content.set(String::new());
                            reload.update(|value| *value += 1);
                        }
                        Err(err) => error.set(Some(user_message(&err))),
                    }
                    busy.set(false);
                });
            }
        }
    };

    let refresh_store = store.clone();
    let on_refresh = move |id: String| {
        error.set(None);
        busy.set(true);
        let store = refresh_store.clone();
        spawn_local(async move {
            match store.refresh_rule_set(&id).await {
                Ok(result) => {
                    if result.status == "failed" {
                        error.set(Some("抓取失败，已保留原内容".to_string()));
                    }
                    reload.update(|value| *value += 1);
                }
                Err(err) => error.set(Some(user_message(&err))),
            }
            busy.set(false);
        });
    };

    let toggle_store = store.clone();
    let on_toggle = move |rule_set: RuleSetResponse| {
        let request = UpdateRuleSetRequest {
            name: None,
            url: None,
            path: None,
            category: None,
            content_format: None,
            interval: None,
            enabled: Some(!rule_set.enabled),
        };
        error.set(None);
        busy.set(true);
        let store = toggle_store.clone();
        spawn_local(async move {
            match store.update_rule_set(&rule_set.id, &request).await {
                Ok(_) => reload.update(|value| *value += 1),
                Err(err) => error.set(Some(user_message(&err))),
            }
            busy.set(false);
        });
    };

    let pin_store = store.clone();
    let on_pin = move |rule_set: RuleSetResponse| {
        let pinned = rule_set
            .content
            .as_ref()
            .map(|content| content.pinned)
            .unwrap_or(false);
        error.set(None);
        busy.set(true);
        let store = pin_store.clone();
        spawn_local(async move {
            match store.set_rule_set_pinned(&rule_set.id, !pinned).await {
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
            match store.delete_rule_set(&id).await {
                Ok(()) => reload.update(|value| *value += 1),
                Err(err) => error.set(Some(user_message(&err))),
            }
            busy.set(false);
        });
    };

    let editing = RwSignal::new(None::<String>);
    let draft = RwSignal::new(String::new());

    let open_store = store.clone();
    let on_open = move |rule_set: RuleSetResponse| {
        let id = rule_set.id.clone();
        if editing.get_untracked() == Some(id.clone()) {
            editing.set(None);
            draft.set(String::new());
            return;
        }
        editing.set(Some(id.clone()));
        draft.set(String::new());
        error.set(None);
        let store = open_store.clone();
        spawn_local(async move {
            match store.rule_set_content(&id).await {
                Ok(text) => draft.set(text),
                Err(err) => {
                    editing.set(None);
                    error.set(Some(user_message(&err)));
                }
            }
        });
    };

    let save_store = store.clone();
    let on_save_content = move || {
        let Some(id) = editing.get_untracked() else {
            return;
        };
        let body = draft.get_untracked();
        error.set(None);
        busy.set(true);
        let store = save_store.clone();
        spawn_local(async move {
            match store.replace_rule_set_content(&id, &body).await {
                Ok(_) => {
                    editing.set(None);
                    reload.update(|value| *value += 1);
                }
                Err(err) => error.set(Some(user_message(&err))),
            }
            busy.set(false);
        });
    };

    view! {
        <RequireAuth>
            <div class="space-y-6">
                <div>
                    <h1 class="text-2xl font-semibold">"规则集"</h1>
                    <p class="mt-1 text-sm text-slate-400">
                        "管理具名规则集：来源、分类与内容。当前仅做管理，尚未接入发布订阅。"
                    </p>
                </div>
                <Alert message=error.into()/>
                <section class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <h2 class="text-lg font-medium">"新建规则集"</h2>
                    <form class="mt-4 space-y-3" on:submit=on_create>
                        <TextField label="名称" value=name/>
                        <label class="block">
                            <span class="mb-1 block text-sm text-slate-300">"来源"</span>
                            <select
                                class="w-full rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-slate-100 outline-none focus:border-slate-500"
                                on:change=move |ev| source_kind.set(event_target_value(&ev))
                            >
                                {SOURCE_KINDS
                                    .iter()
                                    .map(|(value, label)| {
                                        let value = value.to_string();
                                        view! {
                                            <option
                                                value=value.clone()
                                                selected=move || source_kind.get() == value
                                            >
                                                {*label}
                                            </option>
                                        }
                                    })
                                    .collect_view()}
                            </select>
                        </label>
                        <Show when=move || source_kind.get() == "remote">
                            <TextField label="远程 URL" value=url/>
                        </Show>
                        <Show when=move || source_kind.get() == "local">
                            <TextField label="本地路径" value=path/>
                        </Show>
                        <label class="block">
                            <span class="mb-1 block text-sm text-slate-300">"分类"</span>
                            <select
                                class="w-full rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-slate-100 outline-none focus:border-slate-500"
                                on:change=move |ev| category.set(event_target_value(&ev))
                            >
                                {CATEGORIES
                                    .iter()
                                    .map(|(value, label)| {
                                        let value = value.to_string();
                                        view! {
                                            <option
                                                value=value.clone()
                                                selected=move || category.get() == value
                                            >
                                                {*label}
                                            </option>
                                        }
                                    })
                                    .collect_view()}
                            </select>
                        </label>
                        <label class="block">
                            <span class="mb-1 block text-sm text-slate-300">"内容格式"</span>
                            <select
                                class="w-full rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-slate-100 outline-none focus:border-slate-500"
                                on:change=move |ev| content_format.set(event_target_value(&ev))
                            >
                                {CONTENT_FORMATS
                                    .iter()
                                    .map(|(value, label)| {
                                        let value = value.to_string();
                                        view! {
                                            <option
                                                value=value.clone()
                                                selected=move || content_format.get() == value
                                            >
                                                {*label}
                                            </option>
                                        }
                                    })
                                    .collect_view()}
                            </select>
                        </label>
                        <TextField label="刷新间隔（秒，可空）" value=interval/>
                        <Show when=move || source_kind.get() == "inline">
                            <label class="block">
                                <span class="mb-1 block text-sm text-slate-300">"内联内容"</span>
                                <textarea
                                    class="h-40 w-full rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 font-mono text-sm text-slate-100 outline-none focus:border-slate-500"
                                    prop:value=move || content.get()
                                    on:input=move |ev| content.set(event_target_value(&ev))
                                ></textarea>
                            </label>
                        </Show>
                        <button
                            type="submit"
                            disabled=move || busy.get()
                            class="w-full rounded-lg bg-slate-100 px-4 py-2 font-medium text-slate-900 transition hover:bg-white disabled:cursor-not-allowed disabled:opacity-50"
                        >
                            "创建"
                        </button>
                    </form>
                </section>
                <section class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <h2 class="text-lg font-medium">"已创建"</h2>
                    {move || match rule_sets.get() {
                        None => view! { <Spinner/> }.into_any(),
                        Some(list) if list.is_empty() => {
                            view! {
                                <p class="mt-3 text-sm text-slate-500">"还没有规则集"</p>
                            }
                                .into_any()
                        }
                        Some(list) => {
                            view! {
                                <ul class="mt-4 space-y-3">
                                    {list
                                        .into_iter()
                                        .map(|rule_set| {
                                            let id = rule_set.id.clone();
                                            let name = rule_set.name.clone();
                                            let source_kind = rule_set.source_kind.clone();
                                            let is_remote = source_kind == "remote";
                                            let is_local = source_kind == "local";
                                            let enabled = rule_set.enabled;
                                            let location = rule_set
                                                .url
                                                .clone()
                                                .or_else(|| rule_set.path.clone())
                                                .unwrap_or_default();
                                            let category = rule_set.category.clone();
                                            let content_format = rule_set.content_format.clone();
                                            let pinned = rule_set
                                                .content
                                                .as_ref()
                                                .map(|content| content.pinned)
                                                .unwrap_or(false);
                                            let content_line = match &rule_set.content {
                                                Some(content) if content.has_content => {
                                                    let pinned_mark =
                                                        if content.pinned { " · 已钉住" } else { "" };
                                                    Some(format!(
                                                        "{} 条规则 · {} 字节 · {}{}",
                                                        content.rule_count,
                                                        content.byte_size,
                                                        format_time(content.fetched_at),
                                                        pinned_mark,
                                                    ))
                                                }
                                                Some(_) => Some("暂无内容".to_string()),
                                                None => Some("无内容".to_string()),
                                            };
                                            let last_error = rule_set
                                                .content
                                                .as_ref()
                                                .and_then(|content| content.last_error.clone());

                                            let on_refresh = on_refresh.clone();
                                            let on_toggle = on_toggle.clone();
                                            let on_pin = on_pin.clone();
                                            let on_delete = on_delete.clone();
                                            let on_open = on_open.clone();
                                            let on_save_content = on_save_content.clone();

                                            let refresh_id = id.clone();
                                            let toggle_item = rule_set.clone();
                                            let pin_item = rule_set.clone();
                                            let delete_id = id.clone();
                                            let open_item = rule_set.clone();
                                            let is_open = {
                                                let id = id.clone();
                                                move || editing.get() == Some(id.clone())
                                            };

                                            view! {
                                                <li class="rounded-xl border border-slate-800 p-4">
                                                    <div class="min-w-0">
                                                        <p class="truncate font-medium text-slate-200">
                                                            {name}
                                                        </p>
                                                        <p class="text-xs text-slate-500">
                                                            {source_kind}
                                                            {category.map(|value| format!(" · {value}"))}
                                                            {format!(" · {content_format}")}
                                                            {(!enabled).then_some(" · 已禁用")}
                                                        </p>
                                                        {(!location.is_empty())
                                                            .then(|| {
                                                                view! {
                                                                    <p class="truncate text-xs text-slate-500">
                                                                        {location.clone()}
                                                                    </p>
                                                                }
                                                            })}
                                                        {content_line
                                                            .map(|line| {
                                                                view! {
                                                                    <p class="text-xs text-slate-400">{line}</p>
                                                                }
                                                            })}
                                                        {last_error
                                                            .map(|message| {
                                                                view! {
                                                                    <p class="truncate text-xs text-red-300">
                                                                        {message}
                                                                    </p>
                                                                }
                                                            })}
                                                    </div>
                                                    <div class="mt-3 flex flex-wrap gap-3 text-xs">
                                                        {is_remote
                                                            .then(|| {
                                                                view! {
                                                                    <button
                                                                        type="button"
                                                                        class="text-slate-400 transition hover:text-slate-200"
                                                                        on:click=move |_| on_refresh(refresh_id.clone())
                                                                    >
                                                                        "刷新"
                                                                    </button>
                                                                }
                                                            })}
                                                        {(!is_local)
                                                            .then(|| {
                                                                view! {
                                                                    <button
                                                                        type="button"
                                                                        class="text-slate-400 transition hover:text-slate-200"
                                                                        on:click=move |_| on_open(open_item.clone())
                                                                    >
                                                                        {move || {
                                                                            if editing.get() == Some(id.clone()) {
                                                                                "收起"
                                                                            } else {
                                                                                "查看/编辑"
                                                                            }
                                                                        }}
                                                                    </button>
                                                                }
                                                            })}
                                                        {(!is_local)
                                                            .then(|| {
                                                                view! {
                                                                    <button
                                                                        type="button"
                                                                        class="text-slate-400 transition hover:text-slate-200"
                                                                        on:click=move |_| on_pin(pin_item.clone())
                                                                    >
                                                                        {if pinned { "解钉" } else { "钉住" }}
                                                                    </button>
                                                                }
                                                            })}
                                                        <button
                                                            type="button"
                                                            class="text-slate-400 transition hover:text-slate-200"
                                                            on:click=move |_| on_toggle(toggle_item.clone())
                                                        >
                                                            {if enabled { "禁用" } else { "启用" }}
                                                        </button>
                                                        <button
                                                            type="button"
                                                            class="text-red-400 transition hover:text-red-300"
                                                            on:click=move |_| on_delete(delete_id.clone())
                                                        >
                                                            "删除"
                                                        </button>
                                                    </div>
                                                    <Show when=is_open.clone()>
                                                        {
                                                            let on_save_content = on_save_content.clone();
                                                            view! {
                                                                <div class="mt-3 space-y-2">
                                                                    <textarea
                                                                        class="h-48 w-full rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 font-mono text-sm text-slate-100 outline-none focus:border-slate-500"
                                                                        prop:value=move || draft.get()
                                                                        on:input=move |ev| draft.set(event_target_value(&ev))
                                                                    ></textarea>
                                                                    <button
                                                                        type="button"
                                                                        class="rounded-lg bg-slate-100 px-3 py-1.5 text-sm font-medium text-slate-900 transition hover:bg-white"
                                                                        on:click=move |_| on_save_content()
                                                                    >
                                                                        "保存并钉住"
                                                                    </button>
                                                                </div>
                                                            }
                                                        }
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
                </section>
            </div>
        </RequireAuth>
    }
}

#[allow(clippy::too_many_arguments)]
fn build_create_request(
    name: &str,
    source_kind: &str,
    url: &str,
    path: &str,
    category: &str,
    content_format: &str,
    interval: &str,
    content: &str,
) -> Result<CreateRuleSetRequest, String> {
    let name = validate_rule_set_name(name)?;
    let url = url.trim();
    let path = path.trim();
    match source_kind {
        "remote" if url.is_empty() => return Err("请填写远程 URL".to_string()),
        "local" if path.is_empty() => return Err("请填写本地路径".to_string()),
        "inline" if content.trim().is_empty() => return Err("请填写内联内容".to_string()),
        _ => {}
    }

    let interval = if interval.trim().is_empty() {
        None
    } else {
        Some(
            interval
                .trim()
                .parse::<i64>()
                .map_err(|_| "刷新间隔需为整数秒".to_string())?,
        )
    };

    Ok(CreateRuleSetRequest {
        name,
        source_kind: source_kind.to_string(),
        url: (!url.is_empty()).then(|| url.to_string()),
        path: (!path.is_empty()).then(|| path.to_string()),
        category: (!category.is_empty()).then(|| category.to_string()),
        content_format: content_format.to_string(),
        interval,
        content: (source_kind == "inline").then(|| content.to_string()),
    })
}

fn format_time(value: Option<i64>) -> String {
    let Some(value) = value.filter(|value| *value > 0) else {
        return "未抓取".to_string();
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
