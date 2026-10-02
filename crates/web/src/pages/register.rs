use leptos::prelude::*;

use crate::components::RedirectIfAuthenticated;

#[component]
pub fn RegisterPage() -> impl IntoView {
    view! {
        <RedirectIfAuthenticated>
            <h1 class="text-2xl font-semibold">"注册"</h1>
        </RedirectIfAuthenticated>
    }
}
