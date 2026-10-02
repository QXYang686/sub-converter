use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;

use crate::components::{RequireAuth, Spinner};
use crate::state::{AuthStatus, AuthStore};

#[component]
pub fn HomePage() -> impl IntoView {
    let store = AuthStore::expect();
    let navigate = use_navigate();

    let session_store = store.clone();
    Effect::new(move |_| {
        if !matches!(session_store.status.get(), AuthStatus::Authenticated(_)) {
            return;
        }
        let store = session_store.clone();
        spawn_local(async move {
            let _ = store.current_user().await;
        });
    });

    let view_store = store.clone();
    let on_logout = move |_| {
        let store = store.clone();
        let navigate = navigate.clone();
        spawn_local(async move {
            store.logout().await;
            navigate("/login", Default::default());
        });
    };

    view! {
        <RequireAuth>
            <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                {move || match view_store.status.get() {
                    AuthStatus::Authenticated(user) => {
                        view! {
                            <div class="space-y-4">
                                <div>
                                    <p class="text-sm text-slate-400">"当前用户"</p>
                                    <p class="text-xl font-semibold">{user.username}</p>
                                    <p class="mt-1 text-xs text-slate-500">{user.id}</p>
                                </div>
                                <button
                                    class="w-full rounded-lg border border-slate-700 px-4 py-2 text-sm text-slate-200 transition hover:bg-slate-800"
                                    on:click=on_logout.clone()
                                >
                                    "退出登录"
                                </button>
                                <a
                                    href="/settings"
                                    class="block text-center text-sm text-slate-400 underline transition hover:text-slate-200"
                                >
                                    "安全设置"
                                </a>
                            </div>
                        }
                            .into_any()
                    }
                    _ => view! { <Spinner/> }.into_any(),
                }}
            </div>
        </RequireAuth>
    }
}
