use leptos::prelude::*;

use crate::components::RequireAuth;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <RequireAuth>
            <h1 class="text-2xl font-semibold">"主页"</h1>
        </RequireAuth>
    }
}
