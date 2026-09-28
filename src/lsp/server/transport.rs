//! Read ahead while analysis runs, then apply queued edits before analyzing again.

use std::io::{self, BufReader, BufWriter};
use std::sync::mpsc::{self, Receiver};

use serde_json::Value;

use super::Session;
use crate::lsp::rpc;

// Bound both queued source buffers and work per batch so requests cannot starve.
const MESSAGE_CAPACITY: usize = 64;

pub fn serve() -> io::Result<()> {
    let messages = read_messages()?;
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    let mut session = Session::default();
    let mut pending = None;
    while let Some(message) = pending.take().or_else(|| messages.recv().ok()) {
        let message = message?;
        let (response, notifications) = if is_document_change(&message) {
            let changed = apply_queued_changes(&mut session, &message, &messages, &mut pending);
            (
                None,
                if changed {
                    session.refresh()
                } else {
                    Vec::new()
                },
            )
        } else {
            session.handle(&message)
        };
        for notification in notifications {
            rpc::write_message(&mut writer, &notification)?;
        }
        if let Some(response) = response {
            rpc::write_message(&mut writer, &response)?;
        }
        if session.exiting {
            break;
        }
    }
    Ok(())
}

fn read_messages() -> io::Result<Receiver<io::Result<Value>>> {
    let (sender, receiver) = mpsc::sync_channel(MESSAGE_CAPACITY);
    // Do not join on exit: a client may keep stdin open after its exit message.
    std::thread::Builder::new()
        .name("mclang-lsp-input".into())
        .spawn(move || {
            let stdin = io::stdin();
            let mut reader = BufReader::new(stdin.lock());
            loop {
                let message = match rpc::read_message(&mut reader) {
                    Ok(Some(message)) => Ok(message),
                    Ok(None) => break,
                    Err(error) => Err(error),
                };
                let finished = match &message {
                    Ok(message) => {
                        message.get("id").is_none()
                            && message.get("method").and_then(Value::as_str) == Some("exit")
                    }
                    Err(_) => true,
                };
                if sender.send(message).is_err() || finished {
                    break;
                }
            }
        })?;
    Ok(receiver)
}

fn is_document_change(message: &Value) -> bool {
    message.get("id").is_none()
        && message.get("method").and_then(Value::as_str) == Some("textDocument/didChange")
}

fn apply_queued_changes(
    session: &mut Session,
    first: &Value,
    messages: &Receiver<io::Result<Value>>,
    pending: &mut Option<io::Result<Value>>,
) -> bool {
    let mut changed = session.apply_document_change(&first["params"]);
    for _ in 1..MESSAGE_CAPACITY {
        match messages.try_recv() {
            Ok(Ok(message)) if is_document_change(&message) => {
                changed |= session.apply_document_change(&message["params"]);
            }
            Ok(other) => {
                // Every non-edit message is an ordering barrier, including saves,
                // closes and requests that must observe all preceding edits.
                *pending = Some(other);
                break;
            }
            Err(_) => break,
        }
    }
    changed
}
