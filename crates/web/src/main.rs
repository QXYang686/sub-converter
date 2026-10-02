mod api;
mod state;

use leptos::prelude::*;

use state::AuthStore;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let store = AuthStore::provide();
    store.bootstrap();

    view! {
        <main class="flex min-h-screen items-center justify-center bg-slate-950">
            <h1 class="text-4xl font-bold text-white">"Sub Converter"</h1>
        </main>
    }
}
