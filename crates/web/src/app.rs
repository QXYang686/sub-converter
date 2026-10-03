use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::components::AppShell;
use crate::pages::{
    HomePage, LoginPage, PublicationsPage, RegisterPage, RuleSetsPage, SettingsPage, SourcesPage,
};
use crate::state::AuthStore;

#[component]
pub fn App() -> impl IntoView {
    let store = AuthStore::provide();
    store.bootstrap();

    view! {
        <Router>
            <AppShell>
                <Routes fallback=|| view! { <p class="text-slate-400">"页面不存在"</p> }>
                    <Route path=path!("/") view=HomePage/>
                    <Route path=path!("/login") view=LoginPage/>
                    <Route path=path!("/register") view=RegisterPage/>
                    <Route path=path!("/settings") view=SettingsPage/>
                    <Route path=path!("/sources") view=SourcesPage/>
                    <Route path=path!("/rule-sets") view=RuleSetsPage/>
                    <Route path=path!("/publications") view=PublicationsPage/>
                </Routes>
            </AppShell>
        </Router>
    }
}
