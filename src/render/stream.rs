use super::{MarkdownRender, SseEvent};

use crate::utils::{poll_abort_signal, spawn_spinner, strip_think_tag, AbortSignal};

use anyhow::Result;
use crossterm::{
    cursor, queue, style,
    terminal::{self},
};
use std::{
    io::{self, stdout, Stdout, Write},
    time::Duration,
};
use textwrap::core::display_width;
use tokio::sync::mpsc::UnboundedReceiver;

pub async fn markdown_stream(
    rx: UnboundedReceiver<SseEvent>,
    render: &mut MarkdownRender,
    abort_signal: &AbortSignal,
    hide_thinking: bool,
) -> Result<()> {
    let mut stdout = io::stdout();

    let ret = markdown_stream_inner(rx, render, abort_signal, &mut stdout, hide_thinking).await;

    if ret.is_err() {
        println!();
    }
    ret
}

pub async fn raw_stream(
    mut rx: UnboundedReceiver<SseEvent>,
    abort_signal: &AbortSignal,
    hide_thinking: bool,
) -> Result<()> {
    let mut spinner = Some(spawn_spinner("Generating"));
    let mut buffer = String::new();

    loop {
        if abort_signal.aborted() {
            break;
        }
        if let Some(evt) = rx.recv().await {
            if let Some(spinner) = spinner.take() {
                spinner.stop();
            }

            match evt {
                SseEvent::Text(text) => {
                    if hide_thinking {
                        // Accumulate text to properly handle think tags across chunks
                        buffer.push_str(&text);
                    } else {
                        print!("{text}");
                        stdout().flush()?;
                    }
                }
                SseEvent::Done => {
                    if hide_thinking && !buffer.is_empty() {
                        // Process accumulated text and strip think tags
                        let filtered = strip_think_tag(&buffer);
                        print!("{filtered}");
                        stdout().flush()?;
                    }
                    break;
                }
            }
        }
    }
    if let Some(spinner) = spinner.take() {
        spinner.stop();
    }
    Ok(())
}

async fn markdown_stream_inner(
    mut rx: UnboundedReceiver<SseEvent>,
    render: &mut MarkdownRender,
    abort_signal: &AbortSignal,
    writer: &mut Stdout,
    hide_thinking: bool,
) -> Result<()> {
    let mut buffer = String::new();
    let mut buffer_rows = 1;
    let mut full_buffer = String::new();

    let columns = terminal::size()?.0;

    let mut spinner = Some(spawn_spinner("Generating"));

    'outer: loop {
        if abort_signal.aborted() {
            break;
        }
        for reply_event in gather_events(&mut rx).await {
            if let Some(spinner) = spinner.take() {
                spinner.stop();
            }

            match reply_event {
                SseEvent::Text(mut text) => {
                    if hide_thinking {
                        // Accumulate all text for filtering at the end
                        full_buffer.push_str(&text);
                        continue;
                    }
                    // tab width hacking
                    text = text.replace('\t', "    ");

                    let mut attempts = 0;
                    let (col, mut row) = loop {
                        match cursor::position() {
                            Ok(pos) => break pos,
                            Err(_) if attempts < 3 => attempts += 1,
                            Err(e) => return Err(e.into()),
                        }
                    };

                    // Fix unexpected duplicate lines on kitty, see https://github.com/sigoden/aichat/issues/105
                    if col == 0 && row > 0 && display_width(&buffer) == columns as usize {
                        row -= 1;
                    }

                    if row + 1 >= buffer_rows {
                        queue!(writer, cursor::MoveTo(0, row + 1 - buffer_rows),)?;
                    } else {
                        let scroll_rows = buffer_rows - row - 1;
                        queue!(
                            writer,
                            terminal::ScrollUp(scroll_rows),
                            cursor::MoveTo(0, 0),
                        )?;
                    }

                    // No guarantee that text returned by render will not be re-layouted, so it is better to clear it.
                    queue!(writer, terminal::Clear(terminal::ClearType::FromCursorDown))?;

                    if text.contains('\n') {
                        let text = format!("{buffer}{text}");
                        let (head, tail) = split_line_tail(&text);
                        let output = render.render(head);
                        print_block(writer, &output, columns)?;
                        buffer = tail.to_string();
                    } else {
                        buffer = format!("{buffer}{text}");
                    }

                    let output = render.render_line(&buffer);
                    if output.contains('\n') {
                        let (head, tail) = split_line_tail(&output);
                        buffer_rows = print_block(writer, head, columns)?;
                        queue!(writer, style::Print(&tail),)?;

                        // No guarantee the buffer width of the buffer will not exceed the number of columns.
                        // So we calculate the number of rows needed, rather than setting it directly to 1.
                        buffer_rows += need_rows(tail, columns);
                    } else {
                        queue!(writer, style::Print(&output))?;
                        buffer_rows = need_rows(&output, columns);
                    }

                    writer.flush()?;
                }
                SseEvent::Done => {
                    if hide_thinking && !full_buffer.is_empty() {
                        // Filter and render accumulated text
                        let filtered = strip_think_tag(&full_buffer);
                        let output = render.render(&filtered);
                        print!("{output}");
                        stdout().flush()?;
                    }
                    break 'outer;
                }
            }
        }

        if poll_abort_signal(abort_signal)? {
            break;
        }
    }

    if let Some(spinner) = spinner.take() {
        spinner.stop();
    }
    Ok(())
}

async fn gather_events(rx: &mut UnboundedReceiver<SseEvent>) -> Vec<SseEvent> {
    let mut texts = vec![];
    let mut done = false;
    tokio::select! {
        _ = async {
            while let Some(reply_event) = rx.recv().await {
                match reply_event {
                    SseEvent::Text(v) => texts.push(v),
                    SseEvent::Done => {
                        done = true;
                        break;
                    }
                }
            }
        } => {}
        _ = tokio::time::sleep(Duration::from_millis(50)) => {}
    };
    let mut events = vec![];
    if !texts.is_empty() {
        events.push(SseEvent::Text(texts.join("")))
    }
    if done {
        events.push(SseEvent::Done)
    }
    events
}

fn print_block(writer: &mut Stdout, text: &str, columns: u16) -> Result<u16> {
    let mut num = 0;
    for line in text.split('\n') {
        // In raw mode, '\n' may not reset the column in all terminals.
        queue!(
            writer,
            style::Print(line),
            style::Print("\n"),
            cursor::MoveToColumn(0),
        )?;
        num += need_rows(line, columns);
    }
    Ok(num)
}

fn split_line_tail(text: &str) -> (&str, &str) {
    if let Some((head, tail)) = text.rsplit_once('\n') {
        (head, tail)
    } else {
        ("", text)
    }
}

fn need_rows(text: &str, columns: u16) -> u16 {
    let buffer_width = display_width_without_ansi(text).max(1) as u16;
    buffer_width.div_ceil(columns)
}

fn display_width_without_ansi(text: &str) -> usize {
    let stripped = strip_ansi_sequences(text);
    display_width(&stripped)
}

fn strip_ansi_sequences(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1b {
            i += 1;
            if i >= bytes.len() {
                break;
            }

            match bytes[i] {
                b'[' => {
                    // CSI sequence: ESC [ ... <final-byte>
                    i += 1;
                    while i < bytes.len() {
                        let b = bytes[i];
                        i += 1;
                        if (0x40..=0x7e).contains(&b) {
                            break;
                        }
                    }
                    continue;
                }
                b']' => {
                    // OSC sequence: ESC ] ... BEL or ESC \
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                }
                _ => {
                    // Skip one-character ESC sequence.
                    i += 1;
                    continue;
                }
            }
        }

        let mut chars = input[i..].chars();
        if let Some(ch) = chars.next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_keeps_visible_text() {
        let input = "\x1b[38;5;12mhello\x1b[0m world";
        assert_eq!(strip_ansi_sequences(input), "hello world");
    }

    #[test]
    fn need_rows_ignores_ansi_width() {
        let plain = "abcdefghij";
        let colored = "\x1b[31mabcdefghij\x1b[0m";
        assert_eq!(need_rows(plain, 5), 2);
        assert_eq!(need_rows(colored, 5), 2);
    }
}
