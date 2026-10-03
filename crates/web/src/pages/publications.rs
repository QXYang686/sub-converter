use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;

use contract::subscription::{PublicationResponse, SourceResponse, UpdatePublicationRequest};

use crate::api::user_message;
use crate::clipboard;
use crate::components::{Alert, CodeBlock, RequireAuth, Spinner, TextField};
use crate::forms::validate_publication_name;
use crate::state::{AuthStatus, AuthStore};

fn source_checkboxes(
    sources: RwSignal<Option<Vec<SourceResponse>>>,
    selected: RwSignal<Vec<String>>,
) -> impl IntoView {
    view! {
        {move || match sources.get() {
            None => view! { <Spinner/> }.into_any(),
            Some(list) if list.is_empty() => {
                view! {
                    <p class="text-sm text-slate-500">"请先在订阅源页面添加订阅源"</p>
                }
                    .into_any()
            }
            Some(list) => {
                view! {
                    <div class="space-y-1">
                        {list
                            .into_iter()
                            .map(|source| {
                                let checked = selected.get().contains(&source.id);
                                let id = source.id.clone();
                                view! {
                                    <label class="flex items-center gap-2 text-sm text-slate-300">
                                        <input
                                            type="checkbox"
                                            prop:checked=checked
                                            on:change=move |ev| {
                                                let checked = event_target_checked(&ev);
                                                let id = id.clone();
                                                selected
                                                    .update(|ids| {
                                                        if checked {
                                                            if !ids.contains(&id) {
                                                                ids.push(id);
                                                            }
                                                        } else {
                                                            ids.retain(|existing| existing != &id);
                                                        }
                                                    });
                                            }
                                        />
                                        <span>{source.name.clone()}</span>
                                    </label>
                                }
                            })
                            .collect_view()}
                    </div>
                }
                    .into_any()
            }
        }}
    }
}

#[component]
pub fn PublicationsPage() -> impl IntoView {
    let store = AuthStore::expect();
    let status = store.status;
    let publications = RwSignal::new(Option::<Vec<PublicationResponse>>::None);
    let sources = RwSignal::new(Option::<Vec<SourceResponse>>::None);
    let error = RwSignal::new(None::<String>);
    let notice = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let reload = RwSignal::new(0u32);
    let viewing = RwSignal::new(None::<String>);
    let view_content = RwSignal::new(None::<String>);

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
            match store.list_publications().await {
                Ok(list) => publications.set(Some(list)),
                Err(err) => error.set(Some(user_message(&err))),
            }
        });
    });

    let new_name = RwSignal::new(String::new());
    let new_selected = RwSignal::new(Vec::<String>::new());

    let create_store = store.clone();
    let on_create = move |ev: SubmitEvent| {
        ev.prevent_default();
        match validate_publication_name(&new_name.get_untracked()) {
            Err(message) => error.set(Some(message)),
            Ok(clean_name) => {
                let source_ids = new_selected.get_untracked();
                error.set(None);
                notice.set(None);
                busy.set(true);
                let store = create_store.clone();
                spawn_local(async move {
                    match store.create_publication(&clean_name, source_ids).await {
                        Ok(_) => {
                            new_name.set(String::new());
                            new_selected.set(Vec::new());
                            reload.update(|value| *value += 1);
                        }
                        Err(err) => error.set(Some(user_message(&err))),
                    }
                    busy.set(false);
                });
            }
        }
    };

    let editing = RwSignal::new(None::<PublicationResponse>);
    let edit_name = RwSignal::new(String::new());
    let edit_enabled = RwSignal::new(true);
    let edit_selected = RwSignal::new(Vec::<String>::new());

    let save_store = store.clone();
    let on_save = move |_| {
        let Some(publication) = editing.get() else {
            return;
        };
        match validate_publication_name(&edit_name.get_untracked()) {
            Err(message) => error.set(Some(message)),
            Ok(clean_name) => {
                let enabled = edit_enabled.get_untracked();
                let source_ids = edit_selected.get_untracked();
                let request = UpdatePublicationRequest {
                    name: Some(clean_name),
                    enabled: Some(enabled),
                    expires_at: None,
                };
                error.set(None);
                notice.set(None);
                busy.set(true);
                let store = save_store.clone();
                spawn_local(async move {
                    match store.update_publication(&publication.id, &request).await {
                        Ok(_) => {
                            match store
                                .set_publication_sources(&publication.id, source_ids)
                                .await
                            {
                                Ok(_) => {
                                    editing.set(None);
                                    reload.update(|value| *value += 1);
                                }
                                Err(err) => error.set(Some(user_message(&err))),
                            }
                        }
                        Err(err) => error.set(Some(user_message(&err))),
                    }
                    busy.set(false);
                });
            }
        }
    };

    let on_edit = move |publication: PublicationResponse| {
        edit_name.set(publication.name.clone());
        edit_enabled.set(publication.enabled);
        edit_selected.set(publication.source_ids.clone());
        editing.set(Some(publication));
    };

    let view_store = store.clone();
    let on_view = move |publication: PublicationResponse| {
        let id = publication.id.clone();
        if viewing.get() == Some(id.clone()) {
            viewing.set(None);
            view_content.set(None);
            return;
        }
        viewing.set(Some(id.clone()));
        view_content.set(None);
        error.set(None);
        let store = view_store.clone();
        spawn_local(async move {
            match store.publication_config(&id).await {
                Ok(text) => view_content.set(Some(text)),
                Err(err) => {
                    viewing.set(None);
                    error.set(Some(user_message(&err)));
                }
            }
        });
    };

    let on_copy = move |secret: String| {
        error.set(None);
        notice.set(None);
        busy.set(true);
        spawn_local(async move {
            let link = clipboard::subscription_url(&secret);
            match clipboard::copy_text(&link).await {
                Ok(()) => notice.set(Some("订阅链接已复制".to_string())),
                Err(message) => error.set(Some(message)),
            }
            busy.set(false);
        });
    };

    let delete_store = store.clone();
    let on_delete = move |id: String| {
        error.set(None);
        notice.set(None);
        busy.set(true);
        let store = delete_store.clone();
        spawn_local(async move {
            match store.delete_publication(&id).await {
                Ok(()) => reload.update(|value| *value += 1),
                Err(err) => error.set(Some(user_message(&err))),
            }
            busy.set(false);
        });
    };

    view! {
        <RequireAuth>
            <div class="space-y-6">
                <div>
                    <h1 class="text-2xl font-semibold">"发布订阅"</h1>
                    <p class="mt-1 text-sm text-slate-400">
                        "组合订阅源并生成对外订阅链接；公开分发与格式转换将在后续版本启用。"
                    </p>
                </div>
                <Alert message=error.into()/>
                <Show when=move || notice.get().is_some()>
                    <p class="mb-4 rounded-lg border border-emerald-900 bg-emerald-950/50 px-3 py-2 text-sm text-emerald-300">
                        {move || notice.get().unwrap_or_default()}
                    </p>
                </Show>
                <section class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <h2 class="text-lg font-medium">"新建发布订阅"</h2>
                    <form class="mt-4 space-y-3" on:submit=on_create>
                        <TextField label="名称" value=new_name/>
                        <div>
                            <p class="mb-1 text-sm text-slate-300">"订阅源"</p>
                            {source_checkboxes(sources, new_selected)}
                        </div>
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
                    {move || match publications.get() {
                        None => view! { <Spinner/> }.into_any(),
                        Some(list) if list.is_empty() => {
                            view! {
                                <p class="mt-4 text-sm text-slate-500">"还没有发布订阅"</p>
                            }
                                .into_any()
                        }
                        Some(list) => {
                            view! {
                                <ul class="mt-4 space-y-2">
                                    {list
                                        .into_iter()
                                        .map(|publication| {
                                            let id = publication.id.clone();
                                            let is_editing = editing
                                                .get()
                                                .map(|current| current.id == id)
                                                .unwrap_or(false);
                                            let on_copy = on_copy.clone();
                                            let on_delete = on_delete.clone();
                                            let on_edit = on_edit.clone();
                                            let row = if is_editing {
                                                view! {
                                                    <div class="space-y-3">
                                                        <TextField label="名称" value=edit_name/>
                                                        <label class="flex items-center gap-2 text-sm text-slate-300">
                                                            <input
                                                                type="checkbox"
                                                                prop:checked=edit_enabled.get()
                                                                on:change=move |ev| {
                                                                    edit_enabled.set(event_target_checked(&ev))
                                                                }
                                                            />
                                                            <span>"启用"</span>
                                                        </label>
                                                        <div>
                                                            <p class="mb-1 text-sm text-slate-300">"订阅源"</p>
                                                            {source_checkboxes(sources, edit_selected)}
                                                        </div>
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
                                                let copy_secret = publication.secret.clone();
                                                let edit_publication = publication.clone();
                                                let delete_id = id.clone();
                                                let view_publication = publication.clone();
                                                let view_label_id = id.clone();
                                                let view_show_id = id.clone();
                                                let on_view = on_view.clone();
                                                let enabled = publication.enabled;
                                                let link =
                                                    clipboard::subscription_url(&publication.secret);
                                                let bound_count = publication.source_ids.len();
                                                view! {
                                                    <div class="flex items-start justify-between gap-3">
                                                        <div class="min-w-0">
                                                            <p class="truncate text-sm">
                                                                {publication.name.clone()}
                                                                <Show when=move || !enabled>
                                                                    <span class="ml-2 text-xs text-amber-300">
                                                                        "已停用"
                                                                    </span>
                                                                </Show>
                                                            </p>
                                                            <p class="mt-1 truncate text-xs text-slate-500">
                                                                {link}
                                                            </p>
                                                            <p class="mt-1 text-xs text-slate-600">
                                                                {format!("绑定 {bound_count} 个订阅源")}
                                                            </p>
                                                        </div>
                                                        <div class="flex shrink-0 items-center gap-3 text-sm">
                                                            <button
                                                                type="button"
                                                                disabled=move || busy.get()
                                                                class="text-slate-400 transition hover:text-slate-200 disabled:opacity-50"
                                                                on:click=move |_| on_copy(copy_secret.clone())
                                                            >
                                                                "复制链接"
                                                            </button>
                                                            <button
                                                                type="button"
                                                                disabled=move || busy.get()
                                                                class="text-slate-400 transition hover:text-slate-200 disabled:opacity-50"
                                                                on:click=move |_| on_edit(edit_publication.clone())
                                                            >
                                                                "编辑"
                                                            </button>
                                                            <button
                                                                type="button"
                                                                disabled=move || busy.get()
                                                                class="text-slate-400 transition hover:text-slate-200 disabled:opacity-50"
                                                                on:click=move |_| on_view(view_publication.clone())
                                                            >
                                                                {move || {
                                                                    if viewing.get() == Some(view_label_id.clone()) {
                                                                        "收起"
                                                                    } else {
                                                                        "查看配置"
                                                                    }
                                                                }}
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
                                                    <Show when=move || viewing.get() == Some(view_show_id.clone())>
                                                        {move || match view_content.get() {
                                                            Some(text) => view! { <CodeBlock content=text/> }.into_any(),
                                                            None => view! { <Spinner/> }.into_any(),
                                                        }}
                                                    </Show>
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
