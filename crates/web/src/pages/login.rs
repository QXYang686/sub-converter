use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;

use crate::api::user_message;
use crate::components::{Alert, RedirectIfAuthenticated, SubmitButton, TextField};
use crate::state::AuthStore;

#[component]
pub fn LoginPage() -> impl IntoView {
    let store = AuthStore::expect();
    let navigate = use_navigate();
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let loading = RwSignal::new(false);

    let submit_store = store.clone();
    let submit_navigate = navigate.clone();
    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let username_value = username.get_untracked();
        let password_value = password.get_untracked();
        if username_value.trim().is_empty() || password_value.is_empty() {
            error.set(Some("请输入用户名和密码".to_string()));
            return;
        }

        error.set(None);
        loading.set(true);
        let store = submit_store.clone();
        let navigate = submit_navigate.clone();
        spawn_local(async move {
            match store.login(&username_value, &password_value).await {
                Ok(()) => navigate("/", Default::default()),
                Err(err) => error.set(Some(user_message(&err))),
            }
            loading.set(false);
        });
    };

    let on_passkey = move |_| {
        let username_value = username.get_untracked();
        let username_hint = if username_value.trim().is_empty() {
            None
        } else {
            Some(username_value)
        };
        error.set(None);
        loading.set(true);
        let store = store.clone();
        let navigate = navigate.clone();
        spawn_local(async move {
            match store.login_with_passkey(username_hint).await {
                Ok(()) => navigate("/", Default::default()),
                Err(err) => error.set(Some(user_message(&err))),
            }
            loading.set(false);
        });
    };

    view! {
        <RedirectIfAuthenticated>
            <h1 class="mb-6 text-2xl font-semibold">"登录"</h1>
            <Alert message=error.into()/>
            <form class="space-y-4" on:submit=on_submit>
                <TextField label="用户名" value=username autocomplete="username"/>
                <TextField
                    label="密码"
                    value=password
                    input_type="password"
                    autocomplete="current-password"
                />
                <SubmitButton loading=loading.into() label="登录"/>
            </form>
            <div class="my-4 flex items-center gap-3 text-xs text-slate-600">
                <span class="h-px flex-1 bg-slate-800"></span>
                "或"
                <span class="h-px flex-1 bg-slate-800"></span>
            </div>
            <button
                type="button"
                class="w-full rounded-lg border border-slate-700 px-4 py-2 text-sm text-slate-200 transition hover:bg-slate-800"
                on:click=on_passkey
            >
                "使用 Passkey 登录"
            </button>
            <p class="mt-4 text-sm text-slate-400">
                "还没有账号？"
                <a href="/register" class="ml-1 text-slate-200 underline">
                    "注册"
                </a>
            </p>
        </RedirectIfAuthenticated>
    }
}
