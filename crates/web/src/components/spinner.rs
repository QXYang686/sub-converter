use leptos::prelude::*;

#[component]
pub fn Spinner() -> impl IntoView {
    view! {
        <div class="flex justify-center py-10">
            <div class="h-8 w-8 animate-spin rounded-full border-2 border-slate-700 border-t-slate-200"></div>
        </div>
    }
}
