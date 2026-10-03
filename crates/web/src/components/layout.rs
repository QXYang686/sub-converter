use leptos::prelude::*;
use leptos_router::components::A;

use crate::state::{AuthStatus, AuthStore};

#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    let store = AuthStore::expect();
    let authenticated = move || matches!(store.status.get(), AuthStatus::Authenticated(_));

    view! {
        <div class="min-h-screen bg-slate-950 text-slate-100">
            <header class="border-b border-slate-800">
                <div class="mx-auto flex max-w-3xl items-center justify-between px-4 py-4">
                    <A href="/" attr:class="text-lg font-semibold tracking-tight">
                        "Sub Converter"
                    </A>
                    <Show when=authenticated>
                        <nav class="flex items-center gap-4 text-sm text-slate-400">
                            <A
                                href="/sources"
                                attr:class="transition hover:text-slate-200"
                            >
                                "订阅源"
                            </A>
                            <A
                                href="/publications"
                                attr:class="transition hover:text-slate-200"
                            >
                                "发布订阅"
                            </A>
                            <A
                                href="/rule-sets"
                                attr:class="transition hover:text-slate-200"
                            >
                                "规则集"
                            </A>
                            <A
                                href="/settings"
                                attr:class="transition hover:text-slate-200"
                            >
                                "安全设置"
                            </A>
                        </nav>
                    </Show>
                </div>
            </header>
            <main class="mx-auto w-full max-w-3xl px-4 py-10">{children()}</main>
        </div>
    }
}
