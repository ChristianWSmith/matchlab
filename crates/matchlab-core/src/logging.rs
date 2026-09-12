use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::sync::{Arc, Mutex};
struct SharedWriter(Arc<Mutex<BufWriter<File>>>);
impl Clone for SharedWriter {
    fn clone(&self) -> Self {
        SharedWriter(Arc::clone(&self.0))
    }
}
impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}
impl tracing_subscriber::fmt::MakeWriter<'_> for SharedWriter {
    type Writer = SharedWriter;
    fn make_writer(&self) -> Self::Writer {
        self.clone()
    }
}
pub fn init_logging(level: &str) {
    let _ = init_logging_with_options(level, None, false);
}
pub fn init_logging_with_options(
    level: &str,
    log_file: Option<&str>,
    json_logs: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    match (log_file, json_logs) {
        (None, false) => tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_writer(io::stderr)
            .try_init(),
        (None, true) => tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_writer(io::stderr)
            .json()
            .try_init(),
        (Some(path), false) => {
            let file = File::create(path).expect("failed to create log file");
            let writer = SharedWriter(Arc::new(Mutex::new(BufWriter::new(file))));
            let file_layer = tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_target(true)
                .with_filter(env_filter);
            Ok(tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_writer(io::stderr),
                )
                .with(file_layer)
                .try_init()?)
        }
        (Some(path), true) => {
            let file = File::create(path).expect("failed to create log file");
            let writer = SharedWriter(Arc::new(Mutex::new(BufWriter::new(file))));
            let file_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_writer(writer)
                .with_target(true)
                .with_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level)),
                );
            Ok(tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_target(true)
                        .with_writer(io::stderr),
                )
                .with(file_layer)
                .try_init()?)
        }
    }
}
