//! Binary-owned presentation only. No workload or domain-library dependency.
use std::{ffi::OsString,io::{IsTerminal,Write}};
const WORDMARK:&str=include_str!("../../assets/cli/optiflow.txt");

pub(super) fn literal_root_help(args:&[OsString])->bool {
    matches!(args,[arg] if arg=="--help"||arg=="-h")
}
fn decorate(args:&[OsString],tty:bool,term:Option<&str>,columns:Option<usize>,no_color:bool)->bool {
    let known_term=term.is_some_and(|s|s.len()<=64&&["xterm","screen","tmux","rxvt","linux","vt100","st"].iter().any(|prefix|s==*prefix||s.starts_with(&format!("{prefix}-"))));
    literal_root_help(args)&&tty&&!no_color&&known_term&&columns.is_some_and(|n|n>=72&&n<=1000&&WORDMARK.lines().all(|line|line.len()<=n))
}
pub(super) fn selected(args:&[OsString])->Option<&'static str> {
    let term=std::env::var("TERM").ok();
    let columns=std::env::var("COLUMNS").ok().filter(|s|s.len()<=4&&!s.is_empty()&&s.bytes().all(|b|b.is_ascii_digit())).and_then(|s|s.parse::<usize>().ok());
    let no_color=std::env::var_os("NO_COLOR").is_some()||std::env::var_os("CLICOLOR").is_some_and(|v|v=="0");
    decorate(args,std::io::stdout().is_terminal(),term.as_deref(),columns,no_color).then_some(WORDMARK)
}
pub(super) fn write_if_selected(args:&[OsString]) {
    if let Some(mark)=selected(args){let _=std::io::stdout().lock().write_all(mark.as_bytes());}
}
#[cfg(test)]mod tests {
    use super::*;
    fn args(values:&[&str])->Vec<OsString>{values.iter().map(OsString::from).collect()}
    #[test]fn suppresses_machine_subcommand_error_version_and_generator_requests(){
        for a in [vec!["--output","json","--help"],vec!["--format","json","--help"],vec!["--json","--help"],vec!["video","--help"],vec!["--version"],vec!["completion","bash"],vec!["man"],vec!["--unknown"],vec!["doctor"]]{assert!(!decorate(&args(&a),true,Some("xterm-256color"),Some(100),false));}
    }
    #[test]fn unknown_redirected_narrow_and_accessibility_paths_are_plain(){
        let a=args(&["--help"]);assert!(decorate(&a,true,Some("xterm-256color"),Some(100),false));
        for (tty,term,width,no_color) in [(false,Some("xterm"),Some(100),false),(true,Some("dumb"),Some(100),false),(true,None,Some(100),false),(true,Some("xterm"),None,false),(true,Some("xterm"),Some(40),false),(true,Some("xterm"),Some(100),true)]{assert!(!decorate(&a,tty,term,width,no_color));}
        assert!(WORDMARK.is_ascii());assert!(!WORDMARK.contains('\u{1b}'));assert!(WORDMARK.lines().all(|l|l.len()<=72));
    }
}
