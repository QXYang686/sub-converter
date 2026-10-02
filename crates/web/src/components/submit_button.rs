use leptos::prelude::*;

#[component]
pub fn SubmitButton(loading: Signal<bool>, label: &'static str) -> impl IntoView {
    view! {
        <button
            type="submit"
            disabled=move || loading.get()
            class="w-full rounded-lg bg-slate-100 px-4 py-2 font-medium text-slate-900 transition hover:bg-white disabled:cursor-not-allowed disabled:opacity-50"
        >
            {move || if loading.get() { "处理中…" } else { label }}
        </button>
    }
}
