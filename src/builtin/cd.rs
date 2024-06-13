use std::env;
use std::path::Path;

use crate::builtin::Builtin;
use crate::shell::Shell;

enum CdOption {
    L,
    P,
    Back,
    None,
}

pub struct Cd {
    option: CdOption,
    path: Option<String>,
}

impl Builtin for Cd {
    fn build(args: &[&str]) -> Result<Box<dyn Builtin>, String> {
        let mut option = CdOption::None;
        let mut path = None;

        match args.len() {
            0 => {}
            1 => match args.first() {
                Some(&"-") => option = CdOption::Back,
                Some(&"-L") => option = CdOption::L,
                Some(&"-P") => option = CdOption::P,
                Some(&arg) => path = Some(arg.to_string()),
                None => {}
            },
            2 => {
                match args.first() {
                    Some(&"-L") => option = CdOption::L,
                    Some(&"-P") => option = CdOption::P,
                    Some(&arg) => return Err(format!("cd: bad argument: {arg}")),
                    None => {}
                };

                path = Some(args[1].to_string());
            }
            _ => return Err("cd: too many arguments".to_string()),
        }

        Ok(Box::new(Self { option, path }))
    }

    fn run(&self, sh: &mut Shell) -> i32 {
        let path = match (&self.path, &self.option) {
            (None, CdOption::Back) => sh.env.oldpwd.clone(),
            (None, _) => sh.env.home.clone(),
            // -L and -P options not yet implemented
            // todo: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cd.html
            (Some(s), _) => {
                if s.starts_with('~') {
                    s.replacen('~', &sh.env.home, 1)
                } else {
                    s.into()
                }
            }
        };

        if !Path::new(&path).is_dir() {
            sh.eprintln(format!(
                "cd: cannot access '{path}': No such file or directory"
            ));
            return -1;
        }

        if let Err(e) = env::set_current_dir(&path) {
            sh.eprintln(format!("cd: cannot access '{path}': {e}"));
            -1
        } else {
            let pwd = env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or(path);

            env::set_var("OLDPWD", &sh.env.pwd);
            sh.env.oldpwd.clone_from(&sh.env.pwd);
            sh.env.pwd = pwd;
            0
        }
    }
}
