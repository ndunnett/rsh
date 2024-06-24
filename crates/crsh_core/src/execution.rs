use std::io;
use std::process;

use sysexits::ExitCode;

use super::builtin::Builtin;
use super::parsing::Command;
use super::{IOContext, Shell};

impl Shell {
    pub fn execute(&mut self, ctx: Option<IOContext>, ast: &Command) -> ExitCode {
        match ast {
            Command::Empty => ExitCode::Ok,
            Command::Simple { args } => self.execute_simple(ctx, args),
            Command::And { left, right } => self.execute_logical(ctx, true, left, right),
            Command::Or { left, right } => self.execute_logical(ctx, false, left, right),
            Command::Pipeline { cmds } => self.execute_pipeline(ctx, cmds),
            Command::List { cmds } => self.execute_list(ctx, cmds),
            // _ => {
            //     sh.eprintln("crsh: unimplemented functionality");
            //     -1
            // }
        }
    }

    fn execute_simple(&mut self, ctx: Option<IOContext>, args: &[&str]) -> ExitCode {
        let mut io = match ctx {
            Some(ctx) => ctx,
            None => self.io.clone(),
        };

        let keyword = args[0];
        let args = &args[1..args.len()];

        if let Some(builtin) = Builtin::get(keyword) {
            match builtin.run(self, &mut io, args) {
                Ok(code) => code,
                Err(e) => {
                    io.eprintln(e);
                    ExitCode::Usage
                }
            }
        } else if self.find_on_path(keyword).is_some() {
            let args = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            let mut cmd = process::Command::new(keyword);

            let child = cmd
                .stdin(io.input.clone())
                .stdout(io.output.clone())
                .stderr(io.error.clone())
                .args(&args)
                .spawn();

            drop(io);
            drop(cmd);

            match child {
                Ok(mut child) => match child.wait() {
                    Ok(status) => status
                        .code()
                        .unwrap_or(0)
                        .try_into()
                        .unwrap_or(ExitCode::Ok),
                    Err(e) => {
                        self.io.eprintln(format!("crsh: {e}"));
                        ExitCode::OsErr
                    }
                },
                Err(e) => {
                    self.io.eprintln(format!("crsh: {e}"));
                    ExitCode::OsErr
                }
            }
        } else {
            self.io
                .eprintln(format!("crsh: command not found: {keyword}"));
            ExitCode::Unavailable
        }
    }

    fn execute_logical(
        &mut self,
        ctx: Option<IOContext>,
        and: bool,
        left: &Command,
        right: &Command,
    ) -> ExitCode {
        let left_result = self.execute(ctx.clone(), left);

        if (left_result == ExitCode::Ok) == and {
            self.execute(ctx, right)
        } else {
            left_result
        }
    }

    fn execute_pipeline(&mut self, ctx: Option<IOContext>, cmds: &[Command]) -> ExitCode {
        let io = match ctx {
            Some(ctx) => ctx,
            None => self.io.clone(),
        };

        let mut pipes = Vec::new();

        let results = cmds.iter().map(|cmd| {
            let new_ctx = if Some(cmd) == cmds.first() {
                let (reader, writer) = os_pipe::pipe()?;
                pipes.push((reader.try_clone()?, writer.try_clone()?));

                IOContext {
                    input: io.input.clone(),
                    output: writer.into(),
                    error: io.error.clone(),
                }
            } else if Some(cmd) == cmds.last() {
                let (reader, _) = pipes.pop().unwrap();

                IOContext {
                    input: reader.into(),
                    output: io.output.clone(),
                    error: io.error.clone(),
                }
            } else {
                let (last_reader, _) = pipes.pop().unwrap();
                let (reader, writer) = os_pipe::pipe()?;
                pipes.push((reader.try_clone()?, writer.try_clone()?));

                IOContext {
                    input: last_reader.into(),
                    output: writer.into(),
                    error: io.error.clone(),
                }
            };

            Ok::<ExitCode, io::Error>(self.execute(Some(new_ctx), cmd))
        });

        match results.last() {
            Some(Ok(code)) => code,
            Some(Err(e)) => {
                self.io.eprintln(format!("crsh: {e}"));
                ExitCode::IoErr
            }
            None => ExitCode::Ok,
        }
    }

    fn execute_list(&mut self, ctx: Option<IOContext>, cmds: &[Command]) -> ExitCode {
        cmds.iter()
            .map(|cmd| self.execute(ctx.clone(), cmd))
            .last()
            .unwrap_or(ExitCode::Ok)
    }
}