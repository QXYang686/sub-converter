use leptos::prelude::*;
use leptos::task::spawn_local;

use contract::passkey::PasskeyResponse;

use crate::api::user_message;
use crate::components::{Alert, RequireAuth, Spinner};
use crate::state::AuthStore;

#[component]
pub fn SettingsPage() -> impl IntoView {
    let store = AuthStore::expect();
    let list_store = store.clone();
    let passkeys = RwSignal::new(Option::<Vec<PasskeyResponse>>::None);
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let reload = RwSignal::new(0u32);

    Effect::new(move |_| {
        reload.get();
        let store = list_store.clone();
        spawn_local(async move {
            match store.list_passkeys().await {
                Ok(list) => passkeys.set(Some(list)),
                Err(err) => error.set(Some(user_message(&err))),
            }
        });
    });

    let add_store = store.clone();
    let on_add = move |_| {
        let store = add_store.clone();
        error.set(None);
        busy.set(true);
        spawn_local(async move {
            match store.add_passkey(None).await {
                Ok(_) => reload.update(|value| *value += 1),
                Err(err) => error.set(Some(user_message(&err))),
            }
            busy.set(false);
        });
    };

    let on_delete = move |id: String| {
        let store = store.clone();
        error.set(None);
        busy.set(true);
        spawn_local(async move {
            match store.delete_passkey(&id).await {
                Ok(()) => reload.update(|value| *value += 1),
                Err(err) => error.set(Some(user_message(&err))),
            }
            busy.set(false);
        });
    };

    view! {
        <RequireAuth>
            <div class="space-y-6">
                <h1 class="text-2xl font-semibold">"安全设置"</h1>
                <Alert message=error.into()/>
                <section class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <div class="flex items-center justify-between">
                        <h2 class="text-lg font-medium">"Passkey"</h2>
                        <button
                            type="button"
                            disabled=move || busy.get()
                            class="rounded-lg border border-slate-700 px-3 py-1.5 text-sm text-slate-200 transition hover:bg-slate-800 disabled:opacity-50"
                            on:click=on_add
                        >
                            "添加 Passkey"
                        </button>
                    </div>
                    <p class="mt-1 text-sm text-slate-400">
                        "添加后可用设备生物识别或安全密钥直接登录。"
                    </p>
                    {move || match passkeys.get() {
                        None => view! { <Spinner/> }.into_any(),
                        Some(list) if list.is_empty() => {
                            view! {
                                <p class="mt-4 text-sm text-slate-500">"还没有添加 Passkey"</p>
                            }
                                .into_any()
                        }
                        Some(list) => {
                            view! {
                                <ul class="mt-4 space-y-2">
                                    {list
                                        .into_iter()
                                        .map(|passkey| {
                                            let id = passkey.id.clone();
                                            view! {
                                                <li class="flex items-center justify-between rounded-lg border border-slate-800 px-3 py-2">
                                                    <div>
                                                        <p class="text-sm">
                                                            {passkey
                                                                .label
                                                                .clone()
                                                                .unwrap_or_else(|| "Passkey".to_string())}
                                                        </p>
                                                        <p class="text-xs text-slate-500">{passkey.id.clone()}</p>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        class="text-sm text-red-300 transition hover:text-red-200"
                                                        on:click={
                                                            let on_delete = on_delete.clone();
                                                            let id = id.clone();
                                                            move |_| on_delete(id.clone())
                                                        }
                                                    >
                                                        "删除"
                                                    </button>
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
