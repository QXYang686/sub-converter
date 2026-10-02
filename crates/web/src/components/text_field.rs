use leptos::prelude::*;

#[component]
pub fn TextField(
    label: &'static str,
    value: RwSignal<String>,
    #[prop(default = "text")] input_type: &'static str,
    #[prop(optional)] autocomplete: &'static str,
) -> impl IntoView {
    view! {
        <label class="block">
            <span class="mb-1 block text-sm text-slate-300">{label}</span>
            <input
                type=input_type
                autocomplete=autocomplete
                class="w-full rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-slate-100 outline-none transition focus:border-slate-500"
                prop:value=move || value.get()
                on:input=move |ev| value.set(event_target_value(&ev))
            />
        </label>
    }
}
