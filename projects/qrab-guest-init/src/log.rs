//! Token emitter. Real builds write to `/dev/console`; tests use an
//! in-memory `Vec<u8>` to capture emitted lines.

use std::io::{self, Write};
use std::sync::Mutex;

use crate::tokens::Token;

/// Destination for token output. Abstracted so tests can assert on
/// exact byte stream without spawning a pty.
pub trait TokenSink: Send {
    fn emit(&mut self, token: Token) -> io::Result<()>;
}

/// Writes `<token>\n` to the inner writer. Used in real boot; the
/// writer is a `File` opened on `/dev/console`.
pub struct ConsoleSink<W: Write + Send> {
    inner: W,
}

impl<W: Write + Send> ConsoleSink<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W: Write + Send> TokenSink for ConsoleSink<W> {
    fn emit(&mut self, token: Token) -> io::Result<()> {
        self.inner.write_all(token.as_str().as_bytes())?;
        self.inner.write_all(b"\n")?;
        self.inner.flush()
    }
}

/// Thread-safe capture sink used in the integration tests.
#[derive(Default)]
pub struct CaptureSink(Mutex<Vec<Token>>);

impl CaptureSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tokens(&self) -> Vec<Token> {
        self.0.lock().expect("capture mutex not poisoned").clone()
    }
}

impl TokenSink for &CaptureSink {
    fn emit(&mut self, token: Token) -> io::Result<()> {
        self.0.lock().expect("capture mutex not poisoned").push(token);
        Ok(())
    }
}

/// Open the real console. Separate from `ConsoleSink::new` so the
/// caller can decide how to recover from a failed open (typically:
/// still attempt to emit via stderr, so post-mortem boot logs carry
/// the token even if `/dev/console` is unusable).
pub fn open_console() -> io::Result<std::fs::File> {
    std::fs::OpenOptions::new().write(true).open("/dev/console")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_sink_emits_token_with_trailing_newline() {
        let mut buf = Vec::new();
        {
            let mut sink = ConsoleSink::new(&mut buf);
            sink.emit(Token::Start).expect("emit ok");
            sink.emit(Token::FailLockdown).expect("emit ok");
        }
        assert_eq!(
            std::str::from_utf8(&buf).unwrap(),
            "QRAB_INIT_START\nQRAB_INIT_FAIL_LOCKDOWN\n",
        );
    }

    #[test]
    fn capture_sink_records_tokens_in_order() {
        let cap = CaptureSink::new();
        {
            let mut s = &cap;
            s.emit(Token::Start).unwrap();
            s.emit(Token::FailNoSnp).unwrap();
        }
        assert_eq!(cap.tokens(), vec![Token::Start, Token::FailNoSnp]);
    }
}
