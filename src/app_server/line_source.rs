use std::{
    io::{BufRead, BufReader},
    process::ChildStdout,
    sync::mpsc,
    thread,
    time::Duration,
};

use anyhow::Result;

pub(super) enum LineRead {
    Line(String),
    Idle,
    Eof,
}

pub(super) trait LineSource {
    fn read_line(&mut self, timeout: Option<Duration>) -> Result<LineRead>;
}

impl<R> LineSource for R
where
    R: BufRead,
{
    fn read_line(&mut self, _timeout: Option<Duration>) -> Result<LineRead> {
        let mut line = String::new();
        let bytes = BufRead::read_line(self, &mut line)?;
        if bytes == 0 {
            Ok(LineRead::Eof)
        } else {
            Ok(LineRead::Line(line))
        }
    }
}

pub(super) struct ThreadedLineReader {
    rx: mpsc::Receiver<std::io::Result<Option<String>>>,
}

impl ThreadedLineReader {
    pub(super) fn spawn(stdout: ChildStdout) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match BufRead::read_line(&mut reader, &mut line) {
                    Ok(0) => {
                        let _ = tx.send(Ok(None));
                        break;
                    }
                    Ok(_) => {
                        if tx.send(Ok(Some(line))).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = tx.send(Err(error));
                        break;
                    }
                }
            }
        });
        Self { rx }
    }
}

impl LineSource for ThreadedLineReader {
    fn read_line(&mut self, timeout: Option<Duration>) -> Result<LineRead> {
        let received = match timeout {
            Some(timeout) => match self.rx.recv_timeout(timeout) {
                Ok(received) => received,
                Err(mpsc::RecvTimeoutError::Timeout) => return Ok(LineRead::Idle),
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(LineRead::Eof),
            },
            None => match self.rx.recv() {
                Ok(received) => received,
                Err(_) => return Ok(LineRead::Eof),
            },
        };

        match received? {
            Some(line) => Ok(LineRead::Line(line)),
            None => Ok(LineRead::Eof),
        }
    }
}
