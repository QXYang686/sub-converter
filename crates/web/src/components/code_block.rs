use leptos::prelude::*;

/// 以只读代码块展示文本（订阅源原始 YAML / 发布订阅合并结果）。
#[component]
pub fn CodeBlock(content: String) -> impl IntoView {
    view! {
        <pre class="mt-2 max-h-96 overflow-auto rounded-lg border border-slate-800 bg-slate-950 p-3 text-xs leading-relaxed text-slate-300">
            <code>{content}</code>
        </pre>
    }
}
