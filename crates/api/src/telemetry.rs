use std::io;
use std::sync::Once;

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::EnvFilter;

static INIT: Once = Once::new();

pub fn init(log_level: Option<String>) {
    INIT.call_once(|| {
        let filter = EnvFilter::builder()
            .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
            .parse_lossy(log_level.unwrap_or_default());
        tracing_subscriber::fmt()
            .json()
            .without_time()
            .with_ansi(false)
            .with_writer(ConsoleWriter)
            .with_env_filter(filter)
            .init();
    });
}

struct ConsoleWriter;

struct ConsoleBuffer(String);

impl io::Write for ConsoleBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.push_str(&String::from_utf8_lossy(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for ConsoleBuffer {
    fn drop(&mut self) {
        let line = self.0.trim_end();
        if line.is_empty() {
            return;
        }
        match worker::js_sys::JSON::parse(line) {
            Ok(value) => worker::web_sys::console::log_1(&value),
            Err(_) => worker::console_log!("{line}"),
        }
    }
}

impl<'a> MakeWriter<'a> for ConsoleWriter {
    type Writer = ConsoleBuffer;

    fn make_writer(&'a self) -> Self::Writer {
        ConsoleBuffer(String::new())
    }
}
