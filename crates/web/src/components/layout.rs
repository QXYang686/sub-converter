use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    view! {
        <div class="min-h-screen bg-slate-950 text-slate-100">
            <header class="border-b border-slate-800">
                <div class="mx-auto flex max-w-3xl items-center justify-between px-4 py-4">
                    <A href="/" attr:class="text-lg font-semibold tracking-tight">
                        "Sub Converter"
                    </A>
                </div>
            </header>
            <main class="mx-auto w-full max-w-md px-4 py-10">{children()}</main>
        </div>
    }
}
