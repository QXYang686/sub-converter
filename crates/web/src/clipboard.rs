pub fn subscription_url(secret: &str) -> String {
    let origin = web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .unwrap_or_default();
    format!("{origin}/s/{secret}")
}

pub async fn copy_text(text: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "复制失败，请手动复制".to_string())?;
    let promise = window.navigator().clipboard().write_text(text);
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|_| "复制失败，请手动复制".to_string())?;
    Ok(())
}
