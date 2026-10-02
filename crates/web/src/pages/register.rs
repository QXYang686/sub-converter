use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;

use crate::api::user_message;
use crate::components::{Alert, RedirectIfAuthenticated, SubmitButton, TextField};
use crate::forms;
use crate::state::AuthStore;

#[component]
pub fn RegisterPage() -> impl IntoView {
    let store = AuthStore::expect();
    let navigate = use_navigate();
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let loading = RwSignal::new(false);

    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let username_value = username.get_untracked();
        let password_value = password.get_untracked();
        let confirm_value = confirm.get_untracked();

        let normalized = forms::validate_username(&username_value).and_then(|normalized| {
            forms::validate_password(&password_value)?;
            forms::validate_password_confirm(&password_value, &confirm_value)?;
            Ok(normalized)
        });

        let normalized = match normalized {
            Ok(value) => value,
            Err(message) => {
                error.set(Some(message));
                return;
            }
        };

        error.set(None);
        loading.set(true);
        let store = store.clone();
        let navigate = navigate.clone();
        spawn_local(async move {
            match store.register(&normalized, &password_value).await {
                Ok(()) => navigate("/", Default::default()),
                Err(err) => error.set(Some(user_message(&err))),
            }
            loading.set(false);
        });
    };

    view! {
        <RedirectIfAuthenticated>
            <h1 class="mb-6 text-2xl font-semibold">"注册"</h1>
            <Alert message=error.into()/>
            <form class="space-y-4" on:submit=on_submit>
                <TextField label="用户名" value=username autocomplete="username"/>
                <TextField
                    label="密码"
                    value=password
                    input_type="password"
                    autocomplete="new-password"
                />
                <TextField
                    label="确认密码"
                    value=confirm
                    input_type="password"
                    autocomplete="new-password"
                />
                <SubmitButton loading=loading.into() label="注册"/>
            </form>
            <p class="mt-4 text-sm text-slate-400">
                "已有账号？"
                <a href="/login" class="ml-1 text-slate-200 underline">
                    "登录"
                </a>
            </p>
        </RedirectIfAuthenticated>
    }
}
