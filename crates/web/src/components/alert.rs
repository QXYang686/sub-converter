use leptos::prelude::*;

#[component]
pub fn Alert(message: Signal<Option<String>>) -> impl IntoView {
    view! {
        <Show when=move || message.get().is_some()>
            <p class="mb-4 rounded-lg border border-red-900 bg-red-950/50 px-3 py-2 text-sm text-red-300">
                {move || message.get().unwrap_or_default()}
            </p>
        </Show>
    }
}
